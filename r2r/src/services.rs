use futures::{channel::mpsc, task::noop_waker_ref, Stream};
use parking_lot::Mutex;
use std::{
    ffi::{CStr, CString},
    mem::MaybeUninit,
    pin::Pin,
    sync::{Arc, Weak},
    task::{Context, Poll},
};

use crate::{error::*, msg_types::*, Node, QosProfile};
use r2r_rcl::*;

pub(crate) enum ServiceRequestAction {
    Idle,
    Remove,
    Deliver(Box<dyn FnOnce() -> bool + Send>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServiceHandleResult {
    Taken,
    Idle,
    Remove,
}

const MAX_SERVICE_REQUESTS_PER_SPIN: usize = 16;

fn request_delivery<T: Send + 'static>(
    sender_slot: Arc<Mutex<Option<mpsc::Sender<T>>>>, mut sender: mpsc::Sender<T>, request: T,
    description: &'static str,
) -> ServiceRequestAction {
    ServiceRequestAction::Deliver(Box::new(move || {
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
    id: uuid::Uuid,
}

/// A runtime-typed ROS service request stream.
pub struct ServiceUntyped {
    receiver: mpsc::Receiver<ServiceRequestUntyped>,
    id: uuid::Uuid,
}

impl ServiceUntyped {
    pub(crate) fn new(receiver: mpsc::Receiver<ServiceRequestUntyped>, id: uuid::Uuid) -> Self {
        Self { receiver, id }
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

impl<T> Service<T>
where
    T: WrappedServiceTypeSupport,
{
    pub(crate) fn new(receiver: mpsc::Receiver<ServiceRequest<T>>, id: uuid::Uuid) -> Self {
        Self { receiver, id }
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
    service: Weak<Mutex<dyn Service_>>,
}

/// A request received by a runtime-typed service.
pub struct ServiceRequestUntyped {
    /// JSON conversion result for the request payload.
    pub message: Result<serde_json::Value>,
    request_id: rmw_request_id_t,
    service: Weak<Mutex<dyn Service_>>,
    make_response_msg: fn() -> Result<WrappedNativeMsgUntyped>,
}

unsafe impl Send for ServiceRequestUntyped {}

impl ServiceRequestUntyped {
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
    /// Complete the service request, consuming the request in the process.
    pub fn respond(self, msg: T::Response) -> Result<()> {
        let service = self
            .service
            .upgrade()
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        with_prepared_service(
            &service,
            || WrappedNativeMsg::<T::Response>::try_from(&msg),
            |service, native_msg| service.send_response(self.request_id, Box::new(native_msg)),
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
    /// Returns true when the service should be destroyed before entering the next wait.
    fn should_remove(&self) -> bool {
        false
    }
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

    fn should_remove(&self) -> bool {
        self.sender
            .lock()
            .as_ref()
            .is_some_and(mpsc::Sender::is_closed)
    }

    fn handle_request(
        &mut self, service: Arc<Mutex<dyn Service_>>,
    ) -> Result<ServiceRequestAction> {
        {
            let mut sender = self.sender.lock();
            let Some(sender) = sender.as_mut() else {
                return Ok(ServiceRequestAction::Idle);
            };
            if sender.is_closed() {
                return Ok(ServiceRequestAction::Remove);
            }
            let mut cx = Context::from_waker(noop_waker_ref());
            if sender.poll_ready(&mut cx) == Poll::Pending {
                return Ok(ServiceRequestAction::Idle);
            }
        }

        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut request_msg = (self.service_type.make_request_msg)()?;
        let ret = unsafe {
            rcl_take_request(&self.rcl_handle, request_id.as_mut_ptr(), request_msg.void_ptr_mut())
        };
        if ret == RCL_RET_SERVICE_TAKE_FAILED as i32 {
            return Ok(ServiceRequestAction::Idle);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let request = ServiceRequestUntyped {
            message: request_msg.to_json(),
            request_id: unsafe { request_id.assume_init() },
            service: Arc::downgrade(&service),
            make_response_msg: self.service_type.make_response_msg,
        };
        let sender = self
            .sender
            .lock()
            .take()
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        Ok(request_delivery(
            Arc::clone(&self.sender),
            sender,
            request,
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

    fn should_remove(&self) -> bool {
        self.sender
            .lock()
            .as_ref()
            .is_some_and(mpsc::Sender::is_closed)
    }

    fn handle_request(
        &mut self, service: Arc<Mutex<dyn Service_>>,
    ) -> Result<ServiceRequestAction> {
        {
            let mut sender = self.sender.lock();
            let Some(sender) = sender.as_mut() else {
                return Ok(ServiceRequestAction::Idle);
            };
            if sender.is_closed() {
                return Ok(ServiceRequestAction::Remove);
            }
            let mut cx = Context::from_waker(noop_waker_ref());
            if sender.poll_ready(&mut cx) == Poll::Pending {
                return Ok(ServiceRequestAction::Idle);
            }
        }

        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut request_msg = WrappedNativeMsg::<T::Request>::try_new()?;

        let ret = unsafe {
            rcl_take_request(&self.rcl_handle, request_id.as_mut_ptr(), request_msg.void_ptr_mut())
        };
        if ret == RCL_RET_SERVICE_TAKE_FAILED as i32 {
            return Ok(ServiceRequestAction::Idle);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let request_id = unsafe { request_id.assume_init() };
        let request_msg = T::Request::from_native(&request_msg);
        let request = ServiceRequest::<T> {
            message: request_msg,
            request_id,
            service: Arc::downgrade(&service),
        };
        let sender = self
            .sender
            .lock()
            .take()
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        Ok(request_delivery(Arc::clone(&self.sender), sender, request, "service request"))
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

    #[test]
    fn service_batch_limit_is_bounded_for_fairness() {
        assert_eq!(MAX_SERVICE_REQUESTS_PER_SPIN, 16);
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
            Ok(request_delivery(Arc::clone(&sender), detached, 42, "test service request"))
        })
        .unwrap();

        assert!(wake.woke.load(Ordering::SeqCst));
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Ready(Some(42))));
    }
}
