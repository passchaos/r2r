use std::time::Duration;

use futures::StreamExt;
use r2r::{std_msgs::msg::Int32MultiArray, NativeMsg, QosProfile, WrappedTypesupport};

fn main() -> r2r::Result<()> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "native_message_reuse", "")?;
    let publisher =
        node.create_publisher::<Int32MultiArray>("reused_array", QosProfile::default())?;
    let mut subscription =
        node.subscribe::<Int32MultiArray>("reused_array", QosProfile::default())?;
    let matched = publisher.wait_for_subscribers_with_timeout(&mut node, Duration::from_secs(3))?;
    node.spin_until_future_complete(matched, Duration::from_secs(3))?;

    let mut message = Int32MultiArray::default();
    let mut native = NativeMsg::<Int32MultiArray>::try_new()?;
    message.data.reserve(4096);
    for count in [4096, 1024, 0, 4096] {
        message.data.clear();
        message.data.extend(0..count);
        message.try_copy_to_native(&mut native)?;
        publisher.publish_native(&mut native)?;
        let received = node.spin_until_future_complete(
            async {
                subscription
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_SUBSCRIPTION_INVALID)
            },
            Duration::from_secs(3),
        )?;
        assert_eq!(received, message);
        println!("received {count} samples; retained native capacity={}", native.data.capacity);
    }
    Ok(())
}
