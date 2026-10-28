use futures::StreamExt;
use r2r::{
    rcl_interfaces::msg::ParameterValue, Executor, FutureExecutionOptions, NativeMsg, Node,
    NodeOptions, QosProfile, WrappedTypesupport,
};
use std::time::Duration;

fn main() -> r2r::Result<()> {
    let context = r2r::Context::create()?;
    let mut publisher_node = Node::create_with_options(
        context.clone(),
        "string_sequence_source",
        "",
        NodeOptions::default().enable_rosout(false),
    )?;
    let mut subscriber_node = Node::create_with_options(
        context.clone(),
        "string_sequence_sink",
        "",
        NodeOptions::default().enable_rosout(false),
    )?;
    let publisher = publisher_node.create_publisher::<ParameterValue>(
        "string_sequence_data",
        QosProfile::default().keep_last(16),
    )?;
    let mut received = subscriber_node
        .subscribe::<ParameterValue>("string_sequence_data", QosProfile::default().keep_last(16))?;
    let matched = publisher.wait_for_subscribers()?;
    let mut executor = Executor::new(context);
    let source_id = executor.add_node(publisher_node)?;
    let sink_id = executor.add_node(subscriber_node)?;
    executor.spin_until_future_complete_with_options(
        async {
            matched.await?;
            let mut native = NativeMsg::<ParameterValue>::try_new()?;
            let mut retained = std::ptr::null_mut();
            for (sequence, count) in [128, 16, 0, 32, 128].into_iter().enumerate() {
                let source = ParameterValue {
                    type_: 9,
                    integer_value: sequence as i64,
                    string_array_value: (0..count)
                        .map(|index| format!("frame_{sequence}_joint_{index:03}_日本語🦀").into())
                        .collect(),
                    ..Default::default()
                };
                source.try_copy_to_native(&mut native)?;
                if sequence == 0 {
                    retained = native.string_array_value.data;
                }
                assert_eq!(native.string_array_value.data, retained);
                assert_eq!(native.string_array_value.capacity, 128);
                publisher.publish_native(&mut native)?;
                let observed = received
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_SUBSCRIPTION_INVALID)?;
                assert_eq!(observed, source);
            }
            Ok(())
        },
        Duration::from_secs(5),
        FutureExecutionOptions::wake_driven(),
    )?;
    executor.node_mut(source_id)?.destroy_publisher(publisher);
    executor.node_mut(sink_id)?.destroy_subscription(received);
    drop(executor.remove_node(source_id)?);
    drop(executor.remove_node(sink_id)?);
    println!("validated five Unicode string-array messages through native publication and DDS");
    Ok(())
}
