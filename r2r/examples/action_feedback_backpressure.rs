//! A slow feedback consumer resumes native delivery without a spin loop.

use futures::{channel::oneshot, StreamExt};
use r2r::{
    example_interfaces::action::Fibonacci, ActionClientOptions, Context, Error, Executor,
    GoalStatus, Node,
};
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::create()?;
    let mut node = Node::create(context.clone(), "feedback_backpressure_example", "")?;
    let mut requests = node.create_action_server::<Fibonacci::Action>("feedback_backpressure")?;
    let client = node.create_action_client_with_options::<Fibonacci::Action>(
        "feedback_backpressure",
        ActionClientOptions::default().feedback_channel_capacity(0),
    )?;
    let mut executor = Executor::new(context.clone());
    executor.add_node(node)?;
    let task = executor.spawn()?;
    let (ready, accepted) = oneshot::channel();
    let (consumed, finished) = oneshot::channel();
    let serving = async {
        let request = requests
            .next()
            .await
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let (mut goal, _cancel) = request.accept_deferred()?;
        accepted
            .await
            .map_err(|_| Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        goal.execute()?;
        goal.publish_feedback(Fibonacci::Feedback { sequence: vec![1] })?;
        goal.publish_feedback(Fibonacci::Feedback { sequence: vec![2] })?;
        // The consumer completes through Rust notifications, without another
        // ROS message that could conceal a missing feedback capacity wake.
        finished
            .await
            .map_err(|_| Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        goal.succeed(Fibonacci::Result {
            sequence: vec![0, 1, 1],
        })
    };
    let consuming = async {
        client.wait_for_action_server()?.await?;
        let (_goal, result, mut feedback) = client
            .send_goal_request(Fibonacci::Goal { order: 3 })?
            .await?;
        ready
            .send(())
            .map_err(|_| Error::RCL_RET_ACTION_SERVER_INVALID)?;
        tokio::time::sleep(Duration::from_millis(20)).await;
        for expected in [1, 2] {
            let message = feedback
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
            assert_eq!(message.sequence, [expected]);
        }
        consumed
            .send(())
            .map_err(|_| Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let (status, result) = result.await?;
        assert_eq!(status, GoalStatus::Succeeded);
        assert_eq!(result.sequence, [0, 1, 1]);
        Ok::<_, Error>(())
    };
    let outcome = tokio::time::timeout(Duration::from_secs(5), async {
        futures::try_join!(serving, consuming)
    })
    .await;
    let recovered = tokio::time::timeout(Duration::from_secs(5), task.stop()).await??;
    outcome??;
    assert_eq!(recovered.len(), 1);
    assert!(context.is_valid());
    println!("verified ordered feedback after a slow consumer and a successful terminal result");
    Ok(())
}
