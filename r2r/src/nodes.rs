use futures::{
    channel::{mpsc, oneshot},
    future::{self, join_all, FutureExt, TryFutureExt},
    stream::{Stream, StreamExt},
};
use indexmap::IndexMap;
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    future::Future,
    marker::PhantomPinned,
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use r2r_actions::*;
use r2r_rcl::*;

#[cfg(r2r__rosgraph_msgs__msg__Clock)]
use crate::time_source::TimeSource;
use crate::{
    action_clients::*,
    action_clients_untyped::*,
    action_servers::*,
    clients::*,
    clocks::*,
    context::*,
    error::*,
    msg_types::{generated_msgs::rcl_interfaces, *},
    parameters::*,
    publishers::*,
    qos::QosProfile,
    services::*,
    subscribers::*,
};

struct RclParams(*mut rcl_params_t);

impl Drop for RclParams {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { rcl_yaml_node_struct_fini(self.0) };
        }
    }
}

/// A ROS Node.
///
/// This struct owns all subscribes, publishers, etc.  To get events
/// from the ROS network into your ros application, `spin_once` should
/// be called continously.
pub struct Node {
    /// ROS parameters.
    pub params: Arc<Mutex<IndexMap<String, Parameter>>>,
    pub(crate) node_handle: Box<rcl_node_t>,
    // the node owns the subscribers
    pub(crate) subscribers: Vec<Box<dyn Subscriber_>>,
    // services,
    services: Vec<Arc<Mutex<dyn Service_>>>,
    // service clients
    clients: Vec<Arc<Mutex<dyn Client_>>>,
    // action clients
    action_clients: Vec<Arc<Mutex<dyn ActionClient_>>>,
    // action servers
    action_servers: Vec<Arc<Mutex<dyn ActionServer_>>>,
    action_wait_set_capacities: WaitSetCapacities,
    // timers,
    timers: Vec<Timer_>,
    // and the publishers, whom we allow to be shared.. hmm.
    pub(crate) pubs: Vec<Arc<Publisher_>>,
    // RosTime clock used by all timers created by create_timer()
    ros_clock: Arc<Mutex<Clock>>,
    // time source that provides simulated time
    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    time_source: TimeSource,
    wait_set: WaitSet,
    // Keep the ROS context alive until all other node resources have been dropped.
    context: Context,
}

unsafe impl Send for Node {}

impl Node {
    pub(crate) fn owns_publisher(&self, publisher: &Arc<Publisher_>) -> bool {
        self.pubs
            .iter()
            .any(|candidate| Arc::ptr_eq(candidate, publisher))
    }

    /// Returns the name of the node.
    pub fn name(&self) -> Result<String> {
        let cstr = unsafe { rcl_node_get_name(self.node_handle.as_ref()) };
        if cstr.is_null() {
            return Err(Error::RCL_RET_NODE_INVALID);
        }
        let s = unsafe { CStr::from_ptr(cstr) };
        c_str_to_owned(s, "node name")
    }

    /// Returns the fully qualified name of the node.
    pub fn fully_qualified_name(&self) -> Result<String> {
        let cstr = unsafe { rcl_node_get_fully_qualified_name(self.node_handle.as_ref()) };
        if cstr.is_null() {
            return Err(Error::RCL_RET_NODE_INVALID);
        }
        let s = unsafe { CStr::from_ptr(cstr) };
        c_str_to_owned(s, "fully qualified node name")
    }

    /// Returns the namespace of the node.
    pub fn namespace(&self) -> Result<String> {
        let cstr = unsafe { rcl_node_get_namespace(self.node_handle.as_ref()) };
        if cstr.is_null() {
            return Err(Error::RCL_RET_NODE_INVALID);
        }
        let s = unsafe { CStr::from_ptr(cstr) };
        c_str_to_owned(s, "node namespace")
    }

    fn load_params(&mut self) -> Result<()> {
        let ctx = self.context.context_handle.lock();
        let mut params = RclParams(std::ptr::null_mut());

        let ret =
            unsafe { rcl_arguments_get_param_overrides(&ctx.global_arguments, &mut params.0) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not read parameters: {}", ret);
            return Err(Error::from_rcl_error(ret));
        }

        if params.0.is_null() {
            return Ok(());
        }

        unsafe {
            if (*params.0).node_names.is_null() || (*params.0).params.is_null() {
                return Ok(());
            }
        }

        let node_names =
            unsafe { std::slice::from_raw_parts((*params.0).node_names, (*params.0).num_nodes) };

        let node_params =
            unsafe { std::slice::from_raw_parts((*params.0).params, (*params.0).num_nodes) };

        let qualified_name = self.fully_qualified_name()?;
        let name = self.name()?;

        for (nn, np) in node_names.iter().zip(node_params) {
            let node_name_cstr = unsafe { CStr::from_ptr(*nn) };
            let node_name = c_str_to_str(node_name_cstr, "parameter override node name")?;

            // This is copied from rclcpp, but there is a comment there suggesting
            // that more wildcards will be added in the future. Take note and mimic
            // their behavior.
            if !(node_name == "/**"
                || node_name == "**"
                || qualified_name == node_name
                || name == node_name)
            {
                continue;
            }

            // make key value pairs.
            let mut params = self.params.lock();
            if !np.parameter_names.is_null() && !np.parameter_values.is_null() {
                let param_names =
                    unsafe { std::slice::from_raw_parts(np.parameter_names, np.num_params) };

                let param_values =
                    unsafe { std::slice::from_raw_parts(np.parameter_values, np.num_params) };

                for (s, v) in param_names.iter().zip(param_values) {
                    let s = unsafe { CStr::from_ptr(*s) };
                    let key = c_str_to_str(s, "parameter override name")?;
                    let val = ParameterValue::from_rcl(v)?;
                    params.insert(key.to_owned(), Parameter::new(val));
                }
            }
        }

        Ok(())
    }

    /// Creates a ROS node.
    pub fn create(ctx: Context, name: &str, namespace: &str) -> Result<Node> {
        let c_node_name = CString::new(name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let c_node_ns = CString::new(namespace).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let (res, node_handle) = {
            let mut ctx_handle = ctx.context_handle.lock();

            let mut node_handle: Box<rcl_node_t> =
                unsafe { Box::new(rcl_get_zero_initialized_node()) };
            let res = unsafe {
                let node_options = rcl_node_get_default_options();
                rcl_node_init(
                    node_handle.as_mut(),
                    c_node_name.as_ptr(),
                    c_node_ns.as_ptr(),
                    ctx_handle.as_mut(),
                    &node_options as *const _,
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
                node_handle,
                subscribers: Vec::new(),
                services: Vec::new(),
                clients: Vec::new(),
                action_clients: Vec::new(),
                action_servers: Vec::new(),
                action_wait_set_capacities: WaitSetCapacities::default(),
                timers: Vec::new(),
                pubs: Vec::new(),
                ros_clock,
                #[cfg(r2r__rosgraph_msgs__msg__Clock)]
                time_source,
                wait_set: WaitSet::new(),
                context: ctx,
            };
            #[cfg(r2r__rosgraph_msgs__msg__Clock)]
            node.time_source
                .attach_ros_clock(Arc::downgrade(&node.ros_clock))?;
            node.load_params()?;
            Ok(node)
        } else {
            log::error!("could not create node{}", res);
            Err(Error::from_rcl_error(res))
        }
    }

    /// Creates parameter service handlers for the Node.
    ///
    /// This function returns a tuple (`Future`, `Stream`), where the
    /// future should be spawned on onto the executor of choice. The
    /// `Stream` produces events whenever parameters change from
    /// external sources. The event elements of the event stream
    /// include the name of the parameter which was updated as well as
    /// its new value.
    pub fn make_parameter_handler(
        &mut self,
    ) -> Result<(impl Future<Output = ()> + Send, impl Stream<Item = (String, ParameterValue)>)>
    {
        self.make_parameter_handler_internal(None)
    }

    /// Creates parameter service handlers for the Node based on the
    /// [`RosParams`] trait.
    ///
    /// Supported parameter names and types are given by the
    /// `params_struct` parameter (usually referring to a structure).
    /// Fields of the structure will be updated based on the command
    /// line parameters (if any) and later whenever a parameter gets
    /// changed from external sources. Updated fields will be visible
    /// outside of the node via the GetParameters service.
    ///
    /// This function returns a tuple (`Future`, `Stream`), where the
    /// future should be spawned on onto the executor of choice. The
    /// `Stream` produces events whenever parameters change from
    /// external sources. The event elements of the event stream
    /// include the name of the parameter which was updated as well as
    /// its new value.
    pub fn make_derived_parameter_handler(
        &mut self, params_struct: Arc<Mutex<dyn RosParams + Send>>,
    ) -> Result<(impl Future<Output = ()> + Send, impl Stream<Item = (String, ParameterValue)>)>
    {
        self.make_parameter_handler_internal(Some(params_struct))
    }

    fn make_parameter_handler_internal(
        &mut self, params_struct: Option<Arc<Mutex<dyn RosParams + Send>>>,
    ) -> Result<(impl Future<Output = ()> + Send, impl Stream<Item = (String, ParameterValue)>)>
    {
        if let Some(ps) = &params_struct {
            // register all parameters
            ps.lock()
                .register_parameters("", None, &mut self.params.lock())?;
        }
        let mut handlers: Vec<std::pin::Pin<Box<dyn Future<Output = ()> + Send>>> = Vec::new();

        let (mut set_event_tx, event_rx) = mpsc::channel::<(String, ParameterValue)>(10);
        let mut set_atomically_event_tx = set_event_tx.clone();

        let node_name = self.name()?;

        // rcl_interfaces/srv/SetParameters
        let set_params_request_stream = self
            .create_service::<rcl_interfaces::srv::SetParameters::Service>(
                &format!("{}/set_parameters", node_name),
                QosProfile::default(),
            )?;

        let params = self.params.clone();
        let params_struct_clone = params_struct.clone();
        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        let set_params_time_source = self.time_source.clone();
        let set_params_future = set_params_request_stream.for_each(
            move |req: ServiceRequest<rcl_interfaces::srv::SetParameters::Service>| {
                let mut result = rcl_interfaces::srv::SetParameters::Response::default();
                for p in &req.message.parameters {
                    let val = ParameterValue::from_parameter_value_msg(p.value.clone());
                    let changed = params
                        .lock()
                        .get(&p.name)
                        .map(|v| v.value != val)
                        .unwrap_or(true); // changed=true if new
                    let r = if p.name == "use_sim_time" {
                        match val {
                            ParameterValue::Bool(enabled) => {
                                let _ = enabled;
                                params
                                    .lock()
                                    .entry(p.name.clone())
                                    .and_modify(|parameter| parameter.value = val.clone())
                                    .or_insert_with(|| Parameter::new(val.clone()));
                                #[cfg(r2r__rosgraph_msgs__msg__Clock)]
                                set_params_time_source.request_sim_time(enabled);
                                rcl_interfaces::msg::SetParametersResult {
                                    successful: true,
                                    reason: String::new(),
                                }
                            }
                            _ => rcl_interfaces::msg::SetParametersResult {
                                successful: false,
                                reason: "use_sim_time must be a boolean".into(),
                            },
                        }
                    } else if let Some(ps) = &params_struct_clone {
                        // Update parameter structure
                        let result = ps.lock().set_parameter(&p.name, &val);
                        if result.is_ok() {
                            // Also update Node::params
                            params
                                .lock()
                                .entry(p.name.clone())
                                .and_modify(|p| p.value = val.clone());
                        }
                        rcl_interfaces::msg::SetParametersResult {
                            successful: result.is_ok(),
                            reason: result.err().map_or("".into(), |e| e.to_string()),
                        }
                    } else {
                        // No parameter structure - update only Node::params
                        params
                            .lock()
                            .entry(p.name.clone())
                            .and_modify(|p| p.value = val.clone())
                            .or_insert(Parameter::new(val.clone()));
                        rcl_interfaces::msg::SetParametersResult {
                            successful: true,
                            reason: "".into(),
                        }
                    };
                    // if the value changed, send out new value on parameter event stream
                    if changed && r.successful {
                        if let Err(e) = set_event_tx.try_send((p.name.clone(), val)) {
                            log::debug!("Warning: could not send parameter event ({}).", e);
                        }
                    }
                    result.results.push(r);
                }
                if let Err(err) = req.respond(result) {
                    log::warn!("could not send reply to set parameter request: {err}");
                }
                future::ready(())
            },
        );
        handlers.push(Box::pin(set_params_future));

        // rcl_interfaces/srv/GetParameters
        let get_params_request_stream = self
            .create_service::<rcl_interfaces::srv::GetParameters::Service>(
                &format!("{}/get_parameters", node_name),
                QosProfile::default(),
            )?;

        let params = self.params.clone();
        let params_struct_clone = params_struct.clone();
        let get_params_future = get_params_request_stream.for_each(
            move |req: ServiceRequest<rcl_interfaces::srv::GetParameters::Service>| {
                let params = params.lock();
                let values = req
                    .message
                    .names
                    .iter()
                    .map(|n| {
                        // First try to get the parameter from the param structure
                        if let Some(ps) = &params_struct_clone {
                            if let Ok(value) = ps.lock().get_parameter(n) {
                                return value;
                            }
                        }
                        // Otherwise get it from node HashMap
                        match params.get(n) {
                            Some(v) => v.value.clone(),
                            None => ParameterValue::NotSet,
                        }
                    })
                    .map(|v| v.into_parameter_value_msg())
                    .collect::<Vec<rcl_interfaces::msg::ParameterValue>>();

                let result = rcl_interfaces::srv::GetParameters::Response { values };
                if let Err(err) = req.respond(result) {
                    log::warn!("could not send reply to get parameter request: {err}");
                }
                future::ready(())
            },
        );

        handlers.push(Box::pin(get_params_future));

        // rcl_interfaces/srv/ListParameters
        use rcl_interfaces::srv::ListParameters;
        let list_params_request_stream = self.create_service::<ListParameters::Service>(
            &format!("{}/list_parameters", node_name),
            QosProfile::default(),
        )?;

        let params = self.params.clone();
        let list_params_future = list_params_request_stream.for_each(
            move |req: ServiceRequest<ListParameters::Service>| {
                Self::handle_list_parameters(req, &params)
            },
        );

        handlers.push(Box::pin(list_params_future));

        // rcl_interfaces/srv/DescribeParameters
        use rcl_interfaces::srv::DescribeParameters;
        let desc_params_request_stream = self.create_service::<DescribeParameters::Service>(
            &format!("{node_name}/describe_parameters"),
            QosProfile::default(),
        )?;

        let params = self.params.clone();
        let desc_params_future = desc_params_request_stream.for_each(
            move |req: ServiceRequest<DescribeParameters::Service>| {
                Self::handle_desc_parameters(req, &params)
            },
        );

        handlers.push(Box::pin(desc_params_future));

        // rcl_interfaces/srv/GetParameterTypes
        use rcl_interfaces::srv::GetParameterTypes;
        let get_param_types_request_stream = self.create_service::<GetParameterTypes::Service>(
            &format!("{node_name}/get_parameter_types"),
            QosProfile::default(),
        )?;

        let params = self.params.clone();
        let get_param_types_future = get_param_types_request_stream.for_each(
            move |req: ServiceRequest<GetParameterTypes::Service>| {
                let params = params.lock();
                let types = req
                    .message
                    .names
                    .iter()
                    .map(|name| match params.get(name) {
                        Some(param) => param.value.parameter_type(),
                        None => rcl_interfaces::msg::ParameterType::PARAMETER_NOT_SET as u8,
                    })
                    .collect();
                if let Err(err) = req.respond(GetParameterTypes::Response { types }) {
                    log::warn!("could not send reply to get parameter types request: {err}");
                }
                future::ready(())
            },
        );

        handlers.push(Box::pin(get_param_types_future));

        // rcl_interfaces/srv/SetParametersAtomically
        let set_params_atomically_request_stream =
            self.create_service::<rcl_interfaces::srv::SetParametersAtomically::Service>(
                &format!("{}/set_parameters_atomically", node_name),
                QosProfile::default(),
            )?;

        let params = self.params.clone();
        let params_struct_clone = params_struct.clone();
        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        let atomic_time_source = self.time_source.clone();
        let set_params_atomically_future = set_params_atomically_request_stream.for_each(
            move |req: ServiceRequest<rcl_interfaces::srv::SetParametersAtomically::Service>| {
                let mut result = rcl_interfaces::srv::SetParametersAtomically::Response::default();
                let updates = req
                    .message
                    .parameters
                    .iter()
                    .map(|parameter| {
                        (
                            parameter.name.clone(),
                            ParameterValue::from_parameter_value_msg(parameter.value.clone()),
                        )
                    })
                    .collect::<Vec<_>>();
                let sim_time_update = updates
                    .iter()
                    .rev()
                    .find_map(|(name, value)| (name == "use_sim_time").then_some(value));
                let invalid_sim_time = updates.iter().any(|(name, value)| {
                    name == "use_sim_time" && !matches!(value, ParameterValue::Bool(_))
                });
                let derived_updates = updates
                    .iter()
                    .filter(|(name, _)| name != "use_sim_time")
                    .cloned()
                    .collect::<Vec<_>>();

                let update_result = if invalid_sim_time {
                    Err(Error::InvalidParameterType {
                        name: "use_sim_time".into(),
                        ty: "boolean",
                    })
                } else {
                    params_struct_clone
                        .as_ref()
                        .map_or(Ok(()), |ps| ps.lock().set_parameters_atomically(&derived_updates))
                };

                match update_result {
                    Ok(()) => {
                        result.result.successful = true;
                        let changed = {
                            let mut params = params.lock();
                            updates
                                .iter()
                                .filter_map(|(name, value)| {
                                    let changed = params
                                        .get(name)
                                        .map(|parameter| parameter.value != *value)
                                        .unwrap_or(true);
                                    params
                                        .entry(name.clone())
                                        .and_modify(|parameter| parameter.value = value.clone())
                                        .or_insert_with(|| Parameter::new(value.clone()));
                                    changed.then(|| (name.clone(), value.clone()))
                                })
                                .collect::<Vec<_>>()
                        };
                        for event in changed {
                            if let Err(e) = set_atomically_event_tx.try_send(event) {
                                log::debug!("Warning: could not send parameter event ({}).", e);
                            }
                        }
                        if let Some(ParameterValue::Bool(enabled)) = sim_time_update {
                            let _ = enabled;
                            #[cfg(r2r__rosgraph_msgs__msg__Clock)]
                            atomic_time_source.request_sim_time(*enabled);
                        }
                    }
                    Err(err) => {
                        result.result.successful = false;
                        result.result.reason = format!("Can't set parameters atomically: {err}");
                    }
                }
                if let Err(err) = req.respond(result) {
                    log::warn!("could not send reply to atomic set parameter request: {err}");
                }
                future::ready(())
            },
        );
        handlers.push(Box::pin(set_params_atomically_future));

        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        {
            // create TimeSource based on value of use_sim_time parameter
            let use_sim_time = {
                let mut params = self.params.lock();
                match params.get("use_sim_time") {
                    Some(Parameter {
                        value: ParameterValue::Bool(value),
                        ..
                    }) => *value,
                    Some(_) => {
                        return Err(Error::InvalidParameterType {
                            name: "use_sim_time".into(),
                            ty: "boolean",
                        });
                    }
                    None => {
                        params.insert(
                            "use_sim_time".into(),
                            Parameter::new(ParameterValue::Bool(false)),
                        );
                        false
                    }
                }
            };
            if use_sim_time {
                let ts = self.time_source.clone();
                ts.enable_sim_time(self)?;
            }
        }

        // we don't care about the result, the futures will not complete anyway.
        Ok((join_all(handlers).map(|_| ()), event_rx))
    }

    fn handle_list_parameters(
        req: ServiceRequest<rcl_interfaces::srv::ListParameters::Service>,
        params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    ) -> future::Ready<()> {
        use rcl_interfaces::srv::ListParameters;

        let depth = req.message.depth;
        let prefixes = &req.message.prefixes;
        let separator = '.';
        let params = params.lock();
        let mut result = rcl_interfaces::msg::ListParametersResult {
            names: vec![],
            prefixes: vec![],
        };
        for (name, _) in params.iter().filter(|(name, _)| {
            let get_all = prefixes.is_empty()
                && ((depth == ListParameters::Request::DEPTH_RECURSIVE as u64)
                    || name.matches(separator).count() < depth as usize);
            let prefix_matches = prefixes.iter().any(|prefix| {
                if *name == prefix {
                    return true;
                } else if name.starts_with(&format!("{prefix}{separator}")) {
                    let substr = &name[prefix.len()..];
                    return (depth == ListParameters::Request::DEPTH_RECURSIVE as u64)
                        || substr.matches(separator).count() < depth as usize;
                }
                false
            });
            get_all || prefix_matches
        }) {
            result.names.push(name.clone());
            if let Some(last_separator) = name.rfind(separator) {
                let prefix = &name[0..last_separator];
                if result.prefixes.iter().all(|p| p != prefix) {
                    result.prefixes.push(prefix.to_string());
                }
            }
        }
        if let Err(err) = req.respond(ListParameters::Response { result }) {
            log::warn!("could not send reply to list parameter request: {err}");
        }
        future::ready(())
    }

    fn handle_desc_parameters(
        req: ServiceRequest<rcl_interfaces::srv::DescribeParameters::Service>,
        params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    ) -> future::Ready<()> {
        use rcl_interfaces::{msg::ParameterDescriptor, srv::DescribeParameters};
        let mut descriptors = Vec::<ParameterDescriptor>::new();
        let params = params.lock();
        for name in &req.message.names {
            let default = Parameter::empty();
            let param = params.get(name).unwrap_or(&default);
            descriptors.push(ParameterDescriptor {
                name: name.clone(),
                type_: param.value.parameter_type(),
                description: param.description.to_string(),
                ..Default::default()
            });
        }
        if let Err(err) = req.respond(DescribeParameters::Response { descriptors }) {
            log::warn!("could not send reply to describe parameters request: {err}");
        }
        future::ready(())
    }

    /// Fetch a single ROS parameter.
    pub fn get_parameter<T>(&self, name: &str) -> Result<T>
    where
        ParameterValue: TryInto<T, Error = WrongParameterType>,
    {
        let params = self.params.lock();
        let value = params
            .get(name)
            .map(|parameter| parameter.value.clone())
            .unwrap_or(ParameterValue::NotSet);

        let value: T =
            value
                .try_into()
                .map_err(|error: WrongParameterType| Error::ParameterWrongType {
                    name: name.to_string(),
                    expected_type: error.expected_type_name,
                    actual_type: error.actual_type_name,
                })?;

        Ok(value)
    }

    /// Subscribe to a ROS topic.
    ///
    /// This function returns a `Stream` of ros messages.
    pub fn subscribe<T>(
        &mut self, topic: &str, qos_profile: QosProfile,
    ) -> Result<impl Stream<Item = T> + Unpin>
    where
        T: 'static + WrappedTypesupport,
    {
        let subscription_handle =
            create_subscription_helper(self.node_handle.as_mut(), topic, T::get_ts(), qos_profile)?;
        let (sender, receiver) = mpsc::channel::<T>(10);

        let ws = TypedSubscriber {
            rcl_handle: subscription_handle,
            sender,
        };
        self.subscribers.push(Box::new(ws));
        Ok(receiver)
    }

    /// Subscribe to a ROS topic.
    ///
    /// This function returns a `Stream` of ros messages without the rust convenience types.
    pub fn subscribe_native<T>(
        &mut self, topic: &str, qos_profile: QosProfile,
    ) -> Result<impl Stream<Item = WrappedNativeMsg<T>> + Unpin>
    where
        T: 'static + WrappedTypesupport,
    {
        let subscription_handle =
            create_subscription_helper(self.node_handle.as_mut(), topic, T::get_ts(), qos_profile)?;
        let (sender, receiver) = mpsc::channel::<WrappedNativeMsg<T>>(10);

        let ws = NativeSubscriber {
            rcl_handle: subscription_handle,
            sender,
        };
        self.subscribers.push(Box::new(ws));
        Ok(receiver)
    }

    /// Subscribe to a ROS topic.
    ///
    /// This function returns a `Stream` of ros messages as `serde_json::Value`:s.
    /// Useful when you cannot know the type of the message at compile time.
    pub fn subscribe_untyped(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
    ) -> Result<impl Stream<Item = Result<serde_json::Value>> + Unpin> {
        let msg = WrappedNativeMsgUntyped::new_from(topic_type)?;
        let subscription_handle =
            create_subscription_helper(self.node_handle.as_mut(), topic, msg.ts, qos_profile)?;
        let (sender, receiver) = mpsc::channel::<Result<serde_json::Value>>(10);

        let ws = UntypedSubscriber {
            rcl_handle: subscription_handle,
            topic_type: topic_type.to_string(),
            sender,
        };
        self.subscribers.push(Box::new(ws));
        Ok(receiver)
    }

    /// Subscribe to a ROS topic.
    ///
    /// This function returns a `Stream` of ros messages as non-deserialized `Vec<u8>`:s.
    /// Useful if you just want to pass the data along to another part of the system.
    pub fn subscribe_raw(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
    ) -> Result<impl Stream<Item = Vec<u8>> + Unpin> {
        // TODO is it possible to handle the raw message without type support?
        //
        // Passing null ts to rcl_subscription_init throws an error ..
        //
        // It does not seem possible to not have a type support, which is a shame
        // because it means we always have to build the message types even if we
        // are just after the raw bytes.
        let msg = WrappedNativeMsgUntyped::new_from(topic_type)?;

        // Keep a buffer to reduce number of allocations. The rmw will
        // resize it if the message size exceeds the buffer size.
        let mut msg_buf: rcl_serialized_message_t =
            unsafe { rcutils_get_zero_initialized_uint8_array() };
        let ret = unsafe {
            rcutils_uint8_array_init(
                &mut msg_buf as *mut rcl_serialized_message_t,
                0,
                &rcutils_get_default_allocator(),
            )
        };

        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let subscription_handle =
            match create_subscription_helper(self.node_handle.as_mut(), topic, msg.ts, qos_profile)
            {
                Ok(handle) => handle,
                Err(err) => {
                    unsafe {
                        rcutils_uint8_array_fini(&mut msg_buf as *mut rcl_serialized_message_t);
                    }
                    return Err(err);
                }
            };
        let (sender, receiver) = mpsc::channel::<Vec<u8>>(10);

        let ws = RawSubscriber {
            rcl_handle: subscription_handle,
            msg_buf,
            sender,
        };
        self.subscribers.push(Box::new(ws));
        Ok(receiver)
    }

    /// Create a ROS service.
    ///
    /// This function returns a `Stream` of `ServiceRequest`:s. Call
    /// `respond` on the Service Request to send the reply.
    pub fn create_service<T>(
        &mut self, service_name: &str, qos_profile: QosProfile,
    ) -> Result<impl Stream<Item = ServiceRequest<T>> + Unpin>
    where
        T: 'static + WrappedServiceTypeSupport,
    {
        let service_handle = create_service_helper(
            self.node_handle.as_mut(),
            service_name,
            T::get_ts(),
            qos_profile,
        )?;
        let (sender, receiver) = mpsc::channel::<ServiceRequest<T>>(10);

        let ws = TypedService::<T> {
            rcl_handle: service_handle,
            sender,
        };

        self.services.push(Arc::new(Mutex::new(ws)));
        Ok(receiver)
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
            poll_available_channels: Vec::new(),
        };

        let client_arc = Arc::new(Mutex::new(ws));
        let c = make_client(Arc::downgrade(&client_arc));
        self.clients.push(client_arc);
        Ok(c)
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
            poll_available_channels: Vec::new(),
        };

        let client_arc = Arc::new(Mutex::new(client));
        let c = make_untyped_client(Arc::downgrade(&client_arc));
        self.clients.push(client_arc);
        Ok(c)
    }

    /// Register a client for wakeup when the service or action server is available to the node.
    ///
    /// Returns a `Future` that completes when the service/action server is available.
    ///
    /// This function will register the client to be polled in
    /// `spin_once` until available, so spin_once must be called
    /// repeatedly in order to get the wakeup.
    pub fn is_available(
        client: &dyn IsAvailablePollable,
    ) -> Result<impl Future<Output = Result<()>>> {
        let (sender, receiver) = oneshot::channel();
        client.register_poll_available(sender)?;
        Ok(receiver.map_err(|_| Error::RCL_RET_CLIENT_INVALID))
    }

    /// Create a ROS action client.
    ///
    /// An action client is used to make requests to a ROS action server.
    pub fn create_action_client<T>(&mut self, action_name: &str) -> Result<ActionClient<T>>
    where
        T: 'static + WrappedActionTypeSupport,
    {
        let mut client_handle =
            create_action_client_helper(self.node_handle.as_mut(), action_name, T::get_ts())?;
        let action_capacities = match action_client_wait_set_capacities(&client_handle) {
            Ok(capacities) => capacities,
            Err(err) => {
                unsafe {
                    rcl_action_client_fini(&mut client_handle, self.node_handle.as_mut());
                }
                return Err(err);
            }
        };
        let client = WrappedActionClient::<T> {
            rcl_handle: client_handle,
            goal_response_channels: Vec::new(),
            cancel_response_channels: Vec::new(),
            feedback_senders: Vec::new(),
            result_senders: Vec::new(),
            result_requests: Vec::new(),
            goal_status: HashMap::new(),
            poll_available_channels: Vec::new(),
        };

        let client_arc = Arc::new(Mutex::new(client));
        self.action_clients.push(client_arc.clone());
        self.action_wait_set_capacities.add(action_capacities);
        let c = make_action_client(Arc::downgrade(&client_arc));
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
        let action_type_support = UntypedActionSupport::new_from(action_type)?;
        let mut client_handle = create_action_client_helper(
            self.node_handle.as_mut(),
            action_name,
            action_type_support.ts,
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
        let client = WrappedActionClientUntyped {
            action_type_support,
            rcl_handle: client_handle,
            goal_response_channels: Vec::new(),
            cancel_response_channels: Vec::new(),
            feedback_senders: Vec::new(),
            result_senders: Vec::new(),
            result_requests: Vec::new(),
            goal_status: HashMap::new(),
            poll_available_channels: Vec::new(),
        };

        let client_arc = Arc::new(Mutex::new(client));
        self.action_clients.push(client_arc.clone());
        self.action_wait_set_capacities.add(action_capacities);
        let c = make_action_client_untyped(Arc::downgrade(&client_arc));
        Ok(c)
    }

    /// Create a ROS action server.
    ///
    /// This function returns a stream of `GoalRequest`s, which needs
    /// to be either accepted or rejected.
    pub fn create_action_server<T>(
        &mut self, action_name: &str,
    ) -> Result<impl Stream<Item = ActionServerGoalRequest<T>> + Unpin>
    where
        T: 'static + WrappedActionTypeSupport,
    {
        let (goal_request_sender, goal_request_receiver) =
            mpsc::channel::<ActionServerGoalRequest<T>>(10);

        let clock = self.ros_clock.clone();
        let mut server_handle = {
            let mut clock_handle = clock.lock();
            create_action_server_helper(
                self.node_handle.as_mut(),
                action_name,
                clock_handle.clock_handle.as_mut(),
                T::get_ts(),
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
        let server = WrappedActionServer::<T> {
            rcl_handle: server_handle,
            clock,
            goal_request_sender,
            active_cancel_requests: Vec::new(),
            cancel_senders: HashMap::new(),
            goals: HashMap::new(),
            result_msgs: HashMap::new(),
            result_requests: HashMap::new(),
        };

        let server_arc = Arc::new(Mutex::new(server));
        self.action_servers.push(server_arc);
        self.action_wait_set_capacities.add(action_capacities);
        Ok(goal_request_receiver)
    }

    /// Create a ROS publisher.
    pub fn create_publisher<T>(
        &mut self, topic: &str, qos_profile: QosProfile,
    ) -> Result<Publisher<T>>
    where
        T: WrappedTypesupport,
    {
        let publisher_handle =
            create_publisher_helper(self.node_handle.as_mut(), topic, T::get_ts(), qos_profile)?;
        let arc = Arc::new(publisher_handle);
        let p = make_publisher(Arc::downgrade(&arc));
        self.pubs.push(arc);
        Ok(p)
    }

    /// Create a ROS publisher with a type given at runtime, where the data may either be
    /// supplied as JSON (using the `publish` method) or a pre-serialized ROS message
    /// (i.e. &[u8], using the `publish_raw` method).
    pub fn create_publisher_untyped(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
    ) -> Result<PublisherUntyped> {
        let dummy = WrappedNativeMsgUntyped::new_from(topic_type)?;
        let publisher_handle =
            create_publisher_helper(self.node_handle.as_mut(), topic, dummy.ts, qos_profile)?;
        let arc = Arc::new(publisher_handle);
        let p = make_publisher_untyped(Arc::downgrade(&arc), topic_type.to_owned());
        self.pubs.push(arc);
        Ok(p)
    }

    /// Destroy a ROS publisher.
    pub fn destroy_publisher<T: WrappedTypesupport>(&mut self, p: Publisher<T>) {
        if let Some(handle) = p.handle.upgrade() {
            // Remove handle from list of publishers.
            self.pubs
                .iter()
                .position(|p| Arc::ptr_eq(p, &handle))
                .map(|i| self.pubs.swap_remove(i));

            handle.destroy(self.node_handle.as_mut());
        }
    }

    /// Destroy a ROS publisher.
    pub fn destroy_publisher_untyped(&mut self, p: PublisherUntyped) {
        if let Some(handle) = p.handle.upgrade() {
            // Remove handle from list of publishers.
            self.pubs
                .iter()
                .position(|p| Arc::ptr_eq(p, &handle))
                .map(|i| self.pubs.swap_remove(i));

            handle.destroy(self.node_handle.as_mut());
        }
    }

    /// Spin the ROS node.
    ///
    /// This handles wakeups of all subscribes, services, etc on the
    /// ros side. In turn, this will complete future and wake up
    /// streams on the rust side. This needs to be called repeatedly
    /// (see the examples).
    ///
    /// `timeout` is a duration specifying how long the spin should
    /// block for if there are no pending events.
    pub fn spin_once(&mut self, timeout: Duration) {
        if let Err(err) = self.try_spin_once(timeout) {
            log::error!("failed to spin node: {}", err);
        }
    }

    /// Spin the ROS node and report errors from the underlying RCL wait set.
    pub fn try_spin_once(&mut self, timeout: Duration) -> Result<()> {
        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        {
            let time_source = self.time_source.clone();
            time_source.apply_requested_sim_time(self)?;
        }

        // Some internal subscriptions, such as `/clock`, can be disabled without another
        // message arriving. Remove them before rebuilding the wait set so teardown does not
        // depend on readiness from the subscription being removed.
        let mut active_subscribers = Vec::with_capacity(self.subscribers.len());
        for mut subscriber in self.subscribers.drain(..) {
            if subscriber.should_remove() {
                subscriber.destroy(&mut self.node_handle);
            } else {
                active_subscribers.push(subscriber);
            }
        }
        self.subscribers = active_subscribers;

        // first handle any completed action cancellation responses
        for a in &mut self.action_servers {
            a.lock().send_completed_cancel_requests();
        }

        // as well as polling any services/action servers for availability
        for c in &mut self.clients {
            c.lock().poll_available(self.node_handle.as_mut());
        }

        for c in &mut self.action_clients {
            c.lock().poll_available(self.node_handle.as_mut());
        }

        for p in &self.pubs {
            p.poll_has_inter_process_subscribers();
        }

        let timeout = duration_to_rcl_nanos(timeout)?;

        // #[doc = "* This function is thread-safe for unique wait sets with unique contents."]
        // #[doc = "* This function cannot operate on the same wait set in multiple threads, and"]
        // #[doc = "* the wait sets may not share content."]
        // #[doc = "* For example, calling rcl_wait() in two threads on two different wait sets"]
        // #[doc = "* that both contain a single, shared guard condition is undefined behavior."]

        let capacities = WaitSetCapacities {
            subscriptions: self.subscribers.len() + self.action_wait_set_capacities.subscriptions,
            guard_conditions: self.action_wait_set_capacities.guard_conditions,
            timers: self.timers.len() + self.action_wait_set_capacities.timers,
            clients: self.clients.len() + self.action_wait_set_capacities.clients,
            services: self.services.len() + self.action_wait_set_capacities.services,
        };
        {
            let mut ctx = self.context.context_handle.lock();
            self.wait_set.prepare(capacities, ctx.as_mut())?;
        }
        let ws = &mut self.wait_set.handle;

        for s in &self.subscribers {
            check_rcl_ret(unsafe {
                rcl_wait_set_add_subscription(ws, s.handle(), std::ptr::null_mut())
            })?;
        }

        for s in &self.timers {
            check_rcl_ret(unsafe {
                rcl_wait_set_add_timer(ws, s.get_handle(), std::ptr::null_mut())
            })?;
        }

        for s in &self.clients {
            check_rcl_ret(unsafe {
                rcl_wait_set_add_client(ws, s.lock().handle(), std::ptr::null_mut())
            })?;
        }

        for s in &self.services {
            check_rcl_ret(unsafe {
                rcl_wait_set_add_service(ws, s.lock().handle(), std::ptr::null_mut())
            })?;
        }

        // code (further) below assumes that actions are added last... perhaps a
        // bad assumption.  e.g. we add subscriptions and timers of
        // the node before ones created automatically by actions. we
        // then assume that we can count on the waitables created by
        // the actions are added at the end of the wait set arrays
        for ac in &self.action_clients {
            check_rcl_ret(unsafe {
                rcl_action_wait_set_add_action_client(
                    ws,
                    ac.lock().handle(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            })?;
        }
        for acs in &self.action_servers {
            check_rcl_ret(unsafe {
                rcl_action_wait_set_add_action_server(ws, acs.lock().handle(), std::ptr::null_mut())
            })?;
        }

        let ret = unsafe { rcl_wait(ws, timeout) };

        if ret == RCL_RET_TIMEOUT as i32 {
            return Ok(());
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let mut subs_to_remove = vec![];
        if !ws.subscriptions.is_null() {
            let ws_subs =
                unsafe { std::slice::from_raw_parts(ws.subscriptions, self.subscribers.len()) };
            for (s, ws_s) in self.subscribers.iter_mut().zip(ws_subs) {
                if ws_s != &std::ptr::null() {
                    let dropped = s.handle_incoming();
                    if dropped {
                        let handle = s.handle() as *const rcl_subscription_t;
                        s.destroy(&mut self.node_handle);
                        subs_to_remove.push(handle);
                    }
                }
            }
        }
        self.subscribers
            .retain(|s| !subs_to_remove.contains(&(s.handle() as *const rcl_subscription_t)));

        let mut timers_to_remove = vec![];
        if !ws.timers.is_null() {
            let ws_timers = unsafe { std::slice::from_raw_parts(ws.timers, self.timers.len()) };
            for (s, ws_s) in self.timers.iter_mut().zip(ws_timers) {
                if ws_s != &std::ptr::null() {
                    // TODO: move this to impl Timer
                    let dropped = s.handle_incoming();
                    if dropped {
                        timers_to_remove.push(s.get_handle());
                    }
                }
            }
        }
        // drop timers scheduled for deletion
        self.timers
            .retain(|t| !timers_to_remove.contains(&t.get_handle()));

        if !ws.clients.is_null() {
            let ws_clients = unsafe { std::slice::from_raw_parts(ws.clients, self.clients.len()) };
            for (s, ws_s) in self.clients.iter_mut().zip(ws_clients) {
                if ws_s != &std::ptr::null() {
                    let mut s = s.lock();
                    s.handle_response();
                }
            }
        }

        let mut services_to_remove = vec![];
        if !ws.services.is_null() {
            let ws_services =
                unsafe { std::slice::from_raw_parts(ws.services, self.services.len()) };
            for (s, ws_s) in self.services.iter_mut().zip(ws_services) {
                if ws_s != &std::ptr::null() {
                    let mut service = s.lock();
                    let dropped = service.handle_request(s.clone());
                    if dropped {
                        let handle = service.handle() as *const rcl_service_t;
                        service.destroy(&mut self.node_handle);
                        services_to_remove.push(handle);
                    }
                }
            }
        }
        self.services.retain(|s| {
            let service = s.lock();
            !services_to_remove.contains(&(service.handle() as *const rcl_service_t))
        });

        for ac in &self.action_clients {
            let mut ac = ac.lock();
            let mut is_feedback_ready = false;
            let mut is_status_ready = false;
            let mut is_goal_response_ready = false;
            let mut is_cancel_response_ready = false;
            let mut is_result_response_ready = false;

            let ret = unsafe {
                rcl_action_client_wait_set_get_entities_ready(
                    ws,
                    ac.handle(),
                    &mut is_feedback_ready,
                    &mut is_status_ready,
                    &mut is_goal_response_ready,
                    &mut is_cancel_response_ready,
                    &mut is_result_response_ready,
                )
            };

            if ret != RCL_RET_OK as i32 {
                return Err(Error::from_rcl_error(ret));
            }

            if is_feedback_ready {
                ac.handle_feedback_msg();
            }

            if is_status_ready {
                ac.handle_status_msg();
            }

            if is_goal_response_ready {
                ac.handle_goal_response();
            }

            if is_cancel_response_ready {
                ac.handle_cancel_response();
            }

            if is_result_response_ready {
                ac.handle_result_response();
            }
        }

        for s in &self.action_servers {
            let mut server = s.lock();
            let mut is_goal_request_ready = false;
            let mut is_cancel_request_ready = false;
            let mut is_result_request_ready = false;
            let mut is_goal_expired = false;

            let ret = unsafe {
                rcl_action_server_wait_set_get_entities_ready(
                    ws,
                    server.handle(),
                    &mut is_goal_request_ready,
                    &mut is_cancel_request_ready,
                    &mut is_result_request_ready,
                    &mut is_goal_expired,
                )
            };

            if ret != RCL_RET_OK as i32 {
                return Err(Error::from_rcl_error(ret));
            }

            if is_goal_request_ready {
                server.handle_goal_request(s.clone());
            }

            if is_cancel_request_ready {
                server.handle_cancel_request();
            }

            if is_result_request_ready {
                server.handle_result_request();
            }

            if is_goal_expired {
                server.handle_goal_expired();
            }
        }

        Ok(())
    }

    /// Returns a map of topic names and type names of the publishers
    /// visible to this node.
    pub fn get_topic_names_and_types(&self) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types(|node, allocator, names_and_types| unsafe {
            rcl_get_topic_names_and_types(node, allocator, false, names_and_types)
        })
    }

    /// Returns a map of service names and type names visible to this node.
    pub fn get_service_names_and_types(&self) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types(|node, allocator, names_and_types| unsafe {
            rcl_get_service_names_and_types(node, allocator, names_and_types)
        })
    }

    fn get_names_and_types(
        &self,
        query: impl FnOnce(
            *const rcl_node_t,
            *mut rcutils_allocator_t,
            *mut rmw_names_and_types_t,
        ) -> rcl_ret_t,
    ) -> Result<HashMap<String, Vec<String>>> {
        let mut tnat = unsafe { rmw_get_zero_initialized_names_and_types() };
        let mut allocator = unsafe { rcutils_get_default_allocator() };
        let ret = query(self.node_handle.as_ref(), &mut allocator, &mut tnat);
        if ret != RCL_RET_OK as i32 {
            log::error!("could not get ROS graph names and types {}", ret);
            return Err(Error::from_rcl_error(ret));
        }
        let tnat = NamesAndTypes(tnat);

        let mut res = HashMap::new();
        if tnat.0.names.size > 0 {
            if tnat.0.names.data.is_null() || tnat.0.types.is_null() {
                return Err(Error::RCL_RET_ERROR);
            }
            let names = unsafe { std::slice::from_raw_parts(tnat.0.names.data, tnat.0.names.size) };
            let types = unsafe { std::slice::from_raw_parts(tnat.0.types, tnat.0.names.size) };

            for (n, t) in names.iter().zip(types) {
                let entity_name = c_string_from_ptr(*n)?;
                if t.size > 0 && t.data.is_null() {
                    return Err(Error::RCL_RET_ERROR);
                }
                let entity_types = if t.size == 0 {
                    Vec::new()
                } else {
                    unsafe { std::slice::from_raw_parts(t.data, t.size) }
                        .iter()
                        .map(|type_name| c_string_from_ptr(*type_name))
                        .collect::<Result<Vec<_>>>()?
                };
                res.insert(entity_name, entity_types);
            }
        }
        Ok(res)
    }

    pub fn get_publishers_info_by_topic(
        &self, topic_name: &str, no_mangle: bool,
    ) -> Result<Vec<TopicEndpointInfo>> {
        self.get_endpoint_info_by_topic(
            topic_name,
            no_mangle,
            |node, allocator, name, raw| unsafe {
                rcl_get_publishers_info_by_topic(node, allocator, name, no_mangle, raw)
            },
        )
    }

    /// Returns information about subscriptions visible on `topic_name`.
    pub fn get_subscriptions_info_by_topic(
        &self, topic_name: &str, no_mangle: bool,
    ) -> Result<Vec<TopicEndpointInfo>> {
        self.get_endpoint_info_by_topic(
            topic_name,
            no_mangle,
            |node, allocator, name, raw| unsafe {
                rcl_get_subscriptions_info_by_topic(node, allocator, name, no_mangle, raw)
            },
        )
    }

    fn get_endpoint_info_by_topic(
        &self, topic_name: &str, _no_mangle: bool,
        query: impl FnOnce(
            *const rcl_node_t,
            *mut rcutils_allocator_t,
            *const std::os::raw::c_char,
            *mut rcl_topic_endpoint_info_array_t,
        ) -> rcl_ret_t,
    ) -> Result<Vec<TopicEndpointInfo>> {
        let topic_name = CString::new(topic_name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let mut info_array = TopicEndpointInfoArray::new();
        let ret = query(
            self.node_handle.as_ref(),
            &mut info_array.allocator,
            topic_name.as_ptr(),
            &mut info_array.value,
        );
        check_rcl_ret(ret)?;
        convert_info_array_to_vec(&info_array.value)
    }

    /// Create a ROS wall timer.
    ///
    /// Create a ROS timer that is woken up by spin every `period`.
    ///
    /// This timer uses [`ClockType::SteadyTime`] clock.
    pub fn create_wall_timer(&mut self, period: Duration) -> Result<Timer> {
        let mut clock = Clock::create(ClockType::SteadyTime)?;
        let timer_handle = self.create_timer_helper(&mut clock, period)?;

        let (tx, rx) = mpsc::channel::<Duration>(1);

        let timer = Timer_ {
            timer_handle,
            _clock: Some(clock), // The timer owns the clock.
            sender: tx,
        };
        self.timers.push(timer);

        let out_timer = Timer { receiver: rx };

        Ok(out_timer)
    }

    /// Create a ROS timer
    ///
    /// Create a ROS timer that is woken up by spin every `period`.
    ///
    /// This timer uses node's [`ClockType::RosTime`] clock.
    pub fn create_timer(&mut self, period: Duration) -> Result<Timer> {
        let mut clock = self.ros_clock.lock();
        let timer_handle = self.create_timer_helper(&mut clock, period)?;

        let (tx, rx) = mpsc::channel::<Duration>(1);

        let timer = Timer_ {
            timer_handle,
            _clock: None, // The timer does not own the clock (the node owns it).
            sender: tx,
        };
        self.timers.push(timer);

        let out_timer = Timer { receiver: rx };

        Ok(out_timer)
    }

    fn create_timer_helper(
        &self, clock: &mut Clock, period: Duration,
    ) -> Result<Pin<Box<RclTimer>>> {
        // rcl expects that the address of the rcl_timer_t does not change.
        let mut timer_handle = unsafe { Box::pin(RclTimer::new()) };

        let mut ctx = self.context.context_handle.lock();
        let ret = unsafe {
            init_timer(
                &mut timer_handle.as_mut().get_unchecked_mut().handle,
                clock.clock_handle.as_mut(),
                ctx.as_mut(),
                duration_to_rcl_nanos(period)?,
                None,
                rcutils_get_default_allocator(),
            )
        };

        if ret != RCL_RET_OK as i32 {
            log::error!("could not create timer: {}", ret);
            return Err(Error::from_rcl_error(ret));
        }

        Ok(timer_handle)
    }

    /// Get the ros logger name for this node.
    pub fn logger(&self) -> &str {
        self.try_logger().unwrap_or("")
    }

    /// Get the ROS logger name and report invalid UTF-8 from the RCL implementation.
    pub fn try_logger(&self) -> Result<&str> {
        let ptr = unsafe { rcl_node_get_logger_name(self.node_handle.as_ref()) };
        if ptr.is_null() {
            return Err(Error::RCL_RET_NODE_INVALID);
        }
        let s = unsafe { CStr::from_ptr(ptr) };
        c_str_to_str(s, "node logger name")
    }

    /// Get TimeSource of the node
    ///
    /// See: [`TimeSource`]
    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    pub fn get_time_source(&self) -> TimeSource {
        self.time_source.clone()
    }

    /// Get ROS clock of the node
    ///
    /// This is the same clock that is used by ROS timers created in [`Node::create_timer`].
    pub fn get_ros_clock(&self) -> Arc<Mutex<Clock>> {
        self.ros_clock.clone()
    }
}

struct NamesAndTypes(rmw_names_and_types_t);

impl Drop for NamesAndTypes {
    fn drop(&mut self) {
        let ret = unsafe { rmw_names_and_types_fini(&mut self.0) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize topic names and types: {ret}");
        }
    }
}

struct TopicEndpointInfoArray {
    value: rcl_topic_endpoint_info_array_t,
    allocator: rcutils_allocator_t,
}

impl TopicEndpointInfoArray {
    fn new() -> Self {
        Self {
            value: unsafe { rmw_get_zero_initialized_topic_endpoint_info_array() },
            allocator: unsafe { rcutils_get_default_allocator() },
        }
    }
}

impl Drop for TopicEndpointInfoArray {
    fn drop(&mut self) {
        let ret =
            unsafe { rmw_topic_endpoint_info_array_fini(&mut self.value, &mut self.allocator) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize topic endpoint information: {ret}");
        }
    }
}

fn c_string_from_ptr(ptr: *const std::os::raw::c_char) -> Result<String> {
    if ptr.is_null() {
        return Err(Error::RCL_RET_ERROR);
    }
    c_str_to_owned(unsafe { CStr::from_ptr(ptr) }, "ROS graph string")
}

fn c_str_to_str<'a>(value: &'a CStr, context: &'static str) -> Result<&'a str> {
    value.to_str().map_err(|err| Error::InvalidUtf8 {
        context,
        err: err.to_string(),
    })
}

fn c_str_to_owned(value: &CStr, context: &'static str) -> Result<String> {
    c_str_to_str(value, context).map(str::to_owned)
}

#[cfg(any(
    r2r__ros__distro__foxy,
    r2r__ros__distro__galactic,
    r2r__ros__distro__humble,
    r2r__ros__distro__iron
))]
#[inline]
pub unsafe fn init_timer(
    timer: *mut rcl_timer_t, clock: *mut rcl_clock_t, context: *mut rcl_context_t, period: i64,
    callback: rcl_timer_callback_t, allocator: rcl_allocator_t,
) -> rcl_ret_t {
    rcl_timer_init(timer, clock, context, period, callback, allocator)
}

// For Jazzy, Rolling, and newer
#[cfg(not(any(
    r2r__ros__distro__foxy,
    r2r__ros__distro__galactic,
    r2r__ros__distro__humble,
    r2r__ros__distro__iron
)))]
#[inline]
pub unsafe fn init_timer(
    timer: *mut rcl_timer_t, clock: *mut rcl_clock_t, context: *mut rcl_context_t, period: i64,
    callback: rcl_timer_callback_t, allocator: rcl_allocator_t,
) -> rcl_ret_t {
    rcl_timer_init2(timer, clock, context, period, callback, allocator, true)
}

#[derive(Debug)]
struct RclTimer {
    handle: rcl_timer_t,
    _pin: PhantomPinned, // To prevent Unpin implementation
}

impl RclTimer {
    unsafe fn new() -> Self {
        Self {
            handle: rcl_get_zero_initialized_timer(),
            _pin: PhantomPinned,
        }
    }
}

struct Timer_ {
    timer_handle: Pin<Box<RclTimer>>,
    _clock: Option<Clock>, // Some(clock) if the timer owns the clock, just here to be dropped properly later.
    sender: mpsc::Sender<Duration>,
}

impl Timer_ {
    fn get_handle(&self) -> *const rcl_timer_t {
        &self.timer_handle.handle
    }

    /// Get mutable pointer to handle
    ///
    /// SAFETY:
    ///     Pointer valid only for modification.
    ///     Must not invalidate or replace the timer unless in Drop.
    unsafe fn get_handle_mut(&mut self) -> *mut rcl_timer_t {
        self.timer_handle
            .as_mut()
            .map_unchecked_mut(|s| &mut s.handle)
            .get_unchecked_mut()
    }

    fn handle_incoming(&mut self) -> bool {
        let mut is_ready = false;
        let ret = unsafe { rcl_timer_is_ready(self.get_handle(), &mut is_ready) };
        if ret == RCL_RET_OK as i32 && is_ready {
            let mut nanos = 0i64;
            // todo: error handling
            let ret = unsafe { rcl_timer_get_time_since_last_call(self.get_handle(), &mut nanos) };
            if ret == RCL_RET_OK as i32 {
                let ret = unsafe { rcl_timer_call(self.get_handle_mut()) };
                if ret == RCL_RET_OK as i32 {
                    if let Err(e) = self.sender.try_send(Duration::from_nanos(nanos as u64)) {
                        if e.is_disconnected() {
                            // client dropped the timer handle, let's drop our timer as well.
                            return true;
                        }
                        if e.is_full() {
                            log::debug!(
                                "Warning: timer tick not handled in time - no wakeup will occur"
                            );
                        }
                    }
                }
            }
        }
        false
    }
}

impl Drop for Timer_ {
    fn drop(&mut self) {
        let _ret = unsafe { rcl_timer_fini(self.get_handle_mut()) };
    }
}

/// A ROS timer.
pub struct Timer {
    receiver: mpsc::Receiver<Duration>,
}

impl Timer {
    /// Completes when the next instant in the interval has been reached.
    ///
    /// Returns the time passed since the timer was last woken up.
    pub async fn tick(&mut self) -> Result<Duration> {
        let next = self.receiver.next().await;
        if let Some(elapsed) = next {
            Ok(elapsed)
        } else {
            Err(Error::RCL_RET_TIMER_INVALID)
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        // fini functions are not thread safe so lock the context.
        let _ctx_handle = self.context.context_handle.lock();
        self.wait_set.fini();

        for s in &mut self.subscribers {
            s.destroy(&mut self.node_handle);
        }
        for c in &mut self.clients {
            c.lock().destroy(&mut self.node_handle);
        }
        for s in &mut self.services {
            s.lock().destroy(&mut self.node_handle);
        }
        for c in &mut self.action_clients {
            c.lock().destroy(&mut self.node_handle);
        }
        for s in &mut self.action_servers {
            s.lock().destroy(&mut self.node_handle);
        }
        self.timers.clear();
        while let Some(p) = self.pubs.pop() {
            p.destroy(self.node_handle.as_mut());
        }
        unsafe {
            rcl_node_fini(self.node_handle.as_mut());
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct WaitSetCapacities {
    subscriptions: usize,
    guard_conditions: usize,
    timers: usize,
    clients: usize,
    services: usize,
}

impl WaitSetCapacities {
    fn add(&mut self, other: Self) {
        self.subscriptions += other.subscriptions;
        self.guard_conditions += other.guard_conditions;
        self.timers += other.timers;
        self.clients += other.clients;
        self.services += other.services;
    }
}

fn action_client_wait_set_capacities(handle: &rcl_action_client_t) -> Result<WaitSetCapacities> {
    let mut capacities = WaitSetCapacities::default();
    action_client_get_num_waits(
        handle,
        &mut capacities.subscriptions,
        &mut capacities.guard_conditions,
        &mut capacities.timers,
        &mut capacities.clients,
        &mut capacities.services,
    )?;
    Ok(capacities)
}

fn action_server_wait_set_capacities(handle: &rcl_action_server_t) -> Result<WaitSetCapacities> {
    let mut capacities = WaitSetCapacities::default();
    action_server_get_num_waits(
        handle,
        &mut capacities.subscriptions,
        &mut capacities.guard_conditions,
        &mut capacities.timers,
        &mut capacities.clients,
        &mut capacities.services,
    )?;
    Ok(capacities)
}

struct WaitSet {
    handle: rcl_wait_set_t,
    capacities: Option<WaitSetCapacities>,
}

impl WaitSet {
    fn new() -> Self {
        Self {
            handle: unsafe { rcl_get_zero_initialized_wait_set() },
            capacities: None,
        }
    }

    fn prepare(
        &mut self, capacities: WaitSetCapacities, context: &mut rcl_context_t,
    ) -> Result<()> {
        let ret = match self.capacities {
            None => unsafe {
                rcl_wait_set_init(
                    &mut self.handle,
                    capacities.subscriptions,
                    capacities.guard_conditions,
                    capacities.timers,
                    capacities.clients,
                    capacities.services,
                    0,
                    context,
                    rcutils_get_default_allocator(),
                )
            },
            Some(previous) if previous != capacities => unsafe {
                rcl_wait_set_resize(
                    &mut self.handle,
                    capacities.subscriptions,
                    capacities.guard_conditions,
                    capacities.timers,
                    capacities.clients,
                    capacities.services,
                    0,
                )
            },
            Some(_) => unsafe { rcl_wait_set_clear(&mut self.handle) },
        };
        check_rcl_ret(ret)?;
        self.capacities = Some(capacities);
        Ok(())
    }

    fn fini(&mut self) {
        if self.capacities.take().is_some() {
            let ret = unsafe { rcl_wait_set_fini(&mut self.handle) };
            if ret != RCL_RET_OK as i32 {
                log::error!("could not finalize wait set: {}", ret);
            }
        }
    }
}

impl Drop for WaitSet {
    fn drop(&mut self) {
        self.fini();
    }
}

fn check_rcl_ret(ret: rcl_ret_t) -> Result<()> {
    if ret == RCL_RET_OK as i32 {
        Ok(())
    } else {
        Err(Error::from_rcl_error(ret))
    }
}

fn duration_to_rcl_nanos(duration: Duration) -> Result<i64> {
    i64::try_from(duration.as_nanos()).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)
}

pub trait IsAvailablePollable {
    fn register_poll_available(&self, sender: oneshot::Sender<()>) -> Result<()>;
}

pub struct TopicEndpointInfo {
    pub node_name: String,
    pub node_namespace: String,
    pub topic_type: String,
    pub endpoint_gid: [u8; RMW_GID_STORAGE_SIZE as usize],
    pub qos_profile: QosProfile,
}

impl From<rmw_topic_endpoint_info_t> for TopicEndpointInfo {
    fn from(info: rmw_topic_endpoint_info_t) -> Self {
        // Convert C strings to Rust String
        let node_name = c_string_from_ptr(info.node_name).unwrap_or_default();
        let node_namespace = c_string_from_ptr(info.node_namespace).unwrap_or_default();
        let topic_type = c_string_from_ptr(info.topic_type).unwrap_or_default();

        // Copy the endpoint_gid array
        let endpoint_gid: [u8; RMW_GID_STORAGE_SIZE as usize] = info.endpoint_gid;

        // Convert qos_profile
        let qos_profile = QosProfile::from(info.qos_profile); // Adjust this line based on how QosProfile is defined

        TopicEndpointInfo {
            node_name,
            node_namespace,
            topic_type,
            endpoint_gid,
            qos_profile,
        }
    }
}

impl TopicEndpointInfo {
    fn try_from_native(info: rmw_topic_endpoint_info_t) -> Result<Self> {
        Ok(Self {
            node_name: c_string_from_ptr(info.node_name)?,
            node_namespace: c_string_from_ptr(info.node_namespace)?,
            topic_type: c_string_from_ptr(info.topic_type)?,
            endpoint_gid: info.endpoint_gid,
            qos_profile: QosProfile::from(info.qos_profile),
        })
    }
}

fn convert_info_array_to_vec(
    info_array: &rcl_topic_endpoint_info_array_t,
) -> Result<Vec<TopicEndpointInfo>> {
    let mut topic_info_list = Vec::with_capacity(info_array.size);

    if info_array.size > 0 {
        if info_array.info_array.is_null() {
            return Err(Error::RCL_RET_ERROR);
        }
        let infos = unsafe { std::slice::from_raw_parts(info_array.info_array, info_array.size) };
        for &info in infos {
            topic_info_list.push(TopicEndpointInfo::try_from_native(info)?);
        }
    }

    Ok(topic_info_list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{executor::LocalPool, task::LocalSpawnExt};

    #[test]
    fn converts_rcl_durations_without_wrapping() {
        assert_eq!(duration_to_rcl_nanos(Duration::from_nanos(i64::MAX as u64)).unwrap(), i64::MAX);
        assert!(duration_to_rcl_nanos(Duration::from_nanos(i64::MAX as u64 + 1)).is_err());
    }

    #[test]
    fn adds_cached_action_wait_set_capacities() {
        let mut total = WaitSetCapacities {
            subscriptions: 1,
            guard_conditions: 2,
            timers: 3,
            clients: 4,
            services: 5,
        };
        total.add(WaitSetCapacities {
            subscriptions: 5,
            guard_conditions: 4,
            timers: 3,
            clients: 2,
            services: 1,
        });

        assert_eq!(
            total,
            WaitSetCapacities {
                subscriptions: 6,
                guard_conditions: 6,
                timers: 6,
                clients: 6,
                services: 6,
            }
        );
    }

    #[test]
    fn strict_c_string_conversion_rejects_invalid_utf8() {
        let value = CStr::from_bytes_with_nul(&[0xff, 0]).unwrap();

        assert!(matches!(
            c_str_to_owned(value, "test string"),
            Err(Error::InvalidUtf8 {
                context: "test string",
                ..
            })
        ));
    }

    #[test]
    fn discovers_topic_names_and_their_types() {
        let ctx = Context::create().unwrap();
        let mut node = Node::create(ctx, "topic_discovery_test", "").unwrap();
        let topic = format!("/r2r_discovery_{}", uuid::Uuid::new_v4().simple());
        let _publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
            .unwrap();
        let _subscriber = node
            .subscribe::<crate::std_msgs::msg::String>(&topic, QosProfile::default().best_effort())
            .unwrap();

        let topics = node.get_topic_names_and_types().unwrap();
        let publishers = node.get_publishers_info_by_topic(&topic, false).unwrap();
        let subscriptions = node.get_subscriptions_info_by_topic(&topic, false).unwrap();

        assert_eq!(topics.get(&topic), Some(&vec!["std_msgs/msg/String".to_owned()]));
        assert!(publishers
            .iter()
            .any(|publisher| publisher.topic_type == "std_msgs/msg/String"));
        assert!(subscriptions.iter().any(|subscription| {
            subscription.topic_type == "std_msgs/msg/String"
                && subscription.qos_profile.reliability
                    == QosProfile::default().best_effort().reliability
        }));
    }

    #[test]
    fn discovers_service_names_and_their_types() {
        let ctx = Context::create().unwrap();
        let mut node = Node::create(ctx, "service_discovery_test", "").unwrap();
        let service_name = format!("/r2r_service_discovery_{}", uuid::Uuid::new_v4().simple());
        let _requests = node
            .create_service::<crate::example_interfaces::srv::AddTwoInts::Service>(
                &service_name,
                QosProfile::default(),
            )
            .unwrap();

        let services = node.get_service_names_and_types().unwrap();

        assert_eq!(
            services.get(&service_name),
            Some(&vec!["example_interfaces/srv/AddTwoInts".to_owned()])
        );
    }

    #[test]
    fn publisher_loan_is_returned_or_reports_unsupported() {
        let ctx = Context::create().unwrap();
        let mut node = Node::create(ctx, "publisher_loan_test", "").unwrap();
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/r2r_publisher_loan_test",
                QosProfile::default(),
            )
            .unwrap();

        match publisher.borrow_loaned_message(&mut node) {
            Ok(loan) => drop(loan),
            Err(Error::RCL_RET_UNSUPPORTED) => {}
            Err(err) => panic!("unexpected loan error: {err}"),
        };
    }

    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    #[test]
    fn parameter_service_switches_sim_time_at_runtime() {
        use rcl_interfaces::{
            msg,
            srv::{SetParameters, SetParametersAtomically},
        };

        fn sim_time_parameter(value: msg::ParameterValue) -> msg::Parameter {
            msg::Parameter {
                name: "use_sim_time".into(),
                value,
            }
        }

        fn bool_value(enabled: bool) -> msg::ParameterValue {
            msg::ParameterValue {
                type_: msg::ParameterType::PARAMETER_BOOL as u8,
                bool_value: enabled,
                ..Default::default()
            }
        }

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "runtime_sim_time_test", "").unwrap();
        let (handler, _events) = node.make_parameter_handler().unwrap();
        let client = node
            .create_client::<SetParameters::Service>(
                "runtime_sim_time_test/set_parameters",
                QosProfile::default(),
            )
            .unwrap();
        let atomic_client = node
            .create_client::<SetParametersAtomically::Service>(
                "runtime_sim_time_test/set_parameters_atomically",
                QosProfile::default(),
            )
            .unwrap();
        let mut pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();

        for enabled in [true, false] {
            let response = client
                .request(&SetParameters::Request {
                    parameters: vec![sim_time_parameter(bool_value(enabled))],
                })
                .unwrap();
            let mut response = Box::pin(response);
            let result = loop {
                node.try_spin_once(Duration::from_millis(10)).unwrap();
                pool.run_until_stalled();
                if let Some(result) = response.as_mut().now_or_never() {
                    break result.unwrap();
                }
            };
            assert!(result.results[0].successful);

            node.try_spin_once(Duration::ZERO).unwrap();
            assert_eq!(node.time_source.is_sim_time_enabled(), enabled);
        }

        let invalid = client
            .request(&SetParameters::Request {
                parameters: vec![sim_time_parameter(msg::ParameterValue {
                    type_: msg::ParameterType::PARAMETER_STRING as u8,
                    string_value: "true".into(),
                    ..Default::default()
                })],
            })
            .unwrap();
        let mut invalid = Box::pin(invalid);
        let invalid = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            pool.run_until_stalled();
            if let Some(result) = invalid.as_mut().now_or_never() {
                break result.unwrap();
            }
        };
        assert!(!invalid.results[0].successful);
        assert!(!node.time_source.is_sim_time_enabled());

        let atomic = atomic_client
            .request(&SetParametersAtomically::Request {
                parameters: vec![sim_time_parameter(bool_value(true))],
            })
            .unwrap();
        let mut atomic = Box::pin(atomic);
        let atomic = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            pool.run_until_stalled();
            if let Some(result) = atomic.as_mut().now_or_never() {
                break result.unwrap();
            }
        };
        assert!(atomic.result.successful);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.time_source.is_sim_time_enabled());
    }
}
