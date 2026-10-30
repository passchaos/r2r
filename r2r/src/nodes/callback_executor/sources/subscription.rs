use super::*;
use std::{any::TypeId, marker::PhantomData};

struct SubscriptionSource<T: WrappedTypesupport> {
    group_id: CallbackGroupId,
    handle: rcl_subscription_t,
    input: Producer<(WrappedNativeMsg<T>, MessageInfo)>,
    match_state: SubscriptionMatchState,
    maximum: usize,
    marker: PhantomData<T>,
}
impl<T: WrappedTypesupport + 'static> Subscriber_ for SubscriptionSource<T> {
    fn handle(&self) -> &rcl_subscription_t {
        &self.handle
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
        self.maximum
    }
    fn handle_incoming(&mut self) -> Result<SubscriptionHandleResult> {
        if self.input.is_closed() {
            return Ok(SubscriptionHandleResult::Remove);
        }
        if !self.input.is_ready() {
            return Ok(SubscriptionHandleResult::Idle);
        }
        let mut message = WrappedNativeMsg::<T>::try_new()?;
        let mut info = rmw_message_info_t::default();
        let ret = unsafe {
            rcl_take(&self.handle, message.void_ptr_mut(), &mut info, std::ptr::null_mut())
        };
        if ret == RCL_RET_SUBSCRIPTION_TAKE_FAILED as i32 {
            return Ok(SubscriptionHandleResult::Idle);
        }
        check_rcl_ret(ret)?;
        match self.input.push((message, info.into())) {
            Ok(()) => Ok(SubscriptionHandleResult::Taken),
            Err(input) if self.input.is_closed() => {
                drop(input);
                Ok(SubscriptionHandleResult::Remove)
            }
            Err(input) => {
                drop(input);
                Err(Error::RCL_RET_ERROR)
            }
        }
    }
    fn destroy(&mut self, node: &mut rcl_node_t) {
        self.input.close();
        self.match_state.clear_events();
        let ret = unsafe { rcl_subscription_fini(&mut self.handle, node) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize grouped subscription: {ret}");
        }
    }
}
impl<T: WrappedTypesupport + 'static> Source for SubscriptionSource<T> {
    fn id(&self) -> uuid::Uuid {
        self.match_state.id
    }
    fn group_id(&self) -> CallbackGroupId {
        self.group_id
    }
    fn subscriber(&self) -> Option<&dyn Subscriber_> {
        Some(self)
    }
    fn subscriber_mut(&mut self) -> Option<&mut dyn Subscriber_> {
        Some(self)
    }
    fn is_closed(&self) -> bool {
        self.input.is_closed()
    }
    fn is_waitable(&self) -> bool {
        self.input.is_ready()
    }
    fn set_wake(&mut self, wake: Option<Waker>) {
        self.input.control().set_native_wake(wake.clone());
        self.match_state.waiters.set_native_wake(wake);
    }
    fn capacities(&self) -> WaitSetCapacities {
        WaitSetCapacities {
            subscriptions: 1,
            ..Default::default()
        }
    }
    fn register(&self, ws: &mut rcl_wait_set_t, index: &mut usize) -> Result<()> {
        check_rcl_ret(unsafe { rcl_wait_set_add_subscription(ws, &self.handle, index) })
    }
    fn ready(&self, ws: &rcl_wait_set_t, index: usize) -> Result<bool> {
        ready(ws.subscriptions, ws.size_of_subscriptions, index, &self.handle)
    }
    fn maximum(&self) -> usize {
        self.maximum
    }
    fn take(&mut self) -> Result<bool> {
        Ok(matches!(self.handle_incoming()?, SubscriptionHandleResult::Taken))
    }
    fn destroy(&mut self, node: &mut rcl_node_t) {
        Subscriber_::destroy(self, node);
    }
}

impl Node {
    /// Closes input and immediately removes its native endpoint while stopped.
    /// A foreign token still schedules its owning source's closure.
    pub fn destroy_grouped_subscription(&mut self, subscription: GroupedSubscription) {
        let id = subscription.id;
        drop(subscription);
        if let Some(sources) = &mut self.grouped_callbacks {
            sources.remove(id, self.node_handle.as_mut(), &mut self.qos_events);
        }
        self.subscriptions_with_active_waits.remove(&id);
    }
    /// Registers a typed callback whose conversion and user work run in the group.
    pub fn subscribe_callback_in_group<T, F>(
        &mut self, group: &mut CallbackGroup, topic: &str, qos: QosProfile, callback: F,
    ) -> Result<GroupedSubscription>
    where
        T: WrappedTypesupport + 'static,
        F: FnMut(T, MessageInfo) -> Result<()> + Send + 'static,
    {
        self.subscribe_callback_in_group_with_options::<T, F>(
            group,
            topic,
            qos,
            CallbackQueueOptions::default(),
            callback,
        )
    }
    /// Registers typed group work with explicit native input/ring bounds.
    pub fn subscribe_callback_in_group_with_options<T, F>(
        &mut self, group: &mut CallbackGroup, topic: &str, qos: QosProfile,
        options: CallbackQueueOptions, mut callback: F,
    ) -> Result<GroupedSubscription>
    where
        T: WrappedTypesupport + 'static,
        F: FnMut(T, MessageInfo) -> Result<()> + Send + 'static,
    {
        self.subscribe_native_callback_in_group_with_options::<T, _>(
            group,
            topic,
            qos,
            options,
            move |message, info| callback(T::from_native(&message), info),
        )
    }
    /// Registers a scoped native callback in a mutually exclusive group.
    ///
    /// Native take remains on the owned Node job. The group receives an
    /// independently initialized owned native container and exposes its borrowed
    /// read-only view only during invocation. No owned Rust decoding is required.
    /// A full ring parks this source until callback capacity returns.
    ///
    /// ```compile_fail
    /// use r2r::{CallbackGroup,Node,QosProfile,std_msgs::msg::Int32};
    /// fn install(node:&mut Node,group:&mut CallbackGroup) {
    ///     let mut escaped=None;
    ///     node.subscribe_native_callback_in_group::<Int32,_>(
    ///         group,"input",QosProfile::default(),move |message,_| {
    ///             escaped=Some(message);
    ///             Ok(())
    ///         },
    ///     ).unwrap();
    /// }
    /// ```
    pub fn subscribe_native_callback_in_group<T, F>(
        &mut self, group: &mut CallbackGroup, topic: &str, qos: QosProfile, callback: F,
    ) -> Result<GroupedSubscription>
    where
        T: WrappedTypesupport + 'static,
        F: for<'message> FnMut(NativeMessageRef<'message, T>, MessageInfo) -> Result<()>
            + Send
            + 'static,
    {
        self.subscribe_native_callback_in_group_with_options::<T, F>(
            group,
            topic,
            qos,
            CallbackQueueOptions::default(),
            callback,
        )
    }
    /// Registers a scoped native handler with exact ring and native batch bounds.
    /// Invalid buffering fails before creating a subscription or group task.
    pub fn subscribe_native_callback_in_group_with_options<T, F>(
        &mut self, group: &mut CallbackGroup, topic: &str, qos: QosProfile,
        options: impl Into<GroupedSubscriptionOptions>, mut callback: F,
    ) -> Result<GroupedSubscription>
    where
        T: WrappedTypesupport + 'static,
        F: for<'message> FnMut(NativeMessageRef<'message, T>, MessageInfo) -> Result<()>
            + Send
            + 'static,
    {
        let options = options.into();
        let endpoint = options.endpoint.validate()?;
        let options = options.queue.validate()?;
        let (producer, consumer) = group.prepare_input(
            options.capacity,
            move |(message, info): (WrappedNativeMsg<T>, MessageInfo)| {
                callback(NativeMessageRef::from_owned(&message), info)
            },
        )?;
        let handle = create_subscription_helper(
            self.node_handle.as_mut(),
            topic,
            T::get_ts(),
            qos,
            endpoint.ignore_local_publications,
            endpoint.unique_network_flow,
        )?;
        let control = producer.control();
        let mut row = Box::new(SubscriptionSource::<T> {
            group_id: group.id(),
            handle,
            input: producer,
            match_state: SubscriptionMatchState::new(uuid::Uuid::new_v4()),
            maximum: options
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
        group.commit_input(consumer);
        Ok(GroupedSubscription { id, control })
    }
}
