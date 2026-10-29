use super::*;
use crate::rcl_interfaces::{msg::ParameterValue, srv::GetParameters};

static SERIAL: Mutex<()> = Mutex::new(());
static REQUEST_CREATED: AtomicUsize = AtomicUsize::new(0);
static REQUEST_DESTROYED: AtomicUsize = AtomicUsize::new(0);
static REQUEST_DECODED: AtomicUsize = AtomicUsize::new(0);
static RESPONSE_CREATED: AtomicUsize = AtomicUsize::new(0);
static RESPONSE_DESTROYED: AtomicUsize = AtomicUsize::new(0);
static FAIL_RESPONSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Request(GetParameters::Request);
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Response(GetParameters::Response);
#[derive(Debug, Clone)]
struct Service;

// SAFETY: wrappers preserve the exact generated C shape, native allocator and type support.
// The callback's request conversion is instrumented to reject any owned Rust construction.
unsafe impl WrappedTypesupport for Request {
    type CStruct = <GetParameters::Request as WrappedTypesupport>::CStruct;
    fn get_ts() -> &'static rosidl_message_type_support_t {
        GetParameters::Request::get_ts()
    }
    fn create_msg() -> *mut Self::CStruct {
        let message = GetParameters::Request::create_msg();
        if !message.is_null() {
            REQUEST_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }
    fn destroy_msg(message: *mut Self::CStruct) {
        REQUEST_DESTROYED.fetch_add(1, Ordering::Relaxed);
        GetParameters::Request::destroy_msg(message);
    }
    fn from_native(_: &Self::CStruct) -> Self {
        REQUEST_DECODED.fetch_add(1, Ordering::Relaxed);
        panic!("native service callback decoded owned request");
    }
    fn copy_to_native(&self, native: &mut Self::CStruct) {
        self.0.copy_to_native(native);
    }
    fn try_copy_to_native(&self, native: &mut Self::CStruct) -> Result<()> {
        self.0.try_copy_to_native(native)
    }
}

unsafe impl WrappedTypesupport for Response {
    type CStruct = <GetParameters::Response as WrappedTypesupport>::CStruct;
    fn get_ts() -> &'static rosidl_message_type_support_t {
        GetParameters::Response::get_ts()
    }
    fn create_msg() -> *mut Self::CStruct {
        if FAIL_RESPONSE.swap(false, Ordering::Relaxed) {
            return std::ptr::null_mut();
        }
        let message = GetParameters::Response::create_msg();
        if !message.is_null() {
            RESPONSE_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }
    fn destroy_msg(message: *mut Self::CStruct) {
        RESPONSE_DESTROYED.fetch_add(1, Ordering::Relaxed);
        GetParameters::Response::destroy_msg(message);
    }
    fn from_native(native: &Self::CStruct) -> Self {
        Self(GetParameters::Response::from_native(native))
    }
    fn copy_to_native(&self, native: &mut Self::CStruct) {
        self.0.copy_to_native(native);
    }
    fn try_copy_to_native(&self, native: &mut Self::CStruct) -> Result<()> {
        self.0.try_copy_to_native(native)
    }
}

unsafe impl WrappedServiceTypeSupport for Service {
    type Request = Request;
    type Response = Response;
    #[cfg(not(r2r__ros__distro__humble))]
    type Event = GetParameters::Event;
    fn get_ts() -> &'static rosidl_service_type_support_t {
        GetParameters::Service::get_ts()
    }
}

fn values(count: usize) -> GetParameters::Response {
    GetParameters::Response {
        values: (0..count)
            .map(|index| ParameterValue {
                type_: 4,
                string_value: format!("value_{index}_日本語_🦀").into(),
                ..Default::default()
            })
            .collect(),
    }
}

#[test]
fn callback_service_native_request_decode_is_zero_and_dynamic_response_retains_capacity() {
    let _serial = SERIAL.lock();
    let request_created = REQUEST_CREATED.load(Ordering::Relaxed);
    let request_destroyed = REQUEST_DESTROYED.load(Ordering::Relaxed);
    let reply_created = RESPONSE_CREATED.load(Ordering::Relaxed);
    let reply_destroyed = RESPONSE_DESTROYED.load(Ordering::Relaxed);
    let mut node = node("callback_native_storage");
    let mut outer = None;
    let mut string_buffer = None;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let info = seen.clone();
    let service = node
        .create_service_native_callback::<Service, _>(
            &name("callback_native_storage"),
            QosProfile::default(),
            move |request, response, identity| {
                assert!(request.names.size == 1);
                let count: usize = unsafe { (*request.names.data).to_str() }.parse().unwrap();
                Response(values(count)).try_copy_to_native(response)?;
                let address = response.msg as usize;
                assert_eq!(*outer.get_or_insert(address), address);
                if count == 128 {
                    string_buffer.get_or_insert(
                        unsafe { (*response.values.data).string_value.data } as usize,
                    );
                }
                if let Some(address) = string_buffer {
                    assert_eq!(
                        unsafe { (*response.values.data).string_value.data } as usize,
                        address
                    );
                    assert_eq!(response.values.capacity, 128);
                }
                info.lock().push(identity);
                Ok(())
            },
        )
        .unwrap();
    let client = node
        .create_client::<GetParameters::Service>(
            &service.service_name(&node).unwrap(),
            QosProfile::default(),
        )
        .unwrap();
    let ready = client.wait_for_service().unwrap();
    node.spin_until_future_complete_with_options(
        ready,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    for count in [128, 16, 0, 32, 128] {
        let request = client
            .request_with_info(&GetParameters::Request {
                names: vec![count.to_string().into()],
            })
            .unwrap();
        let response = node
            .spin_until_future_complete_with_options(
                request,
                Duration::from_secs(3),
                FutureExecutionOptions::wake_driven(),
            )
            .unwrap();
        assert_eq!(response.response, values(count));
        assert_eq!(seen.lock().last().unwrap().sequence_number, response.info.sequence_number);
        assert_eq!(
            REQUEST_CREATED.load(Ordering::Relaxed) - request_created,
            REQUEST_DESTROYED.load(Ordering::Relaxed) - request_destroyed
        );
        assert_eq!(REQUEST_DECODED.load(Ordering::Relaxed), 0);
    }
    assert_eq!(RESPONSE_CREATED.load(Ordering::Relaxed) - reply_created, 1);
    assert_eq!(RESPONSE_DESTROYED.load(Ordering::Relaxed) - reply_destroyed, 0);
    node.destroy_callback_service(service);
    assert_eq!(RESPONSE_DESTROYED.load(Ordering::Relaxed) - reply_destroyed, 1);
}

#[test]
fn callback_service_native_response_allocation_failure_leaves_no_endpoint() {
    let _serial = SERIAL.lock();
    let mut node = node("callback_allocation_fail");
    FAIL_RESPONSE.store(true, Ordering::Relaxed);
    let service = node.create_service_native_callback::<Service, _>(
        &name("callback_allocation_fail"),
        QosProfile::default(),
        |_, _, _| Ok(()),
    );
    assert!(matches!(service, Err(Error::RCL_RET_BAD_ALLOC)));
    assert!(node.service_callbacks.is_none());
    assert!(node.services.is_empty());
}

#[cfg(not(r2r__ros__distro__humble))]
#[test]
fn callback_service_native_introspection_preserves_request_and_response_event_payloads() {
    let mut node = node("callback_events");
    let service_name = name("callback_events");
    let service = node
        .create_service_native_callback::<AddTwoInts::Service, _>(
            &service_name,
            QosProfile::default(),
            |request, response, _| {
                response.sum = request.a + request.b;
                Ok(())
            },
        )
        .unwrap();
    let mut events = node
        .subscribe::<AddTwoInts::Event>(
            &format!("{service_name}/_service_event"),
            QosProfile::default(),
        )
        .unwrap();
    service
        .configure_introspection(
            &mut node,
            crate::ServiceIntrospectionState::Contents,
            QosProfile::default(),
        )
        .unwrap();
    let client = node
        .create_client::<AddTwoInts::Service>(&service_name, QosProfile::default())
        .unwrap();
    ready(&mut node, &client);
    let match_ready = events.wait_for_publishers(&mut node).unwrap();
    node.spin_until_future_complete_with_options(
        match_ready,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let response = client
        .request_with_info(&AddTwoInts::Request { a: 12, b: 30 })
        .unwrap();
    let response = node
        .spin_until_future_complete_with_options(
            response,
            Duration::from_secs(3),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    assert_eq!(response.response.sum, 42);
    let mut received = Vec::new();
    node.spin_until_future_complete_with_options(
        async {
            while received.len() < 2 {
                received.push(
                    events
                        .next()
                        .await
                        .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?,
                );
            }
            Ok(())
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    use crate::service_msgs::msg::ServiceEventInfo;
    let request = received
        .iter()
        .find(|event| event.info.event_type == ServiceEventInfo::REQUEST_RECEIVED as u8)
        .unwrap();
    let reply = received
        .iter()
        .find(|event| event.info.event_type == ServiceEventInfo::RESPONSE_SENT as u8)
        .unwrap();
    assert_eq!(request.info.sequence_number, response.info.sequence_number);
    assert_eq!(reply.info.sequence_number, response.info.sequence_number);
    assert_eq!(request.request, [AddTwoInts::Request { a: 12, b: 30 }]);
    assert!(request.response.is_empty());
    assert!(reply.request.is_empty());
    assert_eq!(reply.response, [AddTwoInts::Response { sum: 42 }]);
}
