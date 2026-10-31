use super::*;

impl Node {
    /// Decodes owned input on a lane and invokes the shared handler concurrently.
    pub fn subscribe_reentrant_callback<T, F>(
        &mut self, group: &mut ReentrantCallbackGroup, topic: &str, qos: QosProfile, callback: F,
    ) -> Result<GroupedSubscription>
    where
        T: WrappedTypesupport + 'static,
        F: Fn(T, MessageInfo) -> Result<()> + Send + Sync + 'static,
    {
        self.subscribe_reentrant_callback_with_options::<T, F>(
            group,
            topic,
            qos,
            GroupedSubscriptionOptions::default(),
            callback,
        )
    }
    /// Configures bounded input and endpoint policy for shared typed work.
    pub fn subscribe_reentrant_callback_with_options<T, F>(
        &mut self, group: &mut ReentrantCallbackGroup, topic: &str, qos: QosProfile,
        options: impl Into<SubscriptionConfiguration<GroupedSubscriptionOptions>>, callback: F,
    ) -> Result<GroupedSubscription>
    where
        T: WrappedTypesupport + 'static,
        F: Fn(T, MessageInfo) -> Result<()> + Send + Sync + 'static,
    {
        self.subscribe_native_reentrant_callback_with_options::<T, _>(
            group,
            topic,
            qos,
            options,
            move |message, info| callback(T::from_native(&message), info),
        )
    }
    /// Registers immutable shared native work with concurrent owned inputs.
    pub fn subscribe_native_reentrant_callback<T, F>(
        &mut self, group: &mut ReentrantCallbackGroup, topic: &str, qos: QosProfile, callback: F,
    ) -> Result<GroupedSubscription>
    where
        T: WrappedTypesupport + 'static,
        F: for<'message> Fn(NativeMessageRef<'message, T>, MessageInfo) -> Result<()>
            + Send
            + Sync
            + 'static,
    {
        self.subscribe_native_reentrant_callback_with_options::<T, F>(
            group,
            topic,
            qos,
            GroupedSubscriptionOptions::default(),
            callback,
        )
    }
    /// Uses one bounded native input ring and independent invocation receivers.
    pub fn subscribe_native_reentrant_callback_with_options<T, F>(
        &mut self, group: &mut ReentrantCallbackGroup, topic: &str, qos: QosProfile,
        configuration: impl Into<SubscriptionConfiguration<GroupedSubscriptionOptions>>,
        callback: F,
    ) -> Result<GroupedSubscription>
    where
        T: WrappedTypesupport + 'static,
        F: for<'message> Fn(NativeMessageRef<'message, T>, MessageInfo) -> Result<()>
            + Send
            + Sync
            + 'static,
    {
        let configuration = configuration.into();
        let endpoint = configuration.endpoint.endpoint.validate()?;
        let queue = configuration.endpoint.queue.validate()?;
        let callback = Arc::new(callback);
        let callbacks: Vec<_> = (0..group.concurrency())
            .map(|_| {
                let callback = callback.clone();
                move |(message, info): (WrappedNativeMsg<T>, MessageInfo)| {
                    callback(NativeMessageRef::from_owned(&message), info)
                }
            })
            .collect();
        let (producer, consumers) = group.prepare_inputs(queue.capacity, callbacks)?;
        let handle = create_subscription_helper(
            self.node_handle.as_mut(),
            topic,
            T::try_get_ts()?,
            qos,
            endpoint.ignore_local_publications,
            endpoint.unique_network_flow,
            configuration.content_filter.as_ref(),
        )?;
        let control = producer.control();
        let mut row = Box::new(SubscriptionSource::<T> {
            group_id: group.id(),
            handle,
            input: producer,
            match_state: SubscriptionMatchState::new(uuid::Uuid::new_v4()),
            maximum: queue
                .max_inputs_per_spin
                .min(endpoint.max_messages_per_spin),
            marker: PhantomData,
        });
        row.set_wake(self.canceled_subscriptions.lock().wake());
        let id = row.match_state.id;
        let registry = self.grouped_callbacks.get_or_insert_with(Default::default);
        registry.indexes.insert(id, registry.rows.len());
        registry.rows.push(row);
        group
            .sources
            .push((registry.node_id, Arc::downgrade(&control)));
        group.commit_inputs(consumers);
        Ok(GroupedSubscription { id, control })
    }
}
