//! Native ingress and retained samples must match the selected overflow policy.

use super::*;
use crate::{example_interfaces::action::Fibonacci, FeedbackOverflowPolicy, GoalStatus};

macro_rules! ingress_test {
    ($name:ident, $make:expr, $goal:expr, $feedback:expr, $result:expr) => {
        #[test]
        fn $name() {
            for policy in [
                FeedbackOverflowPolicy::Backpressure,
                FeedbackOverflowPolicy::Coalesce,
            ] {
                let context = Context::create().unwrap();
                let mut node = Node::create(context, stringify!($name), "").unwrap();
                let name = format!("/feedback_overflow_{}", uuid::Uuid::new_v4().simple());
                let qos = QosProfile::default().keep_last(64);
                let mut requests = node
                    .create_action_server_with_options::<Fibonacci::Action>(
                        &name,
                        ActionServerOptions::default().feedback_topic_qos(qos.clone()),
                    )
                    .unwrap();
                let client = ($make)(
                    &mut node,
                    &name,
                    ActionClientOptions::default()
                        .feedback_channel_capacity(0)
                        .feedback_topic_qos(qos)
                        .feedback_overflow_policy(policy),
                )
                .unwrap();
                let mut goals = Vec::new();
                let mut observed = Vec::new();
                for order in 0..2 {
                    let response = client.send_goal_request(($goal)(order)).unwrap();
                    let (server, response) = node
                        .spin_until_future_complete_with_options(
                            async {
                                let accept = async {
                                    requests
                                        .next()
                                        .await
                                        .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?
                                        .accept_deferred()
                                };
                                futures::try_join!(accept, response)
                            },
                            Duration::from_secs(5),
                            FutureExecutionOptions::wake_driven(),
                        )
                        .unwrap();
                    server.0.execute().unwrap();
                    goals.push(server);
                    observed.push(response);
                }
                for value in [0, 1] {
                    goals[0]
                        .0
                        .publish_feedback(Fibonacci::Feedback {
                            sequence: vec![value],
                        })
                        .unwrap();
                }
                let deadline = Instant::now() + Duration::from_secs(5);
                while !node.action_clients[0].lock().has_pending_maintenance() {
                    assert!(Instant::now() < deadline);
                    node.try_spin_once(Duration::from_millis(1)).unwrap();
                }
                node.try_spin_once(Duration::ZERO).unwrap();
                for value in 2..32 {
                    goals[0]
                        .0
                        .publish_feedback(Fibonacci::Feedback {
                            sequence: vec![value],
                        })
                        .unwrap();
                }
                goals[1]
                    .0
                    .publish_feedback(Fibonacci::Feedback { sequence: vec![99] })
                    .unwrap();
                let (_healthy, healthy_result, mut healthy_feedback) = observed.pop().unwrap();
                let (_blocked, blocked_result, mut blocked_feedback) = observed.pop().unwrap();
                let healthy = node.spin_until_future_complete_with_options(
                    async {
                        healthy_feedback
                            .next()
                            .await
                            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)
                    },
                    Duration::from_millis(250),
                    FutureExecutionOptions::wake_driven(),
                );
                if policy == FeedbackOverflowPolicy::Backpressure {
                    assert!(
                        matches!(healthy, Err(Error::RCL_RET_TIMEOUT)),
                        "default overflow silently omitted feedback"
                    );
                    for value in 0..32 {
                        let message = node
                            .spin_until_future_complete_with_options(
                                async {
                                    blocked_feedback
                                        .next()
                                        .await
                                        .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)
                                },
                                Duration::from_secs(1),
                                FutureExecutionOptions::wake_driven(),
                            )
                            .unwrap();
                        assert_eq!(($feedback)(message), [value]);
                    }
                    let message = node
                        .spin_until_future_complete_with_options(
                            async {
                                healthy_feedback
                                    .next()
                                    .await
                                    .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)
                            },
                            Duration::from_secs(1),
                            FutureExecutionOptions::wake_driven(),
                        )
                        .unwrap();
                    assert_eq!(($feedback)(message), [99]);
                } else {
                    assert_eq!(
                        ($feedback)(healthy.unwrap()),
                        [99],
                        "blocked goal stopped native feedback ingress"
                    );
                    assert_eq!(
                        ($feedback)(blocked_feedback.next().now_or_never().unwrap().unwrap()),
                        [0]
                    );
                    let message = node
                        .spin_until_future_complete_with_options(
                            async {
                                blocked_feedback
                                    .next()
                                    .await
                                    .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)
                            },
                            Duration::from_secs(1),
                            FutureExecutionOptions::wake_driven(),
                        )
                        .unwrap();
                    assert_eq!(($feedback)(message), [31]);
                    assert!(blocked_feedback.next().now_or_never().is_none());
                }
                for (i, (goal, _)) in goals.iter_mut().enumerate() {
                    goal.succeed(Fibonacci::Result {
                        sequence: vec![i as i32, i as i32 + 1],
                    })
                    .unwrap();
                }
                for (i, result) in [blocked_result, healthy_result].into_iter().enumerate() {
                    let (status, result) = node
                        .spin_until_future_complete(result, Duration::from_secs(5))
                        .unwrap();
                    assert_eq!(status, GoalStatus::Succeeded);
                    assert_eq!(($result)(result), [i as i32, i as i32 + 1]);
                }
                assert!(matches!(healthy_feedback.next().now_or_never(), Some(None)));
                assert!(matches!(blocked_feedback.next().now_or_never(), Some(None)));
            }
        }
    };
}

ingress_test!(
    typed_feedback_overflow_preserves_default_and_coalesces_by_goal,
    |node: &mut Node, name: &str, options| node
        .create_action_client_with_options::<Fibonacci::Action>(name, options),
    |order| Fibonacci::Goal { order },
    |message: Fibonacci::Feedback| message.sequence,
    |message: Fibonacci::Result| message.sequence
);

#[test]
fn typed_native_feedback_stream_delivers_the_taken_message_and_closes_at_result() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "typed_native_feedback", "").unwrap();
    let name = format!("/typed_native_feedback_{}", uuid::Uuid::new_v4().simple());
    let mut requests = node
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = node
        .create_action_client_with_options::<Fibonacci::Action>(
            &name,
            ActionClientOptions::default().feedback_channel_capacity(1),
        )
        .unwrap();
    let response = client
        .send_goal_request_native_feedback(Fibonacci::Goal { order: 4096 })
        .unwrap();
    let (mut server, (goal, result, mut feedback)) = node
        .spin_until_future_complete_with_options(
            async {
                let accept = async {
                    requests
                        .next()
                        .await
                        .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?
                        .accept_deferred()
                };
                futures::try_join!(accept, response)
            },
            Duration::from_secs(5),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    server.0.execute().unwrap();
    let expected = (0..4096).collect::<Vec<_>>();
    server
        .0
        .publish_feedback(Fibonacci::Feedback {
            sequence: expected.clone(),
        })
        .unwrap();

    let native = node
        .spin_until_future_complete_with_options(
            async {
                feedback
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)
            },
            Duration::from_secs(5),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    assert_eq!(native.goal_id(), goal.uuid);
    assert_eq!(native.goal_id.uuid, *goal.uuid.as_bytes());
    let decoded = Fibonacci::FeedbackMessage::from_native(&native);
    assert_eq!(decoded.feedback.sequence, expected);
    drop(native);

    server
        .0
        .succeed(Fibonacci::Result {
            sequence: vec![4096],
        })
        .unwrap();
    let (status, result) = node
        .spin_until_future_complete_with_options(
            result,
            Duration::from_secs(5),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    assert_eq!(status, GoalStatus::Succeeded);
    assert_eq!(result.sequence, [4096]);
    assert!(matches!(feedback.next().now_or_never(), Some(None)));
}
ingress_test!(
    untyped_feedback_overflow_preserves_default_and_coalesces_by_goal,
    |node: &mut Node, name: &str, options| node.create_action_client_untyped_with_options(
        name,
        "example_interfaces/action/Fibonacci",
        options
    ),
    |order| serde_json::json!({"order":order}),
    |message: crate::Result<serde_json::Value>| serde_json::from_value::<Fibonacci::Feedback>(
        message.unwrap()
    )
    .unwrap()
    .sequence,
    |message: crate::Result<serde_json::Value>| serde_json::from_value::<Fibonacci::Result>(
        message.unwrap()
    )
    .unwrap()
    .sequence
);
