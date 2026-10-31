use super::*;
use futures::{FutureExt, StreamExt};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use crate::{std_msgs::msg::UInt64MultiArray, QosProfile, Subscription};

static TRACKED_PUBLISHER_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
static TRACKED_PUBLISHER_CREATED: AtomicUsize = AtomicUsize::new(0);
static TRACKED_PUBLISHER_DESTROYED: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct TrackedPublisher {
    message: UInt64MultiArray,
    fail_copy: bool,
}

unsafe impl WrappedTypesupport for TrackedPublisher {
    type CStruct = <UInt64MultiArray as WrappedTypesupport>::CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t {
        UInt64MultiArray::get_ts()
    }

    fn create_msg() -> *mut Self::CStruct {
        let message = UInt64MultiArray::create_msg();
        if !message.is_null() {
            TRACKED_PUBLISHER_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }

    fn destroy_msg(message: *mut Self::CStruct) {
        TRACKED_PUBLISHER_DESTROYED.fetch_add(1, Ordering::Relaxed);
        UInt64MultiArray::destroy_msg(message);
    }

    fn from_native(message: &Self::CStruct) -> Self {
        Self {
            message: UInt64MultiArray::from_native(message),
            fail_copy: false,
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        self.try_copy_to_native(message)
            .expect("tracked publisher copy failed");
    }

    fn try_copy_to_native(&self, message: &mut Self::CStruct) -> Result<()> {
        if self.fail_copy {
            Err(Error::RCL_RET_ERROR)
        } else {
            self.message.try_copy_to_native(message)
        }
    }
}

fn message(value: u64, elements: usize) -> UInt64MultiArray {
    UInt64MultiArray {
        layout: Default::default(),
        data: vec![value; elements],
    }
}

fn wait_for_match(node: &mut Node, publisher: &Publisher<TrackedPublisher>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while publisher.get_subscription_count().unwrap() == 0 {
        assert!(Instant::now() < deadline, "publisher discovery timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn receive(node: &mut Node, subscription: &mut Subscription<UInt64MultiArray>) -> UInt64MultiArray {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(message) = subscription.next().now_or_never().flatten() {
            return message;
        }
        assert!(Instant::now() < deadline, "published message timed out");
    }
}

#[test]
fn default_typed_publisher_reuses_native_storage_and_retires_failed_copies() {
    let _lock = TRACKED_PUBLISHER_LOCK.lock();
    TRACKED_PUBLISHER_CREATED.store(0, Ordering::Relaxed);
    TRACKED_PUBLISHER_DESTROYED.store(0, Ordering::Relaxed);
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "default_typed_publisher_reuse", "").unwrap();
    let topic = format!("/default_typed_publisher_reuse_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<TrackedPublisher>(&topic, QosProfile::default())
        .unwrap();
    let publisher_clone = publisher.clone();
    let mut subscription = node
        .subscribe::<UInt64MultiArray>(&topic, QosProfile::default())
        .unwrap();
    wait_for_match(&mut node, &publisher);

    publisher
        .publish(&TrackedPublisher {
            message: message(7, 4096),
            fail_copy: false,
        })
        .unwrap();
    assert_eq!(receive(&mut node, &mut subscription).data, [7; 4096]);
    assert_eq!(TRACKED_PUBLISHER_CREATED.load(Ordering::Relaxed), 1);

    publisher_clone
        .publish(&TrackedPublisher {
            message: message(11, 64),
            fail_copy: false,
        })
        .unwrap();
    assert_eq!(receive(&mut node, &mut subscription).data, [11; 64]);
    assert_eq!(TRACKED_PUBLISHER_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(TRACKED_PUBLISHER_DESTROYED.load(Ordering::Relaxed), 0);

    assert!(matches!(
        publisher.publish(&TrackedPublisher {
            message: message(13, 32),
            fail_copy: true,
        }),
        Err(Error::RCL_RET_ERROR)
    ));
    assert_eq!(TRACKED_PUBLISHER_DESTROYED.load(Ordering::Relaxed), 1);

    publisher_clone
        .publish(&TrackedPublisher {
            message: message(17, 16),
            fail_copy: false,
        })
        .unwrap();
    assert_eq!(receive(&mut node, &mut subscription).data, [17; 16]);
    assert_eq!(TRACKED_PUBLISHER_CREATED.load(Ordering::Relaxed), 2);

    node.destroy_publisher(publisher);
    assert_eq!(TRACKED_PUBLISHER_DESTROYED.load(Ordering::Relaxed), 1);
    drop(publisher_clone);
    assert_eq!(TRACKED_PUBLISHER_DESTROYED.load(Ordering::Relaxed), 2);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual default typed publisher native preparation allocation stage; LD_PRELOAD probe required"]
fn benchmark_default_typed_publisher_fresh_vs_reused_native_preparation() {
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
    let source = message(23, ELEMENTS);

    let mut fresh_step = || {
        let native = WrappedNativeMsg::<UInt64MultiArray>::try_from(&source).unwrap();
        assert_eq!(native.data.size, ELEMENTS);
        std::hint::black_box(native.data.data);
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
    let mut reused_step = || {
        let mut native = buffer
            .checkout(|| panic!("typed publisher native slot was not recycled"))
            .unwrap();
        source.try_copy_to_native(&mut native).unwrap();
        assert_eq!(native.data.size, ELEMENTS);
        std::hint::black_box(native.data.data);
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
                "boundary":"default typed publisher Rust-to-native preparation and payload validation; no RCL/DDS",
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
