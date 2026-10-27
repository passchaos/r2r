use std::{task::Poll, time::Duration};

use futures::{channel::oneshot, future, StreamExt};
use r2r::{example_interfaces::action::Fibonacci, GoalStatus};

fn main() -> r2r::Result<()> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "action_lifecycle_example", "")?;
    let mut requests = node.create_action_server::<Fibonacci::Action>("lifecycle_fibonacci")?;
    let client = node.create_action_client::<Fibonacci::Action>("lifecycle_fibonacci")?;
    let available = client.wait_for_action_server()?;
    let execution = r2r::FutureExecutionOptions::wake_driven();
    node.spin_until_future_complete_with_options(available, Duration::from_secs(3), execution)?;

    for cancel in [false, true] {
        let (ready_sender, ready_receiver) = oneshot::channel();
        let (feedback_sender, feedback_receiver) = oneshot::channel();
        let server = async {
            let request = requests
                .next()
                .await
                .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)?;
            assert_eq!(request.goal.order, 5);
            let (mut goal, mut cancels) = request.accept_deferred()?;
            assert_eq!(goal.get_status()?, GoalStatus::Accepted);
            // Start only after the client has installed its feedback/result observers.
            ready_receiver
                .await
                .map_err(|_| r2r::Error::RCL_RET_ACTION_SERVER_INVALID)?;
            goal.execute()?;
            goal.publish_feedback(Fibonacci::Feedback {
                sequence: vec![0, 1, 1, 2, 3],
            })?;
            feedback_receiver
                .await
                .map_err(|_| r2r::Error::RCL_RET_ACTION_SERVER_INVALID)?;
            let result = Fibonacci::Result {
                sequence: vec![0, 1, 1, 2, 3, 5],
            };
            if cancel {
                let request = cancels
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)?;
                assert_eq!(request.uuid, goal.uuid);
                request.accept();
                // Node::spin_until_future_complete polls this state while applying the native
                // cancellation decision; it uses a wall-clock deadline for the whole workflow.
                future::poll_fn(|_| match goal.is_cancelling() {
                    Ok(true) => Poll::Ready(Ok(())),
                    Ok(false) => Poll::Pending,
                    Err(error) => Poll::Ready(Err(error)),
                })
                .await?;
                goal.cancel(result)?;
            } else {
                goal.succeed(result)?;
            }
            Ok::<_, r2r::Error>(())
        };
        let requester = async {
            let (goal, result, mut feedback) = client
                .send_goal_request(Fibonacci::Goal { order: 5 })?
                .await?;
            ready_sender
                .send(())
                .map_err(|_| r2r::Error::RCL_RET_ACTION_CLIENT_INVALID)?;
            let message = feedback
                .next()
                .await
                .ok_or(r2r::Error::RCL_RET_ACTION_CLIENT_INVALID)?;
            assert_eq!(message.sequence, vec![0, 1, 1, 2, 3]);
            feedback_sender
                .send(())
                .map_err(|_| r2r::Error::RCL_RET_ACTION_CLIENT_INVALID)?;
            if cancel {
                goal.cancel()?.await?;
            }
            let (status, result) = result.await?;
            assert_eq!(
                status,
                if cancel {
                    GoalStatus::Canceled
                } else {
                    GoalStatus::Succeeded
                }
            );
            assert_eq!(result.sequence, vec![0, 1, 1, 2, 3, 5]);
            println!("goal {}: {status:?}, result={:?}", goal.uuid, result.sequence);
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
    }
    Ok(())
}
