//! Receives ordinary Rust messages while reusing their dynamic field capacity.

use futures::{FutureExt, StreamExt};
use r2r::{std_msgs::msg::UInt64MultiArray, QosProfile, ReusableSubscriptionSample};
use std::time::{Duration, Instant};

fn next(
    node: &mut r2r::Node,
    subscription: &mut r2r::Subscription<ReusableSubscriptionSample<UInt64MultiArray>>,
) -> r2r::Result<ReusableSubscriptionSample<UInt64MultiArray>> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10))?;
        if let Some(sample) = subscription.next().now_or_never().flatten() {
            return Ok(sample);
        }
        if Instant::now() >= deadline {
            return Err(r2r::Error::RCL_RET_TIMEOUT);
        }
    }
}

fn message(data: Vec<u64>) -> UInt64MultiArray {
    UInt64MultiArray {
        layout: Default::default(),
        data,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "reusable_typed_subscription", "")?;
    let publisher =
        node.create_publisher::<UInt64MultiArray>("typed_samples", QosProfile::default())?;
    let mut subscription =
        node.subscribe_reusable::<UInt64MultiArray>("typed_samples", QosProfile::default())?;
    let deadline = Instant::now() + Duration::from_secs(3);
    while publisher.get_subscription_count()? == 0 {
        if Instant::now() >= deadline {
            return Err(r2r::Error::RCL_RET_TIMEOUT.into());
        }
        node.try_spin_once(Duration::from_millis(10))?;
    }

    publisher.publish(&message((0..4096).collect()))?;
    let first = next(&mut node, &mut subscription)?;
    let data = first.data.as_ptr();
    assert_eq!(first.data.len(), 4096);
    drop(first);

    publisher.publish(&message(vec![7; 128]))?;
    let second = next(&mut node, &mut subscription)?;
    assert_eq!(second.data.as_ptr(), data);
    assert_eq!(second.data, [7; 128]);
    println!("reused ordinary Rust Vec capacity across typed subscription deliveries");
    Ok(())
}
