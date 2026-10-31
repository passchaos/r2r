use super::*;
use futures::{FutureExt, StreamExt};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use crate::{
    example_interfaces::srv::AddTwoInts, rcl_interfaces::srv::DescribeParameters,
    services::Service, QosProfile,
};

static TRACKED_REQUEST_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
static TRACKED_REQUEST_CREATED: AtomicUsize = AtomicUsize::new(0);
static TRACKED_REQUEST_DESTROYED: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct TrackedRequest {
    a: i64,
    b: i64,
    fail_copy: bool,
}

unsafe impl WrappedTypesupport for TrackedRequest {
    type CStruct = <AddTwoInts::Request as WrappedTypesupport>::CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t {
        AddTwoInts::Request::get_ts()
    }

    fn create_msg() -> *mut Self::CStruct {
        let message = AddTwoInts::Request::create_msg();
        if !message.is_null() {
            TRACKED_REQUEST_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }

    fn destroy_msg(message: *mut Self::CStruct) {
        TRACKED_REQUEST_DESTROYED.fetch_add(1, Ordering::Relaxed);
        AddTwoInts::Request::destroy_msg(message);
    }

    fn from_native(message: &Self::CStruct) -> Self {
        let request = AddTwoInts::Request::from_native(message);
        Self {
            a: request.a,
            b: request.b,
            fail_copy: false,
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        self.try_copy_to_native(message)
            .expect("tracked client request copy failed");
    }

    fn try_copy_to_native(&self, message: &mut Self::CStruct) -> Result<()> {
        if self.fail_copy {
            Err(Error::RCL_RET_ERROR)
        } else {
            AddTwoInts::Request {
                a: self.a,
                b: self.b,
            }
            .try_copy_to_native(message)
        }
    }
}

#[derive(Clone, Debug)]
struct TrackedService;

unsafe impl WrappedServiceTypeSupport for TrackedService {
    type Request = TrackedRequest;
    type Response = AddTwoInts::Response;
    #[cfg(any(
        r2r__ros__distro__iron,
        r2r__ros__distro__jazzy,
        r2r__ros__distro__kilted,
        r2r__ros__distro__lyrical,
        r2r__ros__distro__rolling
    ))]
    type Event = AddTwoInts::Event;

    fn get_ts() -> &'static rosidl_service_type_support_t {
        AddTwoInts::Service::get_ts()
    }
}

fn wait_for_service(node: &mut Node, client: &Client<TrackedService>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !client.is_available(node).unwrap() {
        assert!(Instant::now() < deadline, "service discovery timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn roundtrip(
    node: &mut Node, service: &mut Service<AddTwoInts::Service>, client: &Client<TrackedService>,
    request: TrackedRequest,
) -> AddTwoInts::Response {
    let expected = request.a + request.b;
    let mut response = Box::pin(client.request(&request).unwrap());
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut responded = false;
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if !responded {
            if let Some(request) = service.next().now_or_never().flatten() {
                assert_eq!(request.message.a + request.message.b, expected);
                request
                    .respond(AddTwoInts::Response { sum: expected })
                    .unwrap();
                responded = true;
            }
        }
        if let Some(response) = response.as_mut().now_or_never() {
            return response.unwrap();
        }
        assert!(Instant::now() < deadline, "service roundtrip timed out");
    }
}

#[test]
fn default_typed_client_reuses_request_storage_and_retires_failed_copies() {
    let _lock = TRACKED_REQUEST_LOCK.lock();
    TRACKED_REQUEST_CREATED.store(0, Ordering::Relaxed);
    TRACKED_REQUEST_DESTROYED.store(0, Ordering::Relaxed);
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "default_typed_client_request_reuse", "").unwrap();
    let service_name = format!("/typed_client_request_reuse_{}", uuid::Uuid::new_v4().simple());
    let mut service = node
        .create_service::<AddTwoInts::Service>(&service_name, QosProfile::services_default())
        .unwrap();
    let client = node
        .create_client::<TrackedService>(&service_name, QosProfile::services_default())
        .unwrap();
    let client_clone = client.clone();
    wait_for_service(&mut node, &client);

    assert_eq!(
        roundtrip(
            &mut node,
            &mut service,
            &client,
            TrackedRequest {
                a: 19,
                b: 23,
                fail_copy: false,
            },
        )
        .sum,
        42
    );
    assert_eq!(TRACKED_REQUEST_CREATED.load(Ordering::Relaxed), 1);

    assert_eq!(
        roundtrip(
            &mut node,
            &mut service,
            &client_clone,
            TrackedRequest {
                a: 40,
                b: 2,
                fail_copy: false,
            },
        )
        .sum,
        42
    );
    assert_eq!(TRACKED_REQUEST_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(TRACKED_REQUEST_DESTROYED.load(Ordering::Relaxed), 0);

    assert!(matches!(
        client.request(&TrackedRequest {
            a: 1,
            b: 2,
            fail_copy: true,
        }),
        Err(Error::RCL_RET_ERROR)
    ));
    assert_eq!(TRACKED_REQUEST_DESTROYED.load(Ordering::Relaxed), 1);

    assert_eq!(
        roundtrip(
            &mut node,
            &mut service,
            &client_clone,
            TrackedRequest {
                a: -8,
                b: 50,
                fail_copy: false,
            },
        )
        .sum,
        42
    );
    assert_eq!(TRACKED_REQUEST_CREATED.load(Ordering::Relaxed), 2);

    node.destroy_client(client);
    assert_eq!(TRACKED_REQUEST_DESTROYED.load(Ordering::Relaxed), 1);
    drop(client_clone);
    assert_eq!(TRACKED_REQUEST_DESTROYED.load(Ordering::Relaxed), 2);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual typed client request native preparation allocation stage; LD_PRELOAD probe required"]
fn benchmark_typed_client_request_fresh_vs_reused_native_preparation() {
    use crate::allocation_probe;
    const ITERATIONS: usize = 10_000;
    const STRING_BYTES: usize = 4096;

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
    let source = DescribeParameters::Request {
        names: vec![crate::RosString::from("x".repeat(STRING_BYTES))],
    };

    let mut fresh_step = || {
        let native = WrappedNativeMsg::<DescribeParameters::Request>::try_from(&source).unwrap();
        assert_eq!(native.names.size, 1);
        assert!(!native.names.data.is_null());
        let first = unsafe { &*native.names.data };
        assert_eq!(first.size, STRING_BYTES);
        std::hint::black_box(first.data);
    };
    for _ in 0..1000 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let buffer = NativeBuffer::default();
    let initial = buffer
        .checkout(|| WrappedNativeMsg::<DescribeParameters::Request>::try_from(&source))
        .unwrap();
    drop(initial);
    let mut reused_step = || {
        let mut native = buffer
            .checkout(|| panic!("typed client request native slot was not recycled"))
            .unwrap();
        source.try_copy_to_native(&mut native).unwrap();
        assert_eq!(native.names.size, 1);
        assert!(!native.names.data.is_null());
        let first = unsafe { &*native.names.data };
        assert_eq!(first.size, STRING_BYTES);
        std::hint::black_box(first.data);
    };
    for _ in 0..1000 {
        reused_step();
    }
    let (reused, reused_elapsed_ns) = measure(&probe, &mut reused_step);

    let allocations = |stats: &allocation_probe::NativeStats| {
        stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls
    };
    assert!(allocations(&fresh) - allocations(&reused) >= ITERATIONS as u64 * 3);
    assert!(fresh.free_calls - reused.free_calls >= ITERATIONS as u64 * 3);
    assert!(
        fresh.requested_bytes - reused.requested_bytes >= ITERATIONS as u64 * STRING_BYTES as u64
    );
    for (storage, stats, elapsed_ns) in [
        ("fresh_native", &fresh, fresh_elapsed_ns),
        ("reused_native", &reused, reused_elapsed_ns),
    ] {
        println!(
            "{}",
            serde_json::json!({
                "boundary":"typed client request Rust-to-native preparation and payload validation; no RCL/DDS",
                "native_storage":storage,
                "iterations":ITERATIONS,
                "string_bytes":STRING_BYTES,
                "elapsed_ns":elapsed_ns,
                "allocations":allocations(stats),
                "frees":stats.free_calls,
                "requested_bytes":stats.requested_bytes
            })
        );
    }
}
