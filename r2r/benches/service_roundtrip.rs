use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use futures::{FutureExt, StreamExt};
use r2r::{
    example_interfaces::srv::AddTwoInts, Client, Context, Node, NodeOptions, QosProfile, Service,
};

const SPIN_WAIT: Duration = Duration::from_millis(100);
const DEADLINE: Duration = Duration::from_secs(10);

fn operand(sequence: usize) -> i64 {
    ((sequence as u64 * 17 + 3) % 100_003) as i64 - 50_000
}
fn expected_sum(sequence: usize) -> i64 {
    sequence as i64 + operand(sequence)
}

fn receive(
    node: &mut Node, service: &mut Service<AddTwoInts::Service>,
) -> r2r::Result<r2r::ServiceRequest<AddTwoInts::Service>> {
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Some(request) = service.next().now_or_never().flatten() {
            return Ok(request);
        }
        if Instant::now() >= deadline {
            return Err(r2r::Error::RCL_RET_TIMEOUT);
        }
        node.try_spin_once(SPIN_WAIT)?;
    }
}

fn requests(
    node: &mut Node, client: &Client<AddTwoInts::Service>, start: usize, count: usize,
    window: usize, samples: &mut Vec<u64>, measured: bool,
) -> r2r::Result<u64> {
    let mut pending = VecDeque::with_capacity(window);
    let mut sent = 0;
    let mut completed = 0;
    let mut checksum = 0_u64;
    while completed < count {
        while sent < count && pending.len() < window {
            let sequence = start + sent;
            let request = AddTwoInts::Request {
                a: sequence as i64,
                b: operand(sequence),
            };
            let before = Instant::now();
            let response = Box::pin(client.request(&request)?);
            pending.push_back((sequence, before, response));
            sent += 1;
        }
        let (sequence, before, response) = pending.front_mut().unwrap();
        if let Some(response) = response.as_mut().now_or_never() {
            let elapsed = before.elapsed().as_nanos() as u64;
            assert_eq!(response?.sum, expected_sum(*sequence));
            checksum = checksum.wrapping_add(*sequence as u64);
            if measured {
                samples.push(elapsed);
            }
            pending.pop_front();
            completed += 1;
        } else {
            if before.elapsed() >= DEADLINE {
                return Err(r2r::Error::RCL_RET_TIMEOUT);
            }
            node.try_spin_once(SPIN_WAIT)?;
        }
    }
    assert!(pending.is_empty());
    Ok(checksum)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err(
            "usage: service_roundtrip <ping|echo> <window> <iterations> <warmup> <service-name>"
                .into(),
        );
    }
    let role = &args[0];
    let window: usize = args[1].parse()?;
    let iterations: usize = args[2].parse()?;
    let warmup: usize = args[3].parse()?;
    let total = iterations
        .checked_add(warmup)
        .ok_or("iteration count overflow")?;
    if !matches!(role.as_str(), "ping" | "echo")
        || !(1..=1024).contains(&window)
        || iterations == 0
        || total > u32::MAX as usize
    {
        return Err("invalid service benchmark configuration".into());
    }
    let context = Context::create()?;
    let mut node = Node::create_with_options(
        context,
        &format!("r2r_service_{role}"),
        "",
        NodeOptions::default().enable_rosout(false),
    )?;
    let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
        .to_str()?;
    let qos = QosProfile::services_default().keep_last(window.max(16));
    if role == "echo" {
        let mut service = node.create_service::<AddTwoInts::Service>(&args[4], qos)?;
        let mut checksum = 0_u64;
        for sequence in 0..total {
            let request = receive(&mut node, &mut service)?;
            assert_eq!(request.message.a, sequence as i64);
            assert_eq!(request.message.b, operand(sequence));
            checksum = checksum.wrapping_add(sequence as u64);
            request.respond(AddTwoInts::Response {
                sum: expected_sum(sequence),
            })?;
        }
        let stop = receive(&mut node, &mut service)?;
        assert_eq!((stop.message.a, stop.message.b), (-1, 0));
        stop.respond(AddTwoInts::Response { sum: -1 })?;
        let confirm = receive(&mut node, &mut service)?;
        assert_eq!((confirm.message.a, confirm.message.b), (-2, 0));
        confirm.respond(AddTwoInts::Response { sum: -2 })?;
        println!(
            "{}",
            serde_json::json!({"implementation":"r2r", "role":role, "rmw":rmw,
            "window":window, "iterations":iterations, "warmup":warmup, "received":total, "checksum":checksum})
        );
        return Ok(());
    }
    let client = node.create_client::<AddTwoInts::Service>(&args[4], qos)?;
    let deadline = Instant::now() + DEADLINE;
    while !client.is_available(&node)? {
        if Instant::now() >= deadline {
            return Err("service discovery timed out".into());
        }
        node.try_spin_once(Duration::from_millis(10))?;
    }
    let mut samples = Vec::with_capacity(iterations);
    let mut checksum = requests(&mut node, &client, 0, warmup, window, &mut samples, false)?;
    let start = Instant::now();
    checksum = checksum.wrapping_add(requests(
        &mut node,
        &client,
        warmup,
        iterations,
        window,
        &mut samples,
        true,
    )?);
    let elapsed = start.elapsed().as_nanos();
    let stop = client.request(&AddTwoInts::Request { a: -1, b: 0 })?;
    assert_eq!(node.spin_until_future_complete(stop, DEADLINE)?.sum, -1);
    drop(client.request(&AddTwoInts::Request { a: -2, b: 0 })?);
    let mut sorted = samples.clone();
    sorted.sort_unstable();
    let percentile = |percent: usize| sorted[((sorted.len() - 1) * percent).div_ceil(100)];
    println!(
        "{}",
        serde_json::json!({"implementation":"r2r", "role":role, "rmw":rmw,
        "window":window, "iterations":iterations, "warmup":warmup, "received":total, "checksum":checksum,
        "elapsed_ns":elapsed, "roundtrips_per_second":iterations as f64 / (elapsed as f64 * 1e-9),
        "rtt_ns_p50":percentile(50), "rtt_ns_p95":percentile(95), "rtt_ns_p99":percentile(99),
        "rtt_ns_max":sorted.last().unwrap(), "samples_ns":samples})
    );
    Ok(())
}
