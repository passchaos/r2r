use futures::{channel::mpsc, task::noop_waker_ref, Stream};
use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    ffi::{CStr, CString},
    mem::MaybeUninit,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
    task::{Context, Poll},
};

use crate::{error::*, msg_types::*, Node, QosProfile};
use r2r_rcl::*;

pub(crate) enum ServiceRequestAction {
    Idle,
    Remove,
    Deliver(Box<dyn FnOnce() -> bool>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServiceHandleResult {
    Taken,
    Idle,
    Remove,
}

const MAX_SERVICE_REQUESTS_PER_SPIN: usize = 16;

fn service_sender_ready<T>(sender: &mut mpsc::Sender<T>) -> ServiceHandleResult {
    let mut context = Context::from_waker(noop_waker_ref());
    match sender.poll_ready(&mut context) {
        Poll::Ready(Ok(())) => ServiceHandleResult::Taken,
        Poll::Pending => ServiceHandleResult::Idle,
        Poll::Ready(Err(_)) => ServiceHandleResult::Remove,
    }
}

fn deliver_request<T>(
    sender_slot: Arc<Mutex<Option<mpsc::Sender<T>>>>, mut sender: mpsc::Sender<T>, request: T,
    description: &'static str,
) -> bool {
    let remove = if let Err(err) = sender.try_send(request) {
        if err.is_disconnected() {
            true
        } else {
            log::debug!("{description} channel was unexpectedly full ({err})");
            false
        }
    } else {
        false
    };
    *sender_slot.lock() = Some(sender);
    remove
}

fn request_conversion_delivery<N, T>(
    sender_slot: Arc<Mutex<Option<mpsc::Sender<T>>>>, sender: mpsc::Sender<T>, native: N,
    convert: impl FnOnce(N) -> T + 'static, description: &'static str,
) -> ServiceRequestAction
where
    N: 'static,
    T: 'static,
{
    ServiceRequestAction::Deliver(Box::new(move || {
        let request = convert(native);
        deliver_request(sender_slot, sender, request, description)
    }))
}

pub(crate) fn complete_service_request<S: ?Sized>(
    service: &Mutex<S>, operation: impl FnOnce(&mut S) -> Result<ServiceRequestAction>,
) -> Result<ServiceHandleResult> {
    let action = {
        let mut service = service.lock();
        operation(&mut *service)?
    };
    match action {
        ServiceRequestAction::Idle => Ok(ServiceHandleResult::Idle),
        ServiceRequestAction::Remove => Ok(ServiceHandleResult::Remove),
        ServiceRequestAction::Deliver(deliver) => Ok(if deliver() {
            ServiceHandleResult::Remove
        } else {
            ServiceHandleResult::Taken
        }),
    }
}

pub(crate) fn complete_ready_service(
    service: &Arc<Mutex<dyn Service_>>,
) -> Result<ServiceHandleResult> {
    for _ in 0..MAX_SERVICE_REQUESTS_PER_SPIN {
        let service_ref = Arc::clone(service);
        match complete_service_request(service, |service| service.handle_request(service_ref))? {
            ServiceHandleResult::Taken => {}
            result @ (ServiceHandleResult::Idle | ServiceHandleResult::Remove) => {
                return Ok(result);
            }
        }
    }
    Ok(ServiceHandleResult::Taken)
}

fn with_prepared_service<S: ?Sized, P, T>(
    service: &Mutex<S>, prepare: impl FnOnce() -> Result<P>,
    operation: impl FnOnce(&mut S, P) -> Result<T>,
) -> Result<T> {
    let prepared = prepare()?;
    operation(&mut *service.lock(), prepared)
}

/// Rust-side buffering options for a ROS service request stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceOptions {
    pub request_channel_capacity: usize,
}

impl Default for ServiceOptions {
    fn default() -> Self {
        Self {
            request_channel_capacity: 10,
        }
    }
}

impl ServiceOptions {
    pub const fn request_channel_capacity(mut self, request_channel_capacity: usize) -> Self {
        self.request_channel_capacity = request_channel_capacity;
        self
    }

    pub(crate) fn validate(self) -> Result<Self> {
        if self.request_channel_capacity >= usize::MAX >> 2 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        Ok(self)
    }
}

/// A ROS service request stream with endpoint QoS inspection helpers.
pub struct Service<T>
where
    T: WrappedServiceTypeSupport,
{
    receiver: mpsc::Receiver<ServiceRequest<T>>,
    pub(crate) id: uuid::Uuid,
    canceled_services: Arc<Mutex<VecDeque<uuid::Uuid>>>,
}

/// A runtime-typed ROS service request stream.
pub struct ServiceUntyped {
    receiver: mpsc::Receiver<ServiceRequestUntyped>,
    pub(crate) id: uuid::Uuid,
    canceled_services: Arc<Mutex<VecDeque<uuid::Uuid>>>,
}

impl ServiceUntyped {
    pub(crate) fn new(
        receiver: mpsc::Receiver<ServiceRequestUntyped>, id: uuid::Uuid,
        canceled_services: Arc<Mutex<VecDeque<uuid::Uuid>>>,
    ) -> Self {
        Self {
            receiver,
            id,
            canceled_services,
        }
    }

    /// Returns the actual QoS used by this service's request subscription.
    pub fn get_request_subscription_actual_qos(&self, node: &Node) -> Result<QosProfile> {
        node.get_service_request_subscription_actual_qos(self.id)
    }

    /// Returns the actual QoS used by this service's response publisher.
    pub fn get_response_publisher_actual_qos(&self, node: &Node) -> Result<QosProfile> {
        node.get_service_response_publisher_actual_qos(self.id)
    }

    /// Returns the fully resolved service name used by the native server.
    pub fn service_name(&self, node: &Node) -> Result<String> {
        node.get_service_name(self.id)
    }
}

impl Stream for ServiceUntyped {
    type Item = ServiceRequestUntyped;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}

impl Drop for ServiceUntyped {
    fn drop(&mut self) {
        self.receiver.close();
        self.canceled_services.lock().push_back(self.id);
    }
}

impl<T> Service<T>
where
    T: WrappedServiceTypeSupport,
{
    pub(crate) fn new(
        receiver: mpsc::Receiver<ServiceRequest<T>>, id: uuid::Uuid,
        canceled_services: Arc<Mutex<VecDeque<uuid::Uuid>>>,
    ) -> Self {
        Self {
            receiver,
            id,
            canceled_services,
        }
    }

    /// Returns the actual QoS used by this service's request subscription.
    pub fn get_request_subscription_actual_qos(&self, node: &Node) -> Result<QosProfile> {
        node.get_service_request_subscription_actual_qos(self.id)
    }

    /// Returns the actual QoS used by this service's response publisher.
    pub fn get_response_publisher_actual_qos(&self, node: &Node) -> Result<QosProfile> {
        node.get_service_response_publisher_actual_qos(self.id)
    }

    /// Returns the fully resolved service name used by the native server.
    pub fn service_name(&self, node: &Node) -> Result<String> {
        node.get_service_name(self.id)
    }
}

impl<T> Stream for Service<T>
where
    T: WrappedServiceTypeSupport,
{
    type Item = ServiceRequest<T>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}

impl<T> Drop for Service<T>
where
    T: WrappedServiceTypeSupport,
{
    fn drop(&mut self) {
        self.receiver.close();
        self.canceled_services.lock().push_back(self.id);
    }
}

/// Encapsulates a service request.
///
/// In contrast to having a callback from Request -> Response
/// types that is called synchronously, the service request can be
/// moved around and completed asynchronously.
///
/// To complete the request, call the `respond` function.
#[derive(Clone)]
pub struct ServiceRequest<T>
where
    T: WrappedServiceTypeSupport,
{
    pub message: T::Request,
    request_id: rmw_request_id_t,
    request_info: ServiceRequestInfo,
    service: Weak<Mutex<dyn Service_>>,
    response_sent: Arc<AtomicBool>,
}

/// Middleware identity attached to a received service request.
///
/// The timestamps, sequence number, and writer GUID can be used to trace requests and detect
/// duplicates without exposing the native RMW request header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceRequestInfo {
    /// Source timestamp in nanoseconds, or zero when unsupported by the RMW implementation.
    pub source_timestamp: i64,
    /// Receive timestamp in nanoseconds, or zero when unsupported by the RMW implementation.
    pub received_timestamp: i64,
    pub writer_guid: [u8; 16],
    pub sequence_number: i64,
}

impl From<&rmw_service_info_t> for ServiceRequestInfo {
    fn from(info: &rmw_service_info_t) -> Self {
        Self {
            source_timestamp: info.source_timestamp,
            received_timestamp: info.received_timestamp,
            writer_guid: info
                .request_id
                .writer_guid
                .map(|byte| byte.to_ne_bytes()[0]),
            sequence_number: info.request_id.sequence_number,
        }
    }
}

/// A request received by a runtime-typed service.
pub struct ServiceRequestUntyped {
    /// JSON conversion result for the request payload.
    pub message: Result<serde_json::Value>,
    request_id: rmw_request_id_t,
    request_info: ServiceRequestInfo,
    service: Weak<Mutex<dyn Service_>>,
    make_response_msg: fn() -> Result<WrappedNativeMsgUntyped>,
}

unsafe impl Send for ServiceRequestUntyped {}

impl ServiceRequestUntyped {
    /// Returns a stable snapshot of the middleware request identity.
    pub fn request_info(&self) -> ServiceRequestInfo {
        self.request_info
    }

    /// Completes the request with a runtime-typed JSON response.
    pub fn respond(self, msg: serde_json::Value) -> Result<()> {
        let mut native_msg = (self.make_response_msg)()?;
        native_msg.from_json(msg)?;
        let service = self
            .service
            .upgrade()
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        let mut service = service.lock();
        service.send_response(self.request_id, Box::new(native_msg))
    }
}

unsafe impl<T> Send for ServiceRequest<T> where T: WrappedServiceTypeSupport {}

impl<T> ServiceRequest<T>
where
    T: 'static + WrappedServiceTypeSupport,
{
    /// Returns a stable snapshot of the middleware request identity.
    pub fn request_info(&self) -> ServiceRequestInfo {
        self.request_info
    }

    /// Complete the service request, consuming the request in the process.
    ///
    /// Cloned requests share response state. After one clone sends a response successfully, later
    /// calls return [`Error::ServiceRequestAlreadyResponded`]. A failed send does not consume the
    /// response opportunity, allowing another clone to retry.
    pub fn respond(self, msg: T::Response) -> Result<()> {
        let service = self
            .service
            .upgrade()
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        with_prepared_service(
            &service,
            || WrappedNativeMsg::<T::Response>::try_from(&msg),
            |service, native_msg| {
                self.response_sent
                    .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                    .map_err(|_| Error::ServiceRequestAlreadyResponded)?;
                let result = service.send_response(self.request_id, Box::new(native_msg));
                if result.is_err() {
                    self.response_sent.store(false, Ordering::Release);
                }
                result
            },
        )
    }
}

pub trait Service_ {
    fn handle(&self) -> &rcl_service_t;
    fn id(&self) -> Option<uuid::Uuid> {
        None
    }
    fn request_subscription_actual_qos(&self) -> Result<QosProfile> {
        let qos = unsafe { rcl_service_request_subscription_get_actual_qos(self.handle()) };
        let qos = unsafe { qos.as_ref() }.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        QosProfile::try_from(qos)
    }
    fn response_publisher_actual_qos(&self) -> Result<QosProfile> {
        let qos = unsafe { rcl_service_response_publisher_get_actual_qos(self.handle()) };
        let qos = unsafe { qos.as_ref() }.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        QosProfile::try_from(qos)
    }
    fn service_name(&self) -> Result<String> {
        let name = unsafe { rcl_service_get_service_name(self.handle()) };
        if name.is_null() {
            return Err(Error::RCL_RET_SERVICE_INVALID);
        }
        unsafe { CStr::from_ptr(name) }
            .to_str()
            .map(str::to_owned)
            .map_err(|err| Error::InvalidUtf8 {
                context: "service name",
                err: err.to_string(),
            })
    }
    fn send_response(&mut self, request_id: rmw_request_id_t, msg: Box<dyn VoidPtr>) -> Result<()>;
    /// Returns true if the service stream has been dropped.
    fn handle_request(&mut self, service: Arc<Mutex<dyn Service_>>)
        -> Result<ServiceRequestAction>;
    fn destroy(&mut self, node: &mut rcl_node_t);
}

pub struct TypedService<T>
where
    T: WrappedServiceTypeSupport,
{
    pub rcl_handle: rcl_service_t,
    pub sender: Arc<Mutex<Option<mpsc::Sender<ServiceRequest<T>>>>>,
    pub(crate) id: uuid::Uuid,
}

pub struct UntypedService {
    pub service_type: UntypedServiceSupport,
    pub rcl_handle: rcl_service_t,
    pub sender: Arc<Mutex<Option<mpsc::Sender<ServiceRequestUntyped>>>>,
    pub(crate) id: uuid::Uuid,
}

// SAFETY: the node serializes every native service operation through its enclosing mutex, and
// UntypedServiceSupport contains immutable process-lifetime type support plus Send operations.
unsafe impl Send for UntypedService {}

impl Service_ for UntypedService {
    fn handle(&self) -> &rcl_service_t {
        &self.rcl_handle
    }

    fn id(&self) -> Option<uuid::Uuid> {
        Some(self.id)
    }

    fn send_response(
        &mut self, mut request_id: rmw_request_id_t, mut msg: Box<dyn VoidPtr>,
    ) -> Result<()> {
        let ret =
            unsafe { rcl_send_response(&self.rcl_handle, &mut request_id, msg.void_ptr_mut()) };
        if ret == RCL_RET_OK as i32 {
            Ok(())
        } else {
            Err(Error::from_rcl_error(ret))
        }
    }

    fn handle_request(
        &mut self, service: Arc<Mutex<dyn Service_>>,
    ) -> Result<ServiceRequestAction> {
        {
            let mut sender = self.sender.lock();
            let Some(sender) = sender.as_mut() else {
                return Ok(ServiceRequestAction::Idle);
            };
            match service_sender_ready(sender) {
                ServiceHandleResult::Taken => {}
                ServiceHandleResult::Idle => return Ok(ServiceRequestAction::Idle),
                ServiceHandleResult::Remove => return Ok(ServiceRequestAction::Remove),
            }
        }

        let mut request_info = MaybeUninit::<rmw_service_info_t>::uninit();
        let mut request_msg = (self.service_type.make_request_msg)()?;
        let ret = unsafe {
            rcl_take_request_with_info(
                &self.rcl_handle,
                request_info.as_mut_ptr(),
                request_msg.void_ptr_mut(),
            )
        };
        if ret == RCL_RET_SERVICE_TAKE_FAILED as i32 {
            return Ok(ServiceRequestAction::Idle);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let request_info = unsafe { request_info.assume_init() };
        let request_id = request_info.request_id;
        let request_info = ServiceRequestInfo::from(&request_info);
        let service = Arc::downgrade(&service);
        let make_response_msg = self.service_type.make_response_msg;
        let sender = self
            .sender
            .lock()
            .take()
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        Ok(request_conversion_delivery(
            Arc::clone(&self.sender),
            sender,
            request_msg,
            move |request_msg| ServiceRequestUntyped {
                message: request_msg.to_json(),
                request_id,
                request_info,
                service,
                make_response_msg,
            },
            "untyped service request",
        ))
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_service_fini(&mut self.rcl_handle, node);
        }
    }
}

impl<T: 'static> Service_ for TypedService<T>
where
    T: WrappedServiceTypeSupport,
{
    fn handle(&self) -> &rcl_service_t {
        &self.rcl_handle
    }

    fn id(&self) -> Option<uuid::Uuid> {
        Some(self.id)
    }

    fn send_response(
        &mut self, mut request_id: rmw_request_id_t, mut msg: Box<dyn VoidPtr>,
    ) -> Result<()> {
        let res =
            unsafe { rcl_send_response(&self.rcl_handle, &mut request_id, msg.void_ptr_mut()) };
        if res == RCL_RET_OK as i32 {
            Ok(())
        } else {
            Err(Error::from_rcl_error(res))
        }
    }

    fn handle_request(
        &mut self, service: Arc<Mutex<dyn Service_>>,
    ) -> Result<ServiceRequestAction> {
        {
            let mut sender = self.sender.lock();
            let Some(sender) = sender.as_mut() else {
                return Ok(ServiceRequestAction::Idle);
            };
            match service_sender_ready(sender) {
                ServiceHandleResult::Taken => {}
                ServiceHandleResult::Idle => return Ok(ServiceRequestAction::Idle),
                ServiceHandleResult::Remove => return Ok(ServiceRequestAction::Remove),
            }
        }

        let mut request_info = MaybeUninit::<rmw_service_info_t>::uninit();
        let mut request_msg = WrappedNativeMsg::<T::Request>::try_new()?;

        let ret = unsafe {
            rcl_take_request_with_info(
                &self.rcl_handle,
                request_info.as_mut_ptr(),
                request_msg.void_ptr_mut(),
            )
        };
        if ret == RCL_RET_SERVICE_TAKE_FAILED as i32 {
            return Ok(ServiceRequestAction::Idle);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let request_info = unsafe { request_info.assume_init() };
        let request_id = request_info.request_id;
        let request_info = ServiceRequestInfo::from(&request_info);
        let service = Arc::downgrade(&service);
        let sender = self
            .sender
            .lock()
            .take()
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        Ok(request_conversion_delivery(
            Arc::clone(&self.sender),
            sender,
            request_msg,
            move |request_msg| ServiceRequest::<T> {
                message: T::Request::from_native(&request_msg),
                request_id,
                request_info,
                service,
                response_sent: Arc::new(AtomicBool::new(false)),
            },
            "service request",
        ))
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_service_fini(&mut self.rcl_handle, node);
        }
    }
}

pub fn create_service_helper(
    node: &mut rcl_node_t, service_name: &str, service_ts: *const rosidl_service_type_support_t,
    qos_profile: QosProfile,
) -> Result<rcl_service_t> {
    let mut service_handle = unsafe { rcl_get_zero_initialized_service() };
    let service_name_c_string =
        CString::new(service_name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;

    let result = unsafe {
        let mut service_options = rcl_service_get_default_options();
        service_options.qos = qos_profile.into();
        rcl_service_init(
            &mut service_handle,
            node,
            service_ts,
            service_name_c_string.as_ptr(),
            &service_options,
        )
    };
    if result == RCL_RET_OK as i32 {
        Ok(service_handle)
    } else {
        Err(Error::from_rcl_error(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::example_interfaces::srv::AddTwoInts;
    use futures::task::{waker_ref, ArcWake};
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    struct LockingWake {
        service: Arc<Mutex<()>>,
        woke: AtomicBool,
    }

    impl ArcWake for LockingWake {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            let _service = arc_self.service.lock();
            arc_self.woke.store(true, Ordering::SeqCst);
        }
    }

    struct ResponseTrackingService {
        handle: rcl_service_t,
        send_attempts: usize,
        fail_next_send: bool,
    }

    // SAFETY: the test double never dereferences or passes its zero-initialized native handle to
    // RCL; all mutable test state is accessed through the enclosing mutex.
    unsafe impl Send for ResponseTrackingService {}

    impl Service_ for ResponseTrackingService {
        fn handle(&self) -> &rcl_service_t {
            &self.handle
        }

        fn send_response(
            &mut self, _request_id: rmw_request_id_t, _msg: Box<dyn VoidPtr>,
        ) -> Result<()> {
            self.send_attempts += 1;
            if std::mem::take(&mut self.fail_next_send) {
                Err(Error::RCL_RET_ERROR)
            } else {
                Ok(())
            }
        }

        fn handle_request(
            &mut self, _service: Arc<Mutex<dyn Service_>>,
        ) -> Result<ServiceRequestAction> {
            unreachable!()
        }

        fn destroy(&mut self, _node: &mut rcl_node_t) {}
    }

    fn test_service_request(
        service: &Arc<Mutex<ResponseTrackingService>>,
    ) -> ServiceRequest<AddTwoInts::Service> {
        let service: Arc<Mutex<dyn Service_>> = service.clone();
        ServiceRequest {
            message: AddTwoInts::Request { a: 1, b: 2 },
            request_id: unsafe { std::mem::zeroed() },
            request_info: ServiceRequestInfo {
                source_timestamp: 0,
                received_timestamp: 0,
                writer_guid: [0; 16],
                sequence_number: 0,
            },
            service: Arc::downgrade(&service),
            response_sent: Arc::new(AtomicBool::new(false)),
        }
    }

    #[test]
    fn cloned_service_request_can_respond_only_once() {
        let service = Arc::new(Mutex::new(ResponseTrackingService {
            handle: unsafe { rcl_get_zero_initialized_service() },
            send_attempts: 0,
            fail_next_send: false,
        }));
        let request = test_service_request(&service);
        let clone = request.clone();

        request.respond(AddTwoInts::Response { sum: 3 }).unwrap();
        assert!(matches!(
            clone.respond(AddTwoInts::Response { sum: 4 }),
            Err(Error::ServiceRequestAlreadyResponded)
        ));
        assert_eq!(service.lock().send_attempts, 1);
    }

    #[test]
    fn failed_service_response_allows_a_clone_to_retry() {
        let service = Arc::new(Mutex::new(ResponseTrackingService {
            handle: unsafe { rcl_get_zero_initialized_service() },
            send_attempts: 0,
            fail_next_send: true,
        }));
        let request = test_service_request(&service);
        let clone = request.clone();

        assert!(matches!(
            request.respond(AddTwoInts::Response { sum: 3 }),
            Err(Error::RCL_RET_ERROR)
        ));
        clone.respond(AddTwoInts::Response { sum: 3 }).unwrap();
        assert_eq!(service.lock().send_attempts, 2);
    }

    #[test]
    fn dropping_typed_service_stream_queues_its_id() {
        let id = uuid::Uuid::from_u128(1);
        let canceled_services = Arc::new(Mutex::new(VecDeque::new()));
        let (sender, receiver) = mpsc::channel::<ServiceRequest<AddTwoInts::Service>>(1);
        let service = Service::new(receiver, id, Arc::clone(&canceled_services));

        drop(service);

        assert_eq!(canceled_services.lock().iter().copied().collect::<Vec<_>>(), [id]);
        assert!(sender.is_closed());
    }

    #[test]
    fn service_request_info_copies_native_identity() {
        let request_info = rmw_service_info_t {
            source_timestamp: 41,
            received_timestamp: 42,
            request_id: rmw_request_id_t {
                writer_guid: [128, 255, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 126, 127],
                sequence_number: -42,
            },
        };
        let info = ServiceRequestInfo::from(&request_info);

        assert_eq!(info.source_timestamp, 41);
        assert_eq!(info.received_timestamp, 42);
        assert_eq!(info.writer_guid, [128, 255, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 126, 127]);
        assert_eq!(info.sequence_number, -42);
    }

    #[test]
    fn dropping_untyped_service_stream_queues_its_id() {
        let id = uuid::Uuid::from_u128(2);
        let canceled_services = Arc::new(Mutex::new(VecDeque::new()));
        let (sender, receiver) = mpsc::channel::<ServiceRequestUntyped>(1);
        let service = ServiceUntyped::new(receiver, id, Arc::clone(&canceled_services));

        drop(service);

        assert_eq!(canceled_services.lock().iter().copied().collect::<Vec<_>>(), [id]);
        assert!(sender.is_closed());
    }

    #[test]
    fn service_batch_limit_is_bounded_for_fairness() {
        assert_eq!(MAX_SERVICE_REQUESTS_PER_SPIN, 16);
    }

    #[test]
    fn closed_service_sender_is_removed_before_taking_a_request() {
        let (mut sender, receiver) = mpsc::channel::<i32>(1);
        drop(receiver);

        assert_eq!(service_sender_ready(&mut sender), ServiceHandleResult::Remove);
    }

    #[test]
    fn full_service_sender_reports_backpressure() {
        let (mut sender, mut receiver) = mpsc::channel(0);
        let mut context = Context::from_waker(noop_waker_ref());
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Pending));
        sender.try_send(41).unwrap();

        assert_eq!(service_sender_ready(&mut sender), ServiceHandleResult::Idle);
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Ready(Some(41))));
    }

    #[test]
    fn service_payloads_are_prepared_before_locking_the_native_handle() {
        let service = Mutex::new(());

        let prepared = with_prepared_service(
            &service,
            || {
                assert!(service.try_lock().is_some());
                Ok(41)
            },
            |_service, value| Ok(value + 1),
        )
        .unwrap();

        assert_eq!(prepared, 42);
    }

    #[test]
    fn service_requests_wake_streams_after_releasing_the_service_lock() {
        let service = Arc::new(Mutex::new(()));
        let (sender, mut receiver) = mpsc::channel(1);
        let sender = Arc::new(Mutex::new(Some(sender)));
        let wake = Arc::new(LockingWake {
            service: Arc::clone(&service),
            woke: AtomicBool::new(false),
        });
        let waker = waker_ref(&wake);
        let mut context = Context::from_waker(&waker);
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Pending));
        assert!(matches!(
            sender.lock().as_mut().unwrap().poll_ready(&mut context),
            Poll::Ready(Ok(()))
        ));

        complete_service_request(&service, |_| {
            let detached = sender.lock().take().unwrap();
            let sender = Arc::clone(&sender);
            Ok(ServiceRequestAction::Deliver(Box::new(move || {
                deliver_request(sender, detached, 42, "test service request")
            })))
        })
        .unwrap();

        assert!(wake.woke.load(Ordering::SeqCst));
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Ready(Some(42))));
    }

    #[test]
    fn service_requests_are_converted_after_releasing_the_service_lock() {
        let service = Arc::new(Mutex::new(()));
        let (sender, mut receiver) = mpsc::channel(1);
        let sender = Arc::new(Mutex::new(Some(sender)));
        let service_for_conversion = Arc::clone(&service);

        complete_service_request(&*service, |_| {
            let detached = sender.lock().take().unwrap();
            Ok(request_conversion_delivery(
                Arc::clone(&sender),
                detached,
                41,
                move |value| {
                    assert!(service_for_conversion.try_lock().is_some());
                    value + 1
                },
                "test service request",
            ))
        })
        .unwrap();

        assert!(matches!(
            Pin::new(&mut receiver)
                .poll_next(&mut Context::from_waker(futures::task::noop_waker_ref())),
            Poll::Ready(Some(42))
        ));
    }
}
