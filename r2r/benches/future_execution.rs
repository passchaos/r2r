use futures::channel::oneshot;
use std::time::{Duration, Instant};

fn percentile(samples: &[u64], percent: usize) -> u64 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[((sorted.len() - 1) * percent).div_ceil(100)]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: future_execution <iterations> <warmup> <delay-us>".into());
    }
    let iterations: usize = args[0].parse()?;
    let warmup: usize = args[1].parse()?;
    let delay: u64 = args[2].parse()?;
    let total = iterations.checked_add(warmup).ok_or("iteration overflow")?;
    if iterations == 0 || total > u32::MAX as usize || delay == 0 || delay >= 1_000_000 {
        return Err("invalid future benchmark configuration".into());
    }
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create_with_options(
        context,
        "future_execution_bench",
        "",
        r2r::NodeOptions::default().enable_rosout(false),
    )?;
    let (requests, incoming) =
        std::sync::mpsc::channel::<(usize, oneshot::Sender<(usize, Instant)>)>();
    let producer = std::thread::spawn(move || {
        for expected in 0..total {
            let (sequence, sender) = incoming.recv().expect("benchmark stopped early");
            assert_eq!(sequence, expected);
            std::thread::sleep(Duration::from_micros(delay));
            sender
                .send((sequence, Instant::now()))
                .expect("future completion was abandoned");
        }
    });
    let mut samples = Vec::with_capacity(iterations);
    let mut checksum = 0u64;
    let mut started = Instant::now();
    for sequence in 0..total {
        if sequence == warmup {
            started = Instant::now();
        }
        let (sender, receiver) = oneshot::channel();
        requests.send((sequence, sender))?;
        let (received, notified) = node.spin_until_future_complete(
            async { receiver.await.map_err(|_| r2r::Error::RCL_RET_ERROR) },
            Duration::from_secs(1),
        )?;
        let latency = notified.elapsed().as_nanos() as u64;
        assert_eq!(received, sequence);
        checksum = checksum.wrapping_add(received as u64);
        if sequence >= warmup {
            samples.push(latency);
        }
    }
    let elapsed = started.elapsed().as_nanos() as u64;
    producer.join().expect("producer panicked");
    let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
        .to_str()?;
    println!(
        "{}",
        serde_json::json!({"implementation":"r2r","rmw":rmw,"iterations":iterations,"warmup":warmup,"delay_us":delay,
        "completed":total,"checksum":checksum,"samples_ns":samples,"elapsed_ns":elapsed,"completions_per_second":iterations as f64/(elapsed as f64*1e-9),
        "notification_ns_p50":percentile(&samples,50),"notification_ns_p95":percentile(&samples,95),"notification_ns_p99":percentile(&samples,99)})
    );
    Ok(())
}
