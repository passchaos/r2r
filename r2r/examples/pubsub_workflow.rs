use futures::StreamExt;
use r2r::{std_msgs::msg::UInt64MultiArray, Error, FutureExecutionOptions, QosProfile};
use std::time::Duration;

fn main() -> r2r::Result<()> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "pubsub_workflow_example", "")?;
    let publisher =
        node.create_publisher::<UInt64MultiArray>("workflow_data", QosProfile::default())?;
    let mut received =
        node.subscribe::<UInt64MultiArray>("workflow_data", QosProfile::default())?;
    let mut timer = node.create_wall_timer(Duration::from_millis(5))?;
    let publishers = received.wait_for_publishers(&mut node)?;
    let subscribers = publisher.wait_for_subscribers()?;
    let execution = FutureExecutionOptions::wake_driven();
    let count = node.spin_until_future_complete_with_options(
        async {
            futures::try_join!(publishers, subscribers)?;
            timer.tick().await?;
            let data: Vec<_> = (0..8192).collect();
            publisher.publish(&UInt64MultiArray {
                data: data.clone(),
                ..Default::default()
            })?;
            let acknowledged = publisher.wait_for_all_acked_async()?;
            let message = received
                .next()
                .await
                .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
            assert_eq!(message.data, data);
            match acknowledged.await {
                Ok(()) => println!("reliable publication acknowledged"),
                Err(Error::RCL_RET_UNSUPPORTED) => {
                    println!("middleware does not expose reliable acknowledgements")
                }
                Err(error) => return Err(error),
            }
            Ok(message.data.len())
        },
        Duration::from_secs(3),
        execution,
    )?;
    timer.cancel(&mut node)?;
    node.destroy_timer(timer);
    node.destroy_subscription(received);
    node.destroy_publisher(publisher);
    println!("matched, timed, published and validated {count} values");
    Ok(())
}
