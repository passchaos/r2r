use std::time::Duration;

use futures::{channel::oneshot, StreamExt};
use r2r::{example_interfaces::action::Fibonacci, ActionServerGoal, GoalStatus};

const GOALS: i32 = 4;

fn main() -> r2r::Result<()> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "action_goal_batch_example", "")?;
    let mut requests = node.create_action_server::<Fibonacci::Action>("batch_fibonacci")?;
    let client = node.create_action_client::<Fibonacci::Action>("batch_fibonacci")?;
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
        let mut goals = r2r::ActionServerGoalRequest::accept_batch(incoming)?
            .into_iter()
            .map(|(goal, _cancels)| goal)
            .collect::<Vec<_>>();
        assert!(goals
            .iter()
            .all(|goal| matches!(goal.get_status(), Ok(GoalStatus::Executing))));

        // Wait until every client-side feedback/result observer exists.
        ready_receiver
            .await
            .map_err(|_| r2r::Error::RCL_RET_ACTION_SERVER_INVALID)?;
        for goal in &goals {
            goal.publish_feedback(Fibonacci::Feedback {
                sequence: vec![goal.goal.order; 128],
            })?;
        }
        feedback_receiver
            .await
            .map_err(|_| r2r::Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let results = goals
            .iter()
            .map(|goal| Fibonacci::Result {
                sequence: vec![goal.goal.order],
            })
            .collect();
        ActionServerGoal::succeed_batch(&mut goals, results)
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
    println!("completed {GOALS} goals with batched status snapshots");
    Ok(())
}
