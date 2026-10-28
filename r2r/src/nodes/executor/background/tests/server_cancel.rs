//! Fanout cancellation must advance without unrelated native events.

use super::*;
use crate::{
    action_msgs::srv::CancelGoal, example_interfaces::action::Fibonacci, ActionClientOptions,
    ActionServerOptions, GoalStatus,
};
use std::collections::HashSet;
use std::sync::atomic::Ordering;

macro_rules! fanout_test {
    ($name:ident, $count:expr, $server:expr, $finish:expr) => {
        #[tokio::test(flavor = "current_thread")]
        async fn $name() {
            const COUNT: usize = $count;
            let context = RosContext::create().unwrap();
            let mut owned = node(&context, stringify!($name));
            let name = format!("/server_fanout_{}", uuid::Uuid::new_v4().simple());
            let mut requests = ($server)(&mut owned, &name).unwrap();
            let client = owned
                .create_action_client_with_options::<Fibonacci::Action>(
                    &name,
                    ActionClientOptions::default()
                        .result_service_qos(QosProfile::services_default().keep_last(64)),
                )
                .unwrap();
            let cancel = owned
                .create_client::<CancelGoal::Service>(
                    &format!("{name}/_action/cancel_goal"),
                    QosProfile::default(),
                )
                .unwrap();
            let mut executor = Executor::new(context.clone());
            let id = executor.add_node(owned).unwrap();
            let native = executor.nodes[0].node.action_servers[0].clone();
            let guard = FutureGuard::new(&context).unwrap();
            executor.cache_future_guard(guard.clone());
            let task = executor.spawn().unwrap();
            tokio::time::timeout(Duration::from_secs(5), client.wait_for_action_server().unwrap())
                .await
                .unwrap()
                .unwrap();
            let mut goals = Vec::new();
            let mut observations = Vec::new();
            for order in 0..COUNT as i32 {
                let response = client.send_goal_request(Fibonacci::Goal { order }).unwrap();
                let request = tokio::time::timeout(Duration::from_secs(5), requests.next())
                    .await
                    .unwrap()
                    .unwrap();
                let (goal, cancels) = request.accept_deferred().unwrap();
                observations.push(
                    tokio::time::timeout(Duration::from_secs(5), response)
                        .await
                        .unwrap()
                        .unwrap(),
                );
                goal.execute().unwrap();
                goals.push((goal, cancels));
            }
            let first = cancel.request(&CancelGoal::Request::default()).unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                while !native.lock().has_pending_cancel_work()
                    || native.lock().has_pending_cancel_delivery()
                    || native.lock().has_runnable_cancel_work()
                    || !guard.waiting.load(Ordering::Acquire)
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("first fanout delivery or decision registration stalled");
            let second = cancel.request(&CancelGoal::Request::default()).unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                while !native.lock().has_pending_cancel_delivery()
                    || native.lock().has_runnable_cancel_work()
                    || !guard.waiting.load(Ordering::Acquire)
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("second fanout did not park its full senders");
            let triggers = guard.triggers.load(Ordering::Relaxed);
            tokio::time::sleep(Duration::from_millis(10)).await;
            assert_eq!(
                guard.triggers.load(Ordering::Relaxed),
                triggers,
                "blocked fanout kept waking itself"
            );
            // Retain the previous guard and move the blocked node to a fresh
            // executor. Cached per-goal wakes must use the current binding.
            let mut recovered = task.stop().await.unwrap();
            let old_triggers = guard.triggers.load(Ordering::Relaxed);
            let transferred = recovered.remove_node(id).unwrap();
            let mut next = Executor::new(context.clone());
            next.add_node(transferred).unwrap();
            let current = FutureGuard::new(&context).unwrap();
            next.cache_future_guard(current.clone());
            let task = next.spawn().unwrap();
            tokio::time::timeout(Duration::from_secs(5), async {
                while !current.waiting.load(Ordering::Acquire)
                    || native.lock().has_runnable_cancel_work()
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("transferred cancellation worker did not wait");
            // Recover only the final goal first, while 39 streams stay full.
            let request = tokio::time::timeout(Duration::from_secs(1), goals[COUNT - 1].1.next())
                .await
                .expect("last goal lost its initial cancellation")
                .unwrap();
            assert_eq!(request.uuid, goals[COUNT - 1].0.uuid);
            request.reject();
            let request = tokio::time::timeout(Duration::from_secs(1), goals[COUNT - 1].1.next())
                .await
                .expect("last full goal was not retried")
                .unwrap();
            assert_eq!(request.uuid, goals[COUNT - 1].0.uuid);
            request.reject();
            assert!(native.lock().has_pending_cancel_delivery());
            assert!(goals.iter().all(|(goal, _)| !goal.is_cancelling().unwrap()));
            for (i, (goal, stream)) in goals.iter_mut().enumerate().take(COUNT - 1) {
                let request = tokio::time::timeout(Duration::from_secs(1), stream.next())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(request.uuid, goal.uuid);
                request.reject();
                let request = tokio::time::timeout(Duration::from_secs(1), stream.next())
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(request.uuid, goal.uuid);
                if i % 2 == 0 {
                    request.accept();
                } else {
                    request.reject();
                }
            }
            let rejected = tokio::time::timeout(Duration::from_secs(5), first)
                .await
                .expect("first cancellation decisions did not finish")
                .unwrap();
            assert_eq!(rejected.return_code, CancelGoal::Response::ERROR_REJECTED as i8);
            assert!(rejected.goals_canceling.is_empty());
            let accepted = tokio::time::timeout(Duration::from_secs(5), second)
                .await
                .expect("second cancellation decisions did not finish")
                .unwrap();
            assert_eq!(accepted.return_code, CancelGoal::Response::ERROR_NONE as i8);
            let expected: HashSet<_> = goals
                .iter()
                .enumerate()
                .filter(|(i, _)| i % 2 == 0)
                .map(|(_, (goal, _))| goal.uuid)
                .collect();
            let actual: HashSet<_> = accepted
                .goals_canceling
                .iter()
                .map(|info| crate::msg_types::uuid_msg_to_uuid(&info.goal_id).unwrap())
                .collect();
            assert_eq!(actual, expected);
            for (i, (goal, _)) in goals.iter_mut().enumerate() {
                assert_eq!(goal.is_cancelling().unwrap(), i % 2 == 0);
                ($finish)(goal, i % 2 == 0, i as i32).unwrap();
            }
            for (i, (_, result, _)) in observations.into_iter().enumerate() {
                let (status, result) = tokio::time::timeout(Duration::from_secs(5), result)
                    .await
                    .unwrap_or_else(|error| panic!("terminal result {i} timed out: {error}"))
                    .unwrap();
                assert_eq!(
                    status,
                    if i % 2 == 0 {
                        GoalStatus::Canceled
                    } else {
                        GoalStatus::Succeeded
                    }
                );
                assert_eq!(result.sequence, [i as i32]);
            }
            let returned = task.stop().await.unwrap();
            assert_eq!(returned.len(), 1);
            assert!(context.is_valid());
            assert_eq!(
                guard.triggers.load(Ordering::Relaxed),
                old_triggers,
                "capacity/decision wakes used the prior executor"
            );
        }
    };
}

fanout_test!(
    typed_server_cancel_fanout_progresses_after_capacity_recovery_and_transfer,
    40,
    |node: &mut Node, name: &str| node.create_action_server_with_options::<Fibonacci::Action>(
        name,
        ActionServerOptions::default()
            .cancel_request_channel_capacity(0)
            .result_service_qos(QosProfile::services_default().keep_last(64))
    ),
    |goal: &mut crate::ActionServerGoal<Fibonacci::Action>, canceled, order| {
        let result = Fibonacci::Result {
            sequence: vec![order],
        };
        if canceled {
            goal.cancel(result)
        } else {
            goal.succeed(result)
        }
    }
);
fanout_test!(
    untyped_server_cancel_fanout_progresses_after_capacity_recovery_and_transfer,
    40,
    |node: &mut Node, name: &str| node.create_action_server_untyped_with_options(
        name,
        "example_interfaces/action/Fibonacci",
        ActionServerOptions::default()
            .cancel_request_channel_capacity(0)
            .result_service_qos(QosProfile::services_default().keep_last(64))
    ),
    |goal: &mut crate::ActionServerGoalUntyped, canceled, order| {
        let result = serde_json::json!({"sequence":[order]});
        if canceled {
            goal.cancel(result)
        } else {
            goal.succeed(result)
        }
    }
);

fanout_test!(
    small_server_cancel_streams_follow_current_executor_after_transfer,
    2,
    |node: &mut Node, name: &str| node.create_action_server_with_options::<Fibonacci::Action>(
        name,
        ActionServerOptions::default()
            .cancel_request_channel_capacity(0)
            .result_service_qos(QosProfile::services_default().keep_last(64))
    ),
    |goal: &mut crate::ActionServerGoal<Fibonacci::Action>, canceled, order| {
        let result = Fibonacci::Result {
            sequence: vec![order],
        };
        if canceled {
            goal.cancel(result)
        } else {
            goal.succeed(result)
        }
    }
);
