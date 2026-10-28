//! Node construction: rcl initialization, parameter-override loading
//! and the full field initialization path.

use super::*;

impl Node {
    fn append_matching_parameter_overrides(
        &self, params: RclParams, overrides: &mut IndexMap<String, ParameterValue>,
    ) -> Result<()> {
        if params.0.is_null() {
            return Ok(());
        }

        let num_nodes = unsafe { (*params.0).num_nodes };
        if num_nodes == 0 {
            return Ok(());
        }
        unsafe {
            if (*params.0).node_names.is_null() || (*params.0).params.is_null() {
                return Err(Error::RCL_RET_ERROR);
            }
        }

        let node_names = unsafe { std::slice::from_raw_parts((*params.0).node_names, num_nodes) };
        let node_params = unsafe { std::slice::from_raw_parts((*params.0).params, num_nodes) };
        let qualified_name = self.fully_qualified_name()?;

        for (nn, np) in node_names.iter().zip(node_params) {
            if nn.is_null() {
                return Err(Error::RCL_RET_ERROR);
            }
            let node_name_cstr = unsafe { CStr::from_ptr(*nn) };
            let node_name = c_str_to_str(node_name_cstr, "parameter override node name")?;
            if !parameter_rule_matches_node(node_name, &qualified_name) {
                continue;
            }
            if np.num_params == 0 {
                continue;
            }
            if np.parameter_names.is_null() || np.parameter_values.is_null() {
                return Err(Error::RCL_RET_ERROR);
            }
            let param_names =
                unsafe { std::slice::from_raw_parts(np.parameter_names, np.num_params) };
            let param_values =
                unsafe { std::slice::from_raw_parts(np.parameter_values, np.num_params) };
            for (name, value) in param_names.iter().zip(param_values) {
                if name.is_null() {
                    return Err(Error::RCL_RET_ERROR);
                }
                let name = unsafe { CStr::from_ptr(*name) };
                let name = c_str_to_str(name, "parameter override name")?;
                overrides.insert(name.to_owned(), ParameterValue::from_rcl(value)?);
            }
        }
        Ok(())
    }

    fn load_parameter_overrides(
        &self, local_arguments: &rcl_arguments_t, use_global_arguments: bool,
    ) -> Result<IndexMap<String, ParameterValue>> {
        let mut overrides = IndexMap::new();
        if use_global_arguments {
            let params = parameter_overrides(&self.context.context_handle).map_err(|err| {
                log::error!("could not read global parameters: {err}");
                err
            })?;
            self.append_matching_parameter_overrides(params, &mut overrides)?;
        }
        let params = parameter_overrides_from_arguments(local_arguments).map_err(|err| {
            log::error!("could not read parameters: {err}");
            err
        })?;
        self.append_matching_parameter_overrides(params, &mut overrides)?;
        Ok(overrides)
    }

    /// Creates a ROS node.
    pub fn create(ctx: Context, name: &str, namespace: &str) -> Result<Node> {
        Self::create_with_options(ctx, name, namespace, NodeOptions::default())
    }

    /// Creates a ROS node with explicit initialization options.
    pub fn create_with_options(
        ctx: Context, name: &str, namespace: &str, options: NodeOptions,
    ) -> Result<Node> {
        let c_node_name = CString::new(name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let c_node_ns = CString::new(namespace).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let node_options = NodeOptionsNative::new(&options)?;
        let (res, node_handle) = {
            let mut ctx_handle = ctx.context_handle.lock();

            let mut node_handle: Box<rcl_node_t> =
                unsafe { Box::new(rcl_get_zero_initialized_node()) };
            let res = unsafe {
                rcl_node_init(
                    node_handle.as_mut(),
                    c_node_name.as_ptr(),
                    c_node_ns.as_ptr(),
                    ctx_handle.as_mut(),
                    &node_options.0 as *const _,
                )
            };
            (res, node_handle)
        };

        if res == RCL_RET_OK as i32 {
            let ros_clock = match Clock::create(ClockType::RosTime) {
                Ok(clock) => Arc::new(Mutex::new(clock)),
                Err(err) => {
                    let mut node_handle = node_handle;
                    unsafe {
                        rcl_node_fini(node_handle.as_mut());
                    }
                    return Err(err);
                }
            };
            #[cfg(r2r__rosgraph_msgs__msg__Clock)]
            let time_source = TimeSource::new();

            let mut node = Node {
                params: Arc::new(Mutex::new(IndexMap::new())),
                parameter_overrides: IndexMap::new(),
                automatically_declare_parameters_from_overrides: options
                    .automatically_declare_parameters_from_overrides,
                allow_undeclared_parameters: options.allow_undeclared_parameters,
                parameter_descriptors: Arc::new(Mutex::new(IndexMap::new())),
                parameter_provider: None,
                parameter_transaction: Arc::new(Mutex::new(())),
                parameter_pre_set_callbacks: Arc::new(Mutex::new(Default::default())),
                parameter_validation_callbacks: Arc::new(Mutex::new(Default::default())),
                parameter_post_set_callbacks: Arc::new(Mutex::new(Default::default())),
                parameter_event_sink: None,
                parameter_handler_started: false,
                node_handle,
                subscribers: Vec::new(),
                subscription_indexes: HashMap::new(),
                canceled_subscriptions: Arc::new(Mutex::new(VecDeque::new())),
                subscriptions_with_active_waits: HashSet::new(),
                services: Vec::new(),
                service_indexes: HashMap::new(),
                canceled_services: Arc::new(Mutex::new(VecDeque::new())),
                clients: Vec::new(),
                client_identities: HashMap::new(),
                dropped_clients: Arc::new(Mutex::new(VecDeque::new())),
                pending_client_removals: HashSet::new(),
                active_availability_clients: Arc::new(Mutex::new(VecDeque::new())),
                clients_with_active_availability: HashSet::new(),
                action_clients: Vec::new(),
                action_client_identities: HashMap::new(),
                dropped_action_clients: Arc::new(Mutex::new(VecDeque::new())),
                pending_action_client_removals: HashSet::new(),
                active_availability_action_clients: Arc::new(Mutex::new(VecDeque::new())),
                action_clients_with_active_availability: HashSet::new(),
                active_maintenance_action_clients: Arc::new(Mutex::new(VecDeque::new())),
                action_clients_with_pending_maintenance: HashSet::new(),
                action_servers: Vec::new(),
                action_server_indexes: HashMap::new(),
                dropped_action_servers: Arc::new(Mutex::new(VecDeque::new())),
                pending_action_server_removals: HashSet::new(),
                action_servers_with_pending_cancels: HashSet::new(),
                action_wait_set_capacities: WaitSetCapacities::default(),
                timers: Vec::new(),
                timer_indexes: HashMap::new(),
                canceled_timers: Arc::new(Mutex::new(VecDeque::new())),
                pubs: Vec::new(),
                publisher_indexes: HashMap::new(),
                dropped_publishers: Arc::new(Mutex::new(VecDeque::new())),
                pending_publisher_removals: HashSet::new(),
                active_wait_publishers: Arc::new(Mutex::new(VecDeque::new())),
                publishers_with_active_waits: HashSet::new(),
                ros_clock,
                #[cfg(r2r__rosgraph_msgs__msg__Clock)]
                time_source,
                graph_change_waiters: AvailabilityWaiters::default(),
                wait_set: WaitSet::new(),
                future_wake_guard: None,
                future_wake_state: None,
                context: ctx,
            };
            #[cfg(r2r__rosgraph_msgs__msg__Clock)]
            node.time_source
                .attach_ros_clock(Arc::downgrade(&node.ros_clock))?;
            let mut parameter_overrides = node.load_parameter_overrides(
                &node_options.0.arguments,
                options.use_global_arguments,
            )?;
            parameter_overrides.extend(options.parameter_overrides);
            if options.automatically_declare_parameters_from_overrides {
                node.params.lock().extend(
                    parameter_overrides
                        .iter()
                        .map(|(name, value)| (name.clone(), Parameter::new(value.clone()))),
                );
            }
            node.parameter_overrides = parameter_overrides;
            Ok(node)
        } else {
            log::error!("could not create node{}", res);
            Err(Error::from_rcl_error(res))
        }
    }
}
