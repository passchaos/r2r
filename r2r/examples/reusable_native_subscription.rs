//! Receives native dynamic payloads and demonstrates safe outer-container recycling.

use futures::{FutureExt, StreamExt};
use r2r::{std_msgs::msg::UInt64MultiArray, NativeSubscriptionSample, QosProfile};
use std::time::{Duration, Instant};

fn values(sample: &NativeSubscriptionSample<UInt64MultiArray>) -> &[u64] {
    let sequence = &sample.data;
    if sequence.size == 0 {
        &[]
    } else {
        assert!(!sequence.data.is_null());
        unsafe { std::slice::from_raw_parts(sequence.data, sequence.size) }
    }
}

fn next(
    node: &mut r2r::Node,
    subscription: &mut r2r::Subscription<NativeSubscriptionSample<UInt64MultiArray>>,
) -> r2r::Result<NativeSubscriptionSample<UInt64MultiArray>> {
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
    let mut node = r2r::Node::create(context, "reusable_native_subscription", "")?;
    let publisher =
        node.create_publisher::<UInt64MultiArray>("native_samples", QosProfile::default())?;
    let mut subscription = node
        .subscribe_native_reusable::<UInt64MultiArray>("native_samples", QosProfile::default())?;
    let deadline = Instant::now() + Duration::from_secs(3);
    while publisher.get_subscription_count()? == 0 {
        if Instant::now() >= deadline {
            return Err(r2r::Error::RCL_RET_TIMEOUT.into());
        }
        node.try_spin_once(Duration::from_millis(10))?;
    }

    publisher.publish(&message((0..4096).collect()))?;
    let first = next(&mut node, &mut subscription)?;
    let address = first.native_message() as *const _ as usize;
    assert_eq!(values(&first).len(), 4096);
    drop(first);

    publisher.publish(&message(vec![7; 128]))?;
    let second = next(&mut node, &mut subscription)?;
    assert_eq!(second.native_message() as *const _ as usize, address);
    assert_eq!(values(&second), [7; 128]);
    println!("recycled one native subscription container across dynamic payload deliveries");
    Ok(())
}
