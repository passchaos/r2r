//! Runs an action server and client as runtime-agnostic tasks in one mutually exclusive group.

use futures::StreamExt;
use r2r::{example_interfaces::action::Fibonacci, GoalStatus};
use std::{sync::mpsc, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context.clone(), "callback_group_action", "")?;
    let mut requests = node.create_action_server::<Fibonacci::Action>("group_fibonacci")?;
    let client = node.create_action_client::<Fibonacci::Action>("group_fibonacci")?;
    let mut group = r2r::CallbackGroup::new();
    let (outcome, received) = mpsc::sync_channel(1);

    let server = group.spawn_task(async move {
        let request = requests
            .next()
            .await
            .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let (mut goal, _cancels) = request.accept_deferred()?;
        goal.execute()?;
        goal.publish_feedback(Fibonacci::Feedback {
            sequence: vec![0, 1, 1, 2, 3],
        })?;
        goal.succeed(Fibonacci::Result {
            sequence: vec![0, 1, 1, 2, 3, 5],
        })?;
        Ok(())
    });
    let requester = group.spawn_task(async move {
        let (_goal, result, mut feedback) = client
            .send_goal_request(Fibonacci::Goal { order: 5 })?
            .await?;
        let feedback = feedback
            .next()
            .await
            .ok_or(r2r::Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let result = result.await?;
        outcome
            .send((feedback.sequence, result))
            .map_err(|_| r2r::Error::RCL_RET_ERROR)?;
        Ok(())
    });

    let mut executor = r2r::CallbackExecutor::new(context, 2)?;
    executor.add_node(node)?;
    executor.add_group(group)?;
    let task = executor.spawn()?;
    let (feedback, (status, result)) = received
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| r2r::Error::RCL_RET_TIMEOUT)?;
    assert_eq!(feedback, [0, 1, 1, 2, 3]);
    assert_eq!(status, GoalStatus::Succeeded);
    assert_eq!(result.sequence, [0, 1, 1, 2, 3, 5]);
    futures::executor::block_on(task.stop())?;
    assert!(server.is_finished());
    assert!(requester.is_finished());
    println!("action server/client futures completed inside one mutually exclusive callback group");
    Ok(())
}
