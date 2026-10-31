use super::*;
use futures::{FutureExt, StreamExt};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use crate::{
    example_interfaces::srv::{AddTwoInts, SetBool},
    QosProfile,
};

static TRACKED_SERVICE_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
static REQUEST_CREATED: AtomicUsize = AtomicUsize::new(0);
static REQUEST_DESTROYED: AtomicUsize = AtomicUsize::new(0);
static RESPONSE_CREATED: AtomicUsize = AtomicUsize::new(0);
static RESPONSE_DESTROYED: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct TrackedRequest {
    a: i64,
    b: i64,
}

unsafe impl WrappedTypesupport for TrackedRequest {
    type CStruct = <AddTwoInts::Request as WrappedTypesupport>::CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t {
        AddTwoInts::Request::get_ts()
    }

    fn create_msg() -> *mut Self::CStruct {
        let message = AddTwoInts::Request::create_msg();
        if !message.is_null() {
            REQUEST_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }

    fn destroy_msg(message: *mut Self::CStruct) {
        REQUEST_DESTROYED.fetch_add(1, Ordering::Relaxed);
        AddTwoInts::Request::destroy_msg(message);
    }

    fn from_native(message: &Self::CStruct) -> Self {
        let request = AddTwoInts::Request::from_native(message);
        Self {
            a: request.a,
            b: request.b,
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        AddTwoInts::Request {
            a: self.a,
            b: self.b,
        }
        .copy_to_native(message);
    }

    fn try_copy_to_native(&self, message: &mut Self::CStruct) -> Result<()> {
        AddTwoInts::Request {
            a: self.a,
            b: self.b,
        }
        .try_copy_to_native(message)
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct TrackedResponse {
    sum: i64,
    fail_copy: bool,
}

unsafe impl WrappedTypesupport for TrackedResponse {
    type CStruct = <AddTwoInts::Response as WrappedTypesupport>::CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t {
        AddTwoInts::Response::get_ts()
    }

    fn create_msg() -> *mut Self::CStruct {
        let message = AddTwoInts::Response::create_msg();
        if !message.is_null() {
            RESPONSE_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }

    fn destroy_msg(message: *mut Self::CStruct) {
        RESPONSE_DESTROYED.fetch_add(1, Ordering::Relaxed);
        AddTwoInts::Response::destroy_msg(message);
    }

    fn from_native(message: &Self::CStruct) -> Self {
        Self {
            sum: AddTwoInts::Response::from_native(message).sum,
            fail_copy: false,
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        self.try_copy_to_native(message)
            .expect("tracked service response copy failed");
    }

    fn try_copy_to_native(&self, message: &mut Self::CStruct) -> Result<()> {
        if self.fail_copy {
            Err(Error::RCL_RET_ERROR)
        } else {
            AddTwoInts::Response { sum: self.sum }.try_copy_to_native(message)
        }
    }
}

#[derive(Clone, Debug)]
struct TrackedService;

unsafe impl WrappedServiceTypeSupport for TrackedService {
    type Request = TrackedRequest;
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

fn wait_for_service(node: &mut Node, client: &crate::Client<AddTwoInts::Service>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !client.is_available(node).unwrap() {
        assert!(Instant::now() < deadline, "service discovery timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn take_request(
    node: &mut Node, service: &mut Service<TrackedService>,
) -> ServiceRequest<TrackedService> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(request) = service.next().now_or_never().flatten() {
            return request;
        }
        assert!(Instant::now() < deadline, "service request timed out");
    }
}

fn finish_response(
    node: &mut Node,
    response: &mut std::pin::Pin<
        Box<impl std::future::Future<Output = Result<AddTwoInts::Response>>>,
    >,
) -> AddTwoInts::Response {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(response) = response.as_mut().now_or_never() {
            return response.unwrap();
        }
        assert!(Instant::now() < deadline, "service response timed out");
    }
}

#[test]
fn typed_service_reuses_take_and_response_storage_and_recovers_from_copy_failure() {
    let _lock = TRACKED_SERVICE_LOCK.lock();
    for counter in [
        &REQUEST_CREATED,
        &REQUEST_DESTROYED,
        &RESPONSE_CREATED,
        &RESPONSE_DESTROYED,
    ] {
        counter.store(0, Ordering::Relaxed);
    }
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "typed_service_native_reuse", "").unwrap();
    let service_name = format!("/typed_service_native_reuse_{}", uuid::Uuid::new_v4().simple());
    let mut service = node
        .create_service::<TrackedService>(&service_name, QosProfile::services_default())
        .unwrap();
    let client = node
        .create_client::<AddTwoInts::Service>(&service_name, QosProfile::services_default())
        .unwrap();
    wait_for_service(&mut node, &client);

    let mut first_response = Box::pin(
        client
            .request(&AddTwoInts::Request { a: 19, b: 23 })
            .unwrap(),
    );
    let first = take_request(&mut node, &mut service);
    assert_eq!(first.message.a + first.message.b, 42);
    first
        .respond(TrackedResponse {
            sum: 42,
            fail_copy: false,
        })
        .unwrap();
    assert_eq!(finish_response(&mut node, &mut first_response).sum, 42);
    assert_eq!(REQUEST_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(RESPONSE_CREATED.load(Ordering::Relaxed), 1);

    let mut second_response = Box::pin(
        client
            .request(&AddTwoInts::Request { a: 40, b: 2 })
            .unwrap(),
    );
    let second = take_request(&mut node, &mut service);
    let retry = second.clone();
    assert!(matches!(
        second.respond(TrackedResponse {
            sum: 42,
            fail_copy: true
        }),
        Err(Error::RCL_RET_ERROR)
    ));
    assert_eq!(RESPONSE_DESTROYED.load(Ordering::Relaxed), 1);
    retry
        .respond(TrackedResponse {
            sum: 42,
            fail_copy: false,
        })
        .unwrap();
    assert_eq!(finish_response(&mut node, &mut second_response).sum, 42);
    assert_eq!(REQUEST_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(RESPONSE_CREATED.load(Ordering::Relaxed), 2);

    let mut third_response = Box::pin(
        client
            .request(&AddTwoInts::Request { a: -8, b: 50 })
            .unwrap(),
    );
    let third = take_request(&mut node, &mut service);
    third
        .respond(TrackedResponse {
            sum: 42,
            fail_copy: false,
        })
        .unwrap();
    assert_eq!(finish_response(&mut node, &mut third_response).sum, 42);
    assert_eq!(REQUEST_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(RESPONSE_CREATED.load(Ordering::Relaxed), 2);

    node.destroy_service(service);
    assert_eq!(REQUEST_DESTROYED.load(Ordering::Relaxed), 1);
    assert_eq!(RESPONSE_DESTROYED.load(Ordering::Relaxed), 2);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual typed service request/response native reuse allocation stage; LD_PRELOAD probe required"]
fn benchmark_typed_service_fresh_vs_reused_native_storage() {
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
    let request = SetBool::Request { data: true };
    let response = SetBool::Response {
        success: true,
        message: crate::RosString::from("x".repeat(STRING_BYTES)),
    };

    let mut fresh_step = || {
        let native_request = WrappedNativeMsg::<SetBool::Request>::try_from(&request).unwrap();
        assert!(SetBool::Request::from_native(&native_request).data);
        let native_response = WrappedNativeMsg::<SetBool::Response>::try_from(&response).unwrap();
        assert_eq!(native_response.message.size, STRING_BYTES);
        std::hint::black_box(native_response.message.data);
    };
    for _ in 0..1000 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let request_buffer = NativeBuffer::default();
    let response_buffer = NativeBuffer::default();
    drop(
        request_buffer
            .checkout(|| WrappedNativeMsg::<SetBool::Request>::try_from(&request))
            .unwrap(),
    );
    drop(
        response_buffer
            .checkout(|| WrappedNativeMsg::<SetBool::Response>::try_from(&response))
            .unwrap(),
    );
    let mut reused_step = || {
        let mut native_request = request_buffer
            .checkout(|| panic!("typed service request slot was not recycled"))
            .unwrap();
        request.try_copy_to_native(&mut native_request).unwrap();
        assert!(SetBool::Request::from_native(&native_request).data);
        drop(native_request);
        let mut native_response = response_buffer
            .checkout(|| panic!("typed service response slot was not recycled"))
            .unwrap();
        response.try_copy_to_native(&mut native_response).unwrap();
        assert_eq!(native_response.message.size, STRING_BYTES);
        std::hint::black_box(native_response.message.data);
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
                "boundary":"typed service request decode and response preparation; no RCL/DDS/channel",
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
