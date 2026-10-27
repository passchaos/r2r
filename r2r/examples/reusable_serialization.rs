use std::time::Duration;

use futures::StreamExt;
use r2r::{std_msgs::msg::Int32MultiArray, MessageSerializer, QosProfile};

fn main() -> r2r::Result<()> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "reusable_serialization", "")?;
    let publisher = node.create_publisher_untyped(
        "serialized_array",
        "std_msgs/msg/Int32MultiArray",
        QosProfile::default(),
    )?;
    let mut subscription =
        node.subscribe::<Int32MultiArray>("serialized_array", QosProfile::default())?;
    let matched = publisher.wait_for_subscribers_with_timeout(&mut node, Duration::from_secs(3))?;
    node.spin_until_future_complete(matched, Duration::from_secs(3))?;

    let mut serializer = MessageSerializer::<Int32MultiArray>::with_capacity(16_512)?;
    let mut message = Int32MultiArray::default();
    message.data.reserve(4096);
    for size in [4096, 1024, 0, 4096] {
        message.data.clear();
        message.data.extend(0..size);
        // The slice borrows serializer storage. publish_raw consumes it synchronously, so
        // the next iteration can reuse both native message fields and serialized bytes.
        publisher.publish_raw(serializer.serialize(&message)?)?;
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
        println!("received {size} integers; serialized capacity={}", serializer.capacity());
    }
    Ok(())
}
