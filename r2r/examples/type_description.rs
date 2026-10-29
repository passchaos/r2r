//! Query native type schemas through the wake-driven ROS executor.
#[cfg(all(
    not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
    r2r__type_description_interfaces__srv__GetTypeDescription
))]
fn main() -> r2r::Result<()> {
    use r2r::{std_msgs::msg::UInt64, FutureExecutionOptions, NodeOptions, QosProfile};
    use std::time::Duration;
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create_with_options(
        context,
        "type_description_example",
        "",
        NodeOptions::default().start_type_description_service(true),
    )?;
    let _publisher = node.create_publisher::<UInt64>("values", QosProfile::default())?;
    let client = node.create_type_description_client(&node.fully_qualified_name()?)?;
    let hash = r2r::message_type_hash::<UInt64>()?;
    let ready = client.wait_for_service()?;
    let description = node.spin_until_future_complete_with_options(
        async {
            ready.await?;
            client.describe("std_msgs/msg/UInt64", &hash, true)?.await
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )?;
    assert_eq!(description.type_description.type_description.fields[0].name, "data");
    assert!(!description.type_sources.is_empty());
    println!(
        "{}: {} fields, {} source files, {}",
        description.type_description.type_description.type_name,
        description.type_description.type_description.fields.len(),
        description.type_sources.len(),
        hash
    );
    Ok(())
}

#[cfg(not(all(
    not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
    r2r__type_description_interfaces__srv__GetTypeDescription
)))]
fn main() -> r2r::Result<()> {
    println!("standard native type-description service requires Jazzy or newer and type_description_interfaces");
    let mut node = r2r::Node::create(r2r::Context::create()?, "type_description_boundary", "")?;
    assert!(matches!(
        node.start_type_description_service(),
        Err(r2r::Error::RCL_RET_UNSUPPORTED)
    ));
    Ok(())
}
