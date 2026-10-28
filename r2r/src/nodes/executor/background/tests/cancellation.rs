//! Canceled operation backlogs must finish after their original enqueue wake.

use super::*;
use crate::example_interfaces::action::Fibonacci;

macro_rules! canceled_observers_test {
    ($test:ident, $client:expr, $goal:expr) => {
        #[tokio::test(flavor = "current_thread")]
        async fn $test() {
            let context = RosContext::create().unwrap();
            let mut owned = node(&context, stringify!($test));
            let name = format!("/cancel_backlog_{}", uuid::Uuid::new_v4().simple());
            let mut requests = owned
                .create_action_server::<Fibonacci::Action>(&name)
                .unwrap();
            let client = ($client)(&mut owned, &name).unwrap();
            let mut executor = Executor::new(context.clone());
            executor.add_node(owned).unwrap();
            let native = executor.nodes[0].node.action_clients[0].clone();
            let task = executor.spawn().unwrap();
            tokio::time::timeout(Duration::from_secs(5), client.wait_for_action_server().unwrap())
                .await
                .unwrap()
                .unwrap();
            let mut server_goals = Vec::new();
            let mut observed = Vec::new();
            for order in 0..40 {
                let response = client.send_goal_request(($goal)(order)).unwrap();
                let request = tokio::time::timeout(Duration::from_secs(5), requests.next())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(request.goal.order, order);
                server_goals.push(request.accept_deferred().unwrap());
                observed.push(
                    tokio::time::timeout(Duration::from_secs(5), response)
                        .await
                        .unwrap()
                        .unwrap(),
                );
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
            // Freeze native maintenance while publishing all cancellations.
            // Keep goal/status handles alive: dropping those would add a
            // separate wake and could conceal missing batch continuation.
            let handles = {
                let native = native.lock();
                let mut handles = Vec::new();
                for (goal, result, feedback) in observed {
                    handles.push(goal);
                    drop((result, feedback));
                }
                assert!(native.has_pending_maintenance());
                handles
            };
            let outcome = tokio::time::timeout(Duration::from_secs(1), async {
                while native.lock().has_pending_maintenance() {
                    tokio::task::yield_now().await;
                }
            })
            .await;
            let recovered = task.stop().await.unwrap();
            outcome.expect("canceled observers stalled after the first batch");
            assert!(handles.iter().all(|goal| !goal.is_result_aware().unwrap()));
            assert!(handles
                .iter()
                .all(|goal| !goal.is_feedback_aware().unwrap()));
            assert!(!native.lock().has_pending_maintenance());
            assert_eq!(recovered.len(), 1);
            assert!(context.is_valid());
            drop((handles, server_goals));
        }
    };
}

canceled_observers_test!(
    typed_canceled_observer_backlog_finishes_without_external_events,
    |node: &mut Node, name: &str| node.create_action_client::<Fibonacci::Action>(name),
    |order| Fibonacci::Goal { order }
);

canceled_observers_test!(
    untyped_canceled_observer_backlog_finishes_without_external_events,
    |node: &mut Node, name: &str| node
        .create_action_client_untyped(name, "example_interfaces/action/Fibonacci"),
    |order| serde_json::json!({"order": order})
);
