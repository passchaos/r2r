//! Blocked feedback must retry through native capacity notifications.

use super::*;
use crate::{example_interfaces::action::Fibonacci, ActionClientOptions, GoalStatus};
use std::sync::atomic::Ordering;

macro_rules! feedback_test {
    ($name:ident, $make:expr, $goal:expr, $decode:expr) => {
        #[tokio::test(flavor = "current_thread")]
        async fn $name() {
            let context = RosContext::create().unwrap();
            let mut owned = node(&context, stringify!($name));
            let topic = format!("/feedback_ready_{}", uuid::Uuid::new_v4().simple());
            let mut requests = owned
                .create_action_server::<Fibonacci::Action>(&topic)
                .unwrap();
            let client = ($make)(&mut owned, &topic).unwrap();
            let mut executor = Executor::new(context.clone());
            executor.add_node(owned).unwrap();
            let native = executor.nodes[0].node.action_clients[0].clone();
            let guard = FutureGuard::new(&context).unwrap();
            executor.cache_future_guard(guard.clone());
            let task = executor.spawn().unwrap();
            tokio::time::timeout(Duration::from_secs(5), client.wait_for_action_server().unwrap())
                .await
                .unwrap()
                .unwrap();
            let response = client.send_goal_request(($goal)(4)).unwrap();
            let request = tokio::time::timeout(Duration::from_secs(5), requests.next())
                .await
                .unwrap()
                .unwrap();
            let (mut server_goal, _cancels) = request.accept_deferred().unwrap();
            let (_client_goal, result, mut feedback) =
                tokio::time::timeout(Duration::from_secs(5), response)
                    .await
                    .unwrap()
                    .unwrap();
            server_goal.execute().unwrap();
            for phase in 0..4 {
                server_goal
                    .publish_feedback(Fibonacci::Feedback {
                        sequence: vec![phase * 2],
                    })
                    .unwrap();
                server_goal
                    .publish_feedback(Fibonacci::Feedback {
                        sequence: vec![phase * 2 + 1],
                    })
                    .unwrap();
                tokio::time::timeout(Duration::from_secs(5), async {
                    while !native.lock().has_pending_maintenance()
                        || !guard.waiting.load(Ordering::Acquire)
                    {
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .unwrap();
                let triggers = guard.triggers.load(Ordering::Relaxed);
                tokio::time::sleep(Duration::from_millis(10)).await;
                assert_eq!(
                    guard.triggers.load(Ordering::Relaxed),
                    triggers,
                    "blocked feedback kept waking itself"
                );
                let first = tokio::time::timeout(Duration::from_secs(1), feedback.next())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(($decode)(first), vec![phase * 2]);
                let second = tokio::time::timeout(Duration::from_secs(1), feedback.next()).await;
                if second.is_err() {
                    task.stop().await.unwrap();
                    panic!("capacity recovery did not deliver pending feedback");
                }
                assert_eq!(($decode)(second.unwrap().unwrap()), vec![phase * 2 + 1]);
            }
            server_goal
                .succeed(Fibonacci::Result {
                    sequence: vec![0, 1, 1],
                })
                .unwrap();
            let result = tokio::time::timeout(Duration::from_secs(5), result)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(result.0, GoalStatus::Succeeded);
            task.stop().await.unwrap();
            assert!(context.is_valid());
        }
    };
}

feedback_test!(
    typed_feedback_capacity_recovery_progresses_without_native_traffic,
    |node: &mut Node, name: &str| node.create_action_client_with_options::<Fibonacci::Action>(
        name,
        ActionClientOptions::default().feedback_channel_capacity(0)
    ),
    |order| Fibonacci::Goal { order },
    |message: Fibonacci::Feedback| message.sequence
);

feedback_test!(
    untyped_feedback_capacity_recovery_progresses_without_native_traffic,
    |node: &mut Node, name: &str| node.create_action_client_untyped_with_options(
        name,
        "example_interfaces/action/Fibonacci",
        ActionClientOptions::default().feedback_channel_capacity(0)
    ),
    |order| serde_json::json!({"order":order}),
    |message: crate::Result<serde_json::Value>| serde_json::from_value::<Fibonacci::Feedback>(
        message.unwrap()
    )
    .unwrap()
    .sequence
);
