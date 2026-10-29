//! Native packet counts prevent a skipped conversion test from passing without ingress.

use super::*;
use crate::{
    example_interfaces::action::Fibonacci, ActionClientOptions, ActionGoalOptions,
    ActionServerOptions, GoalStatus, QosProfile,
};
use futures::{FutureExt, StreamExt};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

fn drive<F: Future<Output = Result<T>>, T>(server: &mut Node, client: &mut Node, future: F) -> T {
    futures::pin_mut!(future);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline);
        server.try_spin_once(Duration::from_millis(1)).unwrap();
        client.try_spin_once(Duration::from_millis(1)).unwrap();
        if let Some(result) = future.as_mut().now_or_never() {
            return result.unwrap();
        }
    }
}

fn unexpected_decode(
    _: WrappedNativeMsgUntyped,
) -> (unique_identifier_msgs::msg::UUID, Result<serde_json::Value>) {
    panic!("unobserved native feedback converted its payload");
}

static FALLBACK_DECODES: AtomicUsize = AtomicUsize::new(0);
fn fallback_decode(
    native: WrappedNativeMsgUntyped,
) -> (unique_identifier_msgs::msg::UUID, Result<serde_json::Value>) {
    FALLBACK_DECODES.fetch_add(1, Ordering::Relaxed);
    let message = Fibonacci::FeedbackMessage::from_native(unsafe {
        &*(native.void_ptr() as *const <Fibonacci::FeedbackMessage as WrappedTypesupport>::CStruct)
    });
    let (id, feedback) =
        <Fibonacci::Action as WrappedActionTypeSupport>::destructure_feedback_msg(message);
    (
        id,
        serde_json::to_value(feedback).map_err(|err| Error::SerdeError {
            err: err.to_string(),
        }),
    )
}

fn run(other_observer: bool, fallback: bool) {
    let context = crate::Context::create().unwrap();
    let mut server_node = Node::create(context.clone(), "native_feedback_skip_server", "").unwrap();
    let mut client_node = Node::create(context, "native_feedback_skip_client", "").unwrap();
    let name = format!("/native_feedback_skip_{}", uuid::Uuid::new_v4().simple());
    let qos = QosProfile::default().keep_last(16);
    let mut requests = server_node
        .create_action_server_with_options::<Fibonacci::Action>(
            &name,
            ActionServerOptions::default().feedback_topic_qos(qos.clone()),
        )
        .unwrap();
    let client = client_node
        .create_action_client_untyped_with_options(
            &name,
            "example_interfaces/action/Fibonacci",
            ActionClientOptions::default().feedback_topic_qos(qos),
        )
        .unwrap();
    drive(&mut server_node, &mut client_node, client.wait_for_action_server().unwrap());
    let mut goals = Vec::new();
    let mut observations = Vec::new();
    for order in 0..=usize::from(other_observer) {
        let response = client
            .send_goal_request_with_options(
                serde_json::json!({"order":order}),
                ActionGoalOptions::default().track_feedback(order == 1),
            )
            .unwrap();
        let request = drive(&mut server_node, &mut client_node, async {
            requests
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
        });
        let goal = request.accept_deferred().unwrap();
        observations.push(drive(&mut server_node, &mut client_node, response));
        goal.0.execute().unwrap();
        goals.push(goal);
    }
    let native = client.client.upgrade().unwrap();
    let original = native.lock().action_type_support.destructure_feedback_msg;
    if fallback {
        FALLBACK_DECODES.store(0, Ordering::Relaxed);
        native.lock().action_type_support.native_feedback_uuid = |_| None;
        native.lock().action_type_support.destructure_feedback_msg = fallback_decode;
    } else {
        native.lock().action_type_support.destructure_feedback_msg = unexpected_decode;
    }
    for value in 0..8 {
        goals[0]
            .0
            .publish_feedback(Fibonacci::Feedback {
                sequence: vec![value; 4096],
            })
            .unwrap();
    }
    let erased: Arc<Mutex<dyn ActionClient_>> = native.clone();
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut taken = 0;
    while taken < 8 {
        assert!(Instant::now() < deadline, "only took {taken} native feedback messages");
        let mut operations = Vec::new();
        let result = {
            native
                .lock()
                .handle_feedback_msg(erased.clone(), &mut operations)
        }
        .unwrap();
        run_detached_action_client_completions(operations);
        if matches!(result, ActionClientTakeResult::Taken) {
            taken += 1;
        } else {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    if fallback {
        assert_eq!(FALLBACK_DECODES.load(Ordering::Relaxed), 8);
    }
    assert!(native.lock().pending_feedback_goals.lock().is_empty());
    native.lock().action_type_support.destructure_feedback_msg = original;
    if other_observer {
        goals[1]
            .0
            .publish_feedback(Fibonacci::Feedback { sequence: vec![99] })
            .unwrap();
        let feedback = observations[1].feedback.as_mut().unwrap();
        let message = drive(&mut server_node, &mut client_node, async {
            feedback
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)
        });
        assert_eq!(
            serde_json::from_value::<Fibonacci::Feedback>(message.unwrap())
                .unwrap()
                .sequence,
            [99]
        );
    }
    for (i, (goal, _)) in goals.iter_mut().enumerate() {
        goal.succeed(Fibonacci::Result {
            sequence: vec![i as i32],
        })
        .unwrap();
    }
    for (i, observed) in observations.into_iter().enumerate() {
        let (status, result) = drive(&mut server_node, &mut client_node, observed.result.unwrap());
        assert_eq!(status, GoalStatus::Succeeded);
        assert_eq!(
            serde_json::from_value::<Fibonacci::Result>(result.unwrap())
                .unwrap()
                .sequence,
            [i as i32]
        );
    }
}

#[test]
fn native_feedback_skips_unobserved_payload_with_no_or_another_observer() {
    run(false, false);
    run(true, false);
}

#[test]
fn native_feedback_without_uuid_accessor_preserves_legacy_decoding() {
    run(false, true);
}
