use std::time::Duration;

use futures::{channel::oneshot, StreamExt};
use r2r::{
    example_interfaces::action::Fibonacci, ActionServerGoalRequestUntyped, ActionServerGoalUntyped,
    GoalStatus,
};

const GOALS: i32 = 4;

fn order(goal: &ActionServerGoalUntyped) -> r2r::Result<i32> {
    goal.goal
        .as_ref()
        .ok()
        .and_then(|goal| goal.get("order"))
        .and_then(serde_json::Value::as_i64)
        .and_then(|order| i32::try_from(order).ok())
        .ok_or(r2r::Error::RCL_RET_INVALID_ARGUMENT)
}

fn main() -> r2r::Result<()> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "action_goal_batch_untyped_example", "")?;
    let mut requests = node.create_action_server_untyped(
        "batch_fibonacci_untyped",
        "example_interfaces/action/Fibonacci",
    )?;
    let client = node.create_action_client::<Fibonacci::Action>("batch_fibonacci_untyped")?;
    let execution = r2r::FutureExecutionOptions::wake_driven();
    node.spin_until_future_complete_with_options(
        client.wait_for_action_server()?,
        Duration::from_secs(3),
        execution,
    )?;

    let pending = (1..=GOALS)
        .map(|order| client.send_goal_request(Fibonacci::Goal { order }))
        .collect::<r2r::Result<Vec<_>>>()?;
    let (ready_sender, ready_receiver) = oneshot::channel();
    let (feedback_sender, feedback_receiver) = oneshot::channel();
    let server = async {
        let mut incoming = Vec::with_capacity(GOALS as usize);
        for _ in 0..GOALS {
            incoming.push(
                requests
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)?,
            );
        }
        let mut goals = ActionServerGoalRequestUntyped::accept_deferred_batch(incoming)?
            .into_iter()
            .map(|(goal, _cancels)| goal)
            .collect::<Vec<_>>();
        ActionServerGoalUntyped::execute_batch(&goals)?;
        assert!(goals
            .iter()
            .all(|goal| matches!(goal.get_status(), Ok(GoalStatus::Executing))));

        ready_receiver
            .await
            .map_err(|_| r2r::Error::RCL_RET_ACTION_SERVER_INVALID)?;
        for goal in &goals {
            goal.publish_feedback(serde_json::json!({
                "sequence": vec![order(goal)?; 128]
            }))?;
        }
        feedback_receiver
            .await
            .map_err(|_| r2r::Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let results = goals
            .iter()
            .map(|goal| Ok(serde_json::json!({ "sequence": [order(goal)?] })))
            .collect::<r2r::Result<Vec<_>>>()?;
        ActionServerGoalUntyped::succeed_batch(&mut goals, results)
    };
    let requester = async {
        let mut observations = Vec::with_capacity(GOALS as usize);
        for response in pending {
            observations.push(response.await?);
        }
        ready_sender
            .send(())
            .map_err(|_| r2r::Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        for (index, (_, _, feedback)) in observations.iter_mut().enumerate() {
            let message = feedback
                .next()
                .await
                .ok_or(r2r::Error::RCL_RET_ACTION_CLIENT_INVALID)?;
            assert_eq!(message.sequence, vec![index as i32 + 1; 128]);
        }
        feedback_sender
            .send(())
            .map_err(|_| r2r::Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        for (index, (_, result, _)) in observations.into_iter().enumerate() {
            let (status, result) = result.await?;
            assert_eq!(status, GoalStatus::Succeeded);
            assert_eq!(result.sequence, [index as i32 + 1]);
        }
        Ok::<_, r2r::Error>(())
    };
    node.spin_until_future_complete_with_options(
        async {
            futures::try_join!(server, requester)?;
            Ok(())
        },
        Duration::from_secs(5),
        execution,
    )?;
    println!("completed {GOALS} typed client goals through a runtime-typed batched server");
    Ok(())
}
