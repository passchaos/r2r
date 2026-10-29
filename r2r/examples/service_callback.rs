//! Register short service handlers without an application request-stream loop.
use r2r::{example_interfaces::srv::AddTwoInts, FutureExecutionOptions, QosProfile};
use std::time::Duration;

fn main() -> r2r::Result<()> {
    let mut node = r2r::Node::create(r2r::Context::create()?, "service_callback_example", "")?;
    let typed = node.create_service_callback::<AddTwoInts::Service, _>(
        "typed_sum",
        QosProfile::services_default(),
        |request, _info| {
            Ok(AddTwoInts::Response {
                sum: request.a + request.b,
            })
        },
    )?;
    let native = node.create_service_native_callback::<AddTwoInts::Service, _>(
        "native_sum",
        QosProfile::services_default(),
        |request, response, _info| {
            response.sum = request.a + request.b;
            Ok(())
        },
    )?;
    let a = node.create_client::<AddTwoInts::Service>(
        &typed.service_name(&node)?,
        QosProfile::services_default(),
    )?;
    let b = node.create_client::<AddTwoInts::Service>(
        &native.service_name(&node)?,
        QosProfile::services_default(),
    )?;
    let ready_a = a.wait_for_service()?;
    let ready_b = b.wait_for_service()?;
    node.spin_until_future_complete_with_options(
        async {
            futures::try_join!(ready_a, ready_b)?;
            let (typed, native) = futures::try_join!(
                a.request(&AddTwoInts::Request { a: 12, b: 30 })?,
                b.request(&AddTwoInts::Request { a: 17, b: 25 })?,
            )?;
            assert_eq!(typed.sum, 42);
            assert_eq!(native.sum, 42);
            Ok(())
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )?;
    node.destroy_callback_service(typed);
    node.destroy_callback_service(native);
    println!(
        "typed and scoped native service callbacks returned exact replies through one executor"
    );
    Ok(())
}
