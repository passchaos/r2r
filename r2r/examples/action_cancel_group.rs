//! Cancel a group of goals with bounded async execution and partial acceptance.

use futures::StreamExt;
use r2r::{
    example_interfaces::action::Fibonacci, ActionClientOptions, ActionServerOptions, Context,
    Error, Executor, GoalStatus, Node, QosProfile,
};
use std::{collections::HashSet, time::Duration};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    const GOALS: i32 = 40;
    let context = Context::create()?;
    let mut node = Node::create(context.clone(), "action_cancel_group_example", "")?;
    // Retain the complete terminal burst for this application's concurrency.
    let results_qos = QosProfile::services_default().keep_last(64);
    let mut requests = node.create_action_server_with_options::<Fibonacci::Action>(
        "cancel_group",
        ActionServerOptions::default()
            .cancel_request_channel_capacity(0)
            .result_service_qos(results_qos.clone()),
    )?;
    let client = node.create_action_client_with_options::<Fibonacci::Action>(
        "cancel_group",
        ActionClientOptions::default().result_service_qos(results_qos),
    )?;
    let mut executor = Executor::new(context.clone());
    executor.add_node(node)?;
    let task = executor.spawn()?;
    let outcome = tokio::time::timeout(Duration::from_secs(20), async {
        client.wait_for_action_server()?.await?;
        let mut goals = Vec::new();
        let mut observations = Vec::new();
        for order in 0..GOALS {
            let response = client.send_goal_request(Fibonacci::Goal { order })?;
            let request = requests
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
            assert_eq!(request.goal.order, order);
            let (goal, cancels) = request.accept_deferred()?;
            observations.push(response.await?);
            goal.execute()?;
            goals.push((goal, cancels));
        }
        let cancellation = client.cancel_all()?;
        for (i, (goal, requests)) in goals.iter_mut().enumerate() {
            let request = requests
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
            assert_eq!(request.uuid, goal.uuid);
            if i % 2 == 0 {
                request.accept();
            } else {
                request.reject();
            }
        }
        let accepted: HashSet<_> = cancellation.await?.into_iter().collect();
        let expected: HashSet<_> = goals
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 2 == 0)
            .map(|(_, (goal, _))| goal.uuid)
            .collect();
        assert_eq!(accepted, expected);
        for (i, (goal, _)) in goals.iter_mut().enumerate() {
            let result = Fibonacci::Result {
                sequence: vec![i as i32],
            };
            assert_eq!(goal.is_cancelling()?, i % 2 == 0);
            if i % 2 == 0 {
                goal.cancel(result)?;
            } else {
                goal.succeed(result)?;
            }
        }
        for (i, (_goal, result, _feedback)) in observations.into_iter().enumerate() {
            let (status, result) = result.await?;
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
        Ok::<_, Error>(())
    })
    .await;
    let recovered = tokio::time::timeout(Duration::from_secs(5), task.stop()).await??;
    outcome??;
    assert_eq!(recovered.len(), 1);
    assert!(context.is_valid());
    println!("accepted 20 of 40 cancellations and verified all 40 terminal results");
    Ok(())
}
