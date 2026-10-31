use super::*;

impl Node {
    /// Registers a typed handler that executes in its group after native take.
    /// The adapter decodes on the worker and reuses an owned native reply buffer.
    pub fn create_service_callback_in_group<T, F>(
        &mut self, group: &mut CallbackGroup, name: &str, qos: QosProfile, callback: F,
    ) -> Result<GroupedService>
    where
        T: WrappedServiceTypeSupport + 'static,
        F: FnMut(T::Request, ServiceRequestInfo) -> Result<T::Response> + Send + 'static,
    {
        self.create_service_callback_in_group_with_options::<T, F>(
            group,
            name,
            qos,
            CallbackQueueOptions::default(),
            callback,
        )
    }
    /// Registers typed group work with explicit native intake and ring bounds.
    pub fn create_service_callback_in_group_with_options<T, F>(
        &mut self, group: &mut CallbackGroup, name: &str, qos: QosProfile,
        options: CallbackQueueOptions, mut callback: F,
    ) -> Result<GroupedService>
    where
        T: WrappedServiceTypeSupport + 'static,
        F: FnMut(T::Request, ServiceRequestInfo) -> Result<T::Response> + Send + 'static,
    {
        self.create_service_native_callback_in_group_with_options::<T, _>(
            group,
            name,
            qos,
            options,
            move |request, response, info| {
                callback(T::Request::from_native(&request), info)?.try_copy_to_native(response)
            },
        )
    }
    /// Registers a scoped native request/reusable response handler. Application
    /// code holds no native operation mutex; sending is synchronized with take
    /// and endpoint finalization after the handler returns.
    pub fn create_service_native_callback_in_group<T, F>(
        &mut self, group: &mut CallbackGroup, name: &str, qos: QosProfile, callback: F,
    ) -> Result<GroupedService>
    where
        T: WrappedServiceTypeSupport + 'static,
        F: for<'message> FnMut(
                NativeMessageRef<'message, T::Request>,
                &mut WrappedNativeMsg<T::Response>,
                ServiceRequestInfo,
            ) -> Result<()>
            + Send
            + 'static,
    {
        self.create_service_native_callback_in_group_with_options::<T, F>(
            group,
            name,
            qos,
            CallbackQueueOptions::default(),
            callback,
        )
    }
    /// Registers a native service handler with explicit ring and intake bounds.
    pub fn create_service_native_callback_in_group_with_options<T, F>(
        &mut self, group: &mut CallbackGroup, name: &str, qos: QosProfile,
        options: CallbackQueueOptions, mut callback: F,
    ) -> Result<GroupedService>
    where
        T: WrappedServiceTypeSupport + 'static,
        F: for<'message> FnMut(
                NativeMessageRef<'message, T::Request>,
                &mut WrappedNativeMsg<T::Response>,
                ServiceRequestInfo,
            ) -> Result<()>
            + Send
            + 'static,
    {
        let options = options.validate()?;
        let mut response = WrappedNativeMsg::<T::Response>::try_new()?;
        let support = T::try_get_ts()?;
        let handle = create_service_helper(self.node_handle.as_mut(), name, support, qos)?;
        let native = Arc::new(NativeService {
            handle: Box::new(UnsafeCell::new(handle)),
            operations: Mutex::new(()),
            initialized: AtomicBool::new(true),
            context: self.context.clone(),
            #[cfg(not(r2r__ros__distro__humble))]
            support,
        });
        let output = native.clone();
        let (producer, consumer) = group.prepare_input(
            options.capacity,
            move |(request, mut info): (WrappedNativeMsg<T::Request>, rmw_service_info_t)| {
                callback(
                    NativeMessageRef::from_owned(&request),
                    &mut response,
                    ServiceRequestInfo::from(&info),
                )?;
                output.send(&mut info, &mut response)
            },
        )?;
        let control = producer.control();
        control.set_native_wake(self.canceled_subscriptions.lock().wake());
        let id = uuid::Uuid::new_v4();
        let source = Box::new(ServiceSource::<T> {
            id,
            group: group.id(),
            native: native.clone(),
            input: producer,
            maximum: options.max_inputs_per_spin,
        });
        let registry = self.grouped_callbacks.get_or_insert_with(Default::default);
        registry.indexes.insert(id, registry.rows.len());
        registry.rows.push(source);
        group
            .sources
            .push((registry.node_id, Arc::downgrade(&control)));
        group.commit_input(consumer);
        Ok(GroupedService {
            id,
            native: Arc::downgrade(&native),
            control,
        })
    }
}
