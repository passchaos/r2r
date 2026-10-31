#[path = "support/allocation_probe.rs"]
mod allocation_probe;
#[path = "support/callback_groups/mod.rs"]
mod support;
use futures::{stream, StreamExt, TryStreamExt};
use r2r::{
    example_interfaces::srv::AddTwoInts, CallbackExecutor, CallbackGroup, Context, Node,
    NodeOptions, QosProfile, ReentrantCallbackGroup,
};
use std::{
    io::Write as _,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};
use support::{Row, State};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 7 {
        return Err("usage: callback_groups <exclusive|reentrant> <workers> <window> <operations> <iterations> <warmup> <name>".into());
    }
    let kind = &args[0];
    let workers: usize = args[1].parse()?;
    let window: usize = args[2].parse()?;
    let operations: usize = args[3].parse()?;
    let iterations: usize = args[4].parse()?;
    let warmup: usize = args[5].parse()?;
    let total = iterations.checked_add(warmup).ok_or("count overflow")?;
    if !matches!(kind.as_str(), "exclusive" | "reentrant")
        || !(1..=32).contains(&workers)
        || !(1..=1024).contains(&window)
        || operations > 1_000_000
        || iterations == 0
        || total > 1_000_000
    {
        return Err("invalid callback workload dimensions".into());
    }
    let limit = if kind == "exclusive" { 1 } else { workers };
    let state = Arc::new(State {
        origin: Instant::now(),
        rows: (0..total).map(|_| Row::default()).collect(),
        active: AtomicUsize::new(0),
        peak: AtomicUsize::new(0),
        probes: AtomicUsize::new(0),
        barrier: (Mutex::new(false), Condvar::new()),
        limit,
        operations,
    });
    let expected: Vec<_> = (0..total)
        .map(|sequence| support::work(sequence, operations))
        .collect();
    let context = Context::create()?;
    let mut node = Node::create_with_options(
        context.clone(),
        "callback_group_workload",
        "",
        NodeOptions::default()
            .enable_rosout(false)
            .start_type_description_service(false),
    )?;
    let qos = QosProfile::services_default().keep_last(window.max(16));
    let client = node.create_client::<AddTwoInts::Service>(&args[6], qos.clone())?;
    let mut executor = CallbackExecutor::new(context, workers)?;
    let selected = state.clone();
    let callback = move |request: r2r::NativeMessageRef<'_, AddTwoInts::Request>,
                         response: &mut r2r::NativeMsg<AddTwoInts::Response>,
                         info: r2r::ServiceRequestInfo| {
        response.sum = selected.invoke(request.a, request.b, info.sequence_number);
        Ok(())
    };
    let options = r2r::CallbackQueueOptions::default()
        .capacity(window.max(16))
        .max_inputs_per_spin(window.max(16));
    let service = if kind == "exclusive" {
        let mut group = CallbackGroup::new();
        let token = node
            .create_service_native_callback_in_group_with_options::<AddTwoInts::Service, _>(
                &mut group, &args[6], qos, options, callback,
            )?;
        executor.add_group(group)?;
        token
    } else {
        let mut group = ReentrantCallbackGroup::new(limit)?;
        let token = node
            .create_service_native_reentrant_callback_with_options::<AddTwoInts::Service, _>(
                &mut group, &args[6], qos, options, callback,
            )?;
        executor.add_reentrant_group(group)?;
        token
    };
    let available = client.wait_for_service_with_timeout(&mut node, Duration::from_secs(10))?;
    executor.add_node(node)?;
    let task = executor.spawn()?;
    let launched = task.worker_count();
    let result = (|| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        // Node-driven deadlines wake the same native owner; no polling wait thread.
        futures::executor::block_on(available)?;
        if std::env::var_os("R2R_BENCH_STARTUP_MARKER").is_some() {
            println!(
                "{}",
                serde_json::json!({
                    "event": "ready",
                    "implementation": "r2r",
                    "ready_from_main_ns": state.now(),
                })
            );
            std::io::stdout().flush()?;
        }
        let challenge = kind == "reentrant" && workers >= 2 && window >= 2;
        if challenge {
            let first = client.request_with_info(&AddTwoInts::Request { a: -3, b: 0 })?;
            let second = client.request_with_info(&AddTwoInts::Request { a: -4, b: 0 })?;
            let (a, b) = futures::executor::block_on(async { futures::try_join!(first, second) })?;
            assert_eq!((a.response.sum, b.response.sum), (-3, -4));
            assert_eq!(state.probes.load(Ordering::Acquire), 2);
            assert_eq!(state.peak.load(Ordering::Acquire), 2);
        }
        let challenge_peak = state.peak.load(Ordering::Acquire);
        let phase = |first: usize, count: usize, responses: Vec<_>| {
            stream::iter(first..first + count)
                .map(|sequence| {
                    let before = state.now();
                    let response = client.request_with_info(&AddTwoInts::Request {
                        a: sequence as i64,
                        b: operations as i64,
                    });
                    let state = state.as_ref();
                    let expected = &expected;
                    async move {
                        let response = response?.await?;
                        let complete = state.now();
                        assert_eq!(response.response.sum, expected[sequence]);
                        assert_eq!(
                            state.rows[sequence].native.load(Ordering::Acquire) as i64,
                            response.info.sequence_number
                        );
                        Ok::<_, r2r::Error>((
                            sequence,
                            response.info.sequence_number,
                            before,
                            complete,
                            response.response.sum,
                        ))
                    }
                })
                .buffer_unordered(window)
                .try_fold(responses, |mut output, response| async move {
                    output.push(response);
                    Ok(output)
                })
        };
        futures::executor::block_on(phase(0, warmup, Vec::with_capacity(warmup)))?;
        assert_eq!(state.active.load(Ordering::Acquire), 0);
        state.peak.store(0, Ordering::Release);
        let responses = Vec::with_capacity(iterations);
        let probe = allocation_probe::Probe::load()?;
        if let Some(probe) = &probe {
            assert_eq!(unsafe { (probe.begin)() }, 0);
        }
        let cpu = support::cpu_ns();
        let before = Instant::now();
        let responses = futures::executor::block_on(phase(warmup, iterations, responses))?;
        let elapsed = before.elapsed().as_nanos() as u64;
        let cpu = support::cpu_ns() - cpu;
        let stats = if let Some(probe) = probe {
            let mut native = allocation_probe::NativeStats::default();
            assert_eq!(unsafe { (probe.end)(&mut native) }, 0);
            Some(
                serde_json::json!({"abi":1,"malloc_calls":native.malloc_calls,"calloc_calls":native.calloc_calls,"realloc_calls":native.realloc_calls,"aligned_calls":native.aligned_calls,"free_calls":native.free_calls,"null_free_calls":native.null_free_calls,"failed_allocations":native.failed_allocations,"requested_bytes":native.requested_bytes,"size_overflows":native.size_overflows,"counter_overflows":native.counter_overflows,"user_cpu_ns":native.user_cpu_ns,"system_cpu_ns":native.system_cpu_ns,"voluntary_context_switches":native.voluntary_context_switches,"involuntary_context_switches":native.involuntary_context_switches,"process_max_rss_kib":native.process_max_rss_kib}),
            )
        } else {
            None
        };
        let (threads, rss, hwm) = support::process_metrics();
        let mut trace: Vec<_> = responses
            .into_iter()
            .map(|(sequence, native, before, complete, result)| {
                let row = &state.rows[sequence];
                assert_eq!(row.native.load(Ordering::Acquire) as i64, native);
                [
                    sequence as i64,
                    native,
                    row.native.load(Ordering::Acquire) as i64,
                    before as i64,
                    row.enter.load(Ordering::Acquire) as i64,
                    row.finish.load(Ordering::Acquire) as i64,
                    complete as i64,
                    result,
                ]
            })
            .collect();
        trace.sort_unstable_by_key(|row| row[0]);
        assert!(state
            .rows
            .iter()
            .all(|row| row.seen.load(Ordering::Acquire) == 1));
        let samples: Vec<_> = trace.iter().map(|row| (row[6] - row[3]) as u64).collect();
        let mut sorted = samples.clone();
        sorted.sort_unstable();
        let pct = |p: usize| sorted[((sorted.len() - 1) * p).div_ceil(100)];
        let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
            .to_str()?;
        Ok(
            serde_json::json!({"implementation":"r2r","process_threads":threads,"process_rss_kib":rss,"process_hwm_kib":hwm,"kind":kind,"workers":workers,"launched_workers":launched,"window":window,"operations":operations,"iterations":iterations,"warmup":warmup,"rmw":rmw,"challenge":challenge,"challenge_overlap":challenge && challenge_peak==2,"challenge_received":state.probes.load(Ordering::Acquire),"challenge_maximum_active":challenge_peak,"received":total,"checksum":total*(total-1)/2,"maximum_active":state.peak.load(Ordering::Acquire),"elapsed_ns":elapsed,"cpu_ns":cpu,"requests_per_second":iterations as f64/(elapsed as f64*1e-9),"rtt_ns_p50":pct(50),"rtt_ns_p95":pct(95),"rtt_ns_p99":pct(99),"samples_ns":samples,"trace":trace,"probe":stats}),
        )
    })();
    state.release();
    let stopped = futures::executor::block_on(task.stop());
    let output = result?;
    stopped?;
    drop(service);
    println!("{output}");
    Ok(())
}
