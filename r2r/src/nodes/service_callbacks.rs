//! Optional native synchronous service registry; ordinary service streams stay separate.

use super::*;
use crate::services::callback::{CallbackService_, Endpoint, NativeCallbackService};

#[derive(Default)]
pub(super) struct ServiceCallbacks {
    services: Vec<Box<dyn CallbackService_>>,
    indexes: HashMap<uuid::Uuid, usize>,
    canceled: Arc<Mutex<crate::notifications::NotificationQueue<uuid::Uuid>>>,
}

impl ServiceCallbacks {
    pub(super) fn len(&self) -> usize {
        self.services.len()
    }
    pub(super) fn set_wake(&self, wake: Option<std::task::Waker>) {
        self.canceled.lock().set_wake(wake);
    }
    fn insert(&mut self, service: Box<dyn CallbackService_>) -> CallbackService {
        let endpoint = service.endpoint();
        let token =
            CallbackService::new(endpoint.id, endpoint.alive.clone(), self.canceled.clone());
        self.indexes.insert(endpoint.id, self.services.len());
        self.services.push(service);
        token
    }
    fn endpoint(&self, id: uuid::Uuid) -> Result<&Endpoint> {
        let index = *self
            .indexes
            .get(&id)
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        Ok(self.services[index].endpoint())
    }
    fn remove(&mut self, id: uuid::Uuid, node: &mut rcl_node_t) -> bool {
        let Some(index) = self.indexes.remove(&id) else {
            return false;
        };
        let mut removed = self.services.swap_remove(index);
        if let Some(moved) = self.services.get(index) {
            self.indexes.insert(moved.endpoint().id, index);
        }
        removed.destroy(node);
        // Callback captures and response storage are retired without internal locks.
        drop(removed);
        true
    }
    pub(super) fn prepare(&mut self, node: &mut rcl_node_t) -> usize {
        loop {
            let id = self.canceled.lock().pop_front();
            let Some(id) = id else {
                break;
            };
            self.remove(id, node);
        }
        self.services.len()
    }
    pub(super) fn add_to_wait_set(&self, wait_set: &mut rcl_wait_set_t) -> Result<usize> {
        let mut first = 0;
        for (position, service) in self.services.iter().enumerate() {
            let mut index = 0;
            check_rcl_ret(unsafe {
                rcl_wait_set_add_service(wait_set, &service.endpoint().handle, &mut index)
            })?;
            if position == 0 {
                first = index;
            }
        }
        Ok(first)
    }
    pub(super) fn dispatch(&mut self, wait_set: &rcl_wait_set_t, first: usize) -> Result<()> {
        if self.services.is_empty() {
            return Ok(());
        }
        if wait_set.services.is_null() || first + self.services.len() > wait_set.size_of_services {
            return Err(Error::RCL_RET_WAIT_SET_INVALID);
        }
        let ready = unsafe {
            std::slice::from_raw_parts(wait_set.services.add(first), self.services.len())
        };
        let mut error = None;
        for (service, pointer) in self.services.iter_mut().zip(ready) {
            if pointer.is_null() {
                continue;
            }
            for _ in 0..service.endpoint().options.max_requests_per_spin {
                match service.take_and_respond() {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(failure) => {
                        error.get_or_insert(failure);
                        break;
                    }
                }
            }
        }
        error.map_or(Ok(()), Err)
    }
    pub(super) fn destroy_all(&mut self, node: &mut rcl_node_t) {
        for service in &mut self.services {
            service.destroy(node);
        }
    }
}

impl Node {
    /// Register a synchronous typed service handler. It runs during native dispatch without a
    /// service/channel mutex or request stream; the response's native capacity is reused.
    pub fn create_service_callback<T, F>(
        &mut self, name: &str, qos: QosProfile, callback: F,
    ) -> Result<CallbackService>
    where
        T: WrappedServiceTypeSupport + 'static,
        F: FnMut(T::Request, ServiceRequestInfo) -> Result<T::Response> + Send + 'static,
    {
        self.create_service_callback_with_options::<T, F>(
            name,
            qos,
            CallbackServiceOptions::default(),
            callback,
        )
    }

    pub fn create_service_callback_with_options<T, F>(
        &mut self, name: &str, qos: QosProfile, options: CallbackServiceOptions, mut callback: F,
    ) -> Result<CallbackService>
    where
        T: WrappedServiceTypeSupport + 'static,
        F: FnMut(T::Request, ServiceRequestInfo) -> Result<T::Response> + Send + 'static,
    {
        self.create_service_native_callback_with_options::<T, _>(
            name,
            qos,
            options,
            move |request, response, info| {
                let reply = callback(T::Request::from_native(&request), info)?;
                reply.try_copy_to_native(response)
            },
        )
    }

    /// Register a callback with a scoped native request and reusable owned response buffer.
    /// Request ownership cannot escape. No owned Rust request or channel send is needed.
    ///
    /// ```compile_fail
    /// use r2r::{Node, CallbackService, QosProfile, example_interfaces::srv::AddTwoInts};
    /// fn install(node: &mut Node) -> r2r::Result<CallbackService> {
    ///     let mut escaped = None;
    ///     node.create_service_native_callback::<AddTwoInts::Service, _>(
    ///         "/sum", QosProfile::default(), move |request, response, _| {
    ///             response.sum = request.a + request.b;
    ///             escaped = Some(request);
    ///             Ok(())
    ///         },
    ///     )
    /// }
    /// ```
    pub fn create_service_native_callback<T, F>(
        &mut self, name: &str, qos: QosProfile, callback: F,
    ) -> Result<CallbackService>
    where
        T: WrappedServiceTypeSupport + 'static,
        F: for<'request> FnMut(
                NativeMessageRef<'request, T::Request>,
                &mut WrappedNativeMsg<T::Response>,
                ServiceRequestInfo,
            ) -> Result<()>
            + Send
            + 'static,
    {
        self.create_service_native_callback_with_options::<T, F>(
            name,
            qos,
            CallbackServiceOptions::default(),
            callback,
        )
    }

    pub fn create_service_native_callback_with_options<T, F>(
        &mut self, name: &str, qos: QosProfile, options: CallbackServiceOptions, callback: F,
    ) -> Result<CallbackService>
    where
        T: WrappedServiceTypeSupport + 'static,
        F: for<'request> FnMut(
                NativeMessageRef<'request, T::Request>,
                &mut WrappedNativeMsg<T::Response>,
                ServiceRequestInfo,
            ) -> Result<()>
            + Send
            + 'static,
    {
        let service = NativeCallbackService::<T, F>::create(
            self.node_handle.as_mut(),
            name,
            qos,
            options,
            callback,
        )?;
        let wake = self.canceled_services.lock().wake().or_else(|| {
            self.future_wake_guard
                .as_ref()
                .map(|guard| guard.observer_waker())
        });
        let registry = self.service_callbacks.get_or_insert_with(Default::default);
        registry.set_wake(wake);
        Ok(registry.insert(service))
    }

    /// Destroy the callback endpoint immediately; a foreign token schedules its owning cleanup.
    pub fn destroy_callback_service(&mut self, service: CallbackService) {
        let id = service.id;
        drop(service);
        if let Some(registry) = &mut self.service_callbacks {
            if registry.remove(id, self.node_handle.as_mut()) {
                registry.canceled.lock().retain(|queued| *queued != id);
            }
        }
    }
    fn callback_endpoint(&self, id: uuid::Uuid) -> Result<&Endpoint> {
        self.service_callbacks
            .as_ref()
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?
            .endpoint(id)
    }
    pub(crate) fn callback_service_name(&self, id: uuid::Uuid) -> Result<String> {
        self.callback_endpoint(id)?.service_name()
    }
    pub(crate) fn callback_service_request_qos(&self, id: uuid::Uuid) -> Result<QosProfile> {
        self.callback_endpoint(id)?.request_qos()
    }
    pub(crate) fn callback_service_response_qos(&self, id: uuid::Uuid) -> Result<QosProfile> {
        self.callback_endpoint(id)?.response_qos()
    }

    #[cfg(not(r2r__ros__distro__humble))]
    pub(crate) fn configure_callback_service_introspection(
        &mut self, id: uuid::Uuid, state: crate::ServiceIntrospectionState, qos: QosProfile,
    ) -> Result<()> {
        let registry = self
            .service_callbacks
            .as_mut()
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        let index = *registry
            .indexes
            .get(&id)
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        let endpoint = registry.services[index].endpoint_mut();
        let mut clock = self.ros_clock.lock();
        crate::service_introspection::configure_service_native(
            &mut endpoint.handle,
            endpoint.type_support,
            self.node_handle.as_mut(),
            &mut clock,
            state,
            qos,
        )
    }
}

#[cfg(test)]
mod tests;
