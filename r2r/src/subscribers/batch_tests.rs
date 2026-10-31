use super::*;
use crate::{std_msgs::msg::UInt64MultiArray, Node, QosProfile, SubscriptionOptions};
use futures::{FutureExt, StreamExt};
use std::time::{Duration, Instant};

const BATCH_SIZE: usize = 4;

fn message(epoch: u64, sequence: u64, elements: usize) -> UInt64MultiArray {
    let mut data = Vec::with_capacity(elements.max(2));
    data.push(epoch);
    data.push(sequence);
    data.resize(elements.max(2), epoch ^ sequence);
    UInt64MultiArray {
        layout: Default::default(),
        data,
    }
}

fn wait_for_match(node: &mut Node, publisher: &crate::Publisher<UInt64MultiArray>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while publisher.get_subscription_count().unwrap() == 0 {
        assert!(Instant::now() < deadline, "batch subscription discovery timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn publish_batch(publisher: &crate::Publisher<UInt64MultiArray>, epoch: u64, elements: usize) {
    for sequence in 0..BATCH_SIZE as u64 {
        publisher
            .publish(&message(epoch, sequence, elements))
            .unwrap();
    }
    publisher
        .wait_for_all_acked(Duration::from_secs(3))
        .unwrap();
}

fn next_native_batch(
    node: &mut Node, subscription: &mut Subscription<NativeSubscriptionBatch<UInt64MultiArray>>,
) -> NativeSubscriptionBatch<UInt64MultiArray> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(batch) = subscription.next().now_or_never().flatten() {
            return batch;
        }
        assert!(Instant::now() < deadline, "native batch timed out");
    }
}

fn next_typed_batch(
    node: &mut Node, subscription: &mut Subscription<SubscriptionBatch<UInt64MultiArray>>,
) -> SubscriptionBatch<UInt64MultiArray> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(batch) = subscription.next().now_or_never().flatten() {
            return batch;
        }
        assert!(Instant::now() < deadline, "typed batch timed out");
    }
}

fn native_values(message: &<UInt64MultiArray as WrappedTypesupport>::CStruct) -> &[u64] {
    let sequence = &message.data;
    if sequence.size == 0 {
        &[]
    } else {
        assert!(!sequence.data.is_null());
        unsafe { std::slice::from_raw_parts(sequence.data, sequence.size) }
    }
}

#[test]
fn batch_subscriptions_reject_zero_before_creating_an_endpoint() {
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "invalid_batch_subscription", "").unwrap();
    let topic = format!("/invalid_batch_{}", uuid::Uuid::new_v4().simple());

    assert!(matches!(
        node.subscribe_batches::<UInt64MultiArray>(&topic, QosProfile::default(), 0),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert!(matches!(
        node.subscribe_native_batches::<UInt64MultiArray>(&topic, QosProfile::default(), 0),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert_eq!(node.count_subscribers(&topic).unwrap(), 0);
}

#[test]
fn native_batches_use_one_take_preserve_order_and_outlive_the_endpoint() {
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "native_batch_subscription", "").unwrap();
    let topic = format!("/native_batch_{}", uuid::Uuid::new_v4().simple());
    let qos = QosProfile::default().keep_last(16);
    let publisher = node
        .create_publisher::<UInt64MultiArray>(&topic, qos.clone())
        .unwrap();
    let mut subscription = node
        .subscribe_native_batches_with_options::<UInt64MultiArray>(
            &topic,
            qos,
            BATCH_SIZE,
            SubscriptionOptions::default().channel_capacity(1),
        )
        .unwrap();
    wait_for_match(&mut node, &publisher);

    publish_batch(&publisher, 10, 32);
    let first = next_native_batch(&mut node, &mut subscription);
    assert_eq!(first.len(), BATCH_SIZE);
    assert_eq!(first.capacity(), BATCH_SIZE);
    assert_eq!(
        first
            .iter()
            .map(|(message, _)| native_values(message)[..2].to_vec())
            .collect::<Vec<_>>(),
        (0..BATCH_SIZE as u64)
            .map(|sequence| vec![10, sequence])
            .collect::<Vec<_>>()
    );
    let first_addresses = (0..BATCH_SIZE)
        .map(|index| first.message(index).unwrap() as *const _ as usize)
        .collect::<Vec<_>>();
    assert!(first.info(BATCH_SIZE).is_none());

    publish_batch(&publisher, 20, 16);
    let second = next_native_batch(&mut node, &mut subscription);
    assert_eq!(second.len(), BATCH_SIZE);
    let second_addresses = (0..BATCH_SIZE)
        .map(|index| second.message(index).unwrap() as *const _ as usize)
        .collect::<Vec<_>>();
    assert!(first_addresses
        .iter()
        .all(|address| !second_addresses.contains(address)));
    assert_eq!(native_values(first.message(0).unwrap())[..2], [10, 0]);
    assert_eq!(native_values(second.message(0).unwrap())[..2], [20, 0]);
    drop(first);

    publish_batch(&publisher, 30, 8);
    let reused_sequence = next_native_batch(&mut node, &mut subscription);
    assert_eq!(reused_sequence.len(), BATCH_SIZE);
    let reused_addresses = (0..BATCH_SIZE)
        .map(|index| reused_sequence.message(index).unwrap() as *const _ as usize)
        .collect::<Vec<_>>();
    assert_eq!(reused_addresses, first_addresses);
    assert_eq!(native_values(reused_sequence.message(0).unwrap())[..2], [30, 0]);
    assert_eq!(native_values(second.message(0).unwrap())[..2], [20, 0]);

    node.destroy_subscription(subscription);
    assert_eq!(native_values(second.message(3).unwrap())[..2], [20, 3]);
    assert_eq!(native_values(reused_sequence.message(3).unwrap())[..2], [30, 3]);
}

#[test]
fn native_batches_refresh_only_the_previous_successful_take_prefix() {
    const CAPACITY: usize = 16;
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "sparse_native_batch_subscription", "").unwrap();
    let topic = format!("/sparse_native_batch_{}", uuid::Uuid::new_v4().simple());
    let qos = QosProfile::default().keep_last(CAPACITY);
    let publisher = node
        .create_publisher::<UInt64MultiArray>(&topic, qos.clone())
        .unwrap();
    let mut subscription = node
        .subscribe_native_batches_with_options::<UInt64MultiArray>(
            &topic,
            qos,
            CAPACITY,
            SubscriptionOptions::default().channel_capacity(1),
        )
        .unwrap();
    wait_for_match(&mut node, &publisher);

    publisher.publish(&message(1, 0, 32)).unwrap();
    publisher
        .wait_for_all_acked(Duration::from_secs(3))
        .unwrap();
    let first = next_native_batch(&mut node, &mut subscription);
    assert_eq!(first.len(), 1);
    assert_eq!(first.capacity(), CAPACITY);
    assert_eq!(first.last_refresh_count(), 0);
    let first_address = first.message(0).unwrap() as *const _;
    drop(first);

    publisher.publish(&message(2, 0, 16)).unwrap();
    publisher
        .wait_for_all_acked(Duration::from_secs(3))
        .unwrap();
    let second = next_native_batch(&mut node, &mut subscription);
    assert_eq!(second.len(), 1);
    assert_eq!(second.last_refresh_count(), 1);
    assert_eq!(second.message(0).unwrap() as *const _, first_address);
    drop(second);

    publish_batch(&publisher, 3, 8);
    let full = next_native_batch(&mut node, &mut subscription);
    assert_eq!(full.len(), BATCH_SIZE);
    assert_eq!(full.last_refresh_count(), 1);
    drop(full);

    publisher.publish(&message(4, 0, 4)).unwrap();
    publisher
        .wait_for_all_acked(Duration::from_secs(3))
        .unwrap();
    let tail = next_native_batch(&mut node, &mut subscription);
    assert_eq!(tail.len(), 1);
    assert_eq!(tail.last_refresh_count(), BATCH_SIZE);
    assert_eq!(native_values(tail.message(0).unwrap())[..2], [4, 0]);
}

#[test]
fn typed_batches_reuse_dynamic_field_capacity_and_keep_retained_batches_independent() {
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "typed_batch_subscription", "").unwrap();
    let topic = format!("/typed_batch_{}", uuid::Uuid::new_v4().simple());
    let qos = QosProfile::default().keep_last(16);
    let publisher = node
        .create_publisher::<UInt64MultiArray>(&topic, qos.clone())
        .unwrap();
    let mut subscription = node
        .subscribe_batches_with_options::<UInt64MultiArray>(
            &topic,
            qos,
            BATCH_SIZE,
            SubscriptionOptions::default().channel_capacity(1),
        )
        .unwrap();
    wait_for_match(&mut node, &publisher);

    publish_batch(&publisher, 30, 4096);
    let first = next_typed_batch(&mut node, &mut subscription);
    assert_eq!(first.len(), BATCH_SIZE);
    assert_eq!(first.capacity(), BATCH_SIZE);
    assert_eq!(first[3].data[..2], [30, 3]);
    let pointers = first
        .messages()
        .iter()
        .map(|message| message.data.as_ptr())
        .collect::<Vec<_>>();
    let capacities = first
        .messages()
        .iter()
        .map(|message| message.data.capacity())
        .collect::<Vec<_>>();
    drop(first);

    publish_batch(&publisher, 40, 64);
    let reused = next_typed_batch(&mut node, &mut subscription);
    assert_eq!(reused.len(), BATCH_SIZE);
    for index in 0..BATCH_SIZE {
        assert_eq!(reused[index].data[..2], [40, index as u64]);
        assert_eq!(reused[index].data.as_ptr(), pointers[index]);
        assert_eq!(reused[index].data.capacity(), capacities[index]);
    }

    publish_batch(&publisher, 50, 32);
    let retained_peer = next_typed_batch(&mut node, &mut subscription);
    assert_eq!(retained_peer.len(), BATCH_SIZE);
    assert_eq!(reused[0].data[..2], [40, 0]);
    assert_eq!(retained_peer[0].data[..2], [50, 0]);
    assert_ne!(reused[0].data.as_ptr(), retained_peer[0].data.as_ptr());
    assert_eq!(
        retained_peer
            .iter()
            .map(|(message, _)| message.data[1])
            .collect::<Vec<_>>(),
        (0..BATCH_SIZE as u64).collect::<Vec<_>>()
    );

    node.destroy_subscription(subscription);
    assert_eq!(reused[3].data[..2], [40, 3]);
    assert_eq!(retained_peer[3].data[..2], [50, 3]);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual replacement versus in-place native batch refresh allocation stage; LD_PRELOAD probe required"]
fn benchmark_replacement_vs_in_place_native_batch_refresh() {
    use crate::allocation_probe;
    const ITERATIONS: usize = 10_000;
    const ELEMENTS: usize = 4096;

    fn make_messages(source: &UInt64MultiArray) -> Vec<WrappedNativeMsg<UInt64MultiArray>> {
        let mut messages = Vec::new();
        messages.try_reserve_exact(BATCH_SIZE).unwrap();
        for _ in 0..BATCH_SIZE {
            messages.push(WrappedNativeMsg::<UInt64MultiArray>::try_from(source).unwrap());
        }
        messages
    }

    fn measure(
        probe: &allocation_probe::Probe, mut operation: impl FnMut(),
    ) -> (allocation_probe::NativeStats, u128) {
        let mut stats = allocation_probe::NativeStats::default();
        assert_eq!(unsafe { (probe.begin)() }, 0);
        let started = Instant::now();
        for _ in 0..ITERATIONS {
            operation();
        }
        let elapsed_ns = started.elapsed().as_nanos();
        assert_eq!(unsafe { (probe.end)(&mut stats) }, 0);
        assert_eq!(stats.failed_allocations + stats.counter_overflows + stats.size_overflows, 0);
        (stats, elapsed_ns)
    }

    let probe = allocation_probe::Probe::load()
        .unwrap()
        .expect("LD_PRELOAD allocation probe required");
    let source = message(10, 0, ELEMENTS);

    let mut replacement_messages = make_messages(&source);
    let mut replacement_step = || {
        replacement_messages = make_messages(&source);
        std::hint::black_box(replacement_messages[0].data.data);
    };
    for _ in 0..100 {
        replacement_step();
    }
    let (replacement, replacement_elapsed_ns) = measure(&probe, &mut replacement_step);

    let mut in_place_messages = make_messages(&source);
    let addresses = in_place_messages
        .iter()
        .map(|message| std::ptr::from_ref(&**message))
        .collect::<Vec<_>>();
    let mut in_place_step = || {
        for message in &mut in_place_messages {
            assert!(UInt64MultiArray::try_reinitialize_native(message).unwrap());
            source.try_copy_to_native(message).unwrap();
        }
        std::hint::black_box(in_place_messages[0].data.data);
    };
    for _ in 0..100 {
        in_place_step();
    }
    let (in_place, in_place_elapsed_ns) = measure(&probe, &mut in_place_step);
    assert_eq!(
        in_place_messages
            .iter()
            .map(|message| std::ptr::from_ref(&**message))
            .collect::<Vec<_>>(),
        addresses
    );

    let replacement_allocations = replacement.malloc_calls
        + replacement.calloc_calls
        + replacement.realloc_calls
        + replacement.aligned_calls;
    let in_place_allocations = in_place.malloc_calls
        + in_place.calloc_calls
        + in_place.realloc_calls
        + in_place.aligned_calls;
    assert!(
        replacement_allocations - in_place_allocations
            >= ITERATIONS as u64 * (BATCH_SIZE as u64 + 1)
    );
    assert!(
        replacement.free_calls - in_place.free_calls >= ITERATIONS as u64 * (BATCH_SIZE as u64 + 1)
    );
    for (refresh, stats, elapsed_ns) in [
        ("replacement", &replacement, replacement_elapsed_ns),
        ("in_place", &in_place, in_place_elapsed_ns),
    ] {
        let allocations =
            stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls;
        println!(
            "{}",
            serde_json::json!({
                "boundary":"four generated native messages reinitialized and refilled with 4096 u64 values; no RCL/DDS/channel",
                "refresh":refresh,
                "iterations":ITERATIONS,
                "batch_size":BATCH_SIZE,
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
#[ignore = "manual full-capacity versus taken-prefix native batch refresh allocation stage; LD_PRELOAD probe required"]
fn benchmark_full_capacity_vs_taken_prefix_native_batch_refresh() {
    use crate::allocation_probe;
    const ITERATIONS: usize = 10_000;
    const CAPACITY: usize = 16;
    const TAKEN: usize = 1;

    fn measure(
        probe: &allocation_probe::Probe, mut operation: impl FnMut(),
    ) -> (allocation_probe::NativeStats, u128) {
        let mut stats = allocation_probe::NativeStats::default();
        assert_eq!(unsafe { (probe.begin)() }, 0);
        let started = Instant::now();
        for _ in 0..ITERATIONS {
            operation();
        }
        let elapsed_ns = started.elapsed().as_nanos();
        assert_eq!(unsafe { (probe.end)(&mut stats) }, 0);
        assert_eq!(stats.failed_allocations + stats.counter_overflows + stats.size_overflows, 0);
        (stats, elapsed_ns)
    }

    let probe = allocation_probe::Probe::load()
        .unwrap()
        .expect("LD_PRELOAD allocation probe required");
    let mut full = batch::NativeTakeBatch::<UInt64MultiArray>::try_new(CAPACITY).unwrap();
    let mut prefix = batch::NativeTakeBatch::<UInt64MultiArray>::try_new(CAPACITY).unwrap();
    let source = message(10, 0, 4096);
    source
        .try_copy_to_native(full.message_mut_for_test(0))
        .unwrap();
    source
        .try_copy_to_native(prefix.message_mut_for_test(0))
        .unwrap();
    let full_addresses = full.all_message_addresses();
    let prefix_addresses = prefix.all_message_addresses();

    let (full_stats, full_elapsed_ns) = measure(&probe, || {
        full.refresh_messages(CAPACITY).unwrap();
        source
            .try_copy_to_native(full.message_mut_for_test(0))
            .unwrap();
        std::hint::black_box(full.last_refresh_count());
    });
    let (prefix_stats, prefix_elapsed_ns) = measure(&probe, || {
        prefix.refresh_messages(TAKEN).unwrap();
        source
            .try_copy_to_native(prefix.message_mut_for_test(0))
            .unwrap();
        std::hint::black_box(prefix.last_refresh_count());
    });

    assert_eq!(full.all_message_addresses(), full_addresses);
    assert_eq!(prefix.all_message_addresses(), prefix_addresses);
    let allocations = |stats: &allocation_probe::NativeStats| {
        stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls
    };
    for (scope, refreshed_per_iteration, stats, elapsed_ns) in [
        ("full_capacity", CAPACITY, &full_stats, full_elapsed_ns),
        ("taken_prefix", TAKEN, &prefix_stats, prefix_elapsed_ns),
    ] {
        println!(
            "{}",
            serde_json::json!({
                "boundary":"generated native reset for a capacity-16 batch after one successful take, then refill one 4096-element message; no RCL/DDS/decode/channel",
                "refresh_scope":scope,
                "iterations":ITERATIONS,
                "capacity":CAPACITY,
                "taken":TAKEN,
                "elements":4096,
                "refresh_calls":ITERATIONS * refreshed_per_iteration,
                "elapsed_ns":elapsed_ns,
                "allocations":allocations(stats),
                "frees":stats.free_calls,
                "requested_bytes":stats.requested_bytes
            })
        );
    }
}
