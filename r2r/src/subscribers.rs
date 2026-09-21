use futures::{
    channel::{mpsc, oneshot},
    task::noop_waker_ref,
    Future, Stream, TryFutureExt,
};
use std::{
    ffi::CString,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use crate::{
    error::*, msg_types::*, nodes::Node, publishers::UniqueNetworkFlowRequirement, qos::QosProfile,
};
use r2r_rcl::*;

/// Rust-side buffering options for a ROS subscription stream.
///
/// These options are independent from DDS history/depth QoS. They control messages after they
/// have been taken from RCL and before the user polls the returned [`Subscription`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscriptionOptions {
    pub channel_capacity: usize,
    /// Ignore messages published by this process.
    pub ignore_local_publications: bool,
    pub unique_network_flow: UniqueNetworkFlowRequirement,
}

impl Default for SubscriptionOptions {
    fn default() -> Self {
        Self {
            channel_capacity: 10,
            ignore_local_publications: false,
            unique_network_flow: UniqueNetworkFlowRequirement::NotRequired,
        }
    }
}

impl SubscriptionOptions {
    /// Sets the shared futures-mpsc buffer size.
    ///
    /// The channel implementation additionally reserves one slot for its single sender.
    pub const fn channel_capacity(mut self, channel_capacity: usize) -> Self {
        self.channel_capacity = channel_capacity;
        self
    }

    pub const fn ignore_local_publications(mut self, ignore: bool) -> Self {
        self.ignore_local_publications = ignore;
        self
    }

    pub const fn unique_network_flow(mut self, requirement: UniqueNetworkFlowRequirement) -> Self {
        self.unique_network_flow = requirement;
        self
    }

    pub(crate) fn validate(self) -> Result<Self> {
        // Mirrors futures-channel's private MAX_BUFFER boundary so invalid public input returns
        // an error instead of reaching the assertion in mpsc::channel.
        if self.channel_capacity >= usize::MAX >> 2 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        Ok(self)
    }
}

/// Metadata reported by the RMW implementation for a received message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageInfo {
    /// Source timestamp in nanoseconds, or zero when unsupported by the RMW implementation.
    pub source_timestamp: i64,
    /// Receive timestamp in nanoseconds, or zero when unsupported by the RMW implementation.
    pub received_timestamp: i64,
    /// Sequence number assigned by the publisher, when supported.
    pub publication_sequence_number: Option<u64>,
    /// Sequence number assigned by the subscription, when supported.
    pub reception_sequence_number: Option<u64>,
    /// Publisher GID bytes. The length is RMW implementation dependent.
    pub publisher_gid: Vec<u8>,
    /// Whether the message was delivered through intra-process communication.
    pub from_intra_process: bool,
}

impl From<rmw_message_info_t> for MessageInfo {
    fn from(info: rmw_message_info_t) -> Self {
        fn sequence_number(value: u64) -> Option<u64> {
            (value != u64::MAX).then_some(value)
        }

        Self {
            source_timestamp: info.source_timestamp,
            received_timestamp: info.received_timestamp,
            publication_sequence_number: sequence_number(info.publication_sequence_number),
            reception_sequence_number: sequence_number(info.reception_sequence_number),
            publisher_gid: info.publisher_gid.data.to_vec(),
            from_intra_process: info.from_intra_process,
        }
    }
}

/// A received ROS message together with its RMW metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct ReceivedMessage<T> {
    pub message: T,
    pub info: MessageInfo,
}

impl<T> ReceivedMessage<T> {
    pub(crate) fn new(message: T, info: MessageInfo) -> Self {
        Self { message, info }
    }

    pub fn into_parts(self) -> (T, MessageInfo) {
        (self.message, self.info)
    }
}

pub(crate) fn discard_message_info<T>(message: T, _info: MessageInfo) -> T {
    message
}

pub(crate) fn attach_message_info<T>(message: T, info: MessageInfo) -> ReceivedMessage<T> {
    ReceivedMessage::new(message, info)
}

/// A ROS subscription stream with graph matching helpers.
pub struct Subscription<T> {
    receiver: mpsc::Receiver<T>,
    id: uuid::Uuid,
}

impl<T> Subscription<T> {
    pub(crate) fn new(receiver: mpsc::Receiver<T>, id: uuid::Uuid) -> Self {
        Self { receiver, id }
    }

    /// Gets the number of publishers matched to this subscription.
    pub fn get_publisher_count(&self, node: &Node) -> Result<usize> {
        node.get_subscription_publisher_count(self.id)
    }

    /// Returns the QoS profile resolved by the RMW implementation.
    pub fn get_actual_qos(&self, node: &Node) -> Result<QosProfile> {
        node.get_subscription_actual_qos(self.id)
    }

    /// Waits for at least one publisher to match this subscription.
    pub fn wait_for_publishers(&self, node: &mut Node) -> Result<impl Future<Output = Result<()>>> {
        let receiver = node.register_subscription_publisher_wait(self.id)?;
        Ok(receiver.map_err(|_| Error::RCL_RET_SUBSCRIPTION_INVALID))
    }

    /// Waits for a matched publisher up to `timeout`. The node must be spun while awaiting.
    pub fn wait_for_publishers_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        let available = self.wait_for_publishers(node)?;
        node.with_timeout(available, timeout)
    }
}

impl<T> Stream for Subscription<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}

pub(crate) struct SubscriptionMatchState {
    pub(crate) id: uuid::Uuid,
    pub(crate) waiters: Vec<oneshot::Sender<()>>,
}

impl SubscriptionMatchState {
    pub(crate) fn new(id: uuid::Uuid) -> Self {
        Self {
            id,
            waiters: Vec::new(),
        }
    }
}

pub(crate) trait Subscriber_ {
    fn handle(&self) -> &rcl_subscription_t;
    fn match_state(&self) -> Option<&SubscriptionMatchState> {
        None
    }
    fn match_state_mut(&mut self) -> Option<&mut SubscriptionMatchState> {
        None
    }
    fn publisher_count(&self) -> Result<usize> {
        let mut count = 0;
        let ret = unsafe { rcl_subscription_get_publisher_count(self.handle(), &mut count) };
        if ret == RCL_RET_OK as i32 {
            Ok(count)
        } else {
            Err(Error::from_rcl_error(ret))
        }
    }
    fn actual_qos(&self) -> Result<QosProfile> {
        let qos = unsafe { rcl_subscription_get_actual_qos(self.handle()) };
        let qos = unsafe { qos.as_ref() }.ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        QosProfile::try_from(qos)
    }
    fn poll_has_publishers(&mut self) {
        {
            let Some(state) = self.match_state_mut() else {
                return;
            };
            state.waiters.retain(|sender| !sender.is_canceled());
            if state.waiters.is_empty() {
                return;
            }
        };

        match self.publisher_count() {
            Ok(0) => {}
            Ok(_) => {
                let state = self
                    .match_state_mut()
                    .expect("subscriber match state disappeared while polling");
                while let Some(sender) = state.waiters.pop() {
                    let _ = sender.send(());
                }
            }
            Err(_) => {
                self.match_state_mut()
                    .expect("subscriber match state disappeared while polling")
                    .waiters
                    .clear();
            }
        }
    }
    /// Returns true when the subscriber should be destroyed before entering the next wait.
    fn should_remove(&mut self) -> bool {
        false
    }
    /// Returns true if the subscriber stream has been dropped.
    fn handle_incoming(&mut self) -> Result<bool>;
    fn destroy(&mut self, node: &mut rcl_node_t);
}

fn sender_ready<T>(sender: &mut mpsc::Sender<T>) -> Result<bool> {
    let mut cx = Context::from_waker(noop_waker_ref());
    match sender.poll_ready(&mut cx) {
        Poll::Ready(Ok(())) => Ok(true),
        Poll::Pending => Ok(false),
        Poll::Ready(Err(_)) => Err(Error::RCL_RET_SUBSCRIPTION_INVALID),
    }
}

pub struct TypedSubscriber<T, O>
where
    T: WrappedTypesupport,
{
    pub rcl_handle: rcl_subscription_t,
    pub sender: mpsc::Sender<O>,
    pub transform: fn(T, MessageInfo) -> O,
    pub(crate) match_state: SubscriptionMatchState,
}

pub struct NativeSubscriber<T, O>
where
    T: WrappedTypesupport,
{
    pub rcl_handle: rcl_subscription_t,
    pub sender: mpsc::Sender<O>,
    pub transform: fn(WrappedNativeMsg<T>, MessageInfo) -> O,
    pub(crate) match_state: SubscriptionMatchState,
}

pub struct UntypedSubscriber<O> {
    pub rcl_handle: rcl_subscription_t,
    pub topic_type: String,
    pub sender: mpsc::Sender<O>,
    pub transform: fn(Result<serde_json::Value>, MessageInfo) -> O,
    pub(crate) match_state: SubscriptionMatchState,
}

pub struct RawSubscriber<O> {
    pub rcl_handle: rcl_subscription_t,
    pub msg_buf: rcl_serialized_message_t,
    pub sender: mpsc::Sender<O>,
    pub transform: fn(Vec<u8>, MessageInfo) -> O,
    pub(crate) match_state: SubscriptionMatchState,
}

impl<T: 'static, O: 'static> Subscriber_ for TypedSubscriber<T, O>
where
    T: WrappedTypesupport,
{
    fn handle(&self) -> &rcl_subscription_t {
        &self.rcl_handle
    }

    fn match_state(&self) -> Option<&SubscriptionMatchState> {
        Some(&self.match_state)
    }

    fn match_state_mut(&mut self) -> Option<&mut SubscriptionMatchState> {
        Some(&mut self.match_state)
    }

    fn should_remove(&mut self) -> bool {
        self.sender.is_closed()
    }

    fn handle_incoming(&mut self) -> Result<bool> {
        match sender_ready(&mut self.sender) {
            Ok(true) => {}
            Ok(false) => return Ok(false),
            Err(_) => return Ok(true),
        }
        let mut msg_info = rmw_message_info_t::default(); // we dont care for now
        let mut msg = WrappedNativeMsg::<T>::try_new()?;
        let ret = unsafe {
            rcl_take(&self.rcl_handle, msg.void_ptr_mut(), &mut msg_info, std::ptr::null_mut())
        };
        if ret == RCL_RET_OK as i32 {
            let msg = (self.transform)(T::from_native(&msg), msg_info.into());
            if let Err(e) = self.sender.try_send(msg) {
                if e.is_disconnected() {
                    // user dropped the handle to the stream, signal removal.
                    return Ok(true);
                }
                log::debug!("error {:?}", e)
            }
        } else if ret != RCL_RET_SUBSCRIPTION_TAKE_FAILED as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        Ok(false)
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_subscription_fini(&mut self.rcl_handle, node);
        }
    }
}

impl<T: 'static, O: 'static> Subscriber_ for NativeSubscriber<T, O>
where
    T: WrappedTypesupport,
{
    fn handle(&self) -> &rcl_subscription_t {
        &self.rcl_handle
    }

    fn match_state(&self) -> Option<&SubscriptionMatchState> {
        Some(&self.match_state)
    }

    fn match_state_mut(&mut self) -> Option<&mut SubscriptionMatchState> {
        Some(&mut self.match_state)
    }

    fn should_remove(&mut self) -> bool {
        self.sender.is_closed()
    }

    fn handle_incoming(&mut self) -> Result<bool> {
        match sender_ready(&mut self.sender) {
            Ok(true) => {}
            Ok(false) => return Ok(false),
            Err(_) => return Ok(true),
        }
        // The returned stream may outlive the Node. Use caller-owned message storage so
        // dropping an item never has to access an already-finalized subscription handle.
        let mut msg_info = rmw_message_info_t::default();
        let mut msg = WrappedNativeMsg::<T>::try_new()?;
        let ret = unsafe {
            rcl_take(&self.rcl_handle, msg.void_ptr_mut(), &mut msg_info, std::ptr::null_mut())
        };
        if ret == RCL_RET_SUBSCRIPTION_TAKE_FAILED as i32 {
            return Ok(false);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        let msg = (self.transform)(msg, msg_info.into());
        if let Err(e) = self.sender.try_send(msg) {
            if e.is_disconnected() {
                // user dropped the handle to the stream, signal removal.
                return Ok(true);
            }
            log::error!("error {:?}", e)
        }
        Ok(false)
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_subscription_fini(&mut self.rcl_handle, node);
        }
    }
}

impl<O: 'static> Subscriber_ for UntypedSubscriber<O> {
    fn handle(&self) -> &rcl_subscription_t {
        &self.rcl_handle
    }

    fn match_state(&self) -> Option<&SubscriptionMatchState> {
        Some(&self.match_state)
    }

    fn match_state_mut(&mut self) -> Option<&mut SubscriptionMatchState> {
        Some(&mut self.match_state)
    }

    fn should_remove(&mut self) -> bool {
        self.sender.is_closed()
    }

    fn handle_incoming(&mut self) -> Result<bool> {
        match sender_ready(&mut self.sender) {
            Ok(true) => {}
            Ok(false) => return Ok(false),
            Err(_) => return Ok(true),
        }
        let mut msg_info = rmw_message_info_t::default(); // we dont care for now
        let mut msg = WrappedNativeMsgUntyped::new_from(&self.topic_type)?;
        let ret = unsafe {
            rcl_take(&self.rcl_handle, msg.void_ptr_mut(), &mut msg_info, std::ptr::null_mut())
        };
        if ret == RCL_RET_OK as i32 {
            let json = (self.transform)(msg.to_json(), msg_info.into());
            if let Err(e) = self.sender.try_send(json) {
                if e.is_disconnected() {
                    // user dropped the handle to the stream, signal removal.
                    return Ok(true);
                }
                log::debug!("error {:?}", e)
            }
        } else if ret != RCL_RET_SUBSCRIPTION_TAKE_FAILED as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        Ok(false)
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_subscription_fini(&mut self.rcl_handle, node);
        }
    }
}

impl<O: 'static> Subscriber_ for RawSubscriber<O> {
    fn handle(&self) -> &rcl_subscription_t {
        &self.rcl_handle
    }

    fn match_state(&self) -> Option<&SubscriptionMatchState> {
        Some(&self.match_state)
    }

    fn match_state_mut(&mut self) -> Option<&mut SubscriptionMatchState> {
        Some(&mut self.match_state)
    }

    fn should_remove(&mut self) -> bool {
        self.sender.is_closed()
    }

    fn handle_incoming(&mut self) -> Result<bool> {
        match sender_ready(&mut self.sender) {
            Ok(true) => {}
            Ok(false) => return Ok(false),
            Err(_) => return Ok(true),
        }
        let mut msg_info = rmw_message_info_t::default(); // we dont care for now
        let ret = unsafe {
            rcl_take_serialized_message(
                &self.rcl_handle,
                &mut self.msg_buf as *mut rcl_serialized_message_t,
                &mut msg_info,
                std::ptr::null_mut(),
            )
        };
        if ret == RCL_RET_SUBSCRIPTION_TAKE_FAILED as i32 {
            return Ok(false);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let data_bytes = unsafe { serialized_message_bytes(&self.msg_buf) }?.to_vec();

        let message = (self.transform)(data_bytes, msg_info.into());
        if let Err(e) = self.sender.try_send(message) {
            if e.is_disconnected() {
                // user dropped the handle to the stream, signal removal.
                return Ok(true);
            }
            log::debug!("error {:?}", e)
        }

        Ok(false)
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_subscription_fini(&mut self.rcl_handle, node);
            rcutils_uint8_array_fini(&mut self.msg_buf as *mut rcl_serialized_message_t);
        }
    }
}

pub(crate) fn create_subscription_helper(
    node: &mut rcl_node_t, topic: &str, ts: *const rosidl_message_type_support_t,
    qos_profile: QosProfile, ignore_local_publications: bool,
    unique_network_flow: UniqueNetworkFlowRequirement,
) -> Result<rcl_subscription_t> {
    let mut subscription_handle = unsafe { rcl_get_zero_initialized_subscription() };
    let topic_c_string = CString::new(topic).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;

    let result = unsafe {
        let mut subscription_options = rcl_subscription_get_default_options();
        subscription_options.qos = qos_profile.into();
        subscription_options
            .rmw_subscription_options
            .ignore_local_publications = ignore_local_publications;
        subscription_options
            .rmw_subscription_options
            .require_unique_network_flow_endpoints = unique_network_flow.into();
        rcl_subscription_init(
            &mut subscription_handle,
            node,
            ts,
            topic_c_string.as_ptr(),
            &subscription_options,
        )
    };
    if result == RCL_RET_OK as i32 {
        Ok(subscription_handle)
    } else {
        Err(Error::from_rcl_error(result))
    }
}
