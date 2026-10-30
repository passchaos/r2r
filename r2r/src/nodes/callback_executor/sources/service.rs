use super::*;
use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

struct NativeService {
    handle: Box<UnsafeCell<rcl_service_t>>,
    operations: Mutex<()>,
    initialized: AtomicBool,
    context: Context,
    #[cfg(not(r2r__ros__distro__humble))]
    support: &'static rosidl_service_type_support_t,
}
// SAFETY: native storage has a stable address. Only the owned Node registers
// or waits on it. Take, response send, query/configuration and finalization
// share operations; callbacks and independent response buffers hold no lock.
unsafe impl Send for NativeService {}
unsafe impl Sync for NativeService {}
impl NativeService {
    fn with_handle<T>(&self, operation: impl FnOnce(*mut rcl_service_t) -> Result<T>) -> Result<T> {
        let _operation = self.operations.lock();
        if !self.initialized.load(Ordering::Acquire) {
            return Err(Error::RCL_RET_SERVICE_INVALID);
        }
        operation(self.handle.get())
    }
    fn send<T: WrappedTypesupport>(
        &self, info: &mut rmw_service_info_t, response: &mut WrappedNativeMsg<T>,
    ) -> Result<()> {
        if !self.context.is_valid() {
            return Ok(());
        }
        let _operation = self.operations.lock();
        // A selected handler may finish owned input after source/context shutdown.
        // It cannot send through an endpoint that has since been finalized.
        if !self.initialized.load(Ordering::Acquire) {
            return Ok(());
        }
        check_rcl_ret(unsafe {
            rcl_send_response(self.handle.get(), &mut info.request_id, response.void_ptr_mut())
        })
    }
    fn destroy(&self, node: &mut rcl_node_t) {
        let _operation = self.operations.lock();
        if self.initialized.swap(false, Ordering::AcqRel) {
            let ret = unsafe { rcl_service_fini(self.handle.get(), node) };
            if ret != RCL_RET_OK as i32 {
                log::error!("could not finalize grouped service: {ret}");
            }
        }
    }
}

/// Owns a service registration whose handler executes in a callback group.
///
/// Drop stops intake and wakes native finalization. Selected handlers retain
/// their owned requests; a finalized endpoint cannot receive a late response.
/// Queries synchronize with native operations and do not retain Node ownership.
pub struct GroupedService {
    id: uuid::Uuid,
    native: std::sync::Weak<NativeService>,
    control: Arc<dyn input::Control>,
}
impl GroupedService {
    #[cfg(test)]
    pub(in crate::nodes) fn native_validity_probe(&self) -> impl Fn() -> bool {
        let native = self.native.clone();
        move || {
            native.upgrade().is_some_and(|native| {
                let _operation = native.operations.lock();
                unsafe { rcl_service_is_valid(native.handle.get()) }
            })
        }
    }
    fn native(&self) -> Result<Arc<NativeService>> {
        self.native.upgrade().ok_or(Error::RCL_RET_SERVICE_INVALID)
    }
    /// Whether the service's handler/input registration remains open.
    pub fn is_valid(&self) -> bool {
        !self.control.is_closed()
    }
    /// Fully resolved native service name; finalized endpoints report invalidity.
    pub fn service_name(&self) -> Result<String> {
        self.native()?.with_handle(|handle| {
            let pointer = unsafe { rcl_service_get_service_name(handle) };
            if pointer.is_null() {
                return Err(Error::RCL_RET_SERVICE_INVALID);
            }
            unsafe { CStr::from_ptr(pointer) }
                .to_str()
                .map(str::to_owned)
                .map_err(|error| Error::InvalidUtf8 {
                    context: "grouped service name",
                    err: error.to_string(),
                })
        })
    }
    /// Actual request subscription QoS.
    pub fn get_request_subscription_actual_qos(&self) -> Result<QosProfile> {
        self.native()?.with_handle(|handle| {
            QosProfile::try_from(
                unsafe { rcl_service_request_subscription_get_actual_qos(handle).as_ref() }
                    .ok_or(Error::RCL_RET_SERVICE_INVALID)?,
            )
        })
    }
    /// Actual response publisher QoS.
    pub fn get_response_publisher_actual_qos(&self) -> Result<QosProfile> {
        self.native()?.with_handle(|handle| {
            QosProfile::try_from(
                unsafe { rcl_service_response_publisher_get_actual_qos(handle).as_ref() }
                    .ok_or(Error::RCL_RET_SERVICE_INVALID)?,
            )
        })
    }
    #[cfg(not(r2r__ros__distro__humble))]
    /// Configures Iron-and-newer native service events on the owning Node.
    pub fn configure_introspection(
        &self, node: &mut Node, state: crate::ServiceIntrospectionState, qos: QosProfile,
    ) -> Result<()> {
        if !node
            .grouped_callbacks
            .as_ref()
            .is_some_and(|sources| sources.indexes.contains_key(&self.id))
        {
            return Err(Error::RCL_RET_SERVICE_INVALID);
        }
        let native = self.native()?;
        native.with_handle(|handle| {
            let mut clock = node.ros_clock.lock();
            crate::service_introspection::configure_service_native(
                unsafe { &mut *handle },
                native.support,
                node.node_handle.as_mut(),
                &mut clock,
                state,
                qos,
            )
        })
    }
}
impl Drop for GroupedService {
    fn drop(&mut self) {
        self.control.close();
    }
}

struct ServiceSource<T: WrappedServiceTypeSupport> {
    id: uuid::Uuid,
    group: CallbackGroupId,
    native: Arc<NativeService>,
    input: Producer<(WrappedNativeMsg<T::Request>, rmw_service_info_t)>,
    maximum: usize,
}
impl<T: WrappedServiceTypeSupport + 'static> Source for ServiceSource<T> {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
    fn group_id(&self) -> CallbackGroupId {
        self.group
    }
    fn is_closed(&self) -> bool {
        self.input.is_closed()
    }
    fn is_waitable(&self) -> bool {
        self.input.is_ready()
    }
    fn set_wake(&mut self, wake: Option<Waker>) {
        self.input.control().set_native_wake(wake);
    }
    fn capacities(&self) -> WaitSetCapacities {
        WaitSetCapacities {
            services: 1,
            ..Default::default()
        }
    }
    fn register(&self, ws: &mut rcl_wait_set_t, index: &mut usize) -> Result<()> {
        check_rcl_ret(unsafe { rcl_wait_set_add_service(ws, self.native.handle.get(), index) })
    }
    fn ready(&self, ws: &rcl_wait_set_t, index: usize) -> Result<bool> {
        ready(ws.services, ws.size_of_services, index, self.native.handle.get())
    }
    fn maximum(&self) -> usize {
        self.maximum
    }
    fn take(&mut self) -> Result<bool> {
        if self.input.is_closed() || !self.input.is_ready() {
            return Ok(false);
        }
        let mut request = WrappedNativeMsg::<T::Request>::try_new()?;
        let mut info = crate::native_metadata::service_info();
        let ret = self.native.with_handle(|handle| {
            Ok(unsafe { rcl_take_request_with_info(handle, &mut info, request.void_ptr_mut()) })
        })?;
        if ret == RCL_RET_SERVICE_TAKE_FAILED as i32 {
            return Ok(false);
        }
        check_rcl_ret(ret)?;
        match self.input.push((request, info)) {
            Ok(()) => Ok(true),
            Err(input) if self.input.is_closed() => {
                drop(input);
                Ok(false)
            }
            Err(input) => {
                drop(input);
                Err(Error::RCL_RET_ERROR)
            }
        }
    }
    fn destroy(&mut self, node: &mut rcl_node_t) {
        self.input.close();
        self.native.destroy(node);
    }
}

impl Node {
    /// Closes and immediately finalizes an owned grouped service while stopped.
    /// A foreign token still closes its source and schedules its owning cleanup.
    pub fn destroy_grouped_service(&mut self, service: GroupedService) {
        let id = service.id;
        drop(service);
        if let Some(sources) = &mut self.grouped_callbacks {
            sources.remove(id, self.node_handle.as_mut(), &mut self.qos_events);
        }
    }
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
        let support = T::get_ts();
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
