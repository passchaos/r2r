//! Preserve one application future while reporting a failed native callback and completing
//! independent service and timer work. Run with a sourced ROS environment.

use futures::{channel::oneshot, FutureExt};
use r2r::{
    example_interfaces::srv::AddTwoInts, std_msgs::msg::Int32, Context, Error,
    FutureExecutionOptions, Node, QosProfile,
};
use std::{
    sync::{
        atomic::{AtomicI64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut node = Node::create(Context::create()?, "dispatch_recovery", "")?;
    let prefix = format!("/dispatch_recovery_{}", uuid::Uuid::new_v4().simple());
    let (invalid_input, invalid_input_seen) = oneshot::channel();
    let mut invalid_input = Some(invalid_input);
    let subscription = node.subscribe_native_callback::<Int32, _>(
        &format!("{prefix}/input"),
        QosProfile::default(),
        move |message, _| {
            if message.data < 0 {
                if let Some(seen) = invalid_input.take() {
                    let _ = seen.send(());
                }
                return Err(Error::RCL_RET_INVALID_ARGUMENT);
            }
            Ok(())
        },
    )?;
    let publisher =
        node.create_publisher::<Int32>(&format!("{prefix}/input"), QosProfile::default())?;
    let request_sequence = Arc::new(AtomicI64::new(-1));
    let sequence = request_sequence.clone();
    let service = node.create_service_callback::<AddTwoInts::Service, _>(
        &format!("{prefix}/sum"),
        QosProfile::services_default(),
        move |request, info| {
            sequence.store(info.sequence_number, Ordering::Release);
            Ok(AddTwoInts::Response {
                sum: request.a + request.b,
            })
        },
    )?;
    let client = node.create_client::<AddTwoInts::Service>(
        &format!("{prefix}/sum"),
        QosProfile::services_default(),
    )?;
    node.spin_until_future_complete_with_options(
        async {
            futures::try_join!(publisher.wait_for_subscribers()?, client.wait_for_service()?)?;
            Ok(())
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )?;

    let mut timer = node.create_wall_timer(Duration::from_millis(5))?;
    let mut application = async {
        publisher.publish(&Int32 { data: -1 })?;
        let response = client.request_with_info(&AddTwoInts::Request { a: 19, b: 23 })?;
        let invalid_input_seen = invalid_input_seen
            .map(|result| result.map_err(|_| Error::RCL_RET_SUBSCRIPTION_INVALID));
        let (_, response, _) = futures::try_join!(invalid_input_seen, response, timer.tick())?;
        assert_eq!(response.response.sum, 42);
        assert_eq!(response.info.sequence_number, request_sequence.load(Ordering::Acquire));
        Ok::<_, Error>(())
    }
    .boxed_local();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut reported_errors = 0;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match node.spin_until_future_complete_with_options(
            application.as_mut(),
            remaining,
            FutureExecutionOptions::wake_driven(),
        ) {
            Ok(()) => break,
            Err(Error::RCL_RET_INVALID_ARGUMENT) => {
                reported_errors += 1;
                // The future remains pinned in `application`; retrying preserves its
                // requests, timer and received values instead of issuing them again.
                assert_eq!(reported_errors, 1);
            }
            Err(error) => return Err(error.into()),
        }
    }
    assert_eq!(reported_errors, 1);
    drop(application);
    drop(timer);
    node.destroy_native_callback_subscription(subscription);
    node.destroy_callback_service(service);
    node.destroy_client(client);
    node.destroy_publisher(publisher);
    node.try_spin_once(Duration::ZERO)?;
    println!("validated callback error reporting, retained future, healthy service/timer progress and response identity");
    Ok(())
}
