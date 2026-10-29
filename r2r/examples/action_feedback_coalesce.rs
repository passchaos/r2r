//! Keep the newest pending goal progress while another goal continues receiving feedback.

use futures::StreamExt;
use r2r::{
    example_interfaces::action::Fibonacci, ActionClientOptions, ActionServerOptions, Context,
    Error, Executor, FeedbackOverflowPolicy, GoalStatus, Node, QosProfile,
};
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::create()?;
    let mut node = Node::create(context.clone(), "feedback_coalesce_example", "")?;
    // Retain the native burst. Rust buffering and overflow are separate from DDS history.
    let qos = QosProfile::default().keep_last(64);
    let mut requests = node.create_action_server_with_options::<Fibonacci::Action>(
        "feedback_coalesce",
        ActionServerOptions::default().feedback_topic_qos(qos.clone()),
    )?;
    let client = node.create_action_client_with_options::<Fibonacci::Action>(
        "feedback_coalesce",
        ActionClientOptions::default()
            .feedback_topic_qos(qos)
            .feedback_channel_capacity(0)
            .feedback_overflow_policy(FeedbackOverflowPolicy::Coalesce),
    )?;
    let mut executor = Executor::new(context.clone());
    executor.add_node(node)?;
    let task = executor.spawn()?;
    let outcome = tokio::time::timeout(Duration::from_secs(10), async {
        client.wait_for_action_server()?.await?;
        let mut goals = Vec::new();
        let mut observations = Vec::new();
        for order in 0..2 {
            let response = client.send_goal_request(Fibonacci::Goal { order })?;
            let request = requests
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
            assert_eq!(request.goal.order, order);
            let goal = request.accept_deferred()?;
            observations.push(response.await?);
            goal.0.execute()?;
            goals.push(goal);
        }
        for value in 0..32 {
            goals[0].0.publish_feedback(Fibonacci::Feedback {
                sequence: vec![value],
            })?;
        }
        goals[1]
            .0
            .publish_feedback(Fibonacci::Feedback { sequence: vec![99] })?;
        let (_healthy, healthy_result, mut healthy_feedback) = observations.pop().unwrap();
        let (_slow, slow_result, mut slow_feedback) = observations.pop().unwrap();
        assert_eq!(
            healthy_feedback
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?
                .sequence,
            [99]
        );
        // The first goal stayed unconsumed while the second progressed. Its queued sample
        // remains ordered, and consuming it wakes delivery of the coalesced pending sample.
        for expected in [0, 31] {
            assert_eq!(
                slow_feedback
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?
                    .sequence,
                [expected]
            );
        }
        for (i, (goal, _)) in goals.iter_mut().enumerate() {
            goal.succeed(Fibonacci::Result {
                sequence: vec![i as i32, i as i32 + 1],
            })?;
        }
        for (i, result) in [slow_result, healthy_result].into_iter().enumerate() {
            let (status, result) = result.await?;
            assert_eq!(status, GoalStatus::Succeeded);
            assert_eq!(result.sequence, [i as i32, i as i32 + 1]);
        }
        assert!(healthy_feedback.next().await.is_none());
        assert!(slow_feedback.next().await.is_none());
        Ok::<_, Error>(())
    })
    .await;
    let recovered = tokio::time::timeout(Duration::from_secs(5), task.stop()).await??;
    outcome??;
    assert_eq!(recovered.len(), 1);
    assert!(context.is_valid());
    println!("verified independent goal progress, buffered sample 0, coalesced sample 31 and both terminal results");
    Ok(())
}
