use futures::channel::oneshot;
use parking_lot::Mutex;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
use std::{
    collections::{HashMap, VecDeque},
    ffi::{CStr, CString},
    fmt::Debug,
    future::Future,
    mem::MaybeUninit,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
    task::{Context, Poll},
    time::Duration,
};

use crate::{availability::AvailabilityWaiters, error::*, msg_types::*, nodes::Node, QosProfile};
use r2r_rcl::*;

pub(crate) type ClientCompletion = Box<dyn FnOnce()>;

pub(crate) enum ClientResponseAction {
    Idle,
    Taken(Option<ClientCompletion>),
}

const MAX_CLIENT_RESPONSES_PER_SPIN: usize = 16;

/// Globally unique middleware identity of a service client.
#[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientGid {
    /// Name of the RMW implementation that assigned this identity.
    pub implementation_identifier: String,
    /// Opaque identity bytes assigned by the RMW implementation.
    pub data: [u8; RMW_GID_STORAGE_SIZE as usize],
}

/// Middleware metadata attached to a received service response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServiceResponseInfo {
    /// Source timestamp in nanoseconds, or zero when unsupported by the RMW implementation.
    pub source_timestamp: i64,
    /// Receive timestamp in nanoseconds, or zero when unsupported by the RMW implementation.
    pub received_timestamp: i64,
    /// Writer GUID from the original request associated with this response.
    pub writer_guid: [u8; 16],
    /// Sequence number assigned to the original request.
    pub sequence_number: i64,
}

impl From<&rmw_service_info_t> for ServiceResponseInfo {
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

/// A service response together with its RMW metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceResponse<T> {
    pub response: T,
    pub info: ServiceResponseInfo,
}

impl<T> ServiceResponse<T> {
    pub fn into_parts(self) -> (T, ServiceResponseInfo) {
        (self.response, self.info)
    }
}

pub(crate) enum ClientResponseSender<T> {
    Plain(oneshot::Sender<T>),
    WithInfo(oneshot::Sender<ServiceResponse<T>>),
}

impl<T> ClientResponseSender<T> {
    fn is_canceled(&self) -> bool {
        match self {
            Self::Plain(sender) => sender.is_canceled(),
            Self::WithInfo(sender) => sender.is_canceled(),
        }
    }
}

impl<T> From<oneshot::Sender<T>> for ClientResponseSender<T> {
    fn from(sender: oneshot::Sender<T>) -> Self {
        Self::Plain(sender)
    }
}

struct ClientResponseFuture<T, C: ClientResponseRegistry> {
    receiver: oneshot::Receiver<T>,
    client: Option<Weak<Mutex<C>>>,
    sequence: i64,
    completed: bool,
}

impl<T, C: ClientResponseRegistry> ClientResponseFuture<T, C> {
    fn new(receiver: oneshot::Receiver<T>, client: Weak<Mutex<C>>, sequence: i64) -> Self {
        Self {
            receiver,
            client: Some(client),
            sequence,
            completed: false,
        }
    }
}

impl<T, C: ClientResponseRegistry> Future for ClientResponseFuture<T, C> {
    type Output = Result<T>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.receiver).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                self.completed = true;
                self.client = None;
                Poll::Ready(result.map_err(|_| Error::RCL_RET_CLIENT_INVALID))
            }
        }
    }
}

impl<T, C: ClientResponseRegistry> Drop for ClientResponseFuture<T, C> {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        self.receiver.close();
        if let Some(client) = self.client.take().and_then(|client| client.upgrade()) {
            client.lock().remove_canceled_response(self.sequence);
        }
    }
}

trait ClientResponseRegistry {
    fn remove_canceled_response(&mut self, sequence: i64);
}

fn client_response_future<C, T>(
    client: Weak<Mutex<C>>, sequence: i64, receiver: oneshot::Receiver<T>,
) -> ClientResponseFuture<T, C>
where
    C: ClientResponseRegistry,
{
    ClientResponseFuture::new(receiver, client, sequence)
}

fn response_conversion_completion<N, T>(
    sender: impl Into<ClientResponseSender<T>>, native: N, info: ServiceResponseInfo,
    convert: impl FnOnce(N) -> T + Send + 'static,
) -> ClientCompletion
where
    N: 'static,
    T: Debug + 'static,
{
    let sender = sender.into();
    Box::new(move || {
        let response = convert(native);
        let failed = match sender {
            ClientResponseSender::Plain(sender) => sender.send(response).is_err(),
            ClientResponseSender::WithInfo(sender) => {
                sender.send(ServiceResponse { response, info }).is_err()
            }
        };
        if failed {
            log::debug!("client response receiver was dropped");
        }
    })
}

fn availability_completion(
    senders: Vec<oneshot::Sender<()>>, available: Result<bool>,
) -> Option<ClientCompletion> {
    match available {
        Ok(false) => None,
        Ok(true) => Some(Box::new(move || {
            for sender in senders {
                let _ = sender.send(());
            }
        })),
        Err(_) => Some(Box::new(move || drop(senders))),
    }
}

pub(crate) fn complete_client_operation<T: ?Sized>(
    client: &Mutex<T>, operation: impl FnOnce(&mut T) -> Result<Option<ClientCompletion>>,
) -> Result<()> {
    let completion = { operation(&mut *client.lock())? };
    if let Some(completion) = completion {
        completion();
    }
    Ok(())
}

pub(crate) fn complete_client_responses<T: ?Sized>(
    client: &Mutex<T>, mut operation: impl FnMut(&mut T) -> Result<ClientResponseAction>,
) -> Result<()> {
    for _ in 0..MAX_CLIENT_RESPONSES_PER_SPIN {
        let action = { operation(&mut *client.lock())? };
        match action {
            ClientResponseAction::Idle => return Ok(()),
            ClientResponseAction::Taken(completion) => {
                if let Some(completion) = completion {
                    completion();
                }
            }
        }
    }
    Ok(())
}

fn with_prepared_client<C: ?Sized, P, T>(
    client: &Mutex<C>, prepare: impl FnOnce() -> Result<P>,
    operation: impl FnOnce(&mut C, P) -> Result<T>,
) -> Result<T> {
    let prepared = prepare()?;
    operation(&mut *client.lock(), prepared)
}

/// ROS service client.
///
/// This is a handle to a service client wrapped in a `Mutex` inside a
/// `Weak` `Arc`. As such you can pass it between threads safely.
pub struct Client<T>
where
    T: WrappedServiceTypeSupport,
{
    pub(crate) client: Weak<Mutex<TypedClient<T>>>,
    lifecycle: Arc<ClientHandleLifecycle>,
}

pub(crate) struct ClientHandleLifecycle {
    identity: usize,
    dropped_clients: Arc<Mutex<VecDeque<usize>>>,
    active_availability_clients: Arc<Mutex<VecDeque<usize>>>,
    explicitly_destroyed: AtomicBool,
}

impl Drop for ClientHandleLifecycle {
    fn drop(&mut self) {
        if !self.explicitly_destroyed.load(Ordering::Acquire) {
            self.dropped_clients.lock().push_back(self.identity);
        }
    }
}

impl ClientHandleLifecycle {
    fn mark_explicitly_destroyed(&self) {
        self.explicitly_destroyed.store(true, Ordering::Release);
    }
}

impl<T> Clone for Client<T>
where
    T: WrappedServiceTypeSupport,
{
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            lifecycle: Arc::clone(&self.lifecycle),
        }
    }
}

impl<T> Client<T>
where
    T: WrappedServiceTypeSupport,
{
    pub(crate) fn identity(&self) -> usize {
        self.lifecycle.identity
    }

    pub(crate) fn mark_explicitly_destroyed(&self) {
        self.lifecycle.mark_explicitly_destroyed();
    }
}

impl<T: 'static> Client<T>
where
    T: WrappedServiceTypeSupport,
{
    /// Make a service request.
    ///
    /// Returns a `Future` of the `Response` type.
    pub fn request(&self, msg: &T::Request) -> Result<impl Future<Output = Result<T::Response>>>
    where
        T: WrappedServiceTypeSupport,
    {
        // upgrade to actual ref. if still alive
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let client_for_cleanup = Arc::downgrade(&client);
        let (sequence, receiver) = with_prepared_client(
            &client,
            || WrappedNativeMsg::<T::Request>::try_from(msg),
            TypedClient::request_native,
        )?;
        Ok(client_response_future(client_for_cleanup, sequence, receiver))
    }

    /// Makes a service request and returns the response with its RMW metadata.
    pub fn request_with_info(
        &self, msg: &T::Request,
    ) -> Result<impl Future<Output = Result<ServiceResponse<T::Response>>>> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let client_for_cleanup = Arc::downgrade(&client);
        let (sequence, receiver) = with_prepared_client(
            &client,
            || WrappedNativeMsg::<T::Request>::try_from(msg),
            TypedClient::request_native_with_info,
        )?;
        Ok(client_response_future(client_for_cleanup, sequence, receiver))
    }

    /// Make a service request and wait up to `timeout` for its response.
    ///
    /// This future is runtime-agnostic and advances while `node` is spun.
    pub fn request_with_timeout(
        &self, node: &mut Node, msg: &T::Request, timeout: Duration,
    ) -> Result<impl Future<Output = Result<T::Response>>> {
        self.validate_owner(node)?;
        Node::validate_timeout(timeout)?;
        let response = self.request(msg)?;
        node.with_timeout(response, timeout)
    }

    /// Makes a service request and waits up to `timeout` for its response and RMW metadata.
    ///
    /// This future is runtime-agnostic and advances while `node` is spun.
    pub fn request_with_info_with_timeout(
        &self, node: &mut Node, msg: &T::Request, timeout: Duration,
    ) -> Result<impl Future<Output = Result<ServiceResponse<T::Response>>>> {
        self.validate_owner(node)?;
        Node::validate_timeout(timeout)?;
        let response = self.request_with_info(msg)?;
        node.with_timeout(response, timeout)
    }

    /// Returns the actual QoS used by this client's request publisher.
    pub fn get_request_publisher_actual_qos(&self) -> Result<QosProfile> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let client = client.lock();
        client_request_publisher_actual_qos(client.handle())
    }

    /// Returns the actual QoS used by this client's response subscription.
    pub fn get_response_subscription_actual_qos(&self) -> Result<QosProfile> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let client = client.lock();
        client_response_subscription_actual_qos(client.handle())
    }

    /// Returns the fully resolved service name used by the native client.
    pub fn service_name(&self) -> Result<String> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let client = client.lock();
        client_service_name(client.handle())
    }

    /// Returns the globally unique middleware identity of this service client.
    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    pub fn get_gid(&self) -> Result<ClientGid> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let client = client.lock();
        client_gid(client.handle())
    }

    /// Checks whether the service is currently available without registering a waiter.
    pub fn is_available(&self, node: &Node) -> Result<bool> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        if !node.owns_client(&client) {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let client = client.lock();
        service_available_helper(node.node_handle.as_ref(), client.handle())
    }

    pub(crate) fn validate_owner(&self, node: &Node) -> Result<()> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        if node.owns_client(&client) {
            Ok(())
        } else {
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        }
    }

    /// Waits asynchronously until the service is available.
    ///
    /// The owning node must be spun while awaiting.
    pub fn wait_for_service(&self) -> Result<impl Future<Output = Result<()>>> {
        Node::is_available(self)
    }

    /// Waits asynchronously until the service is available or `timeout` expires.
    ///
    /// This future is runtime-agnostic and advances while `node` is spun.
    pub fn wait_for_service_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        self.validate_owner(node)?;
        Node::validate_timeout(timeout)?;
        let available = self.wait_for_service()?;
        node.with_timeout(available, timeout)
    }
}

/// ROS "untyped" service client.
///
/// The untyped client is useful when you don't know the concrete type
/// at compile time. Messages are represented by `serde_json::Value`.
///
/// This is a handle to a service client wrapped in a `Mutex` inside a
/// `Weak` `Arc`. As such you can pass it between threads safely.
pub struct ClientUntyped {
    pub(crate) client: Weak<Mutex<UntypedClient_>>,
    lifecycle: Arc<ClientHandleLifecycle>,
}

impl Clone for ClientUntyped {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            lifecycle: Arc::clone(&self.lifecycle),
        }
    }
}

impl ClientUntyped {
    pub(crate) fn identity(&self) -> usize {
        self.lifecycle.identity
    }

    pub(crate) fn mark_explicitly_destroyed(&self) {
        self.lifecycle.mark_explicitly_destroyed();
    }

    /// Make an "untyped" service request.
    ///
    /// The request is a `serde_json::Value`. It is up to the user to
    /// make sure the fields in the json object are correct.
    ///
    /// Returns a `Future` of Result<serde_json::Value>.
    pub fn request(
        &self, msg: serde_json::Value,
    ) -> Result<impl Future<Output = Result<Result<serde_json::Value>>>> {
        // upgrade to actual ref. if still alive
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let make_request_msg = client.lock().service_type.make_request_msg;
        let client_for_cleanup = Arc::downgrade(&client);
        let (sequence, receiver) = with_prepared_client(
            &client,
            || {
                let mut native_msg = make_request_msg()?;
                native_msg.from_json(msg)?;
                Ok(native_msg)
            },
            UntypedClient_::request_native,
        )?;
        Ok(client_response_future(client_for_cleanup, sequence, receiver))
    }

    /// Makes an untyped service request and returns the JSON response with its RMW metadata.
    pub fn request_with_info(
        &self, msg: serde_json::Value,
    ) -> Result<impl Future<Output = Result<ServiceResponse<Result<serde_json::Value>>>>> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let make_request_msg = client.lock().service_type.make_request_msg;
        let client_for_cleanup = Arc::downgrade(&client);
        let (sequence, receiver) = with_prepared_client(
            &client,
            || {
                let mut native_msg = make_request_msg()?;
                native_msg.from_json(msg)?;
                Ok(native_msg)
            },
            UntypedClient_::request_native_with_info,
        )?;
        Ok(client_response_future(client_for_cleanup, sequence, receiver))
    }

    /// Make an untyped service request and wait up to `timeout` for its response.
    ///
    /// This future is runtime-agnostic and advances while `node` is spun.
    pub fn request_with_timeout(
        &self, node: &mut Node, msg: serde_json::Value, timeout: Duration,
    ) -> Result<impl Future<Output = Result<Result<serde_json::Value>>>> {
        self.validate_owner(node)?;
        Node::validate_timeout(timeout)?;
        let response = self.request(msg)?;
        node.with_timeout(response, timeout)
    }

    /// Makes an untyped service request and waits up to `timeout` for its response and metadata.
    ///
    /// This future is runtime-agnostic and advances while `node` is spun.
    pub fn request_with_info_with_timeout(
        &self, node: &mut Node, msg: serde_json::Value, timeout: Duration,
    ) -> Result<impl Future<Output = Result<ServiceResponse<Result<serde_json::Value>>>>> {
        self.validate_owner(node)?;
        Node::validate_timeout(timeout)?;
        let response = self.request_with_info(msg)?;
        node.with_timeout(response, timeout)
    }

    /// Returns the actual QoS used by this client's request publisher.
    pub fn get_request_publisher_actual_qos(&self) -> Result<QosProfile> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let client = client.lock();
        client_request_publisher_actual_qos(client.handle())
    }

    /// Returns the actual QoS used by this client's response subscription.
    pub fn get_response_subscription_actual_qos(&self) -> Result<QosProfile> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let client = client.lock();
        client_response_subscription_actual_qos(client.handle())
    }

    /// Returns the fully resolved service name used by the native client.
    pub fn service_name(&self) -> Result<String> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let client = client.lock();
        client_service_name(client.handle())
    }

    /// Returns the globally unique middleware identity of this service client.
    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    pub fn get_gid(&self) -> Result<ClientGid> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let client = client.lock();
        client_gid(client.handle())
    }

    /// Checks whether the service is currently available without registering a waiter.
    pub fn is_available(&self, node: &Node) -> Result<bool> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        if !node.owns_client(&client) {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let client = client.lock();
        service_available_helper(node.node_handle.as_ref(), client.handle())
    }

    fn validate_owner(&self, node: &Node) -> Result<()> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        if node.owns_client(&client) {
            Ok(())
        } else {
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        }
    }

    /// Waits asynchronously until the service is available.
    ///
    /// The owning node must be spun while awaiting.
    pub fn wait_for_service(&self) -> Result<impl Future<Output = Result<()>>> {
        Node::is_available(self)
    }

    /// Waits asynchronously until the service is available or `timeout` expires.
    ///
    /// This future is runtime-agnostic and advances while `node` is spun.
    pub fn wait_for_service_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        self.validate_owner(node)?;
        Node::validate_timeout(timeout)?;
        let available = self.wait_for_service()?;
        node.with_timeout(available, timeout)
    }
}

fn client_service_name(handle: &rcl_client_t) -> Result<String> {
    let name = unsafe { rcl_client_get_service_name(handle) };
    if name.is_null() {
        return Err(Error::RCL_RET_CLIENT_INVALID);
    }
    unsafe { CStr::from_ptr(name) }
        .to_str()
        .map(str::to_owned)
        .map_err(|err| Error::InvalidUtf8 {
            context: "client service name",
            err: err.to_string(),
        })
}

#[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
fn client_gid(handle: &rcl_client_t) -> Result<ClientGid> {
    let rmw_handle = unsafe { rcl_client_get_rmw_handle(handle).as_ref() }
        .ok_or(Error::RCL_RET_CLIENT_INVALID)?;
    let mut gid = MaybeUninit::<rmw_gid_t>::uninit();
    let ret = unsafe { rmw_get_gid_for_client(rmw_handle, gid.as_mut_ptr()) };
    if ret != RMW_RET_OK as i32 {
        return Err(Error::from_rcl_error(ret));
    }
    let gid = unsafe { gid.assume_init() };
    if gid.implementation_identifier.is_null() {
        return Err(Error::RCL_RET_ERROR);
    }
    let implementation_identifier = unsafe { CStr::from_ptr(gid.implementation_identifier) }
        .to_str()
        .map(str::to_owned)
        .map_err(|err| Error::InvalidUtf8 {
            context: "client RMW implementation identifier",
            err: err.to_string(),
        })?;
    Ok(ClientGid {
        implementation_identifier,
        data: gid.data,
    })
}

fn client_request_publisher_actual_qos(handle: &rcl_client_t) -> Result<QosProfile> {
    let qos = unsafe { rcl_client_request_publisher_get_actual_qos(handle) };
    let qos = unsafe { qos.as_ref() }.ok_or(Error::RCL_RET_CLIENT_INVALID)?;
    QosProfile::try_from(qos)
}

fn client_response_subscription_actual_qos(handle: &rcl_client_t) -> Result<QosProfile> {
    let qos = unsafe { rcl_client_response_subscription_get_actual_qos(handle) };
    let qos = unsafe { qos.as_ref() }.ok_or(Error::RCL_RET_CLIENT_INVALID)?;
    QosProfile::try_from(qos)
}

pub(crate) fn make_client<T>(
    client: Weak<Mutex<TypedClient<T>>>, identity: usize,
    dropped_clients: Arc<Mutex<VecDeque<usize>>>,
    active_availability_clients: Arc<Mutex<VecDeque<usize>>>,
) -> Client<T>
where
    T: WrappedServiceTypeSupport,
{
    Client {
        client,
        lifecycle: Arc::new(ClientHandleLifecycle {
            identity,
            dropped_clients,
            active_availability_clients,
            explicitly_destroyed: AtomicBool::new(false),
        }),
    }
}

pub(crate) fn make_untyped_client(
    client: Weak<Mutex<UntypedClient_>>, identity: usize,
    dropped_clients: Arc<Mutex<VecDeque<usize>>>,
    active_availability_clients: Arc<Mutex<VecDeque<usize>>>,
) -> ClientUntyped {
    ClientUntyped {
        client,
        lifecycle: Arc::new(ClientHandleLifecycle {
            identity,
            dropped_clients,
            active_availability_clients,
            explicitly_destroyed: AtomicBool::new(false),
        }),
    }
}

unsafe impl<T> Send for TypedClient<T> where T: WrappedServiceTypeSupport {}

impl<T: 'static> TypedClient<T>
where
    T: WrappedServiceTypeSupport,
{
    fn request_native(
        &mut self, native_msg: WrappedNativeMsg<T::Request>,
    ) -> Result<(i64, oneshot::Receiver<T::Response>)>
    where
        T: WrappedServiceTypeSupport,
    {
        let mut seq_no = 0i64;
        let result =
            unsafe { rcl_send_request(&self.rcl_handle, native_msg.void_ptr(), &mut seq_no) };

        let (sender, receiver) = oneshot::channel::<T::Response>();

        if result == RCL_RET_OK as i32 {
            self.response_channels
                .insert(seq_no, ClientResponseSender::Plain(sender));
            Ok((seq_no, receiver))
        } else {
            log::error!("could not send request {}", result);
            Err(Error::from_rcl_error(result))
        }
    }

    fn request_native_with_info(
        &mut self, native_msg: WrappedNativeMsg<T::Request>,
    ) -> Result<(i64, oneshot::Receiver<ServiceResponse<T::Response>>)> {
        let mut seq_no = 0i64;
        let result =
            unsafe { rcl_send_request(&self.rcl_handle, native_msg.void_ptr(), &mut seq_no) };
        let (sender, receiver) = oneshot::channel();
        if result == RCL_RET_OK as i32 {
            self.response_channels
                .insert(seq_no, ClientResponseSender::WithInfo(sender));
            Ok((seq_no, receiver))
        } else {
            log::error!("could not send request {}", result);
            Err(Error::from_rcl_error(result))
        }
    }
}

unsafe impl Send for UntypedClient_ {}

impl UntypedClient_ {
    fn request_native(
        &mut self, native_msg: WrappedNativeMsgUntyped,
    ) -> Result<(i64, oneshot::Receiver<Result<serde_json::Value>>)> {
        let mut seq_no = 0i64;
        let result =
            unsafe { rcl_send_request(&self.rcl_handle, native_msg.void_ptr(), &mut seq_no) };

        let (sender, receiver) = oneshot::channel::<Result<serde_json::Value>>();

        if result == RCL_RET_OK as i32 {
            self.response_channels
                .insert(seq_no, ClientResponseSender::Plain(sender));
            Ok((seq_no, receiver))
        } else {
            log::error!("could not send request {}", result);
            Err(Error::from_rcl_error(result))
        }
    }

    fn request_native_with_info(
        &mut self, native_msg: WrappedNativeMsgUntyped,
    ) -> Result<(i64, oneshot::Receiver<ServiceResponse<Result<serde_json::Value>>>)> {
        let mut seq_no = 0i64;
        let result =
            unsafe { rcl_send_request(&self.rcl_handle, native_msg.void_ptr(), &mut seq_no) };
        let (sender, receiver) = oneshot::channel();
        if result == RCL_RET_OK as i32 {
            self.response_channels
                .insert(seq_no, ClientResponseSender::WithInfo(sender));
            Ok((seq_no, receiver))
        } else {
            log::error!("could not send request {}", result);
            Err(Error::from_rcl_error(result))
        }
    }
}

pub trait Client_ {
    fn handle(&self) -> &rcl_client_t;
    fn handle_response(&mut self) -> Result<ClientResponseAction>;
    fn register_poll_available(&mut self, s: oneshot::Sender<()>) -> bool;
    fn poll_available(&mut self, node: &mut rcl_node_t) -> Option<ClientCompletion>;
    fn has_pending_availability(&mut self) -> bool;
    #[cfg(test)]
    fn availability_poll_count(&self) -> usize;
    fn has_pending_work(&mut self) -> bool;
    fn destroy(&mut self, node: &mut rcl_node_t);
}

pub struct TypedClient<T>
where
    T: WrappedServiceTypeSupport,
{
    pub rcl_handle: rcl_client_t,
    pub(crate) response_channels: HashMap<i64, ClientResponseSender<T::Response>>,
    pub poll_available_channels: AvailabilityWaiters,
    #[cfg(test)]
    pub(crate) availability_poll_count: AtomicUsize,
}

impl<T> ClientResponseRegistry for TypedClient<T>
where
    T: WrappedServiceTypeSupport,
{
    fn remove_canceled_response(&mut self, sequence: i64) {
        if self
            .response_channels
            .get(&sequence)
            .is_some_and(ClientResponseSender::is_canceled)
        {
            self.response_channels.remove(&sequence);
        }
    }
}

impl<T: 'static> Client_ for TypedClient<T>
where
    T: WrappedServiceTypeSupport,
{
    fn handle(&self) -> &rcl_client_t {
        &self.rcl_handle
    }

    fn handle_response(&mut self) -> Result<ClientResponseAction> {
        let mut response_info = MaybeUninit::<rmw_service_info_t>::uninit();
        let mut response_msg = WrappedNativeMsg::<T::Response>::try_new()?;

        let ret = unsafe {
            rcl_take_response_with_info(
                &self.rcl_handle,
                response_info.as_mut_ptr(),
                response_msg.void_ptr_mut(),
            )
        };
        if ret == RCL_RET_CLIENT_TAKE_FAILED as i32 {
            return Ok(ClientResponseAction::Idle);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let response_info = unsafe { response_info.assume_init() };
        let info = ServiceResponseInfo::from(&response_info);
        if let Some(sender) = self.response_channels.remove(&info.sequence_number) {
            Ok(ClientResponseAction::Taken(Some(response_conversion_completion(
                sender,
                response_msg,
                info,
                |response_msg| T::Response::from_native(&response_msg),
            ))))
        } else {
            log::debug!(
                "no such req id: {}, we have {:?}, ignoring",
                info.sequence_number,
                self.response_channels.keys()
            );
            Ok(ClientResponseAction::Taken(None))
        }
    }

    fn register_poll_available(&mut self, s: oneshot::Sender<()>) -> bool {
        self.poll_available_channels.register(s)
    }

    fn poll_available(&mut self, node: &mut rcl_node_t) -> Option<ClientCompletion> {
        #[cfg(test)]
        self.availability_poll_count.fetch_add(1, Ordering::Relaxed);
        self.poll_available_channels.prune_canceled();
        if self.poll_available_channels.is_empty() {
            return None;
        }
        let available = service_available_helper(node, self.handle());
        if matches!(available, Ok(false)) {
            None
        } else {
            let senders = self.poll_available_channels.take_all();
            availability_completion(senders, available)
        }
    }

    fn has_pending_availability(&mut self) -> bool {
        self.poll_available_channels.prune_canceled();
        !self.poll_available_channels.is_empty()
    }

    #[cfg(test)]
    fn availability_poll_count(&self) -> usize {
        self.availability_poll_count.load(Ordering::Relaxed)
    }

    fn has_pending_work(&mut self) -> bool {
        self.poll_available_channels.prune_canceled();
        !self.response_channels.is_empty() || !self.poll_available_channels.is_empty()
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_client_fini(&mut self.rcl_handle, node);
        }
    }
}

pub struct UntypedClient_ {
    pub service_type: UntypedServiceSupport,
    pub rcl_handle: rcl_client_t,
    pub(crate) response_channels: HashMap<i64, ClientResponseSender<Result<serde_json::Value>>>,
    pub poll_available_channels: AvailabilityWaiters,
    #[cfg(test)]
    pub(crate) availability_poll_count: AtomicUsize,
}

impl ClientResponseRegistry for UntypedClient_ {
    fn remove_canceled_response(&mut self, sequence: i64) {
        if self
            .response_channels
            .get(&sequence)
            .is_some_and(ClientResponseSender::is_canceled)
        {
            self.response_channels.remove(&sequence);
        }
    }
}

impl Client_ for UntypedClient_ {
    fn handle(&self) -> &rcl_client_t {
        &self.rcl_handle
    }

    fn handle_response(&mut self) -> Result<ClientResponseAction> {
        let mut response_info = MaybeUninit::<rmw_service_info_t>::uninit();
        let mut response_msg = (self.service_type.make_response_msg)()?;

        let ret = unsafe {
            rcl_take_response_with_info(
                &self.rcl_handle,
                response_info.as_mut_ptr(),
                response_msg.void_ptr_mut(),
            )
        };
        if ret == RCL_RET_CLIENT_TAKE_FAILED as i32 {
            return Ok(ClientResponseAction::Idle);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let response_info = unsafe { response_info.assume_init() };
        let info = ServiceResponseInfo::from(&response_info);
        if let Some(sender) = self.response_channels.remove(&info.sequence_number) {
            Ok(ClientResponseAction::Taken(Some(response_conversion_completion(
                sender,
                response_msg,
                info,
                |response_msg| response_msg.to_json(),
            ))))
        } else {
            log::debug!(
                "no such req id: {}, we have {:?}, ignoring",
                info.sequence_number,
                self.response_channels.keys()
            );
            Ok(ClientResponseAction::Taken(None))
        }
    }

    fn register_poll_available(&mut self, s: oneshot::Sender<()>) -> bool {
        self.poll_available_channels.register(s)
    }

    fn poll_available(&mut self, node: &mut rcl_node_t) -> Option<ClientCompletion> {
        #[cfg(test)]
        self.availability_poll_count.fetch_add(1, Ordering::Relaxed);
        self.poll_available_channels.prune_canceled();
        if self.poll_available_channels.is_empty() {
            return None;
        }
        let available = service_available_helper(node, self.handle());
        if matches!(available, Ok(false)) {
            None
        } else {
            let senders = self.poll_available_channels.take_all();
            availability_completion(senders, available)
        }
    }

    fn has_pending_availability(&mut self) -> bool {
        self.poll_available_channels.prune_canceled();
        !self.poll_available_channels.is_empty()
    }

    #[cfg(test)]
    fn availability_poll_count(&self) -> usize {
        self.availability_poll_count.load(Ordering::Relaxed)
    }

    fn has_pending_work(&mut self) -> bool {
        self.poll_available_channels.prune_canceled();
        !self.response_channels.is_empty() || !self.poll_available_channels.is_empty()
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_client_fini(&mut self.rcl_handle, node);
        }
    }
}

pub fn create_client_helper(
    node: *mut rcl_node_t, service_name: &str, service_ts: *const rosidl_service_type_support_t,
    qos_profile: QosProfile,
) -> Result<rcl_client_t> {
    let mut client_handle = unsafe { rcl_get_zero_initialized_client() };
    let service_name_c_string =
        CString::new(service_name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;

    let result = unsafe {
        let mut client_options = rcl_client_get_default_options();
        client_options.qos = qos_profile.into();
        rcl_client_init(
            &mut client_handle,
            node,
            service_ts,
            service_name_c_string.as_ptr(),
            &client_options,
        )
    };
    if result == RCL_RET_OK as i32 {
        Ok(client_handle)
    } else {
        Err(Error::from_rcl_error(result))
    }
}

pub fn service_available_helper(node: &rcl_node_t, client: &rcl_client_t) -> Result<bool> {
    let mut avail = false;
    let result = unsafe { rcl_service_server_is_available(node, client, &mut avail) };

    if result == RCL_RET_OK as i32 {
        Ok(avail)
    } else {
        Err(Error::from_rcl_error(result))
    }
}

use crate::nodes::IsAvailablePollable;

impl<T: 'static> IsAvailablePollable for Client<T>
where
    T: WrappedServiceTypeSupport,
{
    fn register_poll_available(&self, sender: oneshot::Sender<()>) -> Result<()> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let mut client = client.lock();
        if client.register_poll_available(sender) {
            self.lifecycle
                .active_availability_clients
                .lock()
                .push_back(self.lifecycle.identity);
        }
        Ok(())
    }

    fn validate_availability_owner(&self, node: &Node) -> Result<()> {
        self.validate_owner(node)
    }
}

impl IsAvailablePollable for ClientUntyped {
    fn register_poll_available(&self, sender: oneshot::Sender<()>) -> Result<()> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        let mut client = client.lock();
        if client.register_poll_available(sender) {
            self.lifecycle
                .active_availability_clients
                .lock()
                .push_back(self.lifecycle.identity);
        }
        Ok(())
    }

    fn validate_availability_owner(&self, node: &Node) -> Result<()> {
        self.validate_owner(node)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::example_interfaces::srv::AddTwoInts;
    use futures::{
        task::{waker_ref, ArcWake},
        Future, FutureExt,
    };
    use std::{
        pin::Pin,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        task::{Context, Poll},
    };

    fn assert_clone<T: Clone>() {}

    fn test_response_info(sequence_number: i64) -> ServiceResponseInfo {
        ServiceResponseInfo {
            source_timestamp: 0,
            received_timestamp: 0,
            writer_guid: [0; 16],
            sequence_number,
        }
    }

    fn plain_response_sender<T>(sender: ClientResponseSender<T>) -> oneshot::Sender<T> {
        match sender {
            ClientResponseSender::Plain(sender) => sender,
            ClientResponseSender::WithInfo(_) => panic!("expected a plain response sender"),
        }
    }

    #[test]
    fn service_client_handles_are_cloneable() {
        assert_clone::<Client<AddTwoInts::Service>>();
        assert_clone::<ClientUntyped>();
    }

    #[test]
    fn service_response_info_copies_native_metadata() {
        let native = rmw_service_info_t {
            source_timestamp: 41,
            received_timestamp: 42,
            request_id: rmw_request_id_t {
                writer_guid: [128, 255, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 126, 127],
                sequence_number: 43,
            },
        };

        let info = ServiceResponseInfo::from(&native);

        assert_eq!(info.source_timestamp, 41);
        assert_eq!(info.received_timestamp, 42);
        assert_eq!(info.writer_guid, [128, 255, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 126, 127]);
        assert_eq!(info.sequence_number, 43);
    }

    #[test]
    fn availability_registration_keeps_all_waiters_while_scheduling_once() {
        let mut client = TypedClient::<AddTwoInts::Service> {
            rcl_handle: unsafe { rcl_get_zero_initialized_client() },
            response_channels: HashMap::new(),
            poll_available_channels: AvailabilityWaiters::default(),
            availability_poll_count: AtomicUsize::new(0),
        };
        let (first_sender, first_receiver) = oneshot::channel();
        let (second_sender, second_receiver) = oneshot::channel();

        assert!(client.register_poll_available(first_sender));
        assert!(!client.register_poll_available(second_sender));
        let senders = client.poll_available_channels.take_all();
        assert_eq!(senders.len(), 2);
        for sender in senders {
            sender.send(()).unwrap();
        }
        assert_eq!(futures::executor::block_on(first_receiver), Ok(()));
        assert_eq!(futures::executor::block_on(second_receiver), Ok(()));
    }

    #[test]
    fn final_client_handle_drop_queues_its_identity_once() {
        let dropped_clients = Arc::new(Mutex::new(VecDeque::new()));
        let client = Arc::new(Mutex::new(TypedClient::<AddTwoInts::Service> {
            rcl_handle: unsafe { rcl_get_zero_initialized_client() },
            response_channels: HashMap::new(),
            poll_available_channels: AvailabilityWaiters::default(),
            availability_poll_count: AtomicUsize::new(0),
        }));
        let identity = Arc::as_ptr(&client).cast::<()>() as usize;
        let handle = make_client(
            Arc::downgrade(&client),
            identity,
            Arc::clone(&dropped_clients),
            Arc::new(Mutex::new(VecDeque::new())),
        );
        let clone = handle.clone();

        drop(handle);
        assert!(dropped_clients.lock().is_empty());
        drop(clone);

        assert_eq!(dropped_clients.lock().iter().copied().collect::<Vec<_>>(), [identity]);
    }

    #[test]
    fn dropping_response_future_removes_only_its_pending_request() {
        let (target_sender, target_receiver) = oneshot::channel();
        let (other_sender, _other_receiver) = oneshot::channel();
        let client = Arc::new(Mutex::new(TypedClient::<AddTwoInts::Service> {
            rcl_handle: unsafe { rcl_get_zero_initialized_client() },
            response_channels: HashMap::from([
                (10, target_sender.into()),
                (11, other_sender.into()),
            ]),
            poll_available_channels: AvailabilityWaiters::default(),
            availability_poll_count: AtomicUsize::new(0),
        }));
        let future = client_response_future(Arc::downgrade(&client), 10, target_receiver);

        drop(future);

        let client = client.lock();
        assert!(!client.response_channels.contains_key(&10));
        assert!(client.response_channels.contains_key(&11));
    }

    #[test]
    fn completed_response_future_disarms_drop_cleanup() {
        let (sender, receiver) = oneshot::channel();
        let client = Arc::new(Mutex::new(TypedClient::<AddTwoInts::Service> {
            rcl_handle: unsafe { rcl_get_zero_initialized_client() },
            response_channels: HashMap::from([(10, sender.into())]),
            poll_available_channels: AvailabilityWaiters::default(),
            availability_poll_count: AtomicUsize::new(0),
        }));
        let mut future = Box::pin(client_response_future(Arc::downgrade(&client), 10, receiver));
        let sender = plain_response_sender(client.lock().response_channels.remove(&10).unwrap());
        sender.send(AddTwoInts::Response { sum: 42 }).unwrap();
        assert!(matches!(
            future.as_mut().now_or_never(),
            Some(Ok(AddTwoInts::Response { sum: 42 }))
        ));
        let (replacement, _receiver) = oneshot::channel();
        client
            .lock()
            .response_channels
            .insert(10, replacement.into());

        drop(future);

        assert!(client.lock().response_channels.contains_key(&10));
    }

    #[test]
    fn stale_response_future_does_not_remove_a_reused_sequence() {
        let (old_sender, old_receiver) = oneshot::channel();
        let client = Arc::new(Mutex::new(TypedClient::<AddTwoInts::Service> {
            rcl_handle: unsafe { rcl_get_zero_initialized_client() },
            response_channels: HashMap::from([(10, old_sender.into())]),
            poll_available_channels: AvailabilityWaiters::default(),
            availability_poll_count: AtomicUsize::new(0),
        }));
        let future = client_response_future(Arc::downgrade(&client), 10, old_receiver);
        let old_sender =
            plain_response_sender(client.lock().response_channels.remove(&10).unwrap());
        old_sender.send(AddTwoInts::Response { sum: 42 }).unwrap();
        let (replacement, _receiver) = oneshot::channel();
        client
            .lock()
            .response_channels
            .insert(10, replacement.into());

        drop(future);

        assert!(client.lock().response_channels.contains_key(&10));
    }

    #[test]
    fn typed_client_pending_work_prunes_dropped_availability_waiter() {
        let mut client = TypedClient::<AddTwoInts::Service> {
            rcl_handle: unsafe { rcl_get_zero_initialized_client() },
            response_channels: HashMap::new(),
            poll_available_channels: AvailabilityWaiters::default(),
            availability_poll_count: AtomicUsize::new(0),
        };
        let (sender, receiver) = oneshot::channel();
        client.register_poll_available(sender);

        drop(receiver);

        assert!(!client.has_pending_work());
        assert!(client.poll_available_channels.is_empty());
    }

    #[test]
    fn untyped_client_pending_work_prunes_dropped_availability_waiter() {
        let mut client = UntypedClient_ {
            service_type: UntypedServiceSupport::new_from("example_interfaces/srv/AddTwoInts")
                .unwrap(),
            rcl_handle: unsafe { rcl_get_zero_initialized_client() },
            response_channels: HashMap::new(),
            poll_available_channels: AvailabilityWaiters::default(),
            availability_poll_count: AtomicUsize::new(0),
        };
        let (sender, receiver) = oneshot::channel();
        client.register_poll_available(sender);

        drop(receiver);

        assert!(!client.has_pending_work());
        assert!(client.poll_available_channels.is_empty());
    }

    struct LockingWake {
        client: Arc<Mutex<()>>,
        woke: AtomicBool,
    }

    impl ArcWake for LockingWake {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            let _client = arc_self.client.lock();
            arc_self.woke.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn client_response_batches_stop_at_idle_and_fairness_limit() {
        let client = Mutex::new(0usize);
        let calls = std::cell::Cell::new(0);
        complete_client_responses(&client, |_| {
            let call = calls.get();
            calls.set(call + 1);
            if call == 2 {
                Ok(ClientResponseAction::Idle)
            } else {
                Ok(ClientResponseAction::Taken(None))
            }
        })
        .unwrap();
        assert_eq!(calls.get(), 3);

        calls.set(0);
        complete_client_responses(&client, |_| {
            calls.set(calls.get() + 1);
            Ok(ClientResponseAction::Taken(None))
        })
        .unwrap();
        assert_eq!(calls.get(), MAX_CLIENT_RESPONSES_PER_SPIN);
    }

    #[test]
    fn client_completions_wake_after_releasing_the_client_lock() {
        let client = Arc::new(Mutex::new(()));
        let (sender, mut receiver) = oneshot::channel();
        let wake = Arc::new(LockingWake {
            client: Arc::clone(&client),
            woke: AtomicBool::new(false),
        });
        let waker = waker_ref(&wake);
        let mut context = Context::from_waker(&waker);
        assert!(matches!(Pin::new(&mut receiver).poll(&mut context), Poll::Pending));

        complete_client_operation(&client, |_| {
            Ok(Some(response_conversion_completion(
                sender,
                (),
                test_response_info(1),
                |response| response,
            )))
        })
        .unwrap();

        assert!(wake.woke.load(Ordering::SeqCst));
        assert!(matches!(Pin::new(&mut receiver).poll(&mut context), Poll::Ready(Ok(()))));
    }

    #[test]
    fn client_responses_are_converted_after_releasing_the_client_lock() {
        let client = Arc::new(Mutex::new(()));
        let (sender, mut receiver) = oneshot::channel();
        let client_for_conversion = Arc::clone(&client);

        complete_client_operation(&*client, |_| {
            Ok(Some(response_conversion_completion(
                sender,
                41,
                test_response_info(1),
                move |value| {
                    assert!(client_for_conversion.try_lock().is_some());
                    value + 1
                },
            )))
        })
        .unwrap();

        assert_eq!(receiver.try_recv().unwrap(), Some(42));
    }

    #[test]
    fn client_response_metadata_is_delivered_only_when_requested() {
        let info = ServiceResponseInfo {
            source_timestamp: 41,
            received_timestamp: 42,
            writer_guid: [7; 16],
            sequence_number: 43,
        };
        let (sender, mut receiver) = oneshot::channel();

        response_conversion_completion(ClientResponseSender::WithInfo(sender), 40, info, |value| {
            value + 2
        })();

        assert_eq!(receiver.try_recv().unwrap(), Some(ServiceResponse { response: 42, info }));
    }

    #[test]
    fn client_payloads_are_prepared_before_locking_the_native_handle() {
        let client = Mutex::new(());

        let prepared = with_prepared_client(
            &client,
            || {
                assert!(client.try_lock().is_some());
                Ok(41)
            },
            |_client, value| Ok(value + 1),
        )
        .unwrap();

        assert_eq!(prepared, 42);
    }
}
