//! Service client factories, registry bookkeeping and removal.

use super::*;

impl Node {
    pub(crate) fn client_identity<T: ?Sized>(client: &Arc<Mutex<T>>) -> usize {
        Arc::as_ptr(client).cast::<()>() as usize
    }

    pub(in crate::nodes) fn remove_client(&mut self, identity: usize) -> bool {
        let Some(index) = self.client_identities.get(&identity).copied() else {
            return true;
        };
        let client = &self.clients[index];
        let mut handle = client.lock();
        if handle.has_pending_work() {
            return false;
        }
        handle.destroy(self.node_handle.as_mut());
        drop(handle);

        self.client_identities.remove(&identity);
        self.clients.swap_remove(index);
        if let Some(moved) = self.clients.get(index) {
            self.client_identities
                .insert(Self::client_identity(moved), index);
        }
        true
    }

    pub(in crate::nodes) fn destroy_client_by_identity(&mut self, identity: usize) -> bool {
        let Some(index) = self.client_identities.remove(&identity) else {
            return false;
        };
        let client = self.clients.swap_remove(index);
        if let Some(moved) = self.clients.get(index) {
            self.client_identities
                .insert(Self::client_identity(moved), index);
        }
        self.pending_client_removals.remove(&identity);
        self.clients_with_active_availability.remove(&identity);
        self.dropped_clients
            .lock()
            .retain(|queued| *queued != identity);
        self.active_availability_clients
            .lock()
            .retain(|queued| *queued != identity);
        client.lock().destroy(self.node_handle.as_mut());
        true
    }

    pub(in crate::nodes) fn rollback_clients(&mut self, client_count: usize) {
        let mut removed_identities = HashSet::new();
        while self.clients.len() > client_count {
            let client = self.clients.pop().expect("client count checked");
            let identity = Self::client_identity(&client);
            self.client_identities.remove(&identity);
            self.clients_with_active_availability.remove(&identity);
            self.pending_client_removals.remove(&identity);
            removed_identities.insert(identity);
            client.lock().destroy(self.node_handle.as_mut());
        }
        self.dropped_clients
            .lock()
            .retain(|identity| !removed_identities.contains(identity));
        self.active_availability_clients
            .lock()
            .retain(|identity| !removed_identities.contains(identity));
    }

    /// Create a ROS service client.
    ///
    /// A service client is used to make requests to a ROS service server.
    pub fn create_client<T>(
        &mut self, service_name: &str, qos_profile: QosProfile,
    ) -> Result<Client<T>>
    where
        T: 'static + WrappedServiceTypeSupport,
    {
        let client_handle = create_client_helper(
            self.node_handle.as_mut(),
            service_name,
            T::get_ts(),
            qos_profile,
        )?;
        let ws = TypedClient::<T> {
            rcl_handle: client_handle,
            response_channels: HashMap::new(),
            poll_available_channels: AvailabilityWaiters::default(),
            #[cfg(test)]
            availability_poll_count: std::sync::atomic::AtomicUsize::new(0),
        };

        let client_arc = Arc::new(Mutex::new(ws));
        let identity = Self::client_identity(&client_arc);
        let c = make_client(
            Arc::downgrade(&client_arc),
            identity,
            Arc::clone(&self.dropped_clients),
            Arc::clone(&self.active_availability_clients),
        );
        self.client_identities.insert(identity, self.clients.len());
        self.clients.push(client_arc);
        Ok(c)
    }

    /// Creates a runtime-agnostic client for another node's standard parameter services.
    pub fn create_parameter_client(&mut self, remote_node_name: &str) -> Result<ParameterClient> {
        self.create_parameter_client_with_qos(remote_node_name, QosProfile::parameters())
    }

    /// Creates a parameter client with an explicit QoS profile for all six services.
    pub fn create_parameter_client_with_qos(
        &mut self, remote_node_name: &str, qos: QosProfile,
    ) -> Result<ParameterClient> {
        let client_count = self.clients.len();
        match ParameterClient::new(self, remote_node_name, qos) {
            Ok(client) => Ok(client),
            Err(err) => {
                self.rollback_clients(client_count);
                Err(err)
            }
        }
    }

    /// Creates a runtime-agnostic client for this node's standard parameter services.
    ///
    /// The fully qualified node name is read after ROS name remapping, so this remains correct
    /// when the node is namespaced or renamed through command-line arguments.
    pub fn create_parameter_client_for_self(&mut self) -> Result<ParameterClient> {
        self.create_parameter_client_for_self_with_qos(QosProfile::parameters())
    }

    /// Creates a parameter client for this node with an explicit service QoS profile.
    pub fn create_parameter_client_for_self_with_qos(
        &mut self, qos: QosProfile,
    ) -> Result<ParameterClient> {
        let node_name = self.fully_qualified_name()?;
        self.create_parameter_client_with_qos(&node_name, qos)
    }

    /// Create a ROS service client.
    ///
    /// A service client is used to make requests to a ROS service
    /// server. This function returns an `UntypedClient`, which deals
    /// with `serde_json::Value`s instead of concrete types.  Useful
    /// when you cannot know the type of the message at compile time.
    pub fn create_client_untyped(
        &mut self, service_name: &str, service_type: &str, qos_profile: QosProfile,
    ) -> Result<ClientUntyped> {
        let service_type = UntypedServiceSupport::new_from(service_type)?;
        let client_handle = create_client_helper(
            self.node_handle.as_mut(),
            service_name,
            service_type.ts,
            qos_profile,
        )?;
        let client = UntypedClient_ {
            service_type,
            rcl_handle: client_handle,
            response_channels: HashMap::new(),
            poll_available_channels: AvailabilityWaiters::default(),
            #[cfg(test)]
            availability_poll_count: std::sync::atomic::AtomicUsize::new(0),
        };

        let client_arc = Arc::new(Mutex::new(client));
        let identity = Self::client_identity(&client_arc);
        let c = make_untyped_client(
            Arc::downgrade(&client_arc),
            identity,
            Arc::clone(&self.dropped_clients),
            Arc::clone(&self.active_availability_clients),
        );
        self.client_identities.insert(identity, self.clients.len());
        self.clients.push(client_arc);
        Ok(c)
    }

    /// Immediately destroys a typed service client owned by this node.
    ///
    /// Pending response and availability futures are closed. Passing a client created by another
    /// node leaves this node unchanged and schedules normal cleanup by the owning node.
    pub fn destroy_client<T>(&mut self, client: Client<T>)
    where
        T: WrappedServiceTypeSupport,
    {
        let identity = client.identity();
        if self.destroy_client_by_identity(identity) {
            client.mark_explicitly_destroyed();
        }
    }

    /// Immediately destroys an untyped service client owned by this node.
    ///
    /// Pending response and availability futures are closed before this method returns.
    pub fn destroy_client_untyped(&mut self, client: ClientUntyped) {
        let identity = client.identity();
        if self.destroy_client_by_identity(identity) {
            client.mark_explicitly_destroyed();
        }
    }
}
