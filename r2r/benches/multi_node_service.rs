#[cfg(not(r2r_bench_shared_executor))]
use futures::FutureExt;
use futures::{stream, StreamExt, TryStreamExt};
use r2r::{
    example_interfaces::srv::AddTwoInts, Context, Error, FutureExecutionOptions, Node, NodeOptions,
    QosProfile,
};
use std::{
    future::Future,
    time::{Duration, Instant},
};

fn cpu_time_ns() -> u64 {
    let mut time = std::mem::MaybeUninit::<libc::timespec>::uninit();
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, time.as_mut_ptr()) },
        0
    );
    let time = unsafe { time.assume_init() };
    time.tv_sec as u64 * 1_000_000_000 + time.tv_nsec as u64
}

fn drive<T>(nodes: Vec<Node>, future: impl Future<Output = r2r::Result<T>>) -> r2r::Result<T> {
    #[cfg(r2r_bench_shared_executor)]
    {
        let mut executor = r2r::Executor::new(Context::create()?);
        for node in nodes {
            executor.add_node(node)?;
        }
        executor.spin_until_future_complete_with_options(
            future,
            Duration::from_secs(30),
            FutureExecutionOptions::wake_driven(),
        )
    }
    #[cfg(not(r2r_bench_shared_executor))]
    {
        let mut nodes = nodes;
        if nodes.len() == 1 {
            return nodes[0].spin_until_future_complete_with_options(
                future,
                Duration::from_secs(30),
                FutureExecutionOptions::wake_driven(),
            );
        }
        // The historical API has no shared multi-node wait. Use its fastest single-threaded
        // public polling path, rather than assigning a sequential blocking timeout per node.
        futures::pin_mut!(future);
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(result) = future.as_mut().now_or_never() {
                return result;
            }
            if Instant::now() >= deadline {
                return Err(Error::RCL_RET_TIMEOUT);
            }
            for node in &mut nodes {
                node.try_spin_once(Duration::ZERO)?;
            }
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.len() != 5 {
        return Err(
            "usage: multi_node_service <nodes> <window> <iterations> <warmup> <prefix>".into()
        );
    }
    let node_count: usize = arguments[0].parse()?;
    let window: usize = arguments[1].parse()?;
    let iterations: usize = arguments[2].parse()?;
    let warmup: usize = arguments[3].parse()?;
    let total = iterations
        .checked_add(warmup)
        .ok_or("request count overflow")?;
    if !(1..=64).contains(&node_count)
        || !(1..=1024).contains(&window)
        || iterations == 0
        || total < node_count
        || total > i32::MAX as usize
    {
        return Err("invalid service executor configuration".into());
    }
    let context = Context::create()?;
    let mut nodes: Vec<_> = (0..node_count)
        .map(|index| {
            Node::create_with_options(
                context.clone(),
                &format!("executor_service_{index}"),
                "",
                NodeOptions::default().enable_rosout(false),
            )
        })
        .collect::<r2r::Result<_>>()?;
    let qos = QosProfile::services_default().keep_last(window.max(16));
    let mut servers = Vec::new();
    let mut clients = Vec::new();
    for index in 0..node_count {
        let name = format!("{}/service_{index}", arguments[4]);
        servers.push(nodes[index].create_service::<AddTwoInts::Service>(&name, qos.clone())?);
        clients.push(nodes[0].create_client::<AddTwoInts::Service>(&name, qos.clone())?);
    }
    let available = futures::future::try_join_all(
        clients
            .iter()
            .map(|client| client.wait_for_service())
            .collect::<r2r::Result<Vec<_>>>()?,
    );
    let server = futures::future::try_join_all(servers.into_iter().enumerate().map(
        |(index, mut server)| async move {
            let count = (total + node_count - 1 - index) / node_count;
            let mut checksum = 0_u64;
            for position in 0..count {
                let request = server.next().await.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
                assert_eq!(request.message.a as usize, index + position * node_count);
                assert_eq!(request.message.b, 5);
                checksum += request.message.a as u64;
                let sum = request.message.a + request.message.b;
                request.respond(AddTwoInts::Response { sum })?;
            }
            Ok::<_, Error>((count, checksum))
        },
    ));
    let requester = async {
        available.await?;
        let phase = |start: usize, count: usize| {
            stream::iter(start..start + count)
                .map(|sequence| {
                    let client = &clients[sequence % node_count];
                    async move {
                        let started = Instant::now();
                        let response = client
                            .request(&AddTwoInts::Request {
                                a: sequence as i64,
                                b: 5,
                            })?
                            .await?;
                        let rtt = started.elapsed().as_nanos() as u64;
                        assert_eq!(response.sum, sequence as i64 + 5);
                        Ok::<_, Error>((sequence, rtt))
                    }
                })
                .buffer_unordered(window)
                .try_collect::<Vec<_>>()
        };
        phase(0, warmup).await?;
        let cpu = cpu_time_ns();
        let started = Instant::now();
        let mut responses = phase(warmup, iterations).await?;
        let elapsed = started.elapsed().as_nanos() as u64;
        let cpu = cpu_time_ns() - cpu;
        responses.sort_unstable_by_key(|(sequence, _)| *sequence);
        for (offset, (sequence, _)) in responses.iter().enumerate() {
            assert_eq!(*sequence, warmup + offset);
        }
        let samples: Vec<_> = responses.into_iter().map(|(_, time)| time).collect();
        Ok::<_, Error>((elapsed, cpu, samples))
    };
    let ((elapsed, cpu, samples), served) =
        drive(nodes, async { futures::try_join!(requester, server) })?;
    let received: usize = served.iter().map(|(count, _)| count).sum();
    let checksum: u64 = served.iter().map(|(_, checksum)| checksum).sum();
    assert_eq!(received, total);
    assert_eq!(checksum, total as u64 * (total as u64 - 1) / 2);
    let mut ordered = samples.clone();
    ordered.sort_unstable();
    let percentile = |percent: usize| ordered[((ordered.len() - 1) * percent).div_ceil(100)];
    #[cfg(r2r_bench_shared_executor)]
    let driver = "shared_wait";
    #[cfg(not(r2r_bench_shared_executor))]
    let driver = if node_count == 1 {
        "node_wait"
    } else {
        "nonblocking_round_robin"
    };
    let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
        .to_str()?;
    println!(
        "{}",
        serde_json::json!({"implementation":"r2r","driver":driver,"rmw":rmw,"nodes":node_count,"window":window,
        "iterations":iterations,"warmup":warmup,"received":received,"checksum":checksum,
        "elapsed_ns":elapsed,"cpu_ns":cpu,"requests_per_second":iterations as f64/(elapsed as f64*1e-9),
        "rtt_ns_p50":percentile(50),"rtt_ns_p95":percentile(95),"rtt_ns_p99":percentile(99),
        "samples_ns":samples})
    );
    Ok(())
}
