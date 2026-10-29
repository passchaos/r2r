//! Native timer callbacks publishing typed ROS data, with explicit startup and one-shot work.

use futures::{channel::oneshot, StreamExt};
use r2r::{
    std_msgs::msg::UInt64, CallbackTimerOptions, Context, Error, FutureExecutionOptions, Node,
    QosProfile,
};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut node = Node::create(Context::create()?, "native_timer_callbacks", "")?;
    let topic = format!("/native_timer_callbacks_{}", r2r::uuid::Uuid::new_v4().simple());
    let qos = QosProfile::default().keep_last(64);
    let publisher = node.create_publisher::<UInt64>(&topic, qos.clone())?;
    let mut subscription = node.subscribe::<UInt64>(&topic, qos)?;
    node.spin_until_future_complete_with_options(
        publisher.wait_for_subscribers()?,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )?;
    let output = publisher.clone();
    let mut next = 1;
    let periodic = node.create_wall_timer_callback_with_options(
        Duration::from_millis(1),
        CallbackTimerOptions::default().autostart(false),
        move || {
            if next <= 32 {
                output.publish(&UInt64 { data: next })?;
                next += 1;
            }
            Ok(())
        },
    )?;
    let (completed, completion) = oneshot::channel();
    let mut completed = Some(completed);
    let one = node.create_timer_callback_with_options(
        Duration::from_millis(10),
        CallbackTimerOptions::default().oneshot(true),
        move || {
            if let Some(completed) = completed.take() {
                let _ = completed.send(());
            }
            Ok(())
        },
    )?;
    assert!(periodic.is_canceled(&node)?);
    periodic.reset(&mut node)?;
    node.spin_until_future_complete_with_options(
        async {
            for expected in 1..=32 {
                let value = subscription
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
                assert_eq!(value.data, expected);
            }
            completion.await.map_err(|_| Error::RCL_RET_TIMER_INVALID)?;
            Ok(())
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )?;
    assert!(one.is_canceled(&node)?);
    periodic.cancel(&mut node)?;
    node.destroy_callback_timer(periodic);
    node.destroy_callback_timer(one);
    drop(subscription);
    node.destroy_publisher(publisher);
    node.try_spin_once(Duration::ZERO)?;
    println!(
        "validated 32 timer callback publications, manual startup, one-shot ROS timer and cleanup"
    );
    Ok(())
}
