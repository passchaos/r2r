//! Service factories, registry bookkeeping and per-service accessors.

use super::*;

impl Node {
    pub(crate) fn add_service(&mut self, id: uuid::Uuid, service: Arc<Mutex<dyn Service_>>) {
        let index = self.services.len();
        let previous = self.service_indexes.insert(id, index);
        debug_assert!(previous.is_none());
        self.services.push(service);
    }

    pub(in crate::nodes) fn remove_service(
        services: &mut Vec<Arc<Mutex<dyn Service_>>>,
        service_indexes: &mut HashMap<uuid::Uuid, usize>, id: uuid::Uuid,
        node_handle: &mut rcl_node_t,
    ) -> bool {
        let Some(index) = service_indexes.remove(&id) else {
            return false;
        };
        let removed = services.swap_remove(index);
        {
            let mut removed = removed.lock();
            debug_assert_eq!(removed.id(), Some(id));
            removed.destroy(node_handle);
        }
        if let Some(moved) = services.get(index) {
            if let Some(moved_id) = moved.lock().id() {
                service_indexes.insert(moved_id, index);
            }
        }
        true
    }

    pub(crate) fn service(&self, id: uuid::Uuid) -> Result<&Arc<Mutex<dyn Service_>>> {
        let index = self
            .service_indexes
            .get(&id)
            .copied()
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        self.services
            .get(index)
            .ok_or(Error::RCL_RET_SERVICE_INVALID)
    }

    /// Create a ROS service.
    ///
    /// This function returns a `Stream` of `ServiceRequest`:s. Call
    /// `respond` on the Service Request to send the reply.
    pub fn create_service<T>(
        &mut self, service_name: &str, qos_profile: QosProfile,
    ) -> Result<Service<T>>
    where
        T: 'static + WrappedServiceTypeSupport,
    {
        self.create_service_with_options(service_name, qos_profile, ServiceOptions::default())
    }

    /// Create a ROS service with explicit Rust-side request stream buffering options.
    pub fn create_service_with_options<T>(
        &mut self, service_name: &str, qos_profile: QosProfile, options: ServiceOptions,
    ) -> Result<Service<T>>
    where
        T: 'static + WrappedServiceTypeSupport,
    {
        let options = options.validate()?;
        let service_handle = create_service_helper(
            self.node_handle.as_mut(),
            service_name,
            T::get_ts(),
            qos_profile,
        )?;
        let (sender, receiver) =
            mpsc::channel::<ServiceRequest<T>>(options.request_channel_capacity);
        let id = uuid::Uuid::new_v4();

        let ws = TypedService::<T> {
            rcl_handle: service_handle,
            sender: Arc::new(Mutex::new(Some(sender))),
            id,
        };

        self.add_service(id, Arc::new(Mutex::new(ws)));
        Ok(Service::new(receiver, id, Arc::clone(&self.canceled_services)))
    }

    /// Creates a runtime-typed ROS service whose requests and responses use JSON values.
    pub fn create_service_untyped(
        &mut self, service_name: &str, service_type: &str, qos_profile: QosProfile,
    ) -> Result<ServiceUntyped> {
        self.create_service_untyped_with_options(
            service_name,
            service_type,
            qos_profile,
            ServiceOptions::default(),
        )
    }

    /// Creates a runtime-typed ROS service with explicit request stream buffering.
    pub fn create_service_untyped_with_options(
        &mut self, service_name: &str, service_type: &str, qos_profile: QosProfile,
        options: ServiceOptions,
    ) -> Result<ServiceUntyped> {
        let options = options.validate()?;
        let service_type = UntypedServiceSupport::new_from(service_type)?;
        let service_handle = create_service_helper(
            self.node_handle.as_mut(),
            service_name,
            service_type.ts,
            qos_profile,
        )?;
        let (sender, receiver) =
            mpsc::channel::<ServiceRequestUntyped>(options.request_channel_capacity);
        let id = uuid::Uuid::new_v4();
        let service = UntypedService {
            service_type,
            rcl_handle: service_handle,
            sender: Arc::new(Mutex::new(Some(sender))),
            id,
        };
        self.add_service(id, Arc::new(Mutex::new(service)));
        Ok(ServiceUntyped::new(receiver, id, Arc::clone(&self.canceled_services)))
    }

    /// Immediately destroys a typed service owned by this node.
    ///
    /// Dropping a service schedules its native endpoint for removal during the next spin. This
    /// method is useful when deterministic teardown is required before another spin cycle.
    pub fn destroy_service<T>(&mut self, service: Service<T>)
    where
        T: WrappedServiceTypeSupport,
    {
        let id = service.id;
        drop(service);
        if Self::remove_service(
            &mut self.services,
            &mut self.service_indexes,
            id,
            self.node_handle.as_mut(),
        ) {
            self.canceled_services.lock().retain(|queued| *queued != id);
        }
    }

    /// Immediately destroys an untyped service owned by this node.
    ///
    /// Passing a service created by another node leaves this node unchanged and schedules the
    /// service for normal cleanup by its owning node.
    pub fn destroy_service_untyped(&mut self, service: ServiceUntyped) {
        let id = service.id;
        drop(service);
        if Self::remove_service(
            &mut self.services,
            &mut self.service_indexes,
            id,
            self.node_handle.as_mut(),
        ) {
            self.canceled_services.lock().retain(|queued| *queued != id);
        }
    }

    pub(crate) fn get_service_request_subscription_actual_qos(
        &self, id: uuid::Uuid,
    ) -> Result<QosProfile> {
        let service = self.service(id)?.lock();
        service.request_subscription_actual_qos()
    }

    pub(crate) fn get_service_response_publisher_actual_qos(
        &self, id: uuid::Uuid,
    ) -> Result<QosProfile> {
        let service = self.service(id)?.lock();
        service.response_publisher_actual_qos()
    }

    pub(crate) fn get_service_name(&self, id: uuid::Uuid) -> Result<String> {
        let service = self.service(id)?.lock();
        service.service_name()
    }

    #[cfg(any(
        r2r__ros__distro__iron,
        r2r__ros__distro__jazzy,
        r2r__ros__distro__kilted,
        r2r__ros__distro__lyrical,
        r2r__ros__distro__rolling
    ))]
    pub(crate) fn configure_service_introspection(
        &mut self, id: uuid::Uuid, state: crate::ServiceIntrospectionState, event_qos: QosProfile,
    ) -> Result<()> {
        let service = self.service(id)?.clone();
        let mut service = service.lock();
        service.configure_introspection(self, state, event_qos)
    }
}
