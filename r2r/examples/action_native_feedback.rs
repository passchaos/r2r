use std::time::Duration;

use futures::StreamExt;
use r2r::{example_interfaces::action::Fibonacci, GoalStatus};

fn main() -> r2r::Result<()> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "action_native_feedback_example", "")?;
    let mut requests =
        node.create_action_server::<Fibonacci::Action>("native_feedback_fibonacci")?;
    let client = node.create_action_client::<Fibonacci::Action>("native_feedback_fibonacci")?;

    let server = async {
        let request = requests
            .next()
            .await
            .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let (mut goal, _cancels) = request.accept_deferred()?;
        goal.execute()?;
        goal.publish_feedback(Fibonacci::Feedback {
            sequence: (0..16_384).collect(),
        })?;
        goal.succeed(Fibonacci::Result {
            sequence: vec![16_384],
        })?;
        Ok::<_, r2r::Error>(())
    };

    let requester = async {
        let (goal, result, mut feedback) = client
            .send_goal_request_native_feedback(Fibonacci::Goal { order: 16_384 })?
            .await?;
        let sample = feedback
            .next()
            .await
            .ok_or(r2r::Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        assert_eq!(sample.goal_id(), goal.uuid);
        assert_eq!(sample.feedback.sequence.size, 16_384);
        // `sample` owns the taken native message lease. No `Fibonacci::Feedback` or 16K-element
        // Rust Vec was constructed on delivery; dropping it returns the container for reuse.
        drop(sample);

        let (status, result) = result.await?;
        assert_eq!(status, GoalStatus::Succeeded);
        assert_eq!(result.sequence, [16_384]);
        assert!(feedback.next().await.is_none());
        Ok::<_, r2r::Error>(())
    };

    node.spin_until_future_complete_with_options(
        async {
            futures::try_join!(server, requester)?;
            Ok(())
        },
        Duration::from_secs(5),
        r2r::FutureExecutionOptions::wake_driven(),
    )
}
