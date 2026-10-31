//! Action server factories, registry bookkeeping and removal.

use super::*;
use crate::native_buffer::NativeBuffer;

impl Node {
    fn add_action_server(
        &mut self, id: uuid::Uuid, server: Arc<Mutex<dyn ActionServer_>>,
        capacities: WaitSetCapacities,
    ) {
        let index = self.action_servers.len();
        let previous = self.action_server_indexes.insert(id, index);
        debug_assert!(previous.is_none());
        self.action_servers.push(server);
        self.action_wait_set_capacities.add(capacities);
    }

    pub(in crate::nodes) fn remove_action_server(&mut self, id: uuid::Uuid) -> Result<bool> {
        let Some(index) = self.action_server_indexes.get(&id).copied() else {
            return Ok(true);
        };
        let server = &self.action_servers[index];
        let mut handle = server.lock();
        if !handle.should_remove() {
            return Ok(false);
        }
        let capacities = action_server_wait_set_capacities(handle.handle())?;
        handle.destroy(self.node_handle.as_mut());
        drop(handle);

        self.action_server_indexes.remove(&id);
        self.action_servers.swap_remove(index);
        if let Some(moved) = self.action_servers.get(index) {
            self.action_server_indexes.insert(moved.lock().id(), index);
        }
        self.dropped_action_servers.lock().remove(id);
        self.action_wait_set_capacities.subtract(capacities);
        Ok(true)
    }

    fn destroy_action_server_by_id(&mut self, id: uuid::Uuid) -> Result<bool> {
        let Some(index) = self.action_server_indexes.get(&id).copied() else {
            return Ok(false);
        };
        let capacities = {
            let server = self.action_servers[index].lock();
            action_server_wait_set_capacities(server.handle())?
        };
        let server = self.action_servers.swap_remove(index);
        self.action_server_indexes.remove(&id);
        if let Some(moved) = self.action_servers.get(index) {
            self.action_server_indexes.insert(moved.lock().id(), index);
        }
        self.pending_action_server_removals.remove(&id);
        self.dropped_action_servers.lock().remove(id);
        server.lock().destroy(self.node_handle.as_mut());
        self.action_wait_set_capacities.subtract(capacities);
        Ok(true)
    }

    pub(crate) fn get_action_server_name(&self, id: uuid::Uuid) -> Result<String> {
        let index = self
            .action_server_indexes
            .get(&id)
            .copied()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        self.action_servers[index].lock().action_name()
    }

    /// Create a ROS action server.
    ///
    /// This function returns a stream of `GoalRequest`s, which needs
    /// to be either accepted or rejected.
    pub fn create_action_server<T>(&mut self, action_name: &str) -> Result<ActionServer<T>>
    where
        T: 'static + WrappedActionTypeSupport,
    {
        self.create_action_server_with_options(action_name, ActionServerOptions::default())
    }

    /// Create a ROS action server with per-channel QoS and result-retention overrides.
    pub fn create_action_server_with_options<T>(
        &mut self, action_name: &str, options: ActionServerOptions,
    ) -> Result<ActionServer<T>>
    where
        T: 'static + WrappedActionTypeSupport,
    {
        options.validate()?;
        let goal_request_channel_capacity = options.goal_request_channel_capacity;
        let cancel_request_channel_capacity = options.cancel_request_channel_capacity;
        let (goal_request_sender, goal_request_receiver) =
            mpsc::channel::<ActionServerGoalRequest<T>>(goal_request_channel_capacity);

        let clock = self.ros_clock.clone();
        #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
        let resolved_action_name = self.resolve_action_name(action_name)?;
        #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
        let action_name = resolved_action_name.as_str();
        let mut server_handle = {
            let mut clock_handle = clock.lock();
            create_action_server_helper(
                self.node_handle.as_mut(),
                action_name,
                clock_handle.clock_handle.as_mut(),
                T::try_get_ts()?,
                &options,
            )?
        };
        let action_capacities = match action_server_wait_set_capacities(&server_handle) {
            Ok(capacities) => capacities,
            Err(err) => {
                unsafe {
                    rcl_action_server_fini(&mut server_handle, self.node_handle.as_mut());
                }
                return Err(err);
            }
        };
        let id = uuid::Uuid::new_v4();
        let server = WrappedActionServer::<T> {
            id,
            rcl_handle: server_handle,
            clock,
            goal_request_sender: Arc::new(Mutex::new(Some(goal_request_sender))),
            pending_cancel_request: None,
            active_cancel_requests: Default::default(),
            cancel_senders: HashMap::new(),
            goals: HashMap::new(),
            result_msgs: HashMap::new(),
            result_requests: HashMap::new(),
            goal_request_buffer: NativeBuffer::default(),
            result_request_buffer: NativeBuffer::default(),
            cancel_request_channel_capacity,
            maintenance_wake_owner: Arc::downgrade(&self.dropped_action_servers),
        };

        let server_arc = Arc::new(Mutex::new(server));
        self.add_action_server(id, server_arc, action_capacities);
        Ok(ActionServer::new(
            goal_request_receiver,
            id,
            Arc::clone(&self.dropped_action_servers),
        ))
    }

    /// Creates a runtime-typed ROS action server whose goal, feedback, and result use JSON.
    pub fn create_action_server_untyped(
        &mut self, action_name: &str, action_type: &str,
    ) -> Result<ActionServerUntyped> {
        self.create_action_server_untyped_with_options(
            action_name,
            action_type,
            ActionServerOptions::default(),
        )
    }

    /// Creates a runtime-typed ROS action server with explicit QoS and buffering options.
    pub fn create_action_server_untyped_with_options(
        &mut self, action_name: &str, action_type: &str, options: ActionServerOptions,
    ) -> Result<ActionServerUntyped> {
        options.validate()?;
        let support = UntypedActionSupport::new_from(action_type)?;
        (support.create_server)(self, action_name, options)
    }

    /// Immediately destroys a typed action server owned by this node.
    ///
    /// Pending goal and cancel streams are closed before this method returns.
    pub fn destroy_action_server<T>(&mut self, server: ActionServer<T>) -> Result<()>
    where
        T: WrappedActionTypeSupport,
    {
        let id = server.id;
        drop(server);
        self.destroy_action_server_by_id(id)?;
        Ok(())
    }

    /// Immediately destroys a runtime-typed action server owned by this node.
    ///
    /// Passing a server created by another node leaves this node unchanged and schedules normal
    /// cleanup by its owning node.
    pub fn destroy_action_server_untyped(&mut self, server: ActionServerUntyped) -> Result<()> {
        let id = server.id;
        drop(server);
        self.destroy_action_server_by_id(id)?;
        Ok(())
    }
}
