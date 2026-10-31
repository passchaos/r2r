use super::*;
use crate::{std_msgs::msg::UInt64MultiArray, Node, QosProfile, SubscriptionOptions};
use futures::{FutureExt, StreamExt};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex as StdMutex,
    },
    time::{Duration, Instant},
};

static DEFAULT_TYPED_LOCK: StdMutex<()> = StdMutex::new(());
static DEFAULT_TYPED_CREATED: AtomicUsize = AtomicUsize::new(0);
static DEFAULT_TYPED_DESTROYED: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct TrackedTyped(UInt64MultiArray);

unsafe impl WrappedTypesupport for TrackedTyped {
    type CStruct = <UInt64MultiArray as WrappedTypesupport>::CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t {
        UInt64MultiArray::get_ts()
    }

    fn create_msg() -> *mut Self::CStruct {
        let message = UInt64MultiArray::create_msg();
        if !message.is_null() {
            DEFAULT_TYPED_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }

    fn destroy_msg(message: *mut Self::CStruct) {
        DEFAULT_TYPED_DESTROYED.fetch_add(1, Ordering::Relaxed);
        UInt64MultiArray::destroy_msg(message);
    }

    fn from_native(message: &Self::CStruct) -> Self {
        Self(UInt64MultiArray::from_native(message))
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        self.0.copy_to_native(message);
    }
}

fn wait_for_match(node: &mut Node, publisher: &crate::Publisher<UInt64MultiArray>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while publisher.get_subscription_count().unwrap() == 0 {
        assert!(Instant::now() < deadline, "subscription discovery timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn next_sample(
    node: &mut Node, subscription: &mut Subscription<NativeSubscriptionSample<UInt64MultiArray>>,
) -> NativeSubscriptionSample<UInt64MultiArray> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(sample) = subscription.next().now_or_never().flatten() {
            return sample;
        }
        assert!(Instant::now() < deadline, "native sample timed out");
    }
}

fn next_typed_sample(
    node: &mut Node, subscription: &mut Subscription<ReusableSubscriptionSample<UInt64MultiArray>>,
) -> ReusableSubscriptionSample<UInt64MultiArray> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(sample) = subscription.next().now_or_never().flatten() {
            return sample;
        }
        assert!(Instant::now() < deadline, "typed reusable sample timed out");
    }
}

fn native_values(sample: &NativeSubscriptionSample<UInt64MultiArray>) -> &[u64] {
    let sequence = &sample.native_message().data;
    if sequence.size == 0 {
        &[]
    } else {
        assert!(!sequence.data.is_null());
        unsafe { std::slice::from_raw_parts(sequence.data, sequence.size) }
    }
}

fn message(values: Vec<u64>) -> UInt64MultiArray {
    UInt64MultiArray {
        layout: Default::default(),
        data: values,
    }
}

#[test]
fn default_typed_stream_reuses_native_decode_storage_behind_owned_samples() {
    let _lock = DEFAULT_TYPED_LOCK.lock().unwrap();
    DEFAULT_TYPED_CREATED.store(0, Ordering::Relaxed);
    DEFAULT_TYPED_DESTROYED.store(0, Ordering::Relaxed);
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "default_typed_native_reuse", "").unwrap();
    let topic = format!("/default_typed_native_reuse_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<UInt64MultiArray>(&topic, QosProfile::default())
        .unwrap();
    let mut subscription = node
        .subscribe::<TrackedTyped>(&topic, QosProfile::default())
        .unwrap();
    wait_for_match(&mut node, &publisher);

    {
        let mut receive = |node: &mut Node| {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                node.try_spin_once(Duration::from_millis(10)).unwrap();
                if let Some(sample) = subscription.next().now_or_never().flatten() {
                    break sample;
                }
                assert!(Instant::now() < deadline, "default typed sample timed out");
            }
        };

        publisher.publish(&message(vec![7; 4096])).unwrap();
        let first = receive(&mut node);
        assert_eq!(first.0.data, [7; 4096]);
        assert_eq!(DEFAULT_TYPED_CREATED.load(Ordering::Relaxed), 1);

        publisher.publish(&message(vec![11; 64])).unwrap();
        let second = receive(&mut node);
        assert_eq!(second.0.data, [11; 64]);
        assert_eq!(first.0.data, [7; 4096]);
        assert_eq!(DEFAULT_TYPED_CREATED.load(Ordering::Relaxed), 1);
        assert_eq!(DEFAULT_TYPED_DESTROYED.load(Ordering::Relaxed), 0);
    }

    node.destroy_subscription(subscription);
    assert_eq!(DEFAULT_TYPED_DESTROYED.load(Ordering::Relaxed), 1);
}

#[test]
fn reusable_native_stream_recycles_only_unretained_storage_and_outlives_endpoint() {
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "reusable_native_subscription", "").unwrap();
    let topic = format!("/reusable_native_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<UInt64MultiArray>(&topic, QosProfile::default())
        .unwrap();
    let mut subscription = node
        .subscribe_native_reusable_with_options::<UInt64MultiArray>(
            &topic,
            QosProfile::default(),
            SubscriptionOptions::default().channel_capacity(2),
        )
        .unwrap();
    wait_for_match(&mut node, &publisher);

    let large = (0..4096).map(|value| value as u64).collect::<Vec<_>>();
    publisher.publish(&message(large.clone())).unwrap();
    let first = next_sample(&mut node, &mut subscription);
    let first_address = first.native_message() as *const _ as usize;
    assert_eq!(native_values(&first), large);
    drop(first);

    publisher.publish(&message(vec![7; 128])).unwrap();
    let second = next_sample(&mut node, &mut subscription);
    let second_address = second.native_message() as *const _ as usize;
    assert_eq!(second_address, first_address);
    assert_eq!(native_values(&second), [7; 128]);

    publisher.publish(&message(vec![11; 64])).unwrap();
    let third = next_sample(&mut node, &mut subscription);
    let third_address = third.native_message() as *const _ as usize;
    assert_ne!(third_address, second_address, "retained sample storage was overwritten");
    assert_eq!(native_values(&second), [7; 128]);
    assert_eq!(native_values(&third), [11; 64]);

    drop(second);
    drop(third);
    publisher.publish(&message(vec![13; 32])).unwrap();
    let final_sample = next_sample(&mut node, &mut subscription);
    assert_eq!(final_sample.native_message() as *const _ as usize, third_address);
    assert_eq!(native_values(&final_sample), [13; 32]);

    node.destroy_subscription(subscription);
    assert_eq!(native_values(&final_sample), [13; 32]);
    drop(final_sample);
}

#[test]
fn reusable_typed_stream_retains_rust_capacity_and_sample_independence() {
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "reusable_typed_subscription", "").unwrap();
    let topic = format!("/reusable_typed_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<UInt64MultiArray>(&topic, QosProfile::default())
        .unwrap();
    let mut subscription = node
        .subscribe_reusable_with_options::<UInt64MultiArray>(
            &topic,
            QosProfile::default(),
            SubscriptionOptions::default().channel_capacity(2),
        )
        .unwrap();
    wait_for_match(&mut node, &publisher);

    let large = (0..4096).map(|value| value as u64).collect::<Vec<_>>();
    publisher.publish(&message(large.clone())).unwrap();
    let first = next_typed_sample(&mut node, &mut subscription);
    let first_data = first.data.as_ptr();
    let first_capacity = first.data.capacity();
    assert_eq!(first.data, large);
    drop(first);

    publisher.publish(&message(vec![7; 128])).unwrap();
    let second = next_typed_sample(&mut node, &mut subscription);
    assert_eq!(second.data.as_ptr(), first_data);
    assert_eq!(second.data.capacity(), first_capacity);
    assert_eq!(second.data, [7; 128]);

    publisher.publish(&message(vec![11; 64])).unwrap();
    let third = next_typed_sample(&mut node, &mut subscription);
    let third_data = third.data.as_ptr();
    assert_ne!(third_data, second.data.as_ptr());
    assert_eq!(second.data, [7; 128]);
    assert_eq!(third.data, [11; 64]);

    drop(second);
    drop(third);
    publisher.publish(&message(vec![13; 32])).unwrap();
    let final_sample = next_typed_sample(&mut node, &mut subscription);
    assert_eq!(final_sample.data.as_ptr(), third_data);
    assert_eq!(final_sample.data, [13; 32]);
    node.destroy_subscription(subscription);
    assert_eq!(final_sample.data, [13; 32]);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual 4096-element fresh versus reusable native subscription allocation stage; LD_PRELOAD probe required"]
fn benchmark_fresh_vs_reusable_native_subscription_delivery() {
    use crate::allocation_probe;
    const ITERATIONS: usize = 10_000;
    const ELEMENTS: usize = 4096;

    fn measure(
        probe: &allocation_probe::Probe, mut operation: impl FnMut(),
    ) -> (allocation_probe::NativeStats, u128) {
        let mut stats = allocation_probe::NativeStats::default();
        assert_eq!(unsafe { (probe.begin)() }, 0);
        let begin = Instant::now();
        for _ in 0..ITERATIONS {
            operation();
        }
        let elapsed_ns = begin.elapsed().as_nanos();
        assert_eq!(unsafe { (probe.end)(&mut stats) }, 0);
        assert_eq!(stats.failed_allocations + stats.counter_overflows + stats.size_overflows, 0);
        (stats, elapsed_ns)
    }

    let probe = allocation_probe::Probe::load()
        .unwrap()
        .expect("LD_PRELOAD allocation probe required");
    let source = message((0..ELEMENTS as u64).collect());
    let info: MessageInfo = rmw_message_info_t::default().into();

    let (mut fresh_sender, mut fresh_receiver) = mpsc::channel(0);
    let mut fresh_step = || {
        let mut native = WrappedNativeMsg::<UInt64MultiArray>::try_new().unwrap();
        source.try_copy_to_native(&mut native).unwrap();
        fresh_sender
            .try_send(ReceivedMessage::new(native, info.clone()))
            .unwrap();
        let sample = fresh_receiver.next().now_or_never().unwrap().unwrap();
        assert_eq!(sample.message.data.size, ELEMENTS);
        std::hint::black_box(sample.message.data.data);
    };
    for _ in 0..1000 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let buffer = NativeBuffer::default();
    let mut initial = buffer
        .checkout(WrappedNativeMsg::<UInt64MultiArray>::try_new)
        .unwrap();
    source.try_copy_to_native(&mut initial).unwrap();
    drop(initial);
    let (mut reusable_sender, mut reusable_receiver) = mpsc::channel(0);
    let mut reusable_step = || {
        let mut native = buffer
            .checkout(|| panic!("reusable native subscription slot was not recycled"))
            .unwrap();
        source.try_copy_to_native(&mut native).unwrap();
        reusable_sender
            .try_send(NativeSubscriptionSample {
                message: native,
                info: info.clone(),
            })
            .unwrap();
        let sample = reusable_receiver.next().now_or_never().unwrap().unwrap();
        assert_eq!(sample.data.size, ELEMENTS);
        std::hint::black_box(sample.data.data);
    };
    for _ in 0..1000 {
        reusable_step();
    }
    let (reusable, reusable_elapsed_ns) = measure(&probe, &mut reusable_step);

    let fresh_allocations =
        fresh.malloc_calls + fresh.calloc_calls + fresh.realloc_calls + fresh.aligned_calls;
    let reusable_allocations = reusable.malloc_calls
        + reusable.calloc_calls
        + reusable.realloc_calls
        + reusable.aligned_calls;
    assert!(fresh_allocations - reusable_allocations >= ITERATIONS as u64);
    assert!(fresh.free_calls - reusable.free_calls >= ITERATIONS as u64);
    assert!(
        fresh.requested_bytes - reusable.requested_bytes
            >= ITERATIONS as u64 * (ELEMENTS as u64 * 8)
    );
    for (delivery, stats, elapsed_ns) in [
        ("fresh", &fresh, fresh_elapsed_ns),
        ("reusable", &reusable, reusable_elapsed_ns),
    ] {
        let allocations =
            stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls;
        println!(
            "{}",
            serde_json::json!({
                "boundary":"generated native message fill, channel send/receive and payload validation; no RCL/DDS",
                "delivery":delivery,
                "iterations":ITERATIONS,
                "elements":ELEMENTS,
                "elapsed_ns":elapsed_ns,
                "allocations":allocations,
                "frees":stats.free_calls,
                "requested_bytes":stats.requested_bytes
            })
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual 4096-element fresh versus reusable typed subscription allocation stage; LD_PRELOAD probe required"]
fn benchmark_fresh_vs_reusable_typed_subscription_delivery() {
    use crate::allocation_probe;
    const ITERATIONS: usize = 10_000;
    const ELEMENTS: usize = 4096;

    fn measure(
        probe: &allocation_probe::Probe, mut operation: impl FnMut(),
    ) -> (allocation_probe::NativeStats, u128) {
        let mut stats = allocation_probe::NativeStats::default();
        assert_eq!(unsafe { (probe.begin)() }, 0);
        let begin = Instant::now();
        for _ in 0..ITERATIONS {
            operation();
        }
        let elapsed_ns = begin.elapsed().as_nanos();
        assert_eq!(unsafe { (probe.end)(&mut stats) }, 0);
        assert_eq!(stats.failed_allocations + stats.counter_overflows + stats.size_overflows, 0);
        (stats, elapsed_ns)
    }

    let probe = allocation_probe::Probe::load()
        .unwrap()
        .expect("LD_PRELOAD allocation probe required");
    let source = message((0..ELEMENTS as u64).collect());
    let native = WrappedNativeMsg::<UInt64MultiArray>::try_from(&source).unwrap();
    let info: MessageInfo = rmw_message_info_t::default().into();

    let (mut fresh_sender, mut fresh_receiver) = mpsc::channel(0);
    let mut fresh_step = || {
        fresh_sender
            .try_send(ReceivedMessage::new(UInt64MultiArray::from_native(&native), info.clone()))
            .unwrap();
        let sample = fresh_receiver.next().now_or_never().unwrap().unwrap();
        assert_eq!(sample.message.data.len(), ELEMENTS);
        std::hint::black_box(sample.message.data.as_ptr());
    };
    for _ in 0..1000 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let buffer = NativeBuffer::default();
    let mut initial = buffer.checkout(|| Ok(UInt64MultiArray::default())).unwrap();
    initial.update_from_native(&native);
    drop(initial);
    let (mut reusable_sender, mut reusable_receiver) = mpsc::channel(0);
    let mut reusable_step = || {
        let mut message = buffer
            .checkout(|| panic!("reusable typed subscription slot was not recycled"))
            .unwrap();
        message.update_from_native(&native);
        reusable_sender
            .try_send(ReusableSubscriptionSample {
                message,
                info: info.clone(),
            })
            .unwrap();
        let sample = reusable_receiver.next().now_or_never().unwrap().unwrap();
        assert_eq!(sample.data.len(), ELEMENTS);
        std::hint::black_box(sample.data.as_ptr());
    };
    for _ in 0..1000 {
        reusable_step();
    }
    let (reusable, reusable_elapsed_ns) = measure(&probe, &mut reusable_step);

    let fresh_allocations =
        fresh.malloc_calls + fresh.calloc_calls + fresh.realloc_calls + fresh.aligned_calls;
    let reusable_allocations = reusable.malloc_calls
        + reusable.calloc_calls
        + reusable.realloc_calls
        + reusable.aligned_calls;
    assert!(fresh_allocations - reusable_allocations >= ITERATIONS as u64);
    assert!(fresh.free_calls - reusable.free_calls >= ITERATIONS as u64);
    assert!(
        fresh.requested_bytes - reusable.requested_bytes
            >= ITERATIONS as u64 * (ELEMENTS as u64 * 8 - 256)
    );
    for (delivery, stats, elapsed_ns) in [
        ("fresh", &fresh, fresh_elapsed_ns),
        ("reusable", &reusable, reusable_elapsed_ns),
    ] {
        let allocations =
            stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls;
        println!(
            "{}",
            serde_json::json!({
                "boundary":"generated native-to-Rust decode, channel send/receive and payload validation; no RCL/DDS",
                "delivery":delivery,
                "iterations":ITERATIONS,
                "elements":ELEMENTS,
                "elapsed_ns":elapsed_ns,
                "allocations":allocations,
                "frees":stats.free_calls,
                "requested_bytes":stats.requested_bytes
            })
        );
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual default typed fresh versus reused native decode storage allocation stage; LD_PRELOAD probe required"]
fn benchmark_default_typed_fresh_vs_reused_native_decode_storage() {
    use crate::allocation_probe;
    const ITERATIONS: usize = 10_000;
    const ELEMENTS: usize = 4096;

    fn measure(
        probe: &allocation_probe::Probe, mut operation: impl FnMut(),
    ) -> (allocation_probe::NativeStats, u128) {
        let mut stats = allocation_probe::NativeStats::default();
        assert_eq!(unsafe { (probe.begin)() }, 0);
        let begin = Instant::now();
        for _ in 0..ITERATIONS {
            operation();
        }
        let elapsed_ns = begin.elapsed().as_nanos();
        assert_eq!(unsafe { (probe.end)(&mut stats) }, 0);
        assert_eq!(stats.failed_allocations + stats.counter_overflows + stats.size_overflows, 0);
        (stats, elapsed_ns)
    }

    let probe = allocation_probe::Probe::load()
        .unwrap()
        .expect("LD_PRELOAD allocation probe required");
    let source = message((0..ELEMENTS as u64).collect());

    let (mut fresh_sender, mut fresh_receiver) = mpsc::channel(0);
    let mut fresh_step = || {
        let native = WrappedNativeMsg::<UInt64MultiArray>::try_from(&source).unwrap();
        fresh_sender
            .try_send(UInt64MultiArray::from_native(&native))
            .unwrap();
        let sample = fresh_receiver.next().now_or_never().unwrap().unwrap();
        assert_eq!(sample.data.len(), ELEMENTS);
        std::hint::black_box(sample.data.as_ptr());
    };
    for _ in 0..1000 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let buffer = NativeBuffer::default();
    let initial = buffer
        .checkout(|| WrappedNativeMsg::<UInt64MultiArray>::try_from(&source))
        .unwrap();
    drop(initial);
    let (mut reused_sender, mut reused_receiver) = mpsc::channel(0);
    let mut reused_step = || {
        let mut native = buffer
            .checkout(|| panic!("default typed native decode slot was not recycled"))
            .unwrap();
        source.try_copy_to_native(&mut native).unwrap();
        let decoded = UInt64MultiArray::from_native(&native);
        drop(native);
        reused_sender.try_send(decoded).unwrap();
        let sample = reused_receiver.next().now_or_never().unwrap().unwrap();
        assert_eq!(sample.data.len(), ELEMENTS);
        std::hint::black_box(sample.data.as_ptr());
    };
    for _ in 0..1000 {
        reused_step();
    }
    let (reused, reused_elapsed_ns) = measure(&probe, &mut reused_step);

    let allocations = |stats: &allocation_probe::NativeStats| {
        stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls
    };
    assert!(allocations(&fresh) - allocations(&reused) >= ITERATIONS as u64 * 2);
    assert!(fresh.free_calls - reused.free_calls >= ITERATIONS as u64 * 2);
    assert!(
        fresh.requested_bytes - reused.requested_bytes >= ITERATIONS as u64 * (ELEMENTS as u64 * 8)
    );
    for (storage, stats, elapsed_ns) in [
        ("fresh_native", &fresh, fresh_elapsed_ns),
        ("reused_native", &reused, reused_elapsed_ns),
    ] {
        println!(
            "{}",
            serde_json::json!({
                "boundary":"default typed native fill, owned Rust decode, channel send/receive and payload validation; no RCL/DDS",
                "native_storage":storage,
                "iterations":ITERATIONS,
                "elements":ELEMENTS,
                "elapsed_ns":elapsed_ns,
                "allocations":allocations(stats),
                "frees":stats.free_calls,
                "requested_bytes":stats.requested_bytes
            })
        );
    }
}
