//! Subscription factories, registry bookkeeping and per-subscription
//! accessor methods for [`Node`].

use super::*;

impl Node {
    pub(crate) fn add_subscriber(&mut self, subscriber: Box<dyn Subscriber_>) {
        let index = self.subscribers.len();
        if let Some(state) = subscriber.match_state() {
            let previous = self.subscription_indexes.insert(state.id, index);
            debug_assert!(previous.is_none());
        }
        self.subscribers.push(subscriber);
    }

    pub(in crate::nodes) fn rebuild_subscription_indexes(
        subscribers: &[Box<dyn Subscriber_>], indexes: &mut HashMap<uuid::Uuid, usize>,
    ) {
        indexes.clear();
        indexes.reserve(subscribers.len());
        for (index, subscriber) in subscribers.iter().enumerate() {
            if let Some(state) = subscriber.match_state() {
                let previous = indexes.insert(state.id, index);
                debug_assert!(previous.is_none());
            }
        }
    }

    pub(in crate::nodes) fn remove_subscription(
        subscribers: &mut Vec<Box<dyn Subscriber_>>,
        subscription_indexes: &mut HashMap<uuid::Uuid, usize>, id: uuid::Uuid,
        node_handle: &mut rcl_node_t,
    ) -> bool {
        let Some(index) = subscription_indexes.remove(&id) else {
            return false;
        };
        let mut removed = subscribers.swap_remove(index);
        debug_assert_eq!(removed.match_state().map(|state| state.id), Some(id));
        removed.destroy(node_handle);
        if let Some(moved) = subscribers
            .get(index)
            .and_then(|subscriber| subscriber.match_state())
        {
            subscription_indexes.insert(moved.id, index);
        }
        true
    }

    fn destroy_subscription_by_id(&mut self, id: uuid::Uuid) -> bool {
        let removed = Self::remove_subscription(
            &mut self.subscribers,
            &mut self.subscription_indexes,
            id,
            self.node_handle.as_mut(),
        );
        if removed {
            self.subscriptions_with_active_waits.remove(&id);
            self.canceled_subscriptions
                .lock()
                .retain(|queued| *queued != id);
        }
        removed
    }

    pub(crate) fn subscriber(&self, id: uuid::Uuid) -> Result<&dyn Subscriber_> {
        let index = self
            .subscription_indexes
            .get(&id)
            .copied()
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        self.subscribers
            .get(index)
            .map(|subscriber| &**subscriber)
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)
    }

    pub(crate) fn subscriber_mut(&mut self, id: uuid::Uuid) -> Result<&mut (dyn Subscriber_ + '_)> {
        let index = self
            .subscription_indexes
            .get(&id)
            .copied()
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        match self.subscribers.get_mut(index) {
            Some(subscriber) => Ok(&mut **subscriber),
            None => Err(Error::RCL_RET_SUBSCRIPTION_INVALID),
        }
    }

    /// Subscribe to a ROS topic.
    ///
    /// This function returns a `Stream` of ros messages.
    pub fn subscribe<T>(&mut self, topic: &str, qos_profile: QosProfile) -> Result<Subscription<T>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_with_options(topic, qos_profile, SubscriptionOptions::default())
    }

    /// Subscribe to a ROS topic with explicit Rust-side stream buffering options.
    pub fn subscribe_with_options<T>(
        &mut self, topic: &str, qos_profile: QosProfile, options: SubscriptionOptions,
    ) -> Result<Subscription<T>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_with_transform(topic, qos_profile, options, discard_message_info::<T>)
    }

    /// Subscribe to a ROS topic and retain the RMW metadata for each message.
    pub fn subscribe_with_info<T>(
        &mut self, topic: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<ReceivedMessage<T>>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_with_info_and_options(topic, qos_profile, SubscriptionOptions::default())
    }

    /// Subscribe with RMW metadata and explicit Rust-side stream buffering options.
    pub fn subscribe_with_info_and_options<T>(
        &mut self, topic: &str, qos_profile: QosProfile, options: SubscriptionOptions,
    ) -> Result<Subscription<ReceivedMessage<T>>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_with_transform(topic, qos_profile, options, attach_message_info::<T>)
    }

    fn subscribe_with_transform<T, O>(
        &mut self, topic: &str, qos_profile: QosProfile, options: SubscriptionOptions,
        transform: fn(T, MessageInfo) -> O,
    ) -> Result<Subscription<O>>
    where
        T: 'static + WrappedTypesupport,
        O: 'static,
    {
        let options = options.validate()?;
        let subscription_handle = create_subscription_helper(
            self.node_handle.as_mut(),
            topic,
            T::get_ts(),
            qos_profile,
            options.ignore_local_publications,
            options.unique_network_flow,
        )?;
        let (sender, receiver) = mpsc::channel::<O>(options.channel_capacity);
        let id = uuid::Uuid::new_v4();

        let ws = TypedSubscriber {
            rcl_handle: subscription_handle,
            sender,
            transform,
            max_messages_per_spin: subscription_batch_size(options.channel_capacity),
            match_state: SubscriptionMatchState::new(id),
        };
        self.add_subscriber(Box::new(ws));
        Ok(Subscription::new(receiver, id, Arc::clone(&self.canceled_subscriptions)))
    }

    /// Subscribe to standard ROS parameter events, optionally filtering by node and parameter.
    ///
    /// Each parameter in a ROS event is emitted as an individual [`ParameterEventChange`].
    pub fn subscribe_parameter_events(
        &mut self, node: Option<String>, parameter: Option<String>,
    ) -> Result<impl Stream<Item = ParameterEventChange> + Unpin> {
        self.subscribe_parameter_events_with_options(
            node,
            parameter,
            QosProfile::parameter_events(),
            SubscriptionOptions::default(),
        )
    }

    /// Subscribes to standard ROS parameter events with explicit QoS and stream options.
    pub fn subscribe_parameter_events_with_options(
        &mut self, node: Option<String>, parameter: Option<String>, qos: QosProfile,
        options: SubscriptionOptions,
    ) -> Result<impl Stream<Item = ParameterEventChange> + Unpin> {
        let events = self.subscribe_with_options::<rcl_interfaces::msg::ParameterEvent>(
            "/parameter_events",
            qos,
            options,
        )?;
        Ok(events
            .map(move |event| flatten_parameter_event(event, node.as_deref(), parameter.as_deref()))
            .map(futures::stream::iter)
            .flatten())
    }

    /// Subscribe to a ROS topic.
    ///
    /// This function returns a `Stream` of ros messages without the rust convenience types.
    pub fn subscribe_native<T>(
        &mut self, topic: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<WrappedNativeMsg<T>>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_native_with_options(topic, qos_profile, SubscriptionOptions::default())
    }

    /// Subscribe to native ROS messages with explicit Rust-side stream buffering options.
    pub fn subscribe_native_with_options<T>(
        &mut self, topic: &str, qos_profile: QosProfile, options: SubscriptionOptions,
    ) -> Result<Subscription<WrappedNativeMsg<T>>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_native_with_transform(
            topic,
            qos_profile,
            options,
            discard_message_info::<WrappedNativeMsg<T>>,
        )
    }

    /// Subscribe to native ROS messages and retain the RMW metadata for each message.
    pub fn subscribe_native_with_info<T>(
        &mut self, topic: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<ReceivedMessage<WrappedNativeMsg<T>>>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_native_with_info_and_options(
            topic,
            qos_profile,
            SubscriptionOptions::default(),
        )
    }

    /// Subscribe to native messages with metadata and explicit stream buffering options.
    pub fn subscribe_native_with_info_and_options<T>(
        &mut self, topic: &str, qos_profile: QosProfile, options: SubscriptionOptions,
    ) -> Result<Subscription<ReceivedMessage<WrappedNativeMsg<T>>>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_native_with_transform(
            topic,
            qos_profile,
            options,
            attach_message_info::<WrappedNativeMsg<T>>,
        )
    }

    fn subscribe_native_with_transform<T, O>(
        &mut self, topic: &str, qos_profile: QosProfile, options: SubscriptionOptions,
        transform: fn(WrappedNativeMsg<T>, MessageInfo) -> O,
    ) -> Result<Subscription<O>>
    where
        T: 'static + WrappedTypesupport,
        O: 'static,
    {
        let options = options.validate()?;
        let subscription_handle = create_subscription_helper(
            self.node_handle.as_mut(),
            topic,
            T::get_ts(),
            qos_profile,
            options.ignore_local_publications,
            options.unique_network_flow,
        )?;
        let (sender, receiver) = mpsc::channel::<O>(options.channel_capacity);
        let id = uuid::Uuid::new_v4();

        let ws = NativeSubscriber {
            rcl_handle: subscription_handle,
            sender,
            transform,
            max_messages_per_spin: subscription_batch_size(options.channel_capacity),
            match_state: SubscriptionMatchState::new(id),
        };
        self.add_subscriber(Box::new(ws));
        Ok(Subscription::new(receiver, id, Arc::clone(&self.canceled_subscriptions)))
    }

    /// Subscribe to a ROS topic.
    ///
    /// This function returns a `Stream` of ros messages as `serde_json::Value`:s.
    /// Useful when you cannot know the type of the message at compile time.
    pub fn subscribe_untyped(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<Result<serde_json::Value>>> {
        self.subscribe_untyped_with_options(
            topic,
            topic_type,
            qos_profile,
            SubscriptionOptions::default(),
        )
    }

    /// Subscribe to untyped messages with explicit Rust-side stream buffering options.
    pub fn subscribe_untyped_with_options(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: SubscriptionOptions,
    ) -> Result<Subscription<Result<serde_json::Value>>> {
        self.subscribe_untyped_with_transform(
            topic,
            topic_type,
            qos_profile,
            options,
            discard_message_info::<Result<serde_json::Value>>,
        )
    }

    /// Subscribe to untyped ROS messages and retain the RMW metadata for each message.
    pub fn subscribe_untyped_with_info(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<ReceivedMessage<Result<serde_json::Value>>>> {
        self.subscribe_untyped_with_info_and_options(
            topic,
            topic_type,
            qos_profile,
            SubscriptionOptions::default(),
        )
    }

    /// Subscribe to untyped messages with metadata and explicit stream buffering options.
    pub fn subscribe_untyped_with_info_and_options(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: SubscriptionOptions,
    ) -> Result<Subscription<ReceivedMessage<Result<serde_json::Value>>>> {
        self.subscribe_untyped_with_transform(
            topic,
            topic_type,
            qos_profile,
            options,
            attach_message_info::<Result<serde_json::Value>>,
        )
    }

    fn subscribe_untyped_with_transform<O>(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: SubscriptionOptions, transform: fn(Result<serde_json::Value>, MessageInfo) -> O,
    ) -> Result<Subscription<O>>
    where
        O: 'static,
    {
        let options = options.validate()?;
        let msg = WrappedNativeMsgUntyped::new_from(topic_type)?;
        let subscription_handle = create_subscription_helper(
            self.node_handle.as_mut(),
            topic,
            msg.ts,
            qos_profile,
            options.ignore_local_publications,
            options.unique_network_flow,
        )?;
        let (sender, receiver) = mpsc::channel::<O>(options.channel_capacity);
        let id = uuid::Uuid::new_v4();

        let ws = UntypedSubscriber {
            rcl_handle: subscription_handle,
            topic_type: topic_type.to_string(),
            sender,
            transform,
            max_messages_per_spin: subscription_batch_size(options.channel_capacity),
            match_state: SubscriptionMatchState::new(id),
        };
        self.add_subscriber(Box::new(ws));
        Ok(Subscription::new(receiver, id, Arc::clone(&self.canceled_subscriptions)))
    }

    /// Subscribe to a ROS topic.
    ///
    /// This function returns a `Stream` of ros messages as non-deserialized `Vec<u8>`:s.
    /// Useful if you just want to pass the data along to another part of the system.
    pub fn subscribe_raw(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<Vec<u8>>> {
        self.subscribe_raw_with_options(
            topic,
            topic_type,
            qos_profile,
            SubscriptionOptions::default(),
        )
    }

    /// Subscribe to serialized messages with explicit Rust-side stream buffering options.
    pub fn subscribe_raw_with_options(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: SubscriptionOptions,
    ) -> Result<Subscription<Vec<u8>>> {
        self.subscribe_raw_with_transform(
            topic,
            topic_type,
            qos_profile,
            options,
            discard_message_info::<Vec<u8>>,
        )
    }

    /// Subscribe to serialized ROS messages and retain the RMW metadata for each message.
    pub fn subscribe_raw_with_info(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<ReceivedMessage<Vec<u8>>>> {
        self.subscribe_raw_with_info_and_options(
            topic,
            topic_type,
            qos_profile,
            SubscriptionOptions::default(),
        )
    }

    /// Subscribe to serialized messages with metadata and explicit stream buffering options.
    pub fn subscribe_raw_with_info_and_options(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: SubscriptionOptions,
    ) -> Result<Subscription<ReceivedMessage<Vec<u8>>>> {
        self.subscribe_raw_with_transform(
            topic,
            topic_type,
            qos_profile,
            options,
            attach_message_info::<Vec<u8>>,
        )
    }

    fn subscribe_raw_with_transform<O>(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: SubscriptionOptions, transform: fn(Vec<u8>, MessageInfo) -> O,
    ) -> Result<Subscription<O>>
    where
        O: 'static,
    {
        let options = options.validate()?;
        // TODO is it possible to handle the raw message without type support?
        //
        // Passing null ts to rcl_subscription_init throws an error ..
        //
        // It does not seem possible to not have a type support, which is a shame
        // because it means we always have to build the message types even if we
        // are just after the raw bytes.
        let msg = WrappedNativeMsgUntyped::new_from(topic_type)?;

        // Keep a buffer to reduce number of allocations. The rmw will
        // resize it if the message size exceeds the buffer size.
        let mut msg_buf: rcl_serialized_message_t =
            unsafe { rcutils_get_zero_initialized_uint8_array() };
        let ret = unsafe {
            rcutils_uint8_array_init(
                &mut msg_buf as *mut rcl_serialized_message_t,
                0,
                &rcutils_get_default_allocator(),
            )
        };

        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let subscription_handle = match create_subscription_helper(
            self.node_handle.as_mut(),
            topic,
            msg.ts,
            qos_profile,
            options.ignore_local_publications,
            options.unique_network_flow,
        ) {
            Ok(handle) => handle,
            Err(err) => {
                unsafe {
                    rcutils_uint8_array_fini(&mut msg_buf as *mut rcl_serialized_message_t);
                }
                return Err(err);
            }
        };
        let (sender, receiver) = mpsc::channel::<O>(options.channel_capacity);
        let id = uuid::Uuid::new_v4();

        let ws = RawSubscriber {
            rcl_handle: subscription_handle,
            msg_buf,
            sender,
            transform,
            max_messages_per_spin: subscription_batch_size(options.channel_capacity),
            match_state: SubscriptionMatchState::new(id),
        };
        self.add_subscriber(Box::new(ws));
        Ok(Subscription::new(receiver, id, Arc::clone(&self.canceled_subscriptions)))
    }

    /// Immediately destroys a subscription owned by this node.
    ///
    /// Dropping a subscription schedules its native endpoint for removal during the next spin.
    /// This method is useful when deterministic teardown is required before another spin cycle.
    /// Passing a subscription created by another node leaves this node unchanged and schedules
    /// the subscription for normal cleanup by its owning node.
    pub fn destroy_subscription<T>(&mut self, subscription: Subscription<T>) {
        let id = subscription.id;
        drop(subscription);
        self.destroy_subscription_by_id(id);
    }

    pub(crate) fn get_subscription_publisher_count(&self, id: uuid::Uuid) -> Result<usize> {
        self.subscriber(id)?.publisher_count()
    }

    pub(crate) fn get_subscription_actual_qos(&self, id: uuid::Uuid) -> Result<QosProfile> {
        self.subscriber(id)?.actual_qos()
    }

    pub(crate) fn get_subscription_topic_name(&self, id: uuid::Uuid) -> Result<String> {
        self.subscriber(id)?.topic_name()
    }

    pub(crate) fn get_subscription_can_loan_messages(&self, id: uuid::Uuid) -> Result<bool> {
        Ok(self.subscriber(id)?.can_loan_messages())
    }

    pub(crate) fn get_subscription_network_flow_endpoints(
        &self, id: uuid::Uuid,
    ) -> Result<Vec<NetworkFlowEndpoint>> {
        self.subscriber(id)?.network_flow_endpoints()
    }

    pub(crate) fn get_subscription_content_filter_enabled(&self, id: uuid::Uuid) -> Result<bool> {
        Ok(self.subscriber(id)?.is_content_filter_enabled())
    }

    pub(crate) fn set_subscription_content_filter(
        &mut self, id: uuid::Uuid, options: &ContentFilterOptions,
    ) -> Result<()> {
        self.subscriber(id)?.set_content_filter(options)
    }

    pub(crate) fn get_subscription_content_filter(
        &self, id: uuid::Uuid,
    ) -> Result<ContentFilterOptions> {
        self.subscriber(id)?.get_content_filter()
    }

    pub(crate) fn take_subscription_requested_incompatible_qos(
        &mut self, id: uuid::Uuid,
    ) -> Result<Option<RequestedIncompatibleQosStatus>> {
        self.subscriber_mut(id)?.take_requested_incompatible_qos()
    }

    pub(crate) fn take_subscription_requested_deadline_missed(
        &mut self, id: uuid::Uuid,
    ) -> Result<Option<RequestedDeadlineMissedStatus>> {
        self.subscriber_mut(id)?.take_requested_deadline_missed()
    }

    pub(crate) fn take_subscription_liveliness_changed(
        &mut self, id: uuid::Uuid,
    ) -> Result<Option<LivelinessChangedStatus>> {
        self.subscriber_mut(id)?.take_liveliness_changed()
    }

    pub(crate) fn take_subscription_message_lost(
        &mut self, id: uuid::Uuid,
    ) -> Result<Option<MessageLostStatus>> {
        self.subscriber_mut(id)?.take_message_lost()
    }

    #[cfg(not(r2r__ros__distro__humble))]
    pub(crate) fn take_subscription_matched(
        &mut self, id: uuid::Uuid,
    ) -> Result<Option<MatchedStatus>> {
        self.subscriber_mut(id)?.take_matched()
    }

    #[cfg(not(r2r__ros__distro__humble))]
    pub(crate) fn take_subscription_incompatible_type(
        &mut self, id: uuid::Uuid,
    ) -> Result<Option<IncompatibleTypeStatus>> {
        self.subscriber_mut(id)?.take_incompatible_type()
    }

    pub(crate) fn register_subscription_publisher_wait(
        &mut self, id: uuid::Uuid,
    ) -> Result<oneshot::Receiver<()>> {
        let state = self
            .subscriber_mut(id)?
            .match_state_mut()
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        let (sender, receiver) = oneshot::channel();
        state.waiters.register(sender);
        self.subscriptions_with_active_waits.insert(id);
        Ok(receiver)
    }
}
