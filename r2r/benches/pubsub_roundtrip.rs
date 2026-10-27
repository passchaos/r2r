use std::time::{Duration, Instant};

use futures::{FutureExt, StreamExt};
use r2r::{std_msgs::msg::UInt64MultiArray, Context, Node, NodeOptions, QosProfile, Subscription};

const POLL_TIMEOUT: Duration = Duration::from_millis(100);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(10);

fn payload(index: usize) -> u64 {
    (index as u64).wrapping_mul(17).wrapping_add(3) % 100_003
}

fn validate(message: &UInt64MultiArray, elements: usize, sequence: usize) {
    assert!(message.layout.dim.is_empty());
    assert_eq!(message.layout.data_offset, 0);
    assert_eq!(message.data.len(), elements + 1);
    assert_eq!(message.data[0], sequence as u64);
    for (index, value) in message.data[1..].iter().enumerate() {
        assert_eq!(*value, payload(index));
    }
}

fn receive(
    node: &mut Node, subscription: &mut Subscription<UInt64MultiArray>,
) -> r2r::Result<UInt64MultiArray> {
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    loop {
        if let Some(message) = subscription.next().now_or_never().flatten() {
            return Ok(message);
        }
        if Instant::now() >= deadline {
            return Err(r2r::Error::RCL_RET_TIMEOUT);
        }
        node.try_spin_once(POLL_TIMEOUT)?;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: pubsub_roundtrip <ping|echo> <payload-elements> <iterations> <warmup> <topic-prefix>".into());
    }
    let role = &args[0];
    let elements: usize = args[1].parse()?;
    let iterations: usize = args[2].parse()?;
    let warmup: usize = args[3].parse()?;
    let total = iterations
        .checked_add(warmup)
        .ok_or("iteration count overflow")?;
    if !matches!(role.as_str(), "ping" | "echo")
        || iterations == 0
        || elements > 1_000_000
        || total > u32::MAX as usize
    {
        return Err("invalid roundtrip configuration".into());
    }
    let context = Context::create()?;
    let mut node = Node::create_with_options(
        context,
        &format!("r2r_roundtrip_{role}"),
        "",
        NodeOptions::default().enable_rosout(false),
    )?;
    let request = format!("{}/request", args[4]);
    let response = format!("{}/response", args[4]);
    let (publish_topic, subscribe_topic) = if role == "ping" {
        (&request, &response)
    } else {
        (&response, &request)
    };
    let qos = QosProfile::default().keep_last(16);
    let publisher = node.create_publisher::<UInt64MultiArray>(publish_topic, qos.clone())?;
    let mut subscription = node.subscribe::<UInt64MultiArray>(subscribe_topic, qos)?;
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    while publisher
        .get_subscription_count()
        .map_err(|error| format!("publisher discovery: {error}"))?
        != 1
        || subscription
            .get_publisher_count(&node)
            .map_err(|error| format!("subscription discovery: {error}"))?
            != 1
    {
        if Instant::now() >= deadline {
            return Err("endpoint discovery timed out or unexpected peers".into());
        }
        node.try_spin_once(Duration::from_millis(10))?;
    }
    let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
        .to_str()?;
    let mut checksum = 0_u64;
    let mut samples = Vec::with_capacity(iterations);
    if role == "echo" {
        for sequence in 0..total {
            let message = receive(&mut node, &mut subscription)
                .map_err(|error| format!("receive: {error}"))?;
            validate(&message, elements, sequence);
            checksum = checksum.wrapping_add(message.data[0]);
            publisher.publish(&message)?;
        }
        // The initiator acknowledges complete delivery after the last measured response.
        let done =
            receive(&mut node, &mut subscription).map_err(|error| format!("receive: {error}"))?;
        assert!(done.data.is_empty() && done.layout.dim.is_empty() && done.layout.data_offset == 0);
        println!(
            "{}",
            serde_json::json!({"implementation":"r2r", "role":role, "rmw":rmw,
            "elements":elements, "iterations":iterations, "warmup":warmup, "received":total, "checksum":checksum})
        );
        return Ok(());
    }
    let mut message = UInt64MultiArray {
        data: std::iter::once(0)
            .chain((0..elements).map(payload))
            .collect(),
        ..Default::default()
    };
    let started = Instant::now();
    let mut measured_start = started;
    for sequence in 0..total {
        message.data[0] = sequence as u64;
        if sequence == warmup {
            measured_start = Instant::now();
        }
        let before = Instant::now();
        publisher.publish(&message)?;
        let response =
            receive(&mut node, &mut subscription).map_err(|error| format!("receive: {error}"))?;
        let elapsed = before.elapsed().as_nanos() as u64;
        validate(&response, elements, sequence);
        checksum = checksum.wrapping_add(response.data[0]);
        if sequence >= warmup {
            samples.push(elapsed);
        }
    }
    let elapsed = measured_start.elapsed().as_nanos();
    publisher.publish(&UInt64MultiArray::default())?;
    match publisher.wait_for_all_acked(OPERATION_TIMEOUT) {
        Ok(()) | Err(r2r::Error::RCL_RET_UNSUPPORTED) => {}
        Err(error) => return Err(error.into()),
    }
    let mut sorted = samples.clone();
    sorted.sort_unstable();
    let percentile = |percent: usize| sorted[((sorted.len() - 1) * percent).div_ceil(100)];
    println!(
        "{}",
        serde_json::json!({"implementation":"r2r", "role":role, "rmw":rmw,
        "elements":elements, "iterations":iterations, "warmup":warmup, "received":total, "checksum":checksum,
        "elapsed_ns":elapsed, "roundtrips_per_second":iterations as f64 / (elapsed as f64 * 1e-9),
        "rtt_ns_p50":percentile(50), "rtt_ns_p95":percentile(95), "rtt_ns_p99":percentile(99),
        "rtt_ns_max":sorted.last().unwrap(), "samples_ns":samples})
    );
    Ok(())
}
