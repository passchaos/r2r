//! Action client factories, registry bookkeeping and removal.

use super::*;

impl Node {
    pub(crate) fn action_client_identity<T: ?Sized>(client: &Arc<Mutex<T>>) -> usize {
        Arc::as_ptr(client).cast::<()>() as usize
    }

    pub(in crate::nodes) fn remove_action_client(&mut self, identity: usize) -> Result<bool> {
        let Some(index) = self.action_client_identities.get(&identity).copied() else {
            return Ok(true);
        };
        let client = &self.action_clients[index];
        let mut handle = client.lock();
        if handle.has_pending_work() {
            return Ok(false);
        }
        let capacities = action_client_wait_set_capacities(handle.handle())?;
        handle.disable_maintenance_notifications();
        handle.destroy(self.node_handle.as_mut());
        drop(handle);

        self.action_client_identities.remove(&identity);
        self.action_clients.swap_remove(index);
        if let Some(moved) = self.action_clients.get(index) {
            self.action_client_identities
                .insert(Self::action_client_identity(moved), index);
        }
        self.action_clients_with_pending_maintenance
            .remove(&identity);
        self.action_wait_set_capacities.subtract(capacities);
        Ok(true)
    }

    pub(in crate::nodes) fn destroy_action_client_by_identity(
        &mut self, identity: usize,
    ) -> Result<bool> {
        let Some(index) = self.action_client_identities.get(&identity).copied() else {
            return Ok(false);
        };
        let capacities = {
            let client = self.action_clients[index].lock();
            let capacities = action_client_wait_set_capacities(client.handle())?;
            client.disable_maintenance_notifications();
            capacities
        };
        let client = self.action_clients.swap_remove(index);
        self.action_client_identities.remove(&identity);
        if let Some(moved) = self.action_clients.get(index) {
            self.action_client_identities
                .insert(Self::action_client_identity(moved), index);
        }
        self.pending_action_client_removals.remove(&identity);
        self.action_clients_with_active_availability
            .remove(&identity);
        self.action_clients_with_pending_maintenance
            .remove(&identity);
        self.dropped_action_clients
            .lock()
            .retain(|queued| *queued != identity);
        self.active_availability_action_clients
            .lock()
            .retain(|queued| *queued != identity);
        self.active_maintenance_action_clients
            .lock()
            .retain(|queued| *queued != identity);
        client.lock().destroy(self.node_handle.as_mut());
        self.action_wait_set_capacities.subtract(capacities);
        Ok(true)
    }

    pub(crate) fn owns_action_client<T>(&self, client: &Arc<Mutex<T>>) -> bool
    where
        T: ActionClient_ + ?Sized,
    {
        self.action_client_identities
            .contains_key(&Self::action_client_identity(client))
    }

    #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
    fn resolve_action_name(&self, name: &str) -> Result<String> {
        self.resolve_topic_name(name).map_err(|error| match error {
            Error::RCL_RET_TOPIC_NAME_INVALID => Error::RCL_RET_ACTION_NAME_INVALID,
            error => error,
        })
    }

    /// Create a ROS action client.
    ///
    /// An action client is used to make requests to a ROS action server.
    pub fn create_action_client<T>(&mut self, action_name: &str) -> Result<ActionClient<T>>
    where
        T: 'static + WrappedActionTypeSupport,
    {
        self.create_action_client_with_options(action_name, ActionClientOptions::default())
    }

    /// Create a ROS action client with per-channel QoS overrides.
    pub fn create_action_client_with_options<T>(
        &mut self, action_name: &str, options: ActionClientOptions,
    ) -> Result<ActionClient<T>>
    where
        T: 'static + WrappedActionTypeSupport,
    {
        options.validate()?;
        #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
        let resolved_action_name = self.resolve_action_name(action_name)?;
        #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
        let action_name = resolved_action_name.as_str();
        let mut client_handle = create_action_client_helper(
            self.node_handle.as_mut(),
            action_name,
            T::get_ts(),
            &options,
        )?;
        let action_capacities = match action_client_wait_set_capacities(&client_handle) {
            Ok(capacities) => capacities,
            Err(err) => {
                unsafe {
                    rcl_action_client_fini(&mut client_handle, self.node_handle.as_mut());
                }
                return Err(err);
            }
        };
        let maintenance_notifier = Arc::new(ActionClientMaintenanceNotifier::new(Arc::clone(
            &self.active_maintenance_action_clients,
        )));
        let client = WrappedActionClient::<T> {
            rcl_handle: client_handle,
            goal_response_channels: HashMap::new(),
            goal_response_sequences: HashMap::new(),
            cancel_response_channels: HashMap::new(),
            canceled_cancel_requests: Arc::new(Mutex::new(ActionClientMaintenanceQueue::new(
                Arc::clone(&maintenance_notifier),
            ))),
            feedback_senders: HashMap::new(),
            canceled_feedback_goals: Arc::new(Mutex::new(ActionClientMaintenanceQueue::new(
                Arc::clone(&maintenance_notifier),
            ))),
            pending_feedback_goals: Arc::new(Mutex::new(PendingFeedbackQueue::new(Arc::clone(
                &maintenance_notifier,
            )))),
            result_senders: HashMap::new(),
            canceled_result_goals: Arc::new(Mutex::new(ActionClientMaintenanceQueue::new(
                Arc::clone(&maintenance_notifier),
            ))),
            result_requests: HashMap::new(),
            result_request_sequences: HashMap::new(),
            goal_status: HashMap::new(),
            feedback_channel_capacity: options.feedback_channel_capacity,
            poll_available_channels: AvailabilityWaiters::default(),
        };

        let client_arc = Arc::new(Mutex::new(client));
        let identity = Self::action_client_identity(&client_arc);
        maintenance_notifier.set_identity(identity);
        self.action_client_identities
            .insert(identity, self.action_clients.len());
        self.action_clients.push(client_arc.clone());
        self.action_wait_set_capacities.add(action_capacities);
        let c = make_action_client(
            Arc::downgrade(&client_arc),
            identity,
            Arc::clone(&self.dropped_action_clients),
            Arc::clone(&self.active_availability_action_clients),
        );
        Ok(c)
    }

    /// Create a ROS action client.
    ///
    /// A action client is used to make requests to a ROS service
    /// server. This function returns a `ActionClientUntyped`, which deals
    /// with `serde_json::Value`s instead of concrete types.  Useful
    /// when you cannot know the type of the message at compile time.
    pub fn create_action_client_untyped(
        &mut self, action_name: &str, action_type: &str,
    ) -> Result<ActionClientUntyped> {
        self.create_action_client_untyped_with_options(
            action_name,
            action_type,
            ActionClientOptions::default(),
        )
    }

    /// Create an untyped ROS action client with per-channel QoS overrides.
    pub fn create_action_client_untyped_with_options(
        &mut self, action_name: &str, action_type: &str, options: ActionClientOptions,
    ) -> Result<ActionClientUntyped> {
        options.validate()?;
        let action_type_support = UntypedActionSupport::new_from(action_type)?;
        #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
        let resolved_action_name = self.resolve_action_name(action_name)?;
        #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
        let action_name = resolved_action_name.as_str();
        let mut client_handle = create_action_client_helper(
            self.node_handle.as_mut(),
            action_name,
            action_type_support.ts,
            &options,
        )?;
        let action_capacities = match action_client_wait_set_capacities(&client_handle) {
            Ok(capacities) => capacities,
            Err(err) => {
                unsafe {
                    rcl_action_client_fini(&mut client_handle, self.node_handle.as_mut());
                }
                return Err(err);
            }
        };
        let maintenance_notifier = Arc::new(ActionClientMaintenanceNotifier::new(Arc::clone(
            &self.active_maintenance_action_clients,
        )));
        let client = WrappedActionClientUntyped {
            action_type_support,
            rcl_handle: client_handle,
            goal_response_channels: HashMap::new(),
            goal_response_sequences: HashMap::new(),
            cancel_response_channels: HashMap::new(),
            canceled_cancel_requests: Arc::new(Mutex::new(ActionClientMaintenanceQueue::new(
                Arc::clone(&maintenance_notifier),
            ))),
            feedback_senders: HashMap::new(),
            canceled_feedback_goals: Arc::new(Mutex::new(ActionClientMaintenanceQueue::new(
                Arc::clone(&maintenance_notifier),
            ))),
            pending_feedback_goals: Arc::new(Mutex::new(PendingFeedbackQueue::new(Arc::clone(
                &maintenance_notifier,
            )))),
            result_senders: HashMap::new(),
            canceled_result_goals: Arc::new(Mutex::new(ActionClientMaintenanceQueue::new(
                Arc::clone(&maintenance_notifier),
            ))),
            result_requests: HashMap::new(),
            result_request_sequences: HashMap::new(),
            goal_status: HashMap::new(),
            feedback_channel_capacity: options.feedback_channel_capacity,
            poll_available_channels: AvailabilityWaiters::default(),
        };

        let client_arc = Arc::new(Mutex::new(client));
        let identity = Self::action_client_identity(&client_arc);
        maintenance_notifier.set_identity(identity);
        self.action_client_identities
            .insert(identity, self.action_clients.len());
        self.action_clients.push(client_arc.clone());
        self.action_wait_set_capacities.add(action_capacities);
        let c = make_action_client_untyped(
            Arc::downgrade(&client_arc),
            identity,
            Arc::clone(&self.dropped_action_clients),
            Arc::clone(&self.active_availability_action_clients),
        );
        Ok(c)
    }

    /// Immediately destroys a typed action client owned by this node.
    ///
    /// Pending goal, result, cancel, feedback, and availability operations are closed. Passing a
    /// client created by another node leaves this node unchanged and schedules normal cleanup by
    /// the owning node.
    pub fn destroy_action_client<T>(&mut self, client: ActionClient<T>) -> Result<()>
    where
        T: 'static + WrappedActionTypeSupport,
    {
        let identity = client.identity();
        if self.destroy_action_client_by_identity(identity)? {
            client.mark_explicitly_destroyed();
        }
        Ok(())
    }

    /// Immediately destroys an untyped action client owned by this node.
    ///
    /// Pending operations are closed before this method returns.
    pub fn destroy_action_client_untyped(&mut self, client: ActionClientUntyped) -> Result<()> {
        let identity = client.identity();
        if self.destroy_action_client_by_identity(identity)? {
            client.mark_explicitly_destroyed();
        }
        Ok(())
    }
}
