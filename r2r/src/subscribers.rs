use futures::{
    channel::{mpsc, oneshot},
    task::noop_waker_ref,
    Future, Stream, TryFutureExt,
};
use std::{
    any::TypeId,
    ffi::{CStr, CString},
    marker::PhantomData,
    mem::MaybeUninit,
    pin::Pin,
    ptr::NonNull,
    rc::Rc,
    task::{Context, Poll},
    time::Duration,
};

use crate::{
    error::*,
    msg_types::*,
    nodes::Node,
    publishers::{
        subscription_network_flow_endpoints, NetworkFlowEndpoint, UniqueNetworkFlowRequirement,
    },
    qos::QosProfile,
    QosPolicyKind,
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

/// Middleware-side filter applied before samples enter a subscription.
///
/// The expression follows the SQL-like syntax supported by the selected RMW implementation.
/// Placeholders such as `%0` refer to entries in [`Self::expression_parameters`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentFilterOptions {
    pub filter_expression: String,
    pub expression_parameters: Vec<String>,
}

/// Cumulative status from a requested-incompatible-QoS subscription event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestedIncompatibleQosStatus {
    pub total_count: i32,
    pub total_count_change: i32,
    pub last_policy_kind: QosPolicyKind,
}

/// Cumulative status from a requested-deadline-missed subscription event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestedDeadlineMissedStatus {
    pub total_count: i32,
    pub total_count_change: i32,
}

/// Current publisher liveliness counts reported by a subscription event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LivelinessChangedStatus {
    pub alive_count: i32,
    pub not_alive_count: i32,
    pub alive_count_change: i32,
    pub not_alive_count_change: i32,
}

/// Cumulative status from a subscription message-lost event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageLostStatus {
    pub total_count: usize,
    pub total_count_change: usize,
}

impl ContentFilterOptions {
    pub fn new(filter_expression: impl Into<String>) -> Self {
        Self {
            filter_expression: filter_expression.into(),
            expression_parameters: Vec::new(),
        }
    }

    pub fn expression_parameters<I, S>(mut self, parameters: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.expression_parameters = parameters.into_iter().map(Into::into).collect();
        self
    }
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

const MAX_SUBSCRIPTION_BATCH_SIZE: usize = 16;

pub(crate) fn subscription_batch_size(channel_capacity: usize) -> usize {
    channel_capacity
        .saturating_add(1)
        .min(MAX_SUBSCRIPTION_BATCH_SIZE)
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

/// A message borrowed directly from the subscription middleware.
///
/// The message is returned to the middleware when this value is dropped. While it exists, the
/// owning [`Node`] remains mutably borrowed and therefore cannot be spun or destroyed.
pub struct LoanedSubscriptionMessage<'node, T>
where
    T: WrappedTypesupport,
{
    msg: NonNull<T::CStruct>,
    subscription: &'node rcl_subscription_t,
    pub info: MessageInfo,
    return_message:
        unsafe extern "C" fn(*const rcl_subscription_t, *mut std::os::raw::c_void) -> rcl_ret_t,
    _node: PhantomData<&'node mut Node>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<T> std::ops::Deref for LoanedSubscriptionMessage<'_, T>
where
    T: WrappedTypesupport,
{
    type Target = T::CStruct;

    fn deref(&self) -> &Self::Target {
        unsafe { self.msg.as_ref() }
    }
}

impl<T> Drop for LoanedSubscriptionMessage<'_, T>
where
    T: WrappedTypesupport,
{
    fn drop(&mut self) {
        let ret = unsafe {
            (self.return_message)(
                self.subscription,
                self.msg.as_ptr().cast::<std::os::raw::c_void>(),
            )
        };
        if ret != RCL_RET_OK as i32 {
            log::error!(
                "could not return subscription loaned message: {}",
                Error::from_rcl_error(ret)
            );
        }
    }
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

    /// Returns the fully resolved topic name used by the native subscription.
    pub fn topic_name(&self, node: &Node) -> Result<String> {
        node.get_subscription_topic_name(self.id)
    }

    /// Returns whether the middleware can loan messages for this subscription and message type.
    pub fn can_loan_messages(&self, node: &Node) -> Result<bool> {
        node.get_subscription_can_loan_messages(self.id)
    }

    /// Returns the network flows used to receive messages, when supported by the RMW.
    pub fn get_network_flow_endpoints(&self, node: &Node) -> Result<Vec<NetworkFlowEndpoint>> {
        node.get_subscription_network_flow_endpoints(self.id)
    }

    /// Returns whether middleware-side content filtering is enabled for this subscription.
    pub fn is_content_filter_enabled(&self, node: &Node) -> Result<bool> {
        node.get_subscription_content_filter_enabled(self.id)
    }

    /// Replaces the middleware-side content filter for this subscription.
    ///
    /// An empty expression clears the filter. The selected RMW may return
    /// [`Error::RCL_RET_UNSUPPORTED`].
    pub fn set_content_filter(
        &self, node: &mut Node, options: &ContentFilterOptions,
    ) -> Result<()> {
        node.set_subscription_content_filter(self.id, options)
    }

    /// Clears the middleware-side content filter for this subscription.
    pub fn clear_content_filter(&self, node: &mut Node) -> Result<()> {
        self.set_content_filter(node, &ContentFilterOptions::default())
    }

    /// Returns the middleware-side content filter currently configured on this subscription.
    pub fn get_content_filter(&self, node: &Node) -> Result<ContentFilterOptions> {
        node.get_subscription_content_filter(self.id)
    }

    /// Takes the latest requested-incompatible-QoS status without blocking.
    ///
    /// The first call initializes the native event. Call this once before creating potentially
    /// incompatible publishers when every discovery event must be observed.
    pub fn take_requested_incompatible_qos(
        &self, node: &mut Node,
    ) -> Result<Option<RequestedIncompatibleQosStatus>> {
        node.take_subscription_requested_incompatible_qos(self.id)
    }

    /// Takes the latest requested-deadline-missed status without blocking.
    pub fn take_requested_deadline_missed(
        &self, node: &mut Node,
    ) -> Result<Option<RequestedDeadlineMissedStatus>> {
        node.take_subscription_requested_deadline_missed(self.id)
    }

    /// Takes the latest liveliness-changed status without blocking.
    pub fn take_liveliness_changed(
        &self, node: &mut Node,
    ) -> Result<Option<LivelinessChangedStatus>> {
        node.take_subscription_liveliness_changed(self.id)
    }

    /// Takes the latest message-lost status without blocking.
    pub fn take_message_lost(&self, node: &mut Node) -> Result<Option<MessageLostStatus>> {
        node.take_subscription_message_lost(self.id)
    }

    /// Tries to take a message loaned directly by the middleware without copying it.
    ///
    /// This is a non-blocking operation and returns `Ok(None)` when no message is available. It
    /// returns [`Error::RCL_RET_UNSUPPORTED`] when the selected RMW or message type does not
    /// support subscription loaning. The node cannot be spun again until the returned message is
    /// dropped.
    pub fn take_loaned_message<'node>(
        &self, node: &'node mut Node,
    ) -> Result<Option<LoanedSubscriptionMessage<'node, T>>>
    where
        T: WrappedTypesupport,
    {
        let subscriber = node.subscriber_mut(self.id)?;
        if subscriber.message_type_id() != Some(TypeId::of::<T>()) {
            return Err(Error::RCL_RET_SUBSCRIPTION_INVALID);
        }
        if !subscriber.can_loan_messages() {
            return Err(Error::RCL_RET_UNSUPPORTED);
        }

        let mut msg = std::ptr::null_mut();
        let mut info = rmw_message_info_t::default();
        let ret = unsafe {
            rcl_take_loaned_message(subscriber.handle(), &mut msg, &mut info, std::ptr::null_mut())
        };
        if ret == RCL_RET_SUBSCRIPTION_TAKE_FAILED as i32 {
            return Ok(None);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        let msg = NonNull::new(msg.cast::<T::CStruct>()).ok_or(Error::RCL_RET_ERROR)?;
        Ok(Some(LoanedSubscriptionMessage {
            msg,
            subscription: subscriber.handle(),
            info: info.into(),
            return_message: rcl_return_loaned_message_from_subscription,
            _node: PhantomData,
            _not_send_or_sync: PhantomData,
        }))
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
    requested_incompatible_qos: Option<SubscriptionEvent>,
    requested_deadline_missed: Option<SubscriptionEvent>,
    liveliness_changed: Option<SubscriptionEvent>,
    message_lost: Option<SubscriptionEvent>,
}

impl SubscriptionMatchState {
    pub(crate) fn new(id: uuid::Uuid) -> Self {
        Self {
            id,
            waiters: Vec::new(),
            requested_incompatible_qos: None,
            requested_deadline_missed: None,
            liveliness_changed: None,
            message_lost: None,
        }
    }

    fn take_requested_incompatible_qos(
        &mut self, subscription: *const rcl_subscription_t,
    ) -> Result<Option<RequestedIncompatibleQosStatus>> {
        let event = self.requested_incompatible_qos.get_or_insert_with(|| {
            SubscriptionEvent::uninitialized(
                rcl_subscription_event_type_t::RCL_SUBSCRIPTION_REQUESTED_INCOMPATIBLE_QOS,
            )
        });
        event.ensure_initialized(subscription)?;
        event.take_requested_incompatible_qos()
    }

    fn take_requested_deadline_missed(
        &mut self, subscription: *const rcl_subscription_t,
    ) -> Result<Option<RequestedDeadlineMissedStatus>> {
        let event = self.requested_deadline_missed.get_or_insert_with(|| {
            SubscriptionEvent::uninitialized(
                rcl_subscription_event_type_t::RCL_SUBSCRIPTION_REQUESTED_DEADLINE_MISSED,
            )
        });
        event.ensure_initialized(subscription)?;
        event.take_requested_deadline_missed()
    }

    fn take_liveliness_changed(
        &mut self, subscription: *const rcl_subscription_t,
    ) -> Result<Option<LivelinessChangedStatus>> {
        let event = self.liveliness_changed.get_or_insert_with(|| {
            SubscriptionEvent::uninitialized(
                rcl_subscription_event_type_t::RCL_SUBSCRIPTION_LIVELINESS_CHANGED,
            )
        });
        event.ensure_initialized(subscription)?;
        event.take_liveliness_changed()
    }

    fn take_message_lost(
        &mut self, subscription: *const rcl_subscription_t,
    ) -> Result<Option<MessageLostStatus>> {
        let event = self.message_lost.get_or_insert_with(|| {
            SubscriptionEvent::uninitialized(
                rcl_subscription_event_type_t::RCL_SUBSCRIPTION_MESSAGE_LOST,
            )
        });
        event.ensure_initialized(subscription)?;
        event.take_message_lost()
    }

    fn clear_events(&mut self) {
        self.requested_incompatible_qos = None;
        self.requested_deadline_missed = None;
        self.liveliness_changed = None;
        self.message_lost = None;
    }
}

struct SubscriptionEvent {
    handle: rcl_event_t,
    event_type: rcl_subscription_event_type_t,
    initialized: bool,
}

impl SubscriptionEvent {
    fn uninitialized(event_type: rcl_subscription_event_type_t) -> Self {
        Self {
            handle: unsafe { rcl_get_zero_initialized_event() },
            event_type,
            initialized: false,
        }
    }

    fn ensure_initialized(&mut self, subscription: *const rcl_subscription_t) -> Result<()> {
        if self.initialized {
            return Ok(());
        }
        let ret =
            unsafe { rcl_subscription_event_init(&mut self.handle, subscription, self.event_type) };
        check_subscription_ret(ret)?;
        self.initialized = true;
        Ok(())
    }

    fn take_requested_incompatible_qos(&self) -> Result<Option<RequestedIncompatibleQosStatus>> {
        let mut status = MaybeUninit::<rmw_requested_qos_incompatible_event_status_t>::uninit();
        let ret = unsafe { rcl_take_event(&self.handle, status.as_mut_ptr().cast()) };
        if ret == RCL_RET_EVENT_TAKE_FAILED as i32 {
            return Ok(None);
        }
        check_subscription_ret(ret)?;
        let status = unsafe { status.assume_init() };
        if status.total_count_change == 0 {
            return Ok(None);
        }
        Ok(Some(RequestedIncompatibleQosStatus {
            total_count: status.total_count,
            total_count_change: status.total_count_change,
            last_policy_kind: status.last_policy_kind.into(),
        }))
    }

    fn take_requested_deadline_missed(&self) -> Result<Option<RequestedDeadlineMissedStatus>> {
        let mut status = MaybeUninit::<rmw_requested_deadline_missed_status_t>::uninit();
        let ret = unsafe { rcl_take_event(&self.handle, status.as_mut_ptr().cast()) };
        if ret == RCL_RET_EVENT_TAKE_FAILED as i32 {
            return Ok(None);
        }
        check_subscription_ret(ret)?;
        let status = unsafe { status.assume_init() };
        if status.total_count_change == 0 {
            return Ok(None);
        }
        Ok(Some(RequestedDeadlineMissedStatus {
            total_count: status.total_count,
            total_count_change: status.total_count_change,
        }))
    }

    fn take_liveliness_changed(&self) -> Result<Option<LivelinessChangedStatus>> {
        let mut status = MaybeUninit::<rmw_liveliness_changed_status_t>::uninit();
        let ret = unsafe { rcl_take_event(&self.handle, status.as_mut_ptr().cast()) };
        if ret == RCL_RET_EVENT_TAKE_FAILED as i32 {
            return Ok(None);
        }
        check_subscription_ret(ret)?;
        let status = unsafe { status.assume_init() };
        if status.alive_count_change == 0 && status.not_alive_count_change == 0 {
            return Ok(None);
        }
        Ok(Some(LivelinessChangedStatus {
            alive_count: status.alive_count,
            not_alive_count: status.not_alive_count,
            alive_count_change: status.alive_count_change,
            not_alive_count_change: status.not_alive_count_change,
        }))
    }

    fn take_message_lost(&self) -> Result<Option<MessageLostStatus>> {
        let mut status = MaybeUninit::<rmw_message_lost_status_t>::uninit();
        let ret = unsafe { rcl_take_event(&self.handle, status.as_mut_ptr().cast()) };
        if ret == RCL_RET_EVENT_TAKE_FAILED as i32 {
            return Ok(None);
        }
        check_subscription_ret(ret)?;
        let status = unsafe { status.assume_init() };
        if status.total_count_change == 0 {
            return Ok(None);
        }
        Ok(Some(MessageLostStatus {
            total_count: status.total_count,
            total_count_change: status.total_count_change,
        }))
    }
}

impl Drop for SubscriptionEvent {
    fn drop(&mut self) {
        if !self.initialized {
            return;
        }
        let ret = unsafe { rcl_event_fini(&mut self.handle) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize subscription event: {ret}");
        }
    }
}

pub(crate) trait Subscriber_ {
    fn handle(&self) -> &rcl_subscription_t;
    fn message_type_id(&self) -> Option<TypeId> {
        None
    }
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
    fn topic_name(&self) -> Result<String> {
        let name = unsafe { rcl_subscription_get_topic_name(self.handle()) };
        if name.is_null() {
            return Err(Error::RCL_RET_SUBSCRIPTION_INVALID);
        }
        unsafe { CStr::from_ptr(name) }
            .to_str()
            .map(str::to_owned)
            .map_err(|err| Error::InvalidUtf8 {
                context: "subscription topic name",
                err: err.to_string(),
            })
    }
    fn can_loan_messages(&self) -> bool {
        unsafe { rcl_subscription_can_loan_messages(self.handle()) }
    }
    fn network_flow_endpoints(&self) -> Result<Vec<NetworkFlowEndpoint>> {
        subscription_network_flow_endpoints(self.handle())
    }
    fn is_content_filter_enabled(&self) -> bool {
        #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
        {
            unsafe { rcl_subscription_is_cft_enabled(self.handle()) }
        }
        #[cfg(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic))]
        {
            false
        }
    }
    fn set_content_filter(&self, options: &ContentFilterOptions) -> Result<()> {
        #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
        {
            let native = NativeContentFilterOptions::from_options(self.handle(), options)?;
            let ret = unsafe { rcl_subscription_set_content_filter(self.handle(), &native.value) };
            check_subscription_ret(ret)
        }
        #[cfg(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic))]
        {
            let _ = options;
            Err(Error::RCL_RET_UNSUPPORTED)
        }
    }
    fn get_content_filter(&self) -> Result<ContentFilterOptions> {
        #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
        {
            NativeContentFilterOptions::get(self.handle())?.to_options()
        }
        #[cfg(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic))]
        {
            Err(Error::RCL_RET_UNSUPPORTED)
        }
    }
    fn take_requested_incompatible_qos(
        &mut self,
    ) -> Result<Option<RequestedIncompatibleQosStatus>> {
        let subscription = self.handle() as *const rcl_subscription_t;
        let state = self
            .match_state_mut()
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        state.take_requested_incompatible_qos(subscription)
    }
    fn take_requested_deadline_missed(&mut self) -> Result<Option<RequestedDeadlineMissedStatus>> {
        let subscription = self.handle() as *const rcl_subscription_t;
        let state = self
            .match_state_mut()
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        state.take_requested_deadline_missed(subscription)
    }
    fn take_liveliness_changed(&mut self) -> Result<Option<LivelinessChangedStatus>> {
        let subscription = self.handle() as *const rcl_subscription_t;
        let state = self
            .match_state_mut()
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        state.take_liveliness_changed(subscription)
    }
    fn take_message_lost(&mut self) -> Result<Option<MessageLostStatus>> {
        let subscription = self.handle() as *const rcl_subscription_t;
        let state = self
            .match_state_mut()
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        state.take_message_lost(subscription)
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
                let Some(state) = self.match_state_mut() else {
                    return;
                };
                while let Some(sender) = state.waiters.pop() {
                    let _ = sender.send(());
                }
            }
            Err(_) => {
                if let Some(state) = self.match_state_mut() {
                    state.waiters.clear();
                }
            }
        }
    }
    /// Returns true when the subscriber should be destroyed before entering the next wait.
    fn should_remove(&mut self) -> bool {
        false
    }
    /// Returns true if the subscriber stream has been dropped.
    fn max_messages_per_spin(&self) -> usize {
        1
    }
    fn handle_incoming(&mut self) -> Result<SubscriptionHandleResult>;
    fn handle_ready(&mut self) -> Result<bool> {
        for _ in 0..self.max_messages_per_spin() {
            match self.handle_incoming()? {
                SubscriptionHandleResult::Taken => {}
                SubscriptionHandleResult::Idle | SubscriptionHandleResult::Backpressured => {
                    return Ok(false);
                }
                SubscriptionHandleResult::Remove => return Ok(true),
            }
        }
        Ok(false)
    }
    fn destroy(&mut self, node: &mut rcl_node_t);
}

#[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
struct NativeContentFilterOptions<'subscription> {
    subscription: &'subscription rcl_subscription_t,
    value: rcl_subscription_content_filter_options_t,
    initialized: bool,
}

#[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
impl<'subscription> NativeContentFilterOptions<'subscription> {
    fn from_options(
        subscription: &'subscription rcl_subscription_t, options: &ContentFilterOptions,
    ) -> Result<Self> {
        if options.expression_parameters.len() > 100 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let expression = CString::new(options.filter_expression.as_str())
            .map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let parameters = options
            .expression_parameters
            .iter()
            .map(|value| CString::new(value.as_str()).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT))
            .collect::<Result<Vec<_>>>()?;
        let mut parameter_ptrs = parameters
            .iter()
            .map(|value| value.as_ptr())
            .collect::<Vec<_>>();
        let parameter_ptrs = if parameter_ptrs.is_empty() {
            std::ptr::null_mut()
        } else {
            parameter_ptrs.as_mut_ptr()
        };
        let mut native = Self {
            subscription,
            value: unsafe { rcl_get_zero_initialized_subscription_content_filter_options() },
            initialized: false,
        };
        let ret = unsafe {
            rcl_subscription_content_filter_options_init(
                subscription,
                expression.as_ptr(),
                parameters.len(),
                parameter_ptrs,
                &mut native.value,
            )
        };
        check_subscription_ret(ret)?;
        native.initialized = true;
        Ok(native)
    }

    fn get(subscription: &'subscription rcl_subscription_t) -> Result<Self> {
        let mut native = Self {
            subscription,
            value: unsafe { rcl_get_zero_initialized_subscription_content_filter_options() },
            initialized: false,
        };
        let ret = unsafe { rcl_subscription_get_content_filter(subscription, &mut native.value) };
        check_subscription_ret(ret)?;
        native.initialized = true;
        Ok(native)
    }

    fn to_options(&self) -> Result<ContentFilterOptions> {
        let value = &self.value.rmw_subscription_content_filter_options;
        if value.filter_expression.is_null() {
            return Err(Error::RCL_RET_ERROR);
        }
        let filter_expression = unsafe { CStr::from_ptr(value.filter_expression) }
            .to_str()
            .map(str::to_owned)
            .map_err(|err| Error::InvalidUtf8 {
                context: "subscription content filter expression",
                err: err.to_string(),
            })?;
        let expression_parameters = if value.expression_parameters.size == 0 {
            Vec::new()
        } else {
            if value.expression_parameters.data.is_null() {
                return Err(Error::RCL_RET_ERROR);
            }
            unsafe {
                std::slice::from_raw_parts(
                    value.expression_parameters.data,
                    value.expression_parameters.size,
                )
            }
            .iter()
            .map(|value| {
                if value.is_null() {
                    return Err(Error::RCL_RET_ERROR);
                }
                unsafe { CStr::from_ptr(*value) }
                    .to_str()
                    .map(str::to_owned)
                    .map_err(|err| Error::InvalidUtf8 {
                        context: "subscription content filter parameter",
                        err: err.to_string(),
                    })
            })
            .collect::<Result<Vec<_>>>()?
        };
        Ok(ContentFilterOptions {
            filter_expression,
            expression_parameters,
        })
    }
}

#[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
impl Drop for NativeContentFilterOptions<'_> {
    fn drop(&mut self) {
        if !self.initialized {
            return;
        }
        let ret = unsafe {
            rcl_subscription_content_filter_options_fini(self.subscription, &mut self.value)
        };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize subscription content filter options: {ret}");
        }
    }
}

fn check_subscription_ret(ret: rcl_ret_t) -> Result<()> {
    if ret == RCL_RET_OK as i32 {
        Ok(())
    } else {
        Err(Error::from_rcl_error(ret))
    }
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
    pub max_messages_per_spin: usize,
    pub(crate) match_state: SubscriptionMatchState,
}

pub struct NativeSubscriber<T, O>
where
    T: WrappedTypesupport,
{
    pub rcl_handle: rcl_subscription_t,
    pub sender: mpsc::Sender<O>,
    pub transform: fn(WrappedNativeMsg<T>, MessageInfo) -> O,
    pub max_messages_per_spin: usize,
    pub(crate) match_state: SubscriptionMatchState,
}

pub struct UntypedSubscriber<O> {
    pub rcl_handle: rcl_subscription_t,
    pub topic_type: String,
    pub sender: mpsc::Sender<O>,
    pub transform: fn(Result<serde_json::Value>, MessageInfo) -> O,
    pub max_messages_per_spin: usize,
    pub(crate) match_state: SubscriptionMatchState,
}

pub struct RawSubscriber<O> {
    pub rcl_handle: rcl_subscription_t,
    pub msg_buf: rcl_serialized_message_t,
    pub sender: mpsc::Sender<O>,
    pub transform: fn(Vec<u8>, MessageInfo) -> O,
    pub max_messages_per_spin: usize,
    pub(crate) match_state: SubscriptionMatchState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubscriptionHandleResult {
    Taken,
    Idle,
    Backpressured,
    Remove,
}

impl<T: 'static, O: 'static> Subscriber_ for TypedSubscriber<T, O>
where
    T: WrappedTypesupport,
{
    fn handle(&self) -> &rcl_subscription_t {
        &self.rcl_handle
    }

    fn message_type_id(&self) -> Option<TypeId> {
        Some(TypeId::of::<T>())
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

    fn max_messages_per_spin(&self) -> usize {
        self.max_messages_per_spin
    }

    fn handle_incoming(&mut self) -> Result<SubscriptionHandleResult> {
        match sender_ready(&mut self.sender) {
            Ok(true) => {}
            Ok(false) => return Ok(SubscriptionHandleResult::Backpressured),
            Err(_) => return Ok(SubscriptionHandleResult::Remove),
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
                    return Ok(SubscriptionHandleResult::Remove);
                }
                log::debug!("error {:?}", e);
                return Ok(SubscriptionHandleResult::Backpressured);
            }
        } else if ret != RCL_RET_SUBSCRIPTION_TAKE_FAILED as i32 {
            return Err(Error::from_rcl_error(ret));
        } else {
            return Ok(SubscriptionHandleResult::Idle);
        }
        Ok(SubscriptionHandleResult::Taken)
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        self.match_state.clear_events();
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

    fn message_type_id(&self) -> Option<TypeId> {
        Some(TypeId::of::<T>())
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

    fn max_messages_per_spin(&self) -> usize {
        self.max_messages_per_spin
    }

    fn handle_incoming(&mut self) -> Result<SubscriptionHandleResult> {
        match sender_ready(&mut self.sender) {
            Ok(true) => {}
            Ok(false) => return Ok(SubscriptionHandleResult::Backpressured),
            Err(_) => return Ok(SubscriptionHandleResult::Remove),
        }
        // The returned stream may outlive the Node. Use caller-owned message storage so
        // dropping an item never has to access an already-finalized subscription handle.
        let mut msg_info = rmw_message_info_t::default();
        let mut msg = WrappedNativeMsg::<T>::try_new()?;
        let ret = unsafe {
            rcl_take(&self.rcl_handle, msg.void_ptr_mut(), &mut msg_info, std::ptr::null_mut())
        };
        if ret == RCL_RET_SUBSCRIPTION_TAKE_FAILED as i32 {
            return Ok(SubscriptionHandleResult::Idle);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        let msg = (self.transform)(msg, msg_info.into());
        if let Err(e) = self.sender.try_send(msg) {
            if e.is_disconnected() {
                // user dropped the handle to the stream, signal removal.
                return Ok(SubscriptionHandleResult::Remove);
            }
            log::error!("error {:?}", e);
            return Ok(SubscriptionHandleResult::Backpressured);
        }
        Ok(SubscriptionHandleResult::Taken)
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        self.match_state.clear_events();
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

    fn max_messages_per_spin(&self) -> usize {
        self.max_messages_per_spin
    }

    fn handle_incoming(&mut self) -> Result<SubscriptionHandleResult> {
        match sender_ready(&mut self.sender) {
            Ok(true) => {}
            Ok(false) => return Ok(SubscriptionHandleResult::Backpressured),
            Err(_) => return Ok(SubscriptionHandleResult::Remove),
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
                    return Ok(SubscriptionHandleResult::Remove);
                }
                log::debug!("error {:?}", e);
                return Ok(SubscriptionHandleResult::Backpressured);
            }
        } else if ret != RCL_RET_SUBSCRIPTION_TAKE_FAILED as i32 {
            return Err(Error::from_rcl_error(ret));
        } else {
            return Ok(SubscriptionHandleResult::Idle);
        }
        Ok(SubscriptionHandleResult::Taken)
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        self.match_state.clear_events();
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

    fn max_messages_per_spin(&self) -> usize {
        self.max_messages_per_spin
    }

    fn handle_incoming(&mut self) -> Result<SubscriptionHandleResult> {
        match sender_ready(&mut self.sender) {
            Ok(true) => {}
            Ok(false) => return Ok(SubscriptionHandleResult::Backpressured),
            Err(_) => return Ok(SubscriptionHandleResult::Remove),
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
            return Ok(SubscriptionHandleResult::Idle);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let data_bytes = unsafe { serialized_message_bytes(&self.msg_buf) }?.to_vec();

        let message = (self.transform)(data_bytes, msg_info.into());
        if let Err(e) = self.sender.try_send(message) {
            if e.is_disconnected() {
                // user dropped the handle to the stream, signal removal.
                return Ok(SubscriptionHandleResult::Remove);
            }
            log::debug!("error {:?}", e);
            return Ok(SubscriptionHandleResult::Backpressured);
        }

        Ok(SubscriptionHandleResult::Taken)
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        self.match_state.clear_events();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        cell::Cell,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static RETURNED_LOANS: AtomicUsize = AtomicUsize::new(0);

    struct BatchedTestSubscriber {
        handle: rcl_subscription_t,
        outcomes: std::collections::VecDeque<SubscriptionHandleResult>,
        max_messages_per_spin: usize,
        calls: usize,
    }

    impl Subscriber_ for BatchedTestSubscriber {
        fn handle(&self) -> &rcl_subscription_t {
            &self.handle
        }

        fn max_messages_per_spin(&self) -> usize {
            self.max_messages_per_spin
        }

        fn handle_incoming(&mut self) -> Result<SubscriptionHandleResult> {
            self.calls += 1;
            Ok(self
                .outcomes
                .pop_front()
                .unwrap_or(SubscriptionHandleResult::Idle))
        }

        fn destroy(&mut self, _node: &mut rcl_node_t) {}
    }

    #[test]
    fn ready_subscription_drains_a_bounded_batch() {
        let mut subscriber = BatchedTestSubscriber {
            handle: unsafe { rcl_get_zero_initialized_subscription() },
            outcomes: [
                SubscriptionHandleResult::Taken,
                SubscriptionHandleResult::Taken,
                SubscriptionHandleResult::Taken,
                SubscriptionHandleResult::Taken,
            ]
            .into(),
            max_messages_per_spin: 3,
            calls: 0,
        };

        assert!(!subscriber.handle_ready().unwrap());
        assert_eq!(subscriber.calls, 3);
        assert_eq!(subscriber.outcomes.len(), 1);
    }

    #[test]
    fn ready_subscription_stops_on_idle_backpressure_and_removal() {
        for terminal in [
            SubscriptionHandleResult::Idle,
            SubscriptionHandleResult::Backpressured,
            SubscriptionHandleResult::Remove,
        ] {
            let mut subscriber = BatchedTestSubscriber {
                handle: unsafe { rcl_get_zero_initialized_subscription() },
                outcomes: [SubscriptionHandleResult::Taken, terminal].into(),
                max_messages_per_spin: 8,
                calls: 0,
            };

            assert_eq!(
                subscriber.handle_ready().unwrap(),
                terminal == SubscriptionHandleResult::Remove
            );
            assert_eq!(subscriber.calls, 2);
        }
    }

    #[test]
    fn subscription_batch_size_respects_backpressure_and_fairness_bounds() {
        assert_eq!(subscription_batch_size(0), 1);
        assert_eq!(subscription_batch_size(1), 2);
        assert_eq!(subscription_batch_size(15), MAX_SUBSCRIPTION_BATCH_SIZE);
        assert_eq!(subscription_batch_size(usize::MAX), MAX_SUBSCRIPTION_BATCH_SIZE);
    }

    struct VanishingMatchSubscriber {
        handle: rcl_subscription_t,
        state: SubscriptionMatchState,
        visible: Cell<bool>,
        publisher_count: Result<usize>,
    }

    impl Subscriber_ for VanishingMatchSubscriber {
        fn handle(&self) -> &rcl_subscription_t {
            &self.handle
        }

        fn match_state_mut(&mut self) -> Option<&mut SubscriptionMatchState> {
            self.visible.get().then_some(&mut self.state)
        }

        fn publisher_count(&self) -> Result<usize> {
            self.visible.set(false);
            self.publisher_count.clone()
        }

        fn handle_incoming(&mut self) -> Result<SubscriptionHandleResult> {
            Ok(SubscriptionHandleResult::Idle)
        }

        fn destroy(&mut self, _node: &mut rcl_node_t) {}
    }

    #[test]
    fn publisher_poll_tolerates_disappearing_match_state() {
        for publisher_count in [Ok(1), Err(Error::RCL_RET_SUBSCRIPTION_INVALID)] {
            let mut subscriber = VanishingMatchSubscriber {
                handle: unsafe { rcl_get_zero_initialized_subscription() },
                state: SubscriptionMatchState::new(uuid::Uuid::new_v4()),
                visible: Cell::new(true),
                publisher_count,
            };
            let (sender, _receiver) = oneshot::channel();
            subscriber.state.waiters.push(sender);

            subscriber.poll_has_publishers();

            assert!(!subscriber.visible.get());
            assert_eq!(subscriber.state.waiters.len(), 1);
        }
    }

    unsafe extern "C" fn return_int32_loan(
        _subscription: *const rcl_subscription_t, message: *mut std::os::raw::c_void,
    ) -> rcl_ret_t {
        RETURNED_LOANS.fetch_add(1, Ordering::Relaxed);
        crate::std_msgs::msg::Int32::destroy_msg(
            message.cast::<<crate::std_msgs::msg::Int32 as WrappedTypesupport>::CStruct>(),
        );
        RCL_RET_OK as i32
    }

    #[test]
    fn loaned_subscription_message_returns_native_storage_once() {
        RETURNED_LOANS.store(0, Ordering::Relaxed);
        let subscription = unsafe { rcl_get_zero_initialized_subscription() };
        let msg = NonNull::new(crate::std_msgs::msg::Int32::create_msg()).unwrap();
        let loan = LoanedSubscriptionMessage::<crate::std_msgs::msg::Int32> {
            msg,
            subscription: &subscription,
            info: rmw_message_info_t::default().into(),
            return_message: return_int32_loan,
            _node: PhantomData,
            _not_send_or_sync: PhantomData,
        };

        assert_eq!(loan.data, 0);
        drop(loan);
        assert_eq!(RETURNED_LOANS.load(Ordering::Relaxed), 1);
    }

    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    #[test]
    fn content_filter_options_reject_invalid_strings_and_parameter_counts() {
        let subscription = unsafe { rcl_get_zero_initialized_subscription() };
        let invalid_expression = ContentFilterOptions::new("data > \0");
        assert!(matches!(
            NativeContentFilterOptions::from_options(&subscription, &invalid_expression),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));

        let invalid_parameter =
            ContentFilterOptions::new("data > %0").expression_parameters(["1\0"]);
        assert!(matches!(
            NativeContentFilterOptions::from_options(&subscription, &invalid_parameter),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));

        let too_many_parameters = ContentFilterOptions::new("data > %0")
            .expression_parameters(std::iter::repeat_n("1", 101));
        assert!(matches!(
            NativeContentFilterOptions::from_options(&subscription, &too_many_parameters),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
    }
}
