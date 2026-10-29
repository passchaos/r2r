use std::time::Duration;

use futures::StreamExt;
use r2r::{std_msgs::msg::UInt64MultiArray, FutureExecutionOptions, QosProfile};

fn main() -> r2r::Result<()> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "native_callback_forward", "")?;
    let qos = QosProfile::default().keep_last(16);
    let input = node.create_publisher::<UInt64MultiArray>("native_callback_input", qos.clone())?;
    let output =
        node.create_publisher::<UInt64MultiArray>("native_callback_output", qos.clone())?;
    let forward = output.clone();
    let callback = node.subscribe_native_callback::<UInt64MultiArray, _>(
        "native_callback_input",
        qos.clone(),
        move |message, _info| forward.publish_native_ref(&message),
    )?;
    let mut received = node.subscribe::<UInt64MultiArray>("native_callback_output", qos)?;
    let execution = FutureExecutionOptions::wake_driven();

    let input_ready = input.wait_for_subscribers()?;
    node.spin_until_future_complete_with_options(input_ready, Duration::from_secs(3), execution)?;
    let output_ready = received.wait_for_publishers(&mut node)?;
    node.spin_until_future_complete_with_options(output_ready, Duration::from_secs(3), execution)?;

    input.publish(&UInt64MultiArray {
        data: vec![3, 5, 8, 13],
        ..Default::default()
    })?;
    let message = node.spin_until_future_complete_with_options(
        async {
            received
                .next()
                .await
                .ok_or(r2r::Error::RCL_RET_SUBSCRIPTION_INVALID)
        },
        Duration::from_secs(3),
        execution,
    )?;
    assert_eq!(message.data, [3, 5, 8, 13]);
    assert_eq!(callback.topic_name(&node)?, "/native_callback_input");
    println!(
        "forwarded a native message without constructing an owned Rust payload in the callback"
    );
    Ok(())
}
