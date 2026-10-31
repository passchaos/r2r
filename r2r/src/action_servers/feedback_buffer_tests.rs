use super::*;
use futures::{FutureExt, StreamExt};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use crate::{example_interfaces::action::Fibonacci, ActionGoalOptions};

static FEEDBACK_CREATED: AtomicUsize = AtomicUsize::new(0);
static FEEDBACK_DESTROYED: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct TrackedFeedbackMessage {
    goal_id: unique_identifier_msgs::msg::UUID,
    feedback: Fibonacci::Feedback,
}

unsafe impl WrappedTypesupport for TrackedFeedbackMessage {
    type CStruct = <Fibonacci::FeedbackMessage as WrappedTypesupport>::CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t {
        Fibonacci::FeedbackMessage::get_ts()
    }

    fn create_msg() -> *mut Self::CStruct {
        let message = Fibonacci::FeedbackMessage::create_msg();
        if !message.is_null() {
            FEEDBACK_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }

    fn destroy_msg(message: *mut Self::CStruct) {
        FEEDBACK_DESTROYED.fetch_add(1, Ordering::Relaxed);
        Fibonacci::FeedbackMessage::destroy_msg(message);
    }

    fn from_native(message: &Self::CStruct) -> Self {
        let message = Fibonacci::FeedbackMessage::from_native(message);
        Self {
            goal_id: message.goal_id,
            feedback: message.feedback,
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        self.goal_id.copy_to_native(&mut message.goal_id);
        self.feedback.copy_to_native(&mut message.feedback);
    }
}

#[derive(Clone, Debug)]
struct TrackedFeedbackAction;

unsafe impl WrappedActionTypeSupport for TrackedFeedbackAction {
    type Goal = Fibonacci::Goal;
    type Result = Fibonacci::Result;
    type Feedback = Fibonacci::Feedback;
    type FeedbackMessage = TrackedFeedbackMessage;
    type SendGoal = Fibonacci::SendGoal::Service;
    type GetResult = Fibonacci::GetResult::Service;

    fn get_ts() -> &'static rosidl_action_type_support_t {
        Fibonacci::Action::get_ts()
    }

    fn make_goal_request_msg(
        goal_id: unique_identifier_msgs::msg::UUID, goal: Self::Goal,
    ) -> Fibonacci::SendGoal::Request {
        Fibonacci::SendGoal::Request { goal_id, goal }
    }

    fn make_goal_response_msg(
        accepted: bool, stamp: builtin_interfaces::msg::Time,
    ) -> Fibonacci::SendGoal::Response {
        Fibonacci::SendGoal::Response { accepted, stamp }
    }

    fn make_feedback_msg(
        goal_id: unique_identifier_msgs::msg::UUID, feedback: Self::Feedback,
    ) -> Self::FeedbackMessage {
        TrackedFeedbackMessage { goal_id, feedback }
    }

    fn make_result_request_msg(
        goal_id: unique_identifier_msgs::msg::UUID,
    ) -> Fibonacci::GetResult::Request {
        Fibonacci::GetResult::Request { goal_id }
    }

    fn make_result_response_msg(
        status: i8, result: Self::Result,
    ) -> Fibonacci::GetResult::Response {
        Fibonacci::GetResult::Response { status, result }
    }

    fn destructure_goal_request_msg(
        message: Fibonacci::SendGoal::Request,
    ) -> (unique_identifier_msgs::msg::UUID, Self::Goal) {
        (message.goal_id, message.goal)
    }

    fn destructure_goal_response_msg(
        message: Fibonacci::SendGoal::Response,
    ) -> (bool, builtin_interfaces::msg::Time) {
        (message.accepted, message.stamp)
    }

    fn destructure_feedback_msg(
        message: Self::FeedbackMessage,
    ) -> (unique_identifier_msgs::msg::UUID, Self::Feedback) {
        (message.goal_id, message.feedback)
    }

    fn destructure_result_response_msg(
        message: Fibonacci::GetResult::Response,
    ) -> (i8, Self::Result) {
        (message.status, message.result)
    }

    fn destructure_result_request_msg(
        message: Fibonacci::GetResult::Request,
    ) -> unique_identifier_msgs::msg::UUID {
        message.goal_id
    }
}

fn wait_for_server(node: &mut Node, client: &crate::ActionClient<Fibonacci::Action>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !client.is_available(node).unwrap() {
        assert!(Instant::now() < deadline, "action server discovery timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn complete_goal_with_feedback(
    node: &mut Node, server: &mut ActionServer<TrackedFeedbackAction>,
    client: &crate::ActionClient<Fibonacci::Action>, order: i32, feedback_count: usize,
) {
    let mut response = Box::pin(
        client
            .send_goal_request_with_options(Fibonacci::Goal { order }, ActionGoalOptions::default())
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut server_goal = None;
    let response = loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if server_goal.is_none() {
            if let Some(request) = server.next().now_or_never().flatten() {
                assert_eq!(request.goal.order, order);
                server_goal = Some(request.accept().unwrap().0);
            }
        }
        if let Some(response) = response.as_mut().now_or_never() {
            break response.unwrap();
        }
        assert!(Instant::now() < deadline, "action goal response timed out");
    };
    let mut server_goal = server_goal.unwrap();
    for step in 0..feedback_count {
        server_goal
            .publish_feedback(Fibonacci::Feedback {
                sequence: vec![step as i32; 32],
            })
            .unwrap();
        node.try_spin_once(Duration::from_millis(5)).unwrap();
        assert_eq!(FEEDBACK_CREATED.load(Ordering::Relaxed), 1);
        assert_eq!(FEEDBACK_DESTROYED.load(Ordering::Relaxed), 0);
    }
    server_goal
        .succeed(Fibonacci::Result {
            sequence: vec![order; 32],
        })
        .unwrap();
    let mut result = Box::pin(response.result.unwrap());
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(result) = result.as_mut().now_or_never() {
            let (status, result) = result.unwrap();
            assert_eq!(status, GoalStatus::Succeeded);
            assert_eq!(result.sequence, [order; 32]);
            drop(server_goal);
            return;
        }
        assert!(Instant::now() < deadline, "action result timed out");
    }
}

#[test]
fn typed_action_server_reuses_feedback_native_storage_across_goals() {
    FEEDBACK_CREATED.store(0, Ordering::Relaxed);
    FEEDBACK_DESTROYED.store(0, Ordering::Relaxed);

    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "typed_action_server_feedback_reuse", "").unwrap();
    let action_name =
        format!("/typed_action_server_feedback_reuse_{}", uuid::Uuid::new_v4().simple());
    let mut server = node
        .create_action_server::<TrackedFeedbackAction>(&action_name)
        .unwrap();
    let client = node
        .create_action_client::<Fibonacci::Action>(&action_name)
        .unwrap();
    wait_for_server(&mut node, &client);

    complete_goal_with_feedback(&mut node, &mut server, &client, 7, 4);
    assert_eq!(FEEDBACK_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(FEEDBACK_DESTROYED.load(Ordering::Relaxed), 0);

    complete_goal_with_feedback(&mut node, &mut server, &client, 11, 3);
    assert_eq!(FEEDBACK_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(FEEDBACK_DESTROYED.load(Ordering::Relaxed), 0);

    node.destroy_action_server(server).unwrap();
    assert_eq!(FEEDBACK_DESTROYED.load(Ordering::Relaxed), 1);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual typed action server feedback native reuse allocation stage; LD_PRELOAD probe required"]
fn benchmark_typed_action_server_feedback_fresh_vs_reused_native_decode() {
    use crate::allocation_probe;
    const ITERATIONS: usize = 10_000;

    fn measure(
        probe: &allocation_probe::Probe, mut operation: impl FnMut(),
    ) -> (allocation_probe::NativeStats, u128) {
        let mut stats = allocation_probe::NativeStats::default();
        assert_eq!(unsafe { (probe.begin)() }, 0);
        let begin = Instant::now();
        for _ in 0..ITERATIONS {
            operation();
        }
        let elapsed_ns = begin.elapsed().as_nanos();
        assert_eq!(unsafe { (probe.end)(&mut stats) }, 0);
        assert_eq!(stats.failed_allocations + stats.counter_overflows + stats.size_overflows, 0);
        (stats, elapsed_ns)
    }

    let probe = allocation_probe::Probe::load()
        .unwrap()
        .expect("LD_PRELOAD allocation probe required");
    let message = TrackedFeedbackMessage {
        goal_id: unique_identifier_msgs::msg::UUID { uuid: [7; 16] },
        feedback: Fibonacci::Feedback {
            sequence: vec![42; 32],
        },
    };

    let mut fresh_step = || {
        let native = WrappedNativeMsg::<TrackedFeedbackMessage>::try_from(&message).unwrap();
        std::hint::black_box(native);
    };
    for _ in 0..1000 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let buffer = NativeBuffer::default();
    drop(
        buffer
            .checkout(|| WrappedNativeMsg::try_from(&message))
            .unwrap(),
    );
    let mut reused_step = || {
        let mut native = buffer
            .checkout(|| panic!("typed action server feedback slot was not recycled"))
            .unwrap();
        message.try_copy_to_native(&mut native).unwrap();
        std::hint::black_box(native);
    };
    for _ in 0..1000 {
        reused_step();
    }
    let (reused, reused_elapsed_ns) = measure(&probe, &mut reused_step);

    let allocations = |stats: &allocation_probe::NativeStats| {
        stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls
    };
    assert!(allocations(&fresh) - allocations(&reused) >= ITERATIONS as u64 * 2);
    assert!(fresh.free_calls - reused.free_calls >= ITERATIONS as u64 * 2);
    assert!(fresh.requested_bytes - reused.requested_bytes >= ITERATIONS as u64 * 32 * 4);
    for (storage, stats, elapsed_ns) in [
        ("fresh_native", &fresh, fresh_elapsed_ns),
        ("reused_native", &reused, reused_elapsed_ns),
    ] {
        println!(
            "{}",
            serde_json::json!({
                "boundary":"typed action server feedback native fill; no RCL/DDS",
                "native_storage":storage,
                "iterations":ITERATIONS,
                "elapsed_ns":elapsed_ns,
                "allocations":allocations(stats),
                "frees":stats.free_calls,
                "requested_bytes":stats.requested_bytes
            })
        );
    }
}

#[test]
fn feedback_native_buffer_retains_sequence_capacity() {
    let message = TrackedFeedbackMessage {
        goal_id: unique_identifier_msgs::msg::UUID { uuid: [7; 16] },
        feedback: Fibonacci::Feedback {
            sequence: vec![42; 32],
        },
    };
    let buffer = NativeBuffer::default();
    let mut native = buffer
        .checkout(|| WrappedNativeMsg::try_from(&message))
        .unwrap();
    message.try_copy_to_native(&mut native).unwrap();
    assert_eq!(native.feedback.sequence.capacity, 32);
    assert_eq!(native.feedback.sequence.size, 32);
    drop(native);

    let mut native = buffer
        .checkout(|| panic!("typed action server feedback slot was not recycled"))
        .unwrap();
    assert_eq!(native.feedback.sequence.capacity, 32);
    message.try_copy_to_native(&mut native).unwrap();
    assert_eq!(native.feedback.sequence.size, 32);
}
