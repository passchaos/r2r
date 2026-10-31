//! Scoped receive ownership, native dispatch and callback subscription lifetime.

use super::*;

/// Options for a scoped native subscription callback.
///
/// The callback runs during node dispatch and does not use a Rust-side message channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeCallbackSubscriptionOptions {
    /// Maximum native samples handled for this subscription during one spin dispatch.
    pub max_messages_per_spin: usize,
    /// Ignore messages published by this process.
    pub ignore_local_publications: bool,
    pub unique_network_flow: UniqueNetworkFlowRequirement,
}

impl Default for NativeCallbackSubscriptionOptions {
    fn default() -> Self {
        Self {
            max_messages_per_spin: MAX_SUBSCRIPTION_BATCH_SIZE,
            ignore_local_publications: false,
            unique_network_flow: UniqueNetworkFlowRequirement::NotRequired,
        }
    }
}

impl NativeCallbackSubscriptionOptions {
    /// Sets the per-spin drain bound. A larger value favors throughput over entity fairness.
    pub const fn max_messages_per_spin(mut self, maximum: usize) -> Self {
        self.max_messages_per_spin = maximum;
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

    /// Adds a middleware content filter to native subscription initialization.
    pub fn content_filter(
        self, content_filter: ContentFilterOptions,
    ) -> SubscriptionConfiguration<Self> {
        SubscriptionConfiguration::new(self).content_filter(content_filter)
    }

    pub(crate) fn validate(self) -> Result<Self> {
        if self.max_messages_per_spin == 0 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        Ok(self)
    }
}

/// Ownership token for a scoped native subscription callback.
///
/// Dropping this token schedules the native subscription for removal. Native messages borrowed by
/// the callback cannot escape the callback invocation.
pub struct NativeCallbackSubscription {
    pub(crate) id: uuid::Uuid,
    canceled_subscriptions: Arc<Mutex<crate::notifications::NotificationQueue<uuid::Uuid>>>,
}

impl NativeCallbackSubscription {
    pub(crate) fn new(
        id: uuid::Uuid,
        canceled_subscriptions: Arc<Mutex<crate::notifications::NotificationQueue<uuid::Uuid>>>,
    ) -> Self {
        Self {
            id,
            canceled_subscriptions,
        }
    }

    /// Returns the number of currently matched publishers.
    pub fn get_publisher_count(&self, node: &Node) -> Result<usize> {
        node.get_subscription_publisher_count(self.id)
    }

    /// Returns the fully resolved native topic name.
    pub fn topic_name(&self, node: &Node) -> Result<String> {
        node.get_subscription_topic_name(self.id)
    }

    /// Returns the QoS profile resolved by the middleware.
    pub fn actual_qos(&self, node: &Node) -> Result<QosProfile> {
        node.get_subscription_actual_qos(self.id)
    }

    /// Returns whether middleware-side content filtering is enabled.
    pub fn is_content_filter_enabled(&self, node: &Node) -> Result<bool> {
        node.get_subscription_content_filter_enabled(self.id)
    }

    /// Replaces the middleware-side content filter for this callback subscription.
    ///
    /// An empty expression clears the filter. The selected RMW may return
    /// [`Error::RCL_RET_UNSUPPORTED`].
    pub fn set_content_filter(
        &self, node: &mut Node, options: &ContentFilterOptions,
    ) -> Result<()> {
        node.set_subscription_content_filter(self.id, options)
    }

    /// Clears the middleware-side content filter.
    pub fn clear_content_filter(&self, node: &mut Node) -> Result<()> {
        self.set_content_filter(node, &ContentFilterOptions::default())
    }

    /// Returns the current middleware-side content filter.
    pub fn get_content_filter(&self, node: &Node) -> Result<ContentFilterOptions> {
        node.get_subscription_content_filter(self.id)
    }

    /// Waits for at least one publisher to match this callback subscription.
    pub fn wait_for_publishers(&self, node: &mut Node) -> Result<impl Future<Output = Result<()>>> {
        let receiver = node.register_subscription_publisher_wait(self.id)?;
        Ok(receiver.map_err(|_| Error::RCL_RET_SUBSCRIPTION_INVALID))
    }

    /// Waits for a publisher up to `timeout`. The node must be spun while awaiting.
    pub fn wait_for_publishers_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        Node::validate_timeout(timeout)?;
        let available = self.wait_for_publishers(node)?;
        node.with_timeout(available, timeout)
    }
}

impl Drop for NativeCallbackSubscription {
    fn drop(&mut self) {
        crate::notifications::enqueue(&self.canceled_subscriptions, self.id);
    }
}

pub(crate) struct NativeCallbackSubscriber<T, F>
where
    T: WrappedTypesupport,
    F: for<'message> FnMut(NativeMessageRef<'message, T>, MessageInfo) -> Result<()>,
{
    pub rcl_handle: rcl_subscription_t,
    pub callback: F,
    pub max_messages_per_spin: usize,
    pub(crate) match_state: SubscriptionMatchState,
    _message: PhantomData<T>,
}

impl<T, F> NativeCallbackSubscriber<T, F>
where
    T: WrappedTypesupport,
    F: for<'message> FnMut(NativeMessageRef<'message, T>, MessageInfo) -> Result<()>,
{
    pub(crate) fn new(
        rcl_handle: rcl_subscription_t, callback: F, max_messages_per_spin: usize,
        match_state: SubscriptionMatchState,
    ) -> Self {
        Self {
            rcl_handle,
            callback,
            max_messages_per_spin,
            match_state,
            _message: PhantomData,
        }
    }
}

impl<T, F> Subscriber_ for NativeCallbackSubscriber<T, F>
where
    T: WrappedTypesupport,
    F: for<'message> FnMut(NativeMessageRef<'message, T>, MessageInfo) -> Result<()>
        + Send
        + 'static,
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

    fn max_messages_per_spin(&self) -> usize {
        self.max_messages_per_spin
    }

    fn handle_incoming(&mut self) -> Result<SubscriptionHandleResult> {
        let mut info = rmw_message_info_t::default();
        let mut message = WrappedNativeMsg::<T>::try_new()?;
        let ret = unsafe {
            rcl_take(&self.rcl_handle, message.void_ptr_mut(), &mut info, std::ptr::null_mut())
        };
        if ret == RCL_RET_SUBSCRIPTION_TAKE_FAILED as i32 {
            return Ok(SubscriptionHandleResult::Idle);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        (self.callback)(NativeMessageRef::from_owned(&message), info.into())?;
        Ok(SubscriptionHandleResult::Taken)
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        self.match_state.clear_events();
        unsafe {
            rcl_subscription_fini(&mut self.rcl_handle, node);
        }
    }
}

#[cfg(test)]
mod tests;
