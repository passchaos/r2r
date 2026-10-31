use super::*;
use futures::{FutureExt, StreamExt};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use crate::{
    example_interfaces::srv::{AddTwoInts, SetBool},
    services::Service,
    QosProfile,
};

static TRACKED_RESPONSE_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
static TRACKED_RESPONSE_CREATED: AtomicUsize = AtomicUsize::new(0);
static TRACKED_RESPONSE_DESTROYED: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct TrackedResponse {
    sum: i64,
}

unsafe impl WrappedTypesupport for TrackedResponse {
    type CStruct = <AddTwoInts::Response as WrappedTypesupport>::CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t {
        AddTwoInts::Response::get_ts()
    }

    fn create_msg() -> *mut Self::CStruct {
        let message = AddTwoInts::Response::create_msg();
        if !message.is_null() {
            TRACKED_RESPONSE_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }

    fn destroy_msg(message: *mut Self::CStruct) {
        TRACKED_RESPONSE_DESTROYED.fetch_add(1, Ordering::Relaxed);
        AddTwoInts::Response::destroy_msg(message);
    }

    fn from_native(message: &Self::CStruct) -> Self {
        Self {
            sum: AddTwoInts::Response::from_native(message).sum,
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        AddTwoInts::Response { sum: self.sum }.copy_to_native(message);
    }
}

#[derive(Clone, Debug)]
struct TrackedService;

unsafe impl WrappedServiceTypeSupport for TrackedService {
    type Request = AddTwoInts::Request;
    type Response = TrackedResponse;
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

fn respond(node: &mut Node, service: &mut Service<AddTwoInts::Service>, expected: i64) {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(request) = service.next().now_or_never().flatten() {
            assert_eq!(request.message.a + request.message.b, expected);
            request
                .respond(AddTwoInts::Response { sum: expected })
                .unwrap();
            return;
        }
        assert!(Instant::now() < deadline, "service request timed out");
    }
}

fn finish<T>(
    node: &mut Node,
    response: &mut std::pin::Pin<Box<impl std::future::Future<Output = Result<T>>>>,
) -> T {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(response) = response.as_mut().now_or_never() {
            return response.unwrap();
        }
        assert!(Instant::now() < deadline, "client response timed out");
    }
}

#[test]
fn typed_client_reuses_response_take_storage_for_plain_and_metadata_futures() {
    let _lock = TRACKED_RESPONSE_LOCK.lock();
    TRACKED_RESPONSE_CREATED.store(0, Ordering::Relaxed);
    TRACKED_RESPONSE_DESTROYED.store(0, Ordering::Relaxed);
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "typed_client_response_reuse", "").unwrap();
    let service_name = format!("/typed_client_response_reuse_{}", uuid::Uuid::new_v4().simple());
    let mut service = node
        .create_service::<AddTwoInts::Service>(&service_name, QosProfile::services_default())
        .unwrap();
    let client = node
        .create_client::<TrackedService>(&service_name, QosProfile::services_default())
        .unwrap();
    let client_clone = client.clone();
    wait_for_service(&mut node, &client);

    let mut first = Box::pin(
        client
            .request(&AddTwoInts::Request { a: 19, b: 23 })
            .unwrap(),
    );
    respond(&mut node, &mut service, 42);
    assert_eq!(finish(&mut node, &mut first).sum, 42);
    assert_eq!(TRACKED_RESPONSE_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(TRACKED_RESPONSE_DESTROYED.load(Ordering::Relaxed), 0);

    let mut second = Box::pin(
        client_clone
            .request_with_info(&AddTwoInts::Request { a: 40, b: 2 })
            .unwrap(),
    );
    respond(&mut node, &mut service, 42);
    let second = finish(&mut node, &mut second);
    assert_eq!(second.response.sum, 42);
    assert!(second.info.sequence_number > 0);
    assert_eq!(TRACKED_RESPONSE_CREATED.load(Ordering::Relaxed), 1);

    node.destroy_client(client);
    assert_eq!(TRACKED_RESPONSE_DESTROYED.load(Ordering::Relaxed), 1);
    assert!(matches!(
        client_clone.request(&AddTwoInts::Request { a: 1, b: 2 }),
        Err(Error::RCL_RET_CLIENT_INVALID)
    ));
    drop(client_clone);
    assert_eq!(TRACKED_RESPONSE_DESTROYED.load(Ordering::Relaxed), 1);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual typed client response native reuse allocation stage; LD_PRELOAD probe required"]
fn benchmark_typed_client_response_fresh_vs_reused_native_decode() {
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
    let source = SetBool::Response {
        success: true,
        message: crate::RosString::from("x".repeat(STRING_BYTES)),
    };

    let mut fresh_step = || {
        let native = WrappedNativeMsg::<SetBool::Response>::try_from(&source).unwrap();
        let decoded = SetBool::Response::from_native(&native);
        assert_eq!(decoded.message.as_bytes().len(), STRING_BYTES);
        std::hint::black_box(decoded.message.as_bytes().as_ptr());
    };
    for _ in 0..1000 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let buffer = NativeBuffer::default();
    drop(
        buffer
            .checkout(|| WrappedNativeMsg::<SetBool::Response>::try_from(&source))
            .unwrap(),
    );
    let mut reused_step = || {
        let mut native = buffer
            .checkout(|| panic!("typed client response slot was not recycled"))
            .unwrap();
        source.try_copy_to_native(&mut native).unwrap();
        let decoded = SetBool::Response::from_native(&native);
        drop(native);
        assert_eq!(decoded.message.as_bytes().len(), STRING_BYTES);
        std::hint::black_box(decoded.message.as_bytes().as_ptr());
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
        fresh.requested_bytes - reused.requested_bytes >= ITERATIONS as u64 * STRING_BYTES as u64
    );
    for (storage, stats, elapsed_ns) in [
        ("fresh_native", &fresh, fresh_elapsed_ns),
        ("reused_native", &reused, reused_elapsed_ns),
    ] {
        println!(
            "{}",
            serde_json::json!({
                "boundary":"typed client native response fill, owned decode and validation; no RCL/DDS",
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
