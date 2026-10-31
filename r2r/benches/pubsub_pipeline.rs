use std::time::{Duration, Instant};

#[cfg(not(r2r_bench_native_callbacks))]
use futures::{FutureExt, StreamExt};
#[cfg(r2r_bench_reusable_typed)]
use r2r::ReusableSubscriptionSample;
#[cfg(r2r_bench_batch_typed)]
use r2r::SubscriptionBatch;
use r2r::{std_msgs::msg::UInt64MultiArray, Context, Error, Node, NodeOptions, QosProfile};
#[cfg(r2r_bench_native_callbacks)]
use r2r::{NativeCallbackSubscriptionOptions, WrappedTypesupport};
#[cfg(not(r2r_bench_native_callbacks))]
use r2r::{Subscription, SubscriptionOptions};
#[cfg(r2r_bench_native_callbacks)]
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

const POLL_TIMEOUT: Duration = Duration::from_millis(100);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(any(
    all(r2r_bench_native_callbacks, r2r_bench_reusable_typed),
    all(r2r_bench_native_callbacks, r2r_bench_batch_typed),
    all(r2r_bench_reusable_typed, r2r_bench_batch_typed)
))]
compile_error!("select exactly one pub/sub receive-path cfg");

fn payload(index: usize) -> u64 {
    (index as u64).wrapping_mul(17).wrapping_add(3) % 100_003
}

#[cfg(not(r2r_bench_native_callbacks))]
fn validate(message: &UInt64MultiArray, elements: usize, sequence: usize) {
    assert!(message.layout.dim.is_empty());
    assert_eq!(message.layout.data_offset, 0);
    assert_eq!(message.data.len(), elements + 1);
    assert_eq!(message.data[0], sequence as u64);
    for (index, value) in message.data[1..].iter().enumerate() {
        assert_eq!(*value, payload(index));
    }
}

fn spin_once(node: &mut Node) -> r2r::Result<()> {
    match node.try_spin_once(POLL_TIMEOUT) {
        Ok(()) | Err(Error::RCL_RET_TIMEOUT) => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(all(
    not(r2r_bench_native_callbacks),
    not(r2r_bench_reusable_typed),
    not(r2r_bench_batch_typed)
))]
fn receive(
    node: &mut Node, subscription: &mut Subscription<UInt64MultiArray>,
) -> r2r::Result<UInt64MultiArray> {
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    loop {
        if let Some(message) = subscription.next().now_or_never().flatten() {
            return Ok(message);
        }
        if Instant::now() >= deadline {
            return Err(Error::RCL_RET_TIMEOUT);
        }
        spin_once(node)?;
    }
}

fn percentile(samples: &[u64], percent: usize) -> u64 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[((sorted.len() - 1) * percent).div_ceil(100)]
}

struct PingPhase {
    checksum: u64,
    samples: Vec<u64>,
    publish_samples: Vec<u64>,
    elapsed_ns: u64,
    max_inflight: usize,
}

#[cfg(all(
    not(r2r_bench_native_callbacks),
    not(r2r_bench_reusable_typed),
    not(r2r_bench_batch_typed)
))]
#[allow(clippy::too_many_arguments)]
fn ping_phase(
    node: &mut Node, publisher: &r2r::Publisher<UInt64MultiArray>,
    subscription: &mut Subscription<UInt64MultiArray>, message: &mut UInt64MultiArray,
    elements: usize, first_sequence: usize, count: usize, window: usize, measured: bool,
) -> r2r::Result<PingPhase> {
    if count == 0 {
        return Ok(PingPhase {
            checksum: 0,
            samples: Vec::new(),
            publish_samples: Vec::new(),
            elapsed_ns: 0,
            max_inflight: 0,
        });
    }
    let mut sent = vec![Instant::now(); count];
    let capacity = if measured { count } else { 0 };
    let mut samples = Vec::with_capacity(capacity);
    let mut publish_samples = Vec::with_capacity(capacity);
    let mut next = 0;
    let mut received = 0;
    let mut checksum = 0_u64;
    let mut inflight = 0;
    let mut max_inflight = 0;
    let started = Instant::now();
    while next < count && inflight < window {
        message.data[0] = (first_sequence + next) as u64;
        let before = Instant::now();
        publisher.publish(message)?;
        sent[next] = before;
        if measured {
            publish_samples.push(before.elapsed().as_nanos() as u64);
        }
        next += 1;
        inflight += 1;
        max_inflight = max_inflight.max(inflight);
    }
    while received < count {
        let response = receive(node, subscription)?;
        let sequence = first_sequence + received;
        validate(&response, elements, sequence);
        if measured {
            samples.push(sent[received].elapsed().as_nanos() as u64);
        }
        checksum = checksum.wrapping_add(response.data[0]);
        received += 1;
        inflight -= 1;
        if next < count {
            message.data[0] = (first_sequence + next) as u64;
            let before = Instant::now();
            publisher.publish(message)?;
            sent[next] = before;
            if measured {
                publish_samples.push(before.elapsed().as_nanos() as u64);
            }
            next += 1;
            inflight += 1;
            max_inflight = max_inflight.max(inflight);
        }
    }
    Ok(PingPhase {
        checksum,
        samples,
        publish_samples,
        elapsed_ns: started.elapsed().as_nanos() as u64,
        max_inflight,
    })
}

#[cfg(r2r_bench_reusable_typed)]
fn receive_reusable(
    node: &mut Node, subscription: &mut Subscription<ReusableSubscriptionSample<UInt64MultiArray>>,
) -> r2r::Result<ReusableSubscriptionSample<UInt64MultiArray>> {
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    loop {
        if let Some(message) = subscription.next().now_or_never().flatten() {
            return Ok(message);
        }
        if Instant::now() >= deadline {
            return Err(Error::RCL_RET_TIMEOUT);
        }
        spin_once(node)?;
    }
}

#[cfg(r2r_bench_reusable_typed)]
#[allow(clippy::too_many_arguments)]
fn reusable_ping_phase(
    node: &mut Node, publisher: &r2r::Publisher<UInt64MultiArray>,
    subscription: &mut Subscription<ReusableSubscriptionSample<UInt64MultiArray>>,
    message: &mut UInt64MultiArray, elements: usize, first_sequence: usize, count: usize,
    window: usize, measured: bool,
) -> r2r::Result<PingPhase> {
    if count == 0 {
        return Ok(PingPhase {
            checksum: 0,
            samples: Vec::new(),
            publish_samples: Vec::new(),
            elapsed_ns: 0,
            max_inflight: 0,
        });
    }
    let mut sent = vec![Instant::now(); count];
    let capacity = if measured { count } else { 0 };
    let mut phase = PingPhase {
        checksum: 0,
        samples: Vec::with_capacity(capacity),
        publish_samples: Vec::with_capacity(capacity),
        elapsed_ns: 0,
        max_inflight: 0,
    };
    let mut next = 0;
    let mut received = 0;
    let mut inflight = 0;
    let started = Instant::now();
    while next < count && inflight < window {
        message.data[0] = (first_sequence + next) as u64;
        let before = Instant::now();
        publisher.publish(message)?;
        sent[next] = before;
        if measured {
            phase
                .publish_samples
                .push(before.elapsed().as_nanos() as u64);
        }
        next += 1;
        inflight += 1;
        phase.max_inflight = phase.max_inflight.max(inflight);
    }
    while received < count {
        let response = receive_reusable(node, subscription)?;
        let sequence = first_sequence + received;
        validate(&response, elements, sequence);
        if measured {
            phase
                .samples
                .push(sent[received].elapsed().as_nanos() as u64);
        }
        phase.checksum = phase.checksum.wrapping_add(response.data[0]);
        received += 1;
        inflight -= 1;
        if next < count {
            message.data[0] = (first_sequence + next) as u64;
            let before = Instant::now();
            publisher.publish(message)?;
            sent[next] = before;
            if measured {
                phase
                    .publish_samples
                    .push(before.elapsed().as_nanos() as u64);
            }
            next += 1;
            inflight += 1;
            phase.max_inflight = phase.max_inflight.max(inflight);
        }
    }
    phase.elapsed_ns = started.elapsed().as_nanos() as u64;
    Ok(phase)
}

#[cfg(r2r_bench_batch_typed)]
fn receive_batch(
    node: &mut Node, subscription: &mut Subscription<SubscriptionBatch<UInt64MultiArray>>,
) -> r2r::Result<SubscriptionBatch<UInt64MultiArray>> {
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    loop {
        if let Some(batch) = subscription.next().now_or_never().flatten() {
            assert!(!batch.is_empty());
            return Ok(batch);
        }
        if Instant::now() >= deadline {
            return Err(Error::RCL_RET_TIMEOUT);
        }
        spin_once(node)?;
    }
}

#[cfg(r2r_bench_batch_typed)]
#[allow(clippy::too_many_arguments)]
fn batch_ping_phase(
    node: &mut Node, publisher: &r2r::Publisher<UInt64MultiArray>,
    subscription: &mut Subscription<SubscriptionBatch<UInt64MultiArray>>,
    message: &mut UInt64MultiArray, elements: usize, first_sequence: usize, count: usize,
    window: usize, measured: bool,
) -> r2r::Result<PingPhase> {
    if count == 0 {
        return Ok(PingPhase {
            checksum: 0,
            samples: Vec::new(),
            publish_samples: Vec::new(),
            elapsed_ns: 0,
            max_inflight: 0,
        });
    }
    let mut sent = vec![Instant::now(); count];
    let capacity = if measured { count } else { 0 };
    let mut phase = PingPhase {
        checksum: 0,
        samples: Vec::with_capacity(capacity),
        publish_samples: Vec::with_capacity(capacity),
        elapsed_ns: 0,
        max_inflight: 0,
    };
    let mut next = 0;
    let mut received = 0;
    let mut inflight = 0;
    let started = Instant::now();
    while next < count && inflight < window {
        message.data[0] = (first_sequence + next) as u64;
        let before = Instant::now();
        publisher.publish(message)?;
        sent[next] = before;
        if measured {
            phase
                .publish_samples
                .push(before.elapsed().as_nanos() as u64);
        }
        next += 1;
        inflight += 1;
        phase.max_inflight = phase.max_inflight.max(inflight);
    }
    while received < count {
        let batch = receive_batch(node, subscription)?;
        for response in batch.messages() {
            assert!(received < count, "batch returned more responses than requested");
            let sequence = first_sequence + received;
            validate(response, elements, sequence);
            if measured {
                phase
                    .samples
                    .push(sent[received].elapsed().as_nanos() as u64);
            }
            phase.checksum = phase.checksum.wrapping_add(response.data[0]);
            received += 1;
            inflight -= 1;
            if next < count {
                message.data[0] = (first_sequence + next) as u64;
                let before = Instant::now();
                publisher.publish(message)?;
                sent[next] = before;
                if measured {
                    phase
                        .publish_samples
                        .push(before.elapsed().as_nanos() as u64);
                }
                next += 1;
                inflight += 1;
                phase.max_inflight = phase.max_inflight.max(inflight);
            }
        }
    }
    phase.elapsed_ns = started.elapsed().as_nanos() as u64;
    Ok(phase)
}

#[cfg(r2r_bench_batch_typed)]
#[derive(Default)]
struct BatchCursor {
    batch: Option<SubscriptionBatch<UInt64MultiArray>>,
    index: usize,
}

#[cfg(r2r_bench_batch_typed)]
impl BatchCursor {
    fn next<'a>(
        &'a mut self, node: &mut Node,
        subscription: &mut Subscription<SubscriptionBatch<UInt64MultiArray>>,
    ) -> r2r::Result<&'a UInt64MultiArray> {
        if self
            .batch
            .as_ref()
            .is_some_and(|batch| self.index < batch.len())
        {
            let index = self.index;
            self.index += 1;
            return Ok(&self.batch.as_ref().unwrap()[index]);
        }
        self.batch = Some(receive_batch(node, subscription)?);
        self.index = 1;
        Ok(&self.batch.as_ref().unwrap()[0])
    }
}

#[cfg(r2r_bench_native_callbacks)]
fn validate_native(
    message: &<UInt64MultiArray as WrappedTypesupport>::CStruct, elements: usize, sequence: usize,
) {
    assert_eq!(message.layout.dim.size, 0);
    assert_eq!(message.layout.data_offset, 0);
    assert_eq!(message.data.size, elements + 1);
    let data = if message.data.size == 0 {
        &[]
    } else {
        assert!(!message.data.data.is_null());
        unsafe { std::slice::from_raw_parts(message.data.data, message.data.size) }
    };
    assert_eq!(data[0], sequence as u64);
    for (index, value) in data[1..].iter().enumerate() {
        assert_eq!(*value, payload(index));
    }
}

#[cfg(r2r_bench_native_callbacks)]
fn receive_callback_sequence(
    node: &mut Node, received: &Arc<Mutex<VecDeque<usize>>>,
) -> r2r::Result<usize> {
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    loop {
        if let Some(sequence) = received.lock().unwrap().pop_front() {
            return Ok(sequence);
        }
        if Instant::now() >= deadline {
            return Err(Error::RCL_RET_TIMEOUT);
        }
        spin_once(node)?;
    }
}

#[cfg(r2r_bench_native_callbacks)]
fn native_ping_phase(
    node: &mut Node, publisher: &r2r::Publisher<UInt64MultiArray>,
    received: &Arc<Mutex<VecDeque<usize>>>, message: &mut UInt64MultiArray, first_sequence: usize,
    count: usize, window: usize, measured: bool,
) -> r2r::Result<PingPhase> {
    if count == 0 {
        return Ok(PingPhase {
            checksum: 0,
            samples: Vec::new(),
            publish_samples: Vec::new(),
            elapsed_ns: 0,
            max_inflight: 0,
        });
    }
    let mut sent = vec![Instant::now(); count];
    let mut phase = PingPhase {
        checksum: 0,
        samples: Vec::with_capacity(if measured { count } else { 0 }),
        publish_samples: Vec::with_capacity(if measured { count } else { 0 }),
        elapsed_ns: 0,
        max_inflight: 0,
    };
    let mut next = 0;
    let mut completed = 0;
    let mut inflight = 0;
    let started = Instant::now();
    while next < count && inflight < window {
        message.data[0] = (first_sequence + next) as u64;
        let before = Instant::now();
        publisher.publish(message)?;
        sent[next] = before;
        if measured {
            phase
                .publish_samples
                .push(before.elapsed().as_nanos() as u64);
        }
        next += 1;
        inflight += 1;
        phase.max_inflight = phase.max_inflight.max(inflight);
    }
    while completed < count {
        let sequence = receive_callback_sequence(node, received)?;
        assert_eq!(sequence, first_sequence + completed);
        if measured {
            phase
                .samples
                .push(sent[completed].elapsed().as_nanos() as u64);
        }
        phase.checksum = phase.checksum.wrapping_add(sequence as u64);
        completed += 1;
        inflight -= 1;
        if next < count {
            message.data[0] = (first_sequence + next) as u64;
            let before = Instant::now();
            publisher.publish(message)?;
            sent[next] = before;
            if measured {
                phase
                    .publish_samples
                    .push(before.elapsed().as_nanos() as u64);
            }
            next += 1;
            inflight += 1;
            phase.max_inflight = phase.max_inflight.max(inflight);
        }
    }
    phase.elapsed_ns = started.elapsed().as_nanos() as u64;
    Ok(phase)
}

#[cfg(r2r_bench_native_callbacks)]
struct NativeEchoState {
    copies: Vec<usize>,
    first_seen: Vec<Option<Instant>>,
    checksum: u64,
    fanout_checksum: u64,
    responses: usize,
    controls: usize,
    collect_samples: Vec<u64>,
    publish_samples: Vec<u64>,
}

#[cfg(r2r_bench_native_callbacks)]
#[allow(clippy::too_many_arguments)]
fn run_native_callbacks(
    node: &mut Node, role: &str, request: &str, response: &str, qos: QosProfile, elements: usize,
    window: usize, fanout: usize, iterations: usize, warmup: usize, total: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let depth = window.max(16);
    let options = NativeCallbackSubscriptionOptions::default().max_messages_per_spin(depth);
    let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
        .to_str()?;
    let expected_checksum = total as u64 * total.saturating_sub(1) as u64 / 2;
    if role == "ping" {
        let publisher = node.create_publisher::<UInt64MultiArray>(request, qos.clone())?;
        let received = Arc::new(Mutex::new(VecDeque::with_capacity(depth)));
        let callback_received = Arc::clone(&received);
        let callback = node.subscribe_native_callback_with_options::<UInt64MultiArray, _>(
            response,
            qos,
            options,
            move |message, _info| {
                assert_ne!(message.data.size, 0);
                assert!(!message.data.data.is_null());
                let sequence = unsafe { *message.data.data } as usize;
                validate_native(&message, elements, sequence);
                callback_received.lock().unwrap().push_back(sequence);
                Ok(())
            },
        )?;
        let deadline = Instant::now() + OPERATION_TIMEOUT;
        while publisher.get_subscription_count()? != fanout
            || callback.get_publisher_count(node)? != 1
        {
            if Instant::now() >= deadline {
                return Err("native callback endpoint discovery timed out".into());
            }
            spin_once(node)?;
        }
        let mut message = UInt64MultiArray {
            data: std::iter::once(0)
                .chain((0..elements).map(payload))
                .collect(),
            ..Default::default()
        };
        let warmup_result =
            native_ping_phase(node, &publisher, &received, &mut message, 0, warmup, window, false)?;
        let measured = native_ping_phase(
            node,
            &publisher,
            &received,
            &mut message,
            warmup,
            iterations,
            window,
            true,
        )?;
        publisher.publish(&UInt64MultiArray::default())?;
        match publisher.wait_for_all_acked(OPERATION_TIMEOUT) {
            Ok(()) | Err(Error::RCL_RET_UNSUPPORTED) => {}
            Err(error) => return Err(error.into()),
        }
        println!(
            "{}",
            serde_json::json!({"implementation":"r2r","role":role,"rmw":rmw,
            "receive_path":"native_callback","elements":elements,"window":window,
            "fanout":fanout,"iterations":iterations,"warmup":warmup,"received":total,
            "fanout_received":total*fanout,
            "checksum":warmup_result.checksum.wrapping_add(measured.checksum),
            "fanout_checksum":expected_checksum.wrapping_mul(fanout as u64),
            "max_inflight":warmup_result.max_inflight.max(measured.max_inflight),
            "elapsed_ns":measured.elapsed_ns,
            "messages_per_second":iterations as f64/(measured.elapsed_ns as f64*1e-9),
            "rtt_ns_p50":percentile(&measured.samples,50),
            "rtt_ns_p95":percentile(&measured.samples,95),
            "rtt_ns_p99":percentile(&measured.samples,99),
            "rtt_ns_max":measured.samples.iter().max().unwrap(),
            "publish_ns_p50":percentile(&measured.publish_samples,50),
            "publish_ns_p95":percentile(&measured.publish_samples,95),
            "publish_ns_p99":percentile(&measured.publish_samples,99),
            "samples_ns":measured.samples,"publish_samples_ns":measured.publish_samples})
        );
        return Ok(());
    }

    let publisher = node.create_publisher::<UInt64MultiArray>(response, qos.clone())?;
    let state = Arc::new(Mutex::new(NativeEchoState {
        copies: vec![0; total],
        first_seen: vec![None; total],
        checksum: 0,
        fanout_checksum: 0,
        responses: 0,
        controls: 0,
        collect_samples: Vec::with_capacity(iterations),
        publish_samples: Vec::with_capacity(iterations),
    }));
    let mut callbacks = Vec::with_capacity(fanout);
    for _ in 0..fanout {
        let state = Arc::clone(&state);
        let publisher = publisher.clone();
        let mut expected_sequence = 0;
        callbacks.push(node.subscribe_native_callback_with_options::<UInt64MultiArray, _>(
            request,
            qos.clone(),
            options,
            move |message, _info| {
                if message.data.size == 0 {
                    assert_eq!(expected_sequence, total);
                    state.lock().unwrap().controls += 1;
                    return Ok(());
                }
                let sequence = unsafe { *message.data.data } as usize;
                assert_eq!(sequence, expected_sequence);
                expected_sequence += 1;
                validate_native(&message, elements, sequence);
                let collect_started = {
                    let mut state = state.lock().unwrap();
                    state.fanout_checksum = state.fanout_checksum.wrapping_add(sequence as u64);
                    if state.copies[sequence] == 0 {
                        state.first_seen[sequence] = Some(Instant::now());
                    }
                    state.copies[sequence] += 1;
                    (state.copies[sequence] == fanout)
                        .then(|| state.first_seen[sequence].take().unwrap())
                };
                if let Some(collect_started) = collect_started {
                    let collect_ns = collect_started.elapsed().as_nanos() as u64;
                    let before = Instant::now();
                    publisher.publish_native_ref(&message)?;
                    let publish_ns = before.elapsed().as_nanos() as u64;
                    let mut state = state.lock().unwrap();
                    state.checksum = state.checksum.wrapping_add(sequence as u64);
                    state.responses += 1;
                    if sequence >= warmup {
                        state.collect_samples.push(collect_ns);
                        state.publish_samples.push(publish_ns);
                    }
                }
                Ok(())
            },
        )?);
    }
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    let callbacks_ready = |node: &Node| -> r2r::Result<bool> {
        callbacks
            .iter()
            .try_fold(true, |ready, callback| Ok(ready && callback.get_publisher_count(node)? == 1))
    };
    while publisher.get_subscription_count()? != 1 || !callbacks_ready(node)? {
        if Instant::now() >= deadline {
            return Err("native callback endpoint discovery timed out".into());
        }
        spin_once(node)?;
    }
    loop {
        {
            let state = state.lock().unwrap();
            if state.responses == total && state.controls == fanout {
                break;
            }
        }
        spin_once(node)?;
    }
    let state = state.lock().unwrap();
    println!(
        "{}",
        serde_json::json!({"implementation":"r2r","role":role,"rmw":rmw,
        "receive_path":"native_callback","elements":elements,"window":window,"fanout":fanout,
        "iterations":iterations,"warmup":warmup,"received":state.responses,
        "fanout_received":state.copies.iter().sum::<usize>(),"checksum":state.checksum,
        "fanout_checksum":state.fanout_checksum,"max_inflight":window.min(iterations.max(warmup)),
        "collect_ns_p50":percentile(&state.collect_samples,50),
        "collect_ns_p95":percentile(&state.collect_samples,95),
        "collect_ns_p99":percentile(&state.collect_samples,99),
        "publish_ns_p50":percentile(&state.publish_samples,50),
        "publish_ns_p95":percentile(&state.publish_samples,95),
        "publish_ns_p99":percentile(&state.publish_samples,99),
        "collect_samples_ns":state.collect_samples,"publish_samples_ns":state.publish_samples})
    );
    Ok(())
}

#[cfg(r2r_bench_reusable_typed)]
#[allow(clippy::too_many_arguments)]
fn run_reusable_typed(
    node: &mut Node, role: &str, request: &str, response: &str, qos: QosProfile, elements: usize,
    window: usize, fanout: usize, iterations: usize, warmup: usize, total: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let (publish_topic, subscribe_topic, subscription_count) = if role == "ping" {
        (request, response, 1)
    } else {
        (response, request, fanout)
    };
    let depth = window.max(16);
    let publisher = node.create_publisher::<UInt64MultiArray>(publish_topic, qos.clone())?;
    let mut subscriptions = (0..subscription_count)
        .map(|_| {
            node.subscribe_reusable_with_options::<UInt64MultiArray>(
                subscribe_topic,
                qos.clone(),
                SubscriptionOptions::default().channel_capacity(depth),
            )
        })
        .collect::<r2r::Result<Vec<_>>>()?;
    let expected_publish_subscribers = if role == "ping" { fanout } else { 1 };
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    let subscriptions_ready = |node: &Node| -> r2r::Result<bool> {
        subscriptions.iter().try_fold(true, |ready, subscription| {
            Ok(ready && subscription.get_publisher_count(node)? == 1)
        })
    };
    while publisher.get_subscription_count()? != expected_publish_subscribers
        || !subscriptions_ready(node)?
    {
        if Instant::now() >= deadline {
            return Err("reusable typed endpoint discovery timed out or unexpected peers".into());
        }
        spin_once(node)?;
    }
    let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
        .to_str()?;
    let checksum = total as u64 * total.saturating_sub(1) as u64 / 2;
    if role == "echo" {
        let mut collect_samples = Vec::with_capacity(iterations);
        let mut publish_samples = Vec::with_capacity(iterations);
        for sequence in 0..total {
            let before = Instant::now();
            let mut response_message = None;
            for (index, subscription) in subscriptions.iter_mut().enumerate() {
                let message = receive_reusable(node, subscription)?;
                validate(&message, elements, sequence);
                if index == 0 {
                    response_message = Some(message);
                }
            }
            if sequence >= warmup {
                collect_samples.push(before.elapsed().as_nanos() as u64);
            }
            let before = Instant::now();
            publisher.publish(response_message.as_ref().unwrap())?;
            if sequence >= warmup {
                publish_samples.push(before.elapsed().as_nanos() as u64);
            }
        }
        for subscription in &mut subscriptions {
            let done = receive_reusable(node, subscription)?;
            assert!(done.data.is_empty() && done.layout.dim.is_empty());
        }
        println!(
            "{}",
            serde_json::json!({"implementation":"r2r","role":role,"rmw":rmw,
            "receive_path":"reusable_typed","elements":elements,"window":window,
            "fanout":fanout,"iterations":iterations,"warmup":warmup,"received":total,
            "fanout_received":total*fanout,"checksum":checksum,
            "fanout_checksum":checksum.wrapping_mul(fanout as u64),
            "max_inflight":window.min(iterations.max(warmup)),
            "collect_ns_p50":percentile(&collect_samples,50),
            "collect_ns_p95":percentile(&collect_samples,95),
            "collect_ns_p99":percentile(&collect_samples,99),
            "publish_ns_p50":percentile(&publish_samples,50),
            "publish_ns_p95":percentile(&publish_samples,95),
            "publish_ns_p99":percentile(&publish_samples,99),
            "collect_samples_ns":collect_samples,"publish_samples_ns":publish_samples})
        );
        return Ok(());
    }

    let mut message = UInt64MultiArray {
        data: std::iter::once(0)
            .chain((0..elements).map(payload))
            .collect(),
        ..Default::default()
    };
    let warmup_result = reusable_ping_phase(
        node,
        &publisher,
        &mut subscriptions[0],
        &mut message,
        elements,
        0,
        warmup,
        window,
        false,
    )?;
    let measured = reusable_ping_phase(
        node,
        &publisher,
        &mut subscriptions[0],
        &mut message,
        elements,
        warmup,
        iterations,
        window,
        true,
    )?;
    publisher.publish(&UInt64MultiArray::default())?;
    match publisher.wait_for_all_acked(OPERATION_TIMEOUT) {
        Ok(()) | Err(Error::RCL_RET_UNSUPPORTED) => {}
        Err(error) => return Err(error.into()),
    }
    println!(
        "{}",
        serde_json::json!({"implementation":"r2r","role":role,"rmw":rmw,
        "receive_path":"reusable_typed","elements":elements,"window":window,
        "fanout":fanout,"iterations":iterations,"warmup":warmup,"received":total,
        "fanout_received":total*fanout,
        "checksum":warmup_result.checksum.wrapping_add(measured.checksum),
        "fanout_checksum":checksum.wrapping_mul(fanout as u64),
        "max_inflight":warmup_result.max_inflight.max(measured.max_inflight),
        "elapsed_ns":measured.elapsed_ns,
        "messages_per_second":iterations as f64/(measured.elapsed_ns as f64*1e-9),
        "rtt_ns_p50":percentile(&measured.samples,50),
        "rtt_ns_p95":percentile(&measured.samples,95),
        "rtt_ns_p99":percentile(&measured.samples,99),
        "rtt_ns_max":measured.samples.iter().max().unwrap(),
        "publish_ns_p50":percentile(&measured.publish_samples,50),
        "publish_ns_p95":percentile(&measured.publish_samples,95),
        "publish_ns_p99":percentile(&measured.publish_samples,99),
        "samples_ns":measured.samples,"publish_samples_ns":measured.publish_samples})
    );
    Ok(())
}

#[cfg(r2r_bench_batch_typed)]
#[allow(clippy::too_many_arguments)]
fn run_batch_typed(
    node: &mut Node, role: &str, request: &str, response: &str, qos: QosProfile, elements: usize,
    window: usize, fanout: usize, iterations: usize, warmup: usize, total: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let (publish_topic, subscribe_topic, subscription_count) = if role == "ping" {
        (request, response, 1)
    } else {
        (response, request, fanout)
    };
    let publisher = node.create_publisher::<UInt64MultiArray>(publish_topic, qos.clone())?;
    let mut subscriptions = (0..subscription_count)
        .map(|_| {
            node.subscribe_batches_with_options::<UInt64MultiArray>(
                subscribe_topic,
                qos.clone(),
                window,
                SubscriptionOptions::default().channel_capacity(1),
            )
        })
        .collect::<r2r::Result<Vec<_>>>()?;
    let expected_publish_subscribers = if role == "ping" { fanout } else { 1 };
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    let subscriptions_ready = |node: &Node| -> r2r::Result<bool> {
        subscriptions.iter().try_fold(true, |ready, subscription| {
            Ok(ready && subscription.get_publisher_count(node)? == 1)
        })
    };
    while publisher.get_subscription_count()? != expected_publish_subscribers
        || !subscriptions_ready(node)?
    {
        if Instant::now() >= deadline {
            return Err("batch typed endpoint discovery timed out or unexpected peers".into());
        }
        spin_once(node)?;
    }
    let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
        .to_str()?;
    let checksum = total as u64 * total.saturating_sub(1) as u64 / 2;
    if role == "echo" {
        let mut cursors = (0..subscription_count)
            .map(|_| BatchCursor::default())
            .collect::<Vec<_>>();
        let mut collect_samples = Vec::with_capacity(iterations);
        let mut publish_samples = Vec::with_capacity(iterations);
        for sequence in 0..total {
            let before = Instant::now();
            let (first_subscription, remaining_subscriptions) =
                subscriptions.split_first_mut().unwrap();
            let (first_cursor, remaining_cursors) = cursors.split_first_mut().unwrap();
            let response_message = first_cursor.next(node, first_subscription)?;
            validate(response_message, elements, sequence);
            for (cursor, subscription) in remaining_cursors
                .iter_mut()
                .zip(remaining_subscriptions.iter_mut())
            {
                validate(cursor.next(node, subscription)?, elements, sequence);
            }
            if sequence >= warmup {
                collect_samples.push(before.elapsed().as_nanos() as u64);
            }
            let before = Instant::now();
            publisher.publish(response_message)?;
            if sequence >= warmup {
                publish_samples.push(before.elapsed().as_nanos() as u64);
            }
        }
        for (cursor, subscription) in cursors.iter_mut().zip(&mut subscriptions) {
            let done = cursor.next(node, subscription)?;
            assert!(done.data.is_empty() && done.layout.dim.is_empty());
        }
        println!(
            "{}",
            serde_json::json!({"implementation":"r2r","role":role,"rmw":rmw,
            "receive_path":"batch_typed","elements":elements,"window":window,
            "fanout":fanout,"iterations":iterations,"warmup":warmup,"received":total,
            "fanout_received":total*fanout,"checksum":checksum,
            "fanout_checksum":checksum.wrapping_mul(fanout as u64),
            "max_inflight":window.min(iterations.max(warmup)),
            "collect_ns_p50":percentile(&collect_samples,50),
            "collect_ns_p95":percentile(&collect_samples,95),
            "collect_ns_p99":percentile(&collect_samples,99),
            "publish_ns_p50":percentile(&publish_samples,50),
            "publish_ns_p95":percentile(&publish_samples,95),
            "publish_ns_p99":percentile(&publish_samples,99),
            "collect_samples_ns":collect_samples,"publish_samples_ns":publish_samples})
        );
        return Ok(());
    }

    let mut message = UInt64MultiArray {
        data: std::iter::once(0)
            .chain((0..elements).map(payload))
            .collect(),
        ..Default::default()
    };
    let warmup_result = batch_ping_phase(
        node,
        &publisher,
        &mut subscriptions[0],
        &mut message,
        elements,
        0,
        warmup,
        window,
        false,
    )?;
    let measured = batch_ping_phase(
        node,
        &publisher,
        &mut subscriptions[0],
        &mut message,
        elements,
        warmup,
        iterations,
        window,
        true,
    )?;
    publisher.publish(&UInt64MultiArray::default())?;
    match publisher.wait_for_all_acked(OPERATION_TIMEOUT) {
        Ok(()) | Err(Error::RCL_RET_UNSUPPORTED) => {}
        Err(error) => return Err(error.into()),
    }
    println!(
        "{}",
        serde_json::json!({"implementation":"r2r","role":role,"rmw":rmw,
        "receive_path":"batch_typed","elements":elements,"window":window,
        "fanout":fanout,"iterations":iterations,"warmup":warmup,"received":total,
        "fanout_received":total*fanout,
        "checksum":warmup_result.checksum.wrapping_add(measured.checksum),
        "fanout_checksum":checksum.wrapping_mul(fanout as u64),
        "max_inflight":warmup_result.max_inflight.max(measured.max_inflight),
        "elapsed_ns":measured.elapsed_ns,
        "messages_per_second":iterations as f64/(measured.elapsed_ns as f64*1e-9),
        "rtt_ns_p50":percentile(&measured.samples,50),
        "rtt_ns_p95":percentile(&measured.samples,95),
        "rtt_ns_p99":percentile(&measured.samples,99),
        "rtt_ns_max":measured.samples.iter().max().unwrap(),
        "publish_ns_p50":percentile(&measured.publish_samples,50),
        "publish_ns_p95":percentile(&measured.publish_samples,95),
        "publish_ns_p99":percentile(&measured.publish_samples,99),
        "samples_ns":measured.samples,"publish_samples_ns":measured.publish_samples})
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 7 {
        return Err("usage: pubsub_pipeline <ping|echo> <payload-elements> <window> <fanout> <iterations> <warmup> <topic-prefix>".into());
    }
    let role = &args[0];
    let elements: usize = args[1].parse()?;
    let window: usize = args[2].parse()?;
    let fanout: usize = args[3].parse()?;
    let iterations: usize = args[4].parse()?;
    let warmup: usize = args[5].parse()?;
    let total = iterations
        .checked_add(warmup)
        .ok_or("iteration count overflow")?;
    if !matches!(role.as_str(), "ping" | "echo")
        || iterations == 0
        || window == 0
        || fanout == 0
        || elements > 1_000_000
        || window > 4096
        || fanout > 64
        || window.checked_mul(fanout).is_none_or(|value| value > 4096)
        || total > u32::MAX as usize
    {
        return Err("invalid pipeline configuration".into());
    }
    let context = Context::create()?;
    let mut node = Node::create_with_options(
        context,
        &format!("r2r_pipeline_{role}"),
        "",
        NodeOptions::default().enable_rosout(false),
    )?;
    let request = format!("{}/request", args[6]);
    let response = format!("{}/response", args[6]);
    let depth = window.max(16);
    let qos = QosProfile::default().keep_last(depth);
    #[cfg(r2r_bench_native_callbacks)]
    return run_native_callbacks(
        &mut node, role, &request, &response, qos, elements, window, fanout, iterations, warmup,
        total,
    );
    #[cfg(r2r_bench_reusable_typed)]
    return run_reusable_typed(
        &mut node, role, &request, &response, qos, elements, window, fanout, iterations, warmup,
        total,
    );
    #[cfg(r2r_bench_batch_typed)]
    return run_batch_typed(
        &mut node, role, &request, &response, qos, elements, window, fanout, iterations, warmup,
        total,
    );
    #[cfg(all(
        not(r2r_bench_native_callbacks),
        not(r2r_bench_reusable_typed),
        not(r2r_bench_batch_typed)
    ))]
    {
        let (publish_topic, subscribe_topic, subscription_count) = if role == "ping" {
            (&request, &response, 1)
        } else {
            (&response, &request, fanout)
        };
        let publisher = node.create_publisher::<UInt64MultiArray>(publish_topic, qos.clone())?;
        let mut subscriptions = (0..subscription_count)
            .map(|_| {
                node.subscribe_with_options::<UInt64MultiArray>(
                    subscribe_topic,
                    qos.clone(),
                    SubscriptionOptions::default().channel_capacity(depth),
                )
            })
            .collect::<r2r::Result<Vec<_>>>()?;
        let expected_publish_subscribers = if role == "ping" { fanout } else { 1 };
        let deadline = Instant::now() + OPERATION_TIMEOUT;
        let subscriptions_ready = |node: &Node| -> r2r::Result<bool> {
            subscriptions.iter().try_fold(true, |ready, subscription| {
                Ok(ready && subscription.get_publisher_count(node)? == 1)
            })
        };
        while publisher.get_subscription_count()? != expected_publish_subscribers
            || !subscriptions_ready(&node)?
        {
            if Instant::now() >= deadline {
                return Err("endpoint discovery timed out or unexpected peers".into());
            }
            match node.try_spin_once(Duration::from_millis(10)) {
                Ok(()) | Err(Error::RCL_RET_TIMEOUT) => {}
                Err(error) => return Err(error.into()),
            }
        }
        let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
            .to_str()?;
        let checksum = total as u64 * total.saturating_sub(1) as u64 / 2;
        if role == "echo" {
            let mut response_message = UInt64MultiArray::default();
            let mut collect_samples = Vec::with_capacity(iterations);
            let mut publish_samples = Vec::with_capacity(iterations);
            for sequence in 0..total {
                let before = Instant::now();
                for (index, subscription) in subscriptions.iter_mut().enumerate() {
                    let message = receive(&mut node, subscription)?;
                    validate(&message, elements, sequence);
                    if index == 0 {
                        response_message = message;
                    }
                }
                if sequence >= warmup {
                    collect_samples.push(before.elapsed().as_nanos() as u64);
                }
                let before = Instant::now();
                publisher.publish(&response_message)?;
                if sequence >= warmup {
                    publish_samples.push(before.elapsed().as_nanos() as u64);
                }
            }
            for subscription in &mut subscriptions {
                let done = receive(&mut node, subscription)?;
                assert!(done.data.is_empty() && done.layout.dim.is_empty());
            }
            println!(
                "{}",
                serde_json::json!({"implementation":"r2r","role":role,"rmw":rmw,
            "receive_path":"stream","elements":elements,"window":window,"fanout":fanout,"iterations":iterations,
            "warmup":warmup,"received":total,"fanout_received":total*fanout,
            "checksum":checksum,"fanout_checksum":checksum.wrapping_mul(fanout as u64),
            "max_inflight":window.min(iterations.max(warmup)),
            "collect_ns_p50":percentile(&collect_samples,50),
            "collect_ns_p95":percentile(&collect_samples,95),
            "collect_ns_p99":percentile(&collect_samples,99),
            "publish_ns_p50":percentile(&publish_samples,50),
            "publish_ns_p95":percentile(&publish_samples,95),
            "publish_ns_p99":percentile(&publish_samples,99),
            "collect_samples_ns":collect_samples,"publish_samples_ns":publish_samples})
            );
            return Ok(());
        }

        let mut message = UInt64MultiArray {
            data: std::iter::once(0)
                .chain((0..elements).map(payload))
                .collect(),
            ..Default::default()
        };
        let warmup_result = ping_phase(
            &mut node,
            &publisher,
            &mut subscriptions[0],
            &mut message,
            elements,
            0,
            warmup,
            window,
            false,
        )?;
        let measured = ping_phase(
            &mut node,
            &publisher,
            &mut subscriptions[0],
            &mut message,
            elements,
            warmup,
            iterations,
            window,
            true,
        )?;
        publisher.publish(&UInt64MultiArray::default())?;
        match publisher.wait_for_all_acked(OPERATION_TIMEOUT) {
            Ok(()) | Err(Error::RCL_RET_UNSUPPORTED) => {}
            Err(error) => return Err(error.into()),
        }
        println!(
            "{}",
            serde_json::json!({"implementation":"r2r","role":role,"rmw":rmw,
        "receive_path":"stream","elements":elements,"window":window,"fanout":fanout,"iterations":iterations,
        "warmup":warmup,"received":total,"fanout_received":total*fanout,
        "checksum":warmup_result.checksum.wrapping_add(measured.checksum),
        "fanout_checksum":checksum.wrapping_mul(fanout as u64),
        "max_inflight":warmup_result.max_inflight.max(measured.max_inflight),
        "elapsed_ns":measured.elapsed_ns,
        "messages_per_second":iterations as f64/(measured.elapsed_ns as f64*1e-9),
        "rtt_ns_p50":percentile(&measured.samples,50),
        "rtt_ns_p95":percentile(&measured.samples,95),
        "rtt_ns_p99":percentile(&measured.samples,99),
        "rtt_ns_max":measured.samples.iter().max().unwrap(),
        "publish_ns_p50":percentile(&measured.publish_samples,50),
        "publish_ns_p95":percentile(&measured.publish_samples,95),
        "publish_ns_p99":percentile(&measured.publish_samples,99),
        "samples_ns":measured.samples,"publish_samples_ns":measured.publish_samples})
        );
        Ok(())
    }
}
