#[cfg(any(
    r2r__ros__distro__iron,
    r2r__ros__distro__jazzy,
    r2r__ros__distro__kilted,
    r2r__ros__distro__lyrical,
    r2r__ros__distro__rolling
))]
fn main() -> r2r::Result<()> {
    use std::time::Duration;

    use futures::StreamExt;
    use r2r::{example_interfaces::srv::AddTwoInts, QosProfile, ServiceIntrospectionState};

    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "service_introspection_example", "")?;
    let mut service =
        node.create_service::<AddTwoInts::Service>("add_two_ints", QosProfile::default())?;
    let client =
        node.create_client::<AddTwoInts::Service>("add_two_ints", QosProfile::default())?;

    // Retain events during discovery so the example also works with a late subscriber.
    let event_qos = QosProfile::default().transient_local();
    let mut events =
        node.subscribe::<AddTwoInts::Event>("add_two_ints/_service_event", event_qos.clone())?;
    service.configure_introspection(
        &mut node,
        ServiceIntrospectionState::Contents,
        event_qos.clone(),
    )?;
    client.configure_introspection(&mut node, ServiceIntrospectionState::Contents, event_qos)?;

    let available = client.wait_for_service()?;
    node.spin_until_future_complete(available, Duration::from_secs(5))?;

    let work = async move {
        let response = client.request(&AddTwoInts::Request { a: 19, b: 23 })?;
        let serve = async {
            let request = service
                .next()
                .await
                .ok_or(r2r::Error::RCL_RET_SERVICE_INVALID)?;
            let sum = request.message.a + request.message.b;
            request.respond(AddTwoInts::Response { sum })
        };
        let inspect = async {
            for _ in 0..4 {
                let event = events
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_SUBSCRIPTION_INVALID)?;
                println!(
                    "event={} sequence={} timestamp={}.{:09} request={:?} response={:?}",
                    event.info.event_type,
                    event.info.sequence_number,
                    event.info.stamp.sec,
                    event.info.stamp.nanosec,
                    event.request,
                    event.response,
                );
            }
            Ok::<_, r2r::Error>(())
        };
        let (_, response, _) = futures::try_join!(serve, response, inspect)?;
        println!("sum={}", response.sum);
        Ok(())
    };
    node.spin_until_future_complete(work, Duration::from_secs(5))
}

#[cfg(not(any(
    r2r__ros__distro__iron,
    r2r__ros__distro__jazzy,
    r2r__ros__distro__kilted,
    r2r__ros__distro__lyrical,
    r2r__ros__distro__rolling
)))]
fn main() {
    eprintln!("Service introspection requires ROS 2 Iron or newer.");
}
