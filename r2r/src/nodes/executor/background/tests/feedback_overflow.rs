//! Coalesced native feedback and pending capacity recovery survive executor transfer.

use super::*;
use crate::{
    example_interfaces::action::Fibonacci, ActionClientOptions, ActionServerOptions,
    FeedbackOverflowPolicy, GoalStatus, QosProfile,
};
use futures::{FutureExt, StreamExt};

macro_rules! background_overflow_test {
    ($name:ident, $make:expr, $goal:expr, $feedback:expr, $result:expr) => {
        #[tokio::test(flavor = "current_thread")]
        async fn $name() {
            let context = RosContext::create().unwrap();
            let mut owned = node(&context, stringify!($name));
            let name = format!("/background_overflow_{}", uuid::Uuid::new_v4().simple());
            let qos = QosProfile::default().keep_last(64);
            let mut requests = owned
                .create_action_server_with_options::<Fibonacci::Action>(
                    &name,
                    ActionServerOptions::default().feedback_topic_qos(qos.clone()),
                )
                .unwrap();
            let client = ($make)(
                &mut owned,
                &name,
                ActionClientOptions::default()
                    .feedback_channel_capacity(0)
                    .feedback_topic_qos(qos)
                    .feedback_overflow_policy(FeedbackOverflowPolicy::Coalesce),
            )
            .unwrap();
            let mut executor = Executor::new(context.clone());
            let id = executor.add_node(owned).unwrap();
            let old_guard = FutureGuard::new(&context).unwrap();
            executor.cache_future_guard(old_guard.clone());
            let task = executor.spawn().unwrap();
            tokio::time::timeout(Duration::from_secs(5), client.wait_for_action_server().unwrap())
                .await
                .unwrap()
                .unwrap();
            let mut goals = Vec::new();
            let mut observations = Vec::new();
            for order in 0..2 {
                let response = client.send_goal_request(($goal)(order)).unwrap();
                let request = tokio::time::timeout(Duration::from_secs(5), requests.next())
                    .await
                    .unwrap()
                    .unwrap();
                let goal = request.accept_deferred().unwrap();
                observations.push(
                    tokio::time::timeout(Duration::from_secs(5), response)
                        .await
                        .unwrap()
                        .unwrap(),
                );
                goal.0.execute().unwrap();
                goals.push(goal);
            }
            for value in 0..32 {
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
            let (_healthy, healthy_result, mut healthy_feedback) = observations.pop().unwrap();
            let (_blocked, blocked_result, mut blocked_feedback) = observations.pop().unwrap();
            let healthy =
                tokio::time::timeout(Duration::from_secs(1), healthy_feedback.next()).await;
            if healthy.is_err() {
                task.stop().await.unwrap();
                panic!("parked goal blocked background ingress");
            }
            assert_eq!(($feedback)(healthy.unwrap().unwrap()), [99]);
            let mut recovered = task.stop().await.unwrap();
            let owned = recovered.remove_node(id).unwrap();
            let triggers = old_guard
                .triggers
                .load(std::sync::atomic::Ordering::Relaxed);
            assert_eq!(($feedback)(blocked_feedback.next().now_or_never().unwrap().unwrap()), [0]);
            assert_eq!(
                old_guard
                    .triggers
                    .load(std::sync::atomic::Ordering::Relaxed),
                triggers,
                "unbound capacity recovery woke the old native guard"
            );
            let mut replacement = Executor::new(context.clone());
            replacement.add_node(owned).unwrap();
            let task = replacement.spawn().unwrap();
            let latest =
                tokio::time::timeout(Duration::from_secs(1), blocked_feedback.next()).await;
            if latest.is_err() {
                task.stop().await.unwrap();
                panic!("coalesced capacity work did not follow node transfer");
            }
            assert_eq!(($feedback)(latest.unwrap().unwrap()), [31]);
            assert!(blocked_feedback.next().now_or_never().is_none());
            for (i, (goal, _)) in goals.iter_mut().enumerate() {
                goal.succeed(Fibonacci::Result {
                    sequence: vec![i as i32, i as i32 + 1],
                })
                .unwrap();
            }
            for (i, result) in [blocked_result, healthy_result].into_iter().enumerate() {
                let (status, result) = tokio::time::timeout(Duration::from_secs(5), result)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(status, GoalStatus::Succeeded);
                assert_eq!(($result)(result), [i as i32, i as i32 + 1]);
            }
            assert!(matches!(healthy_feedback.next().now_or_never(), Some(None)));
            assert!(matches!(blocked_feedback.next().now_or_never(), Some(None)));
            task.stop().await.unwrap();
            assert!(context.is_valid());
        }
    };
}

background_overflow_test!(
    typed_feedback_overflow_native_and_capacity_transfer,
    |node: &mut Node, name: &str, options| node
        .create_action_client_with_options::<Fibonacci::Action>(name, options),
    |order| Fibonacci::Goal { order },
    |message: Fibonacci::Feedback| message.sequence,
    |message: Fibonacci::Result| message.sequence
);
background_overflow_test!(
    untyped_feedback_overflow_native_and_capacity_transfer,
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
