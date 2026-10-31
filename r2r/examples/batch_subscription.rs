//! Takes several queued ROS messages with one RCL call and reuses decoded Rust storage.

use futures::{FutureExt, StreamExt};
use r2r::{std_msgs::msg::UInt64MultiArray, QosProfile, SubscriptionBatch, SubscriptionOptions};
use std::time::{Duration, Instant};

const BATCH_SIZE: usize = 4;

fn message(epoch: u64, sequence: u64, elements: usize) -> UInt64MultiArray {
    let mut data = vec![epoch ^ sequence; elements];
    data[0] = epoch;
    data[1] = sequence;
    UInt64MultiArray {
        layout: Default::default(),
        data,
    }
}

fn publish_batch(
    publisher: &r2r::Publisher<UInt64MultiArray>, epoch: u64, elements: usize,
) -> r2r::Result<()> {
    for sequence in 0..BATCH_SIZE as u64 {
        publisher.publish(&message(epoch, sequence, elements))?;
    }
    publisher.wait_for_all_acked(Duration::from_secs(3))
}

fn next_batch(
    node: &mut r2r::Node, subscription: &mut r2r::Subscription<SubscriptionBatch<UInt64MultiArray>>,
) -> r2r::Result<SubscriptionBatch<UInt64MultiArray>> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10))?;
        if let Some(batch) = subscription.next().now_or_never().flatten() {
            return Ok(batch);
        }
        if Instant::now() >= deadline {
            return Err(r2r::Error::RCL_RET_TIMEOUT);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "batch_subscription", "")?;
    let qos = QosProfile::default().keep_last(16);
    let publisher = node.create_publisher::<UInt64MultiArray>("batched_samples", qos.clone())?;
    let mut subscription = node.subscribe_batches_with_options::<UInt64MultiArray>(
        "batched_samples",
        qos,
        BATCH_SIZE,
        SubscriptionOptions::default().channel_capacity(1),
    )?;

    let deadline = Instant::now() + Duration::from_secs(3);
    while publisher.get_subscription_count()? == 0 {
        if Instant::now() >= deadline {
            return Err(r2r::Error::RCL_RET_TIMEOUT.into());
        }
        node.try_spin_once(Duration::from_millis(10))?;
    }

    publish_batch(&publisher, 10, 4096)?;
    let first = next_batch(&mut node, &mut subscription)?;
    assert_eq!(first.len(), BATCH_SIZE);
    let pointers = first
        .messages()
        .iter()
        .map(|message| message.data.as_ptr())
        .collect::<Vec<_>>();
    drop(first);

    publish_batch(&publisher, 20, 64)?;
    let second = next_batch(&mut node, &mut subscription)?;
    assert_eq!(second.len(), BATCH_SIZE);
    for (index, (message, info)) in second.iter().enumerate() {
        assert_eq!(message.data[..2], [20, index as u64]);
        assert_eq!(message.data.as_ptr(), pointers[index]);
        std::hint::black_box(info);
    }
    println!("received four ordered messages per native take and reused every Rust Vec");
    Ok(())
}
