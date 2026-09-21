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
    action_common::{ActionClientOptions, ActionServerOptions},
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

struct NodeOptionsNative(rcl_node_options_t);

impl NodeOptionsNative {
    fn new(config: &NodeOptions) -> Result<Self> {
        let mut options = unsafe { rcl_node_get_default_options() };
        options.use_global_arguments = config.use_global_arguments;
        options.enable_rosout = config.enable_rosout;
        if let Some(qos) = &config.rosout_qos {
            options.rosout_qos = qos.clone().into();
        }
        if !config.use_global_arguments {
            let ret = unsafe {
                rcl_parse_arguments(0, std::ptr::null(), options.allocator, &mut options.arguments)
            };
            if ret != RCL_RET_OK as i32 {
                unsafe { rcl_node_options_fini(&mut options) };
                return Err(Error::from_rcl_error(ret));
            }
        }
        Ok(Self(options))
    }
}

impl Drop for NodeOptionsNative {
    fn drop(&mut self) {
        let ret = unsafe { rcl_node_options_fini(&mut self.0) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize node options: {ret}");
        }
    }
}

#[derive(Default)]
struct ParameterEventChanges {
    new_parameters: Vec<rcl_interfaces::msg::Parameter>,
    changed_parameters: Vec<rcl_interfaces::msg::Parameter>,
    deleted_parameters: Vec<rcl_interfaces::msg::Parameter>,
}

impl ParameterEventChanges {
    fn record(&mut self, name: &str, previous: Option<&ParameterValue>, value: &ParameterValue) {
        let parameter = rcl_interfaces::msg::Parameter {
            name: name.to_owned(),
            value: value.clone().into_parameter_value_msg(),
        };
        match (previous, value) {
            (Some(ParameterValue::NotSet) | None, ParameterValue::NotSet) => {}
            (Some(_), ParameterValue::NotSet) => self.deleted_parameters.push(parameter),
            (Some(ParameterValue::NotSet) | None, _) => self.new_parameters.push(parameter),
            (Some(_), _) => self.changed_parameters.push(parameter),
        }
    }

    fn publish(
        self, publisher: &Publisher<rcl_interfaces::msg::ParameterEvent>,
        clock: &Arc<Mutex<Clock>>, node_name: &str,
    ) {
        if self.new_parameters.is_empty()
            && self.changed_parameters.is_empty()
            && self.deleted_parameters.is_empty()
        {
            return;
        }
        let stamp = clock
            .lock()
            .get_now()
            .and_then(|now| Clock::try_to_builtin_time(&now));
        let Ok(stamp) = stamp else {
            log::error!("could not timestamp parameter event");
            return;
        };
        let event = rcl_interfaces::msg::ParameterEvent {
            stamp,
            node: node_name.to_owned(),
            new_parameters: self.new_parameters,
            changed_parameters: self.changed_parameters,
            deleted_parameters: self.deleted_parameters,
        };
        if let Err(err) = publisher.publish(&event) {
            log::warn!("could not publish parameter event: {err}");
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

/// Options applied while creating a [`Node`].
#[derive(Debug, Clone)]
pub struct NodeOptions {
    /// Whether the node consumes global ROS arguments from its [`Context`].
    pub use_global_arguments: bool,
    /// Programmatic parameter overrides. These take precedence over global CLI/YAML overrides.
    pub parameter_overrides: IndexMap<String, ParameterValue>,
    /// Whether the node publishes logs on `/rosout`.
    pub enable_rosout: bool,
    /// Optional override for the `/rosout` publisher QoS.
    pub rosout_qos: Option<QosProfile>,
}

impl Default for NodeOptions {
    fn default() -> Self {
        Self {
            use_global_arguments: true,
            parameter_overrides: IndexMap::new(),
            enable_rosout: true,
            rosout_qos: None,
        }
    }
}

impl NodeOptions {
    pub fn use_global_arguments(mut self, enabled: bool) -> Self {
        self.use_global_arguments = enabled;
        self
    }

    pub fn parameter_override(mut self, name: impl Into<String>, value: ParameterValue) -> Self {
        self.parameter_overrides.insert(name.into(), value);
        self
    }

    pub fn parameter_overrides(
        mut self, overrides: impl IntoIterator<Item = (String, ParameterValue)>,
    ) -> Self {
        self.parameter_overrides.extend(overrides);
        self
    }

    pub fn enable_rosout(mut self, enabled: bool) -> Self {
        self.enable_rosout = enabled;
        self
    }

    pub fn rosout_qos(mut self, qos: QosProfile) -> Self {
        self.rosout_qos = Some(qos);
        self
    }
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
        let name = self.name()?;

        for (nn, np) in node_names.iter().zip(node_params) {
            if nn.is_null() {
                return Err(Error::RCL_RET_ERROR);
            }
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

            for (s, v) in param_names.iter().zip(param_values) {
                if s.is_null() {
                    return Err(Error::RCL_RET_ERROR);
                }
                let s = unsafe { CStr::from_ptr(*s) };
                let key = c_str_to_str(s, "parameter override name")?;
                let val = ParameterValue::from_rcl(v)?;
                params.insert(key.to_owned(), Parameter::new(val));
            }
        }

        Ok(())
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
            if options.use_global_arguments {
                node.load_params()?;
            }
            {
                let mut params = node.params.lock();
                for (name, value) in options.parameter_overrides {
                    params.insert(name, Parameter::new(value));
                }
            }
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
        let fully_qualified_node_name = self.fully_qualified_name()?;
        let parameter_event_publisher = self
            .create_publisher::<rcl_interfaces::msg::ParameterEvent>(
                "/parameter_events",
                QosProfile::parameter_events(),
            )?;
        let parameter_event_clock = self.ros_clock.clone();

        // rcl_interfaces/srv/SetParameters
        let set_params_request_stream = self
            .create_service::<rcl_interfaces::srv::SetParameters::Service>(
                &format!("{}/set_parameters", node_name),
                QosProfile::default(),
            )?;

        let params = self.params.clone();
        let params_struct_clone = params_struct.clone();
        let set_parameter_event_publisher = parameter_event_publisher.clone();
        let set_parameter_event_clock = parameter_event_clock.clone();
        let set_parameter_event_node = fully_qualified_node_name.clone();
        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        let set_params_time_source = self.time_source.clone();
        let set_params_future = set_params_request_stream.for_each(
            move |req: ServiceRequest<rcl_interfaces::srv::SetParameters::Service>| {
                let mut result = rcl_interfaces::srv::SetParameters::Response::default();
                let mut ros_events = ParameterEventChanges::default();
                for p in &req.message.parameters {
                    let val = ParameterValue::from_parameter_value_msg(p.value.clone());
                    let previous = params.lock().get(&p.name).map(|v| v.value.clone());
                    let changed = previous.as_ref() != Some(&val);
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
                        ros_events.record(&p.name, previous.as_ref(), &val);
                        if let Err(e) = set_event_tx.try_send((p.name.clone(), val)) {
                            log::debug!("Warning: could not send parameter event ({}).", e);
                        }
                    }
                    result.results.push(r);
                }
                ros_events.publish(
                    &set_parameter_event_publisher,
                    &set_parameter_event_clock,
                    &set_parameter_event_node,
                );
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
        let atomic_parameter_event_publisher = parameter_event_publisher;
        let atomic_parameter_event_clock = parameter_event_clock;
        let atomic_parameter_event_node = fully_qualified_node_name;
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
                        let (changed, ros_events) = {
                            let mut params = params.lock();
                            let mut local_events = Vec::new();
                            let mut ros_events = ParameterEventChanges::default();
                            for (name, value) in &updates {
                                let previous = params.get(name).map(|parameter| &parameter.value);
                                if previous != Some(value) {
                                    local_events.push((name.clone(), value.clone()));
                                    ros_events.record(name, previous, value);
                                    params
                                        .entry(name.clone())
                                        .and_modify(|parameter| parameter.value = value.clone())
                                        .or_insert_with(|| Parameter::new(value.clone()));
                                }
                            }
                            (local_events, ros_events)
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
                        ros_events.publish(
                            &atomic_parameter_event_publisher,
                            &atomic_parameter_event_clock,
                            &atomic_parameter_event_node,
                        );
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
    pub fn subscribe<T>(&mut self, topic: &str, qos_profile: QosProfile) -> Result<Subscription<T>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_with_options(topic, qos_profile, SubscriptionOptions::default())
    }

    /// Subscribe to a ROS topic with explicit Rust-side stream buffering options.
    pub fn subscribe_with_options<T>(
        &mut self, topic: &str, qos_profile: QosProfile, options: SubscriptionOptions,
    ) -> Result<Subscription<T>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_with_transform(topic, qos_profile, options, discard_message_info::<T>)
    }

    /// Subscribe to a ROS topic and retain the RMW metadata for each message.
    pub fn subscribe_with_info<T>(
        &mut self, topic: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<ReceivedMessage<T>>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_with_info_and_options(topic, qos_profile, SubscriptionOptions::default())
    }

    /// Subscribe with RMW metadata and explicit Rust-side stream buffering options.
    pub fn subscribe_with_info_and_options<T>(
        &mut self, topic: &str, qos_profile: QosProfile, options: SubscriptionOptions,
    ) -> Result<Subscription<ReceivedMessage<T>>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_with_transform(topic, qos_profile, options, attach_message_info::<T>)
    }

    fn subscribe_with_transform<T, O>(
        &mut self, topic: &str, qos_profile: QosProfile, options: SubscriptionOptions,
        transform: fn(T, MessageInfo) -> O,
    ) -> Result<Subscription<O>>
    where
        T: 'static + WrappedTypesupport,
        O: 'static,
    {
        let options = options.validate()?;
        let subscription_handle = create_subscription_helper(
            self.node_handle.as_mut(),
            topic,
            T::get_ts(),
            qos_profile,
            options.ignore_local_publications,
            options.unique_network_flow,
        )?;
        let (sender, receiver) = mpsc::channel::<O>(options.channel_capacity);
        let id = uuid::Uuid::new_v4();

        let ws = TypedSubscriber {
            rcl_handle: subscription_handle,
            sender,
            transform,
            match_state: SubscriptionMatchState::new(id),
        };
        self.subscribers.push(Box::new(ws));
        Ok(Subscription::new(receiver, id))
    }

    /// Subscribe to standard ROS parameter events, optionally filtering by node and parameter.
    ///
    /// Each parameter in a ROS event is emitted as an individual [`ParameterEventChange`].
    pub fn subscribe_parameter_events(
        &mut self, node: Option<String>, parameter: Option<String>,
    ) -> Result<impl Stream<Item = ParameterEventChange> + Unpin> {
        let events = self.subscribe::<rcl_interfaces::msg::ParameterEvent>(
            "/parameter_events",
            QosProfile::parameter_events(),
        )?;
        Ok(events
            .map(move |event| flatten_parameter_event(event, node.as_deref(), parameter.as_deref()))
            .map(futures::stream::iter)
            .flatten())
    }

    /// Subscribe to a ROS topic.
    ///
    /// This function returns a `Stream` of ros messages without the rust convenience types.
    pub fn subscribe_native<T>(
        &mut self, topic: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<WrappedNativeMsg<T>>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_native_with_options(topic, qos_profile, SubscriptionOptions::default())
    }

    /// Subscribe to native ROS messages with explicit Rust-side stream buffering options.
    pub fn subscribe_native_with_options<T>(
        &mut self, topic: &str, qos_profile: QosProfile, options: SubscriptionOptions,
    ) -> Result<Subscription<WrappedNativeMsg<T>>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_native_with_transform(
            topic,
            qos_profile,
            options,
            discard_message_info::<WrappedNativeMsg<T>>,
        )
    }

    /// Subscribe to native ROS messages and retain the RMW metadata for each message.
    pub fn subscribe_native_with_info<T>(
        &mut self, topic: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<ReceivedMessage<WrappedNativeMsg<T>>>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_native_with_info_and_options(
            topic,
            qos_profile,
            SubscriptionOptions::default(),
        )
    }

    /// Subscribe to native messages with metadata and explicit stream buffering options.
    pub fn subscribe_native_with_info_and_options<T>(
        &mut self, topic: &str, qos_profile: QosProfile, options: SubscriptionOptions,
    ) -> Result<Subscription<ReceivedMessage<WrappedNativeMsg<T>>>>
    where
        T: 'static + WrappedTypesupport,
    {
        self.subscribe_native_with_transform(
            topic,
            qos_profile,
            options,
            attach_message_info::<WrappedNativeMsg<T>>,
        )
    }

    fn subscribe_native_with_transform<T, O>(
        &mut self, topic: &str, qos_profile: QosProfile, options: SubscriptionOptions,
        transform: fn(WrappedNativeMsg<T>, MessageInfo) -> O,
    ) -> Result<Subscription<O>>
    where
        T: 'static + WrappedTypesupport,
        O: 'static,
    {
        let options = options.validate()?;
        let subscription_handle = create_subscription_helper(
            self.node_handle.as_mut(),
            topic,
            T::get_ts(),
            qos_profile,
            options.ignore_local_publications,
            options.unique_network_flow,
        )?;
        let (sender, receiver) = mpsc::channel::<O>(options.channel_capacity);
        let id = uuid::Uuid::new_v4();

        let ws = NativeSubscriber {
            rcl_handle: subscription_handle,
            sender,
            transform,
            match_state: SubscriptionMatchState::new(id),
        };
        self.subscribers.push(Box::new(ws));
        Ok(Subscription::new(receiver, id))
    }

    /// Subscribe to a ROS topic.
    ///
    /// This function returns a `Stream` of ros messages as `serde_json::Value`:s.
    /// Useful when you cannot know the type of the message at compile time.
    pub fn subscribe_untyped(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<Result<serde_json::Value>>> {
        self.subscribe_untyped_with_options(
            topic,
            topic_type,
            qos_profile,
            SubscriptionOptions::default(),
        )
    }

    /// Subscribe to untyped messages with explicit Rust-side stream buffering options.
    pub fn subscribe_untyped_with_options(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: SubscriptionOptions,
    ) -> Result<Subscription<Result<serde_json::Value>>> {
        self.subscribe_untyped_with_transform(
            topic,
            topic_type,
            qos_profile,
            options,
            discard_message_info::<Result<serde_json::Value>>,
        )
    }

    /// Subscribe to untyped ROS messages and retain the RMW metadata for each message.
    pub fn subscribe_untyped_with_info(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<ReceivedMessage<Result<serde_json::Value>>>> {
        self.subscribe_untyped_with_info_and_options(
            topic,
            topic_type,
            qos_profile,
            SubscriptionOptions::default(),
        )
    }

    /// Subscribe to untyped messages with metadata and explicit stream buffering options.
    pub fn subscribe_untyped_with_info_and_options(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: SubscriptionOptions,
    ) -> Result<Subscription<ReceivedMessage<Result<serde_json::Value>>>> {
        self.subscribe_untyped_with_transform(
            topic,
            topic_type,
            qos_profile,
            options,
            attach_message_info::<Result<serde_json::Value>>,
        )
    }

    fn subscribe_untyped_with_transform<O>(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: SubscriptionOptions, transform: fn(Result<serde_json::Value>, MessageInfo) -> O,
    ) -> Result<Subscription<O>>
    where
        O: 'static,
    {
        let options = options.validate()?;
        let msg = WrappedNativeMsgUntyped::new_from(topic_type)?;
        let subscription_handle = create_subscription_helper(
            self.node_handle.as_mut(),
            topic,
            msg.ts,
            qos_profile,
            options.ignore_local_publications,
            options.unique_network_flow,
        )?;
        let (sender, receiver) = mpsc::channel::<O>(options.channel_capacity);
        let id = uuid::Uuid::new_v4();

        let ws = UntypedSubscriber {
            rcl_handle: subscription_handle,
            topic_type: topic_type.to_string(),
            sender,
            transform,
            match_state: SubscriptionMatchState::new(id),
        };
        self.subscribers.push(Box::new(ws));
        Ok(Subscription::new(receiver, id))
    }

    /// Subscribe to a ROS topic.
    ///
    /// This function returns a `Stream` of ros messages as non-deserialized `Vec<u8>`:s.
    /// Useful if you just want to pass the data along to another part of the system.
    pub fn subscribe_raw(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<Vec<u8>>> {
        self.subscribe_raw_with_options(
            topic,
            topic_type,
            qos_profile,
            SubscriptionOptions::default(),
        )
    }

    /// Subscribe to serialized messages with explicit Rust-side stream buffering options.
    pub fn subscribe_raw_with_options(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: SubscriptionOptions,
    ) -> Result<Subscription<Vec<u8>>> {
        self.subscribe_raw_with_transform(
            topic,
            topic_type,
            qos_profile,
            options,
            discard_message_info::<Vec<u8>>,
        )
    }

    /// Subscribe to serialized ROS messages and retain the RMW metadata for each message.
    pub fn subscribe_raw_with_info(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
    ) -> Result<Subscription<ReceivedMessage<Vec<u8>>>> {
        self.subscribe_raw_with_info_and_options(
            topic,
            topic_type,
            qos_profile,
            SubscriptionOptions::default(),
        )
    }

    /// Subscribe to serialized messages with metadata and explicit stream buffering options.
    pub fn subscribe_raw_with_info_and_options(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: SubscriptionOptions,
    ) -> Result<Subscription<ReceivedMessage<Vec<u8>>>> {
        self.subscribe_raw_with_transform(
            topic,
            topic_type,
            qos_profile,
            options,
            attach_message_info::<Vec<u8>>,
        )
    }

    fn subscribe_raw_with_transform<O>(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: SubscriptionOptions, transform: fn(Vec<u8>, MessageInfo) -> O,
    ) -> Result<Subscription<O>>
    where
        O: 'static,
    {
        let options = options.validate()?;
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

        let subscription_handle = match create_subscription_helper(
            self.node_handle.as_mut(),
            topic,
            msg.ts,
            qos_profile,
            options.ignore_local_publications,
            options.unique_network_flow,
        ) {
            Ok(handle) => handle,
            Err(err) => {
                unsafe {
                    rcutils_uint8_array_fini(&mut msg_buf as *mut rcl_serialized_message_t);
                }
                return Err(err);
            }
        };
        let (sender, receiver) = mpsc::channel::<O>(options.channel_capacity);
        let id = uuid::Uuid::new_v4();

        let ws = RawSubscriber {
            rcl_handle: subscription_handle,
            msg_buf,
            sender,
            transform,
            match_state: SubscriptionMatchState::new(id),
        };
        self.subscribers.push(Box::new(ws));
        Ok(Subscription::new(receiver, id))
    }

    pub(crate) fn get_subscription_publisher_count(&self, id: uuid::Uuid) -> Result<usize> {
        self.subscribers
            .iter()
            .find(|subscriber| subscriber.match_state().is_some_and(|state| state.id == id))
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?
            .publisher_count()
    }

    pub(crate) fn get_subscription_actual_qos(&self, id: uuid::Uuid) -> Result<QosProfile> {
        self.subscribers
            .iter()
            .find(|subscriber| subscriber.match_state().is_some_and(|state| state.id == id))
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?
            .actual_qos()
    }

    pub(crate) fn get_subscription_topic_name(&self, id: uuid::Uuid) -> Result<String> {
        self.subscribers
            .iter()
            .find(|subscriber| subscriber.match_state().is_some_and(|state| state.id == id))
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?
            .topic_name()
    }

    pub(crate) fn register_subscription_publisher_wait(
        &mut self, id: uuid::Uuid,
    ) -> Result<oneshot::Receiver<()>> {
        let state = self
            .subscribers
            .iter_mut()
            .filter_map(|subscriber| subscriber.match_state_mut())
            .find(|state| state.id == id)
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        let (sender, receiver) = oneshot::channel();
        state.waiters.push(sender);
        Ok(receiver)
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
            sender,
            id,
        };

        self.services.push(Arc::new(Mutex::new(ws)));
        Ok(Service::new(receiver, id))
    }

    pub(crate) fn get_service_request_subscription_actual_qos(
        &self, id: uuid::Uuid,
    ) -> Result<QosProfile> {
        let service = self
            .services
            .iter()
            .find(|service| service.lock().id() == Some(id))
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?
            .lock();
        service.request_subscription_actual_qos()
    }

    pub(crate) fn get_service_response_publisher_actual_qos(
        &self, id: uuid::Uuid,
    ) -> Result<QosProfile> {
        let service = self
            .services
            .iter()
            .find(|service| service.lock().id() == Some(id))
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?
            .lock();
        service.response_publisher_actual_qos()
    }

    pub(crate) fn get_service_name(&self, id: uuid::Uuid) -> Result<String> {
        let service = self
            .services
            .iter()
            .find(|service| service.lock().id() == Some(id))
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?
            .lock();
        service.service_name()
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

    /// Wait for a service or action server to become available, up to `timeout`.
    ///
    /// This future is runtime-agnostic and advances while the node is spun.
    pub fn is_available_with_timeout(
        &mut self, client: &dyn IsAvailablePollable, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        let available = Self::is_available(client)?;
        self.with_timeout(available, timeout)
    }

    /// Wraps an r2r future with a runtime-agnostic wall-clock timeout.
    ///
    /// The returned future advances while this node is spun.
    pub fn with_timeout<T>(
        &mut self, future: impl Future<Output = Result<T>>, timeout: Duration,
    ) -> Result<impl Future<Output = Result<T>>> {
        let timer = self.create_wall_timer(timeout)?;
        Ok(future_with_timeout(future, timer))
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
        let client = WrappedActionClient::<T> {
            rcl_handle: client_handle,
            goal_response_channels: Vec::new(),
            cancel_response_channels: Vec::new(),
            feedback_senders: Vec::new(),
            pending_feedback: None,
            result_senders: Vec::new(),
            result_requests: Vec::new(),
            goal_status: HashMap::new(),
            feedback_channel_capacity: options.feedback_channel_capacity,
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
        let client = WrappedActionClientUntyped {
            action_type_support,
            rcl_handle: client_handle,
            goal_response_channels: Vec::new(),
            cancel_response_channels: Vec::new(),
            feedback_senders: Vec::new(),
            pending_feedback: None,
            result_senders: Vec::new(),
            result_requests: Vec::new(),
            goal_status: HashMap::new(),
            feedback_channel_capacity: options.feedback_channel_capacity,
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
        self.create_action_server_with_options(action_name, ActionServerOptions::default())
    }

    /// Create a ROS action server with per-channel QoS and result-retention overrides.
    pub fn create_action_server_with_options<T>(
        &mut self, action_name: &str, options: ActionServerOptions,
    ) -> Result<impl Stream<Item = ActionServerGoalRequest<T>> + Unpin>
    where
        T: 'static + WrappedActionTypeSupport,
    {
        options.validate()?;
        let goal_request_channel_capacity = options.goal_request_channel_capacity;
        let cancel_request_channel_capacity = options.cancel_request_channel_capacity;
        let (goal_request_sender, goal_request_receiver) =
            mpsc::channel::<ActionServerGoalRequest<T>>(goal_request_channel_capacity);

        let clock = self.ros_clock.clone();
        let mut server_handle = {
            let mut clock_handle = clock.lock();
            create_action_server_helper(
                self.node_handle.as_mut(),
                action_name,
                clock_handle.clock_handle.as_mut(),
                T::get_ts(),
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
        let server = WrappedActionServer::<T> {
            rcl_handle: server_handle,
            clock,
            goal_request_sender,
            pending_cancel_request: None,
            active_cancel_requests: Vec::new(),
            cancel_senders: HashMap::new(),
            goals: HashMap::new(),
            result_msgs: HashMap::new(),
            result_requests: HashMap::new(),
            cancel_request_channel_capacity,
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
        self.create_publisher_with_options(topic, qos_profile, PublisherOptions::default())
    }

    /// Create a ROS publisher with endpoint creation options.
    pub fn create_publisher_with_options<T>(
        &mut self, topic: &str, qos_profile: QosProfile, options: PublisherOptions,
    ) -> Result<Publisher<T>>
    where
        T: WrappedTypesupport,
    {
        let publisher_handle = create_publisher_helper(
            self.node_handle.as_mut(),
            topic,
            T::get_ts(),
            qos_profile,
            options,
        )?;
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
        self.create_publisher_untyped_with_options(
            topic,
            topic_type,
            qos_profile,
            PublisherOptions::default(),
        )
    }

    /// Create an untyped ROS publisher with endpoint creation options.
    pub fn create_publisher_untyped_with_options(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: PublisherOptions,
    ) -> Result<PublisherUntyped> {
        let dummy = WrappedNativeMsgUntyped::new_from(topic_type)?;
        let publisher_handle = create_publisher_helper(
            self.node_handle.as_mut(),
            topic,
            dummy.ts,
            qos_profile,
            options,
        )?;
        let arc = Arc::new(publisher_handle);
        let p = make_publisher_untyped(Arc::downgrade(&arc), topic_type.to_owned());
        self.pubs.push(arc);
        Ok(p)
    }

    /// Destroy a ROS publisher.
    pub fn destroy_publisher<T: WrappedTypesupport>(&mut self, p: Publisher<T>) {
        if let Some(handle) = p.handle.upgrade() {
            // Only the node that created the publisher may pass its native node handle to fini.
            if let Some(index) = self.pubs.iter().position(|p| Arc::ptr_eq(p, &handle)) {
                self.pubs.swap_remove(index);
                handle.destroy(self.node_handle.as_mut());
            }
        }
    }

    /// Destroy a ROS publisher.
    pub fn destroy_publisher_untyped(&mut self, p: PublisherUntyped) {
        if let Some(handle) = p.handle.upgrade() {
            // Only the node that created the publisher may pass its native node handle to fini.
            if let Some(index) = self.pubs.iter().position(|p| Arc::ptr_eq(p, &handle)) {
                self.pubs.swap_remove(index);
                handle.destroy(self.node_handle.as_mut());
            }
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
        let node_handle = self.node_handle.as_mut();
        self.subscribers.retain_mut(|subscriber| {
            if subscriber.should_remove() {
                subscriber.destroy(node_handle);
                false
            } else {
                true
            }
        });

        // A dropped service stream cannot receive another request to trigger its old reactive
        // cleanup path. Remove it before rebuilding the wait set so it leaves the ROS graph even
        // when the service is idle.
        let node_handle = self.node_handle.as_mut();
        self.services.retain(|service| {
            let mut handle = service.lock();
            if handle.should_remove() {
                handle.destroy(node_handle);
                false
            } else {
                true
            }
        });

        // Public clients only keep weak references to their native state. Retain a client while
        // any request or availability future is still live, then release it once both the public
        // handle and all asynchronous work have gone away.
        let node_handle = self.node_handle.as_mut();
        self.clients.retain(|client| {
            let mut handle = client.lock();
            if Arc::weak_count(client) == 0 && !handle.has_pending_work() {
                handle.destroy(node_handle);
                false
            } else {
                true
            }
        });

        // A publisher handle is represented by a Weak reference, while the node owns the native
        // publisher. Release the native resource after the final public handle and any matching
        // waiter have both gone away.
        let node_handle = self.node_handle.as_mut();
        self.pubs.retain(|publisher| {
            if Arc::weak_count(publisher) == 0 && !publisher.has_pending_subscriber_waits() {
                publisher.destroy(node_handle);
                false
            } else {
                true
            }
        });

        // Do not wait for a long-period timer to become ready before noticing that its public
        // handle has gone away. Dropping Timer closes the receiver immediately.
        self.timers.retain(|timer| !timer.sender.is_closed());

        let node_handle = self.node_handle.as_mut();
        let mut removed_action_capacities = WaitSetCapacities::default();
        self.action_clients.retain(|client| {
            let mut handle = client.lock();
            if Arc::weak_count(client) == 0 && !handle.has_pending_work() {
                match action_client_wait_set_capacities(handle.handle()) {
                    Ok(capacities) => {
                        handle.destroy(node_handle);
                        removed_action_capacities.add(capacities);
                        false
                    }
                    Err(err) => {
                        log::warn!("could not inspect dropped action client: {err}");
                        true
                    }
                }
            } else {
                true
            }
        });
        self.action_wait_set_capacities
            .subtract(removed_action_capacities);

        // first handle any completed action cancellation responses
        for a in &mut self.action_servers {
            a.lock().send_completed_cancel_requests()?;
        }

        let node_handle = self.node_handle.as_mut();
        let mut removed_action_capacities = WaitSetCapacities::default();
        self.action_servers.retain(|server| {
            let mut handle = server.lock();
            if handle.should_remove() {
                match action_server_wait_set_capacities(handle.handle()) {
                    Ok(capacities) => {
                        handle.destroy(node_handle);
                        removed_action_capacities.add(capacities);
                        false
                    }
                    Err(err) => {
                        log::warn!("could not inspect dropped action server: {err}");
                        true
                    }
                }
            } else {
                true
            }
        });
        self.action_wait_set_capacities
            .subtract(removed_action_capacities);

        // as well as polling any services/action servers for availability
        for c in &mut self.clients {
            c.lock().poll_available(self.node_handle.as_mut());
        }

        for c in &mut self.action_clients {
            c.lock().poll_available(self.node_handle.as_mut());
        }

        for p in &self.pubs {
            p.poll_has_subscribers();
        }
        for subscriber in &mut self.subscribers {
            subscriber.poll_has_publishers();
        }

        let timeout_nanos = duration_to_rcl_nanos(timeout)?;

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
        if capacities == WaitSetCapacities::default() {
            // RCL rejects an empty wait set instead of treating it as an idle wait. Honor the
            // documented spin timeout and avoid a busy loop for nodes that only publish or have
            // not created any waitables yet.
            std::thread::sleep(timeout);
            return Ok(());
        }
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

        let ret = unsafe { rcl_wait(ws, timeout_nanos) };

        if ret == RCL_RET_TIMEOUT as i32 {
            return Ok(());
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let mut subs_to_remove = vec![];
        let mut subscriber_error = None;
        if !ws.subscriptions.is_null() {
            let ws_subs =
                unsafe { std::slice::from_raw_parts(ws.subscriptions, self.subscribers.len()) };
            for (s, ws_s) in self.subscribers.iter_mut().zip(ws_subs) {
                if ws_s != &std::ptr::null() {
                    match s.handle_incoming() {
                        Ok(true) => {
                            subs_to_remove.push(s.handle() as *const rcl_subscription_t);
                        }
                        Ok(false) => {}
                        Err(err) => {
                            subscriber_error.get_or_insert(err);
                        }
                    }
                }
            }
        }
        let node_handle = self.node_handle.as_mut();
        self.subscribers.retain_mut(|subscriber| {
            if subs_to_remove.contains(&(subscriber.handle() as *const rcl_subscription_t)) {
                subscriber.destroy(node_handle);
                false
            } else {
                true
            }
        });
        if let Some(err) = subscriber_error {
            return Err(err);
        }

        let mut timers_to_remove = vec![];
        let mut timer_error = None;
        if !ws.timers.is_null() {
            let ws_timers = unsafe { std::slice::from_raw_parts(ws.timers, self.timers.len()) };
            for (s, ws_s) in self.timers.iter_mut().zip(ws_timers) {
                if ws_s != &std::ptr::null() {
                    // TODO: move this to impl Timer
                    match s.handle_incoming() {
                        Ok(true) => timers_to_remove.push(s.get_handle()),
                        Ok(false) => {}
                        Err(err) => {
                            timer_error.get_or_insert(err);
                        }
                    }
                }
            }
        }
        // drop timers scheduled for deletion
        self.timers
            .retain(|t| !timers_to_remove.contains(&t.get_handle()));
        if let Some(err) = timer_error {
            return Err(err);
        }

        if !ws.clients.is_null() {
            let ws_clients = unsafe { std::slice::from_raw_parts(ws.clients, self.clients.len()) };
            for (s, ws_s) in self.clients.iter_mut().zip(ws_clients) {
                if ws_s != &std::ptr::null() {
                    let mut s = s.lock();
                    s.handle_response()?;
                }
            }
        }

        let mut services_to_remove = vec![];
        let mut service_error = None;
        if !ws.services.is_null() {
            let ws_services =
                unsafe { std::slice::from_raw_parts(ws.services, self.services.len()) };
            for (s, ws_s) in self.services.iter_mut().zip(ws_services) {
                if ws_s != &std::ptr::null() {
                    let mut service = s.lock();
                    match service.handle_request(s.clone()) {
                        Ok(true) => {
                            services_to_remove.push(service.handle() as *const rcl_service_t);
                        }
                        Ok(false) => {}
                        Err(err) => {
                            service_error.get_or_insert(err);
                        }
                    }
                }
            }
        }
        let node_handle = self.node_handle.as_mut();
        self.services.retain(|service| {
            let mut handle = service.lock();
            if services_to_remove.contains(&(handle.handle() as *const rcl_service_t)) {
                handle.destroy(node_handle);
                false
            } else {
                true
            }
        });
        if let Some(err) = service_error {
            return Err(err);
        }

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
                ac.handle_feedback_msg()?;
            }

            if is_status_ready {
                ac.handle_status_msg()?;
            }

            if is_goal_response_ready {
                ac.handle_goal_response()?;
            }

            if is_cancel_response_ready {
                ac.handle_cancel_response()?;
            }

            if is_result_response_ready {
                ac.handle_result_response()?;
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
                server.handle_goal_request(s.clone())?;
            }

            if is_cancel_request_ready {
                server.handle_cancel_request()?;
            }

            if is_result_request_ready {
                server.handle_result_request()?;
            }

            if is_goal_expired {
                server.handle_goal_expired()?;
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

    /// Returns fully qualified names of nodes visible in the ROS graph.
    pub fn get_node_names(&self) -> Result<Vec<String>> {
        self.get_node_names_and_namespaces().map(|nodes| {
            nodes
                .into_iter()
                .map(|(name, namespace)| qualify_node_name(&name, &namespace))
                .collect()
        })
    }

    /// Returns node names and namespaces visible in the ROS graph.
    pub fn get_node_names_and_namespaces(&self) -> Result<Vec<(String, String)>> {
        let mut names = StringArray::new();
        let mut namespaces = StringArray::new();
        let ret = unsafe {
            rcl_get_node_names(
                self.node_handle.as_ref(),
                rcutils_get_default_allocator(),
                &mut names.0,
                &mut namespaces.0,
            )
        };
        check_rcl_ret(ret)?;
        zip_graph_strings(names.to_vec()?, namespaces.to_vec()?)
    }

    /// Returns node names, namespaces, and security enclaves visible in the ROS graph.
    pub fn get_node_names_with_enclaves(&self) -> Result<Vec<(String, String, String)>> {
        let mut names = StringArray::new();
        let mut namespaces = StringArray::new();
        let mut enclaves = StringArray::new();
        let ret = unsafe {
            rcl_get_node_names_with_enclaves(
                self.node_handle.as_ref(),
                rcutils_get_default_allocator(),
                &mut names.0,
                &mut namespaces.0,
                &mut enclaves.0,
            )
        };
        check_rcl_ret(ret)?;
        let names = names.to_vec()?;
        let namespaces = namespaces.to_vec()?;
        let enclaves = enclaves.to_vec()?;
        if names.len() != namespaces.len() || names.len() != enclaves.len() {
            return Err(Error::RCL_RET_ERROR);
        }
        Ok(names
            .into_iter()
            .zip(namespaces)
            .zip(enclaves)
            .map(|((name, namespace), enclave)| (name, namespace, enclave))
            .collect())
    }

    /// Returns the number of publishers advertised on `topic_name`.
    pub fn count_publishers(&self, topic_name: &str) -> Result<usize> {
        self.count_graph_entities(topic_name, |node, name, count| unsafe {
            rcl_count_publishers(node, name, count)
        })
    }

    /// Returns the number of subscriptions created for `topic_name`.
    pub fn count_subscribers(&self, topic_name: &str) -> Result<usize> {
        self.count_graph_entities(topic_name, |node, name, count| unsafe {
            rcl_count_subscribers(node, name, count)
        })
    }

    /// Returns the number of clients created for `service_name`.
    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble,
        r2r__ros__distro__iron
    )))]
    pub fn count_clients(&self, service_name: &str) -> Result<usize> {
        self.count_graph_entities(service_name, |node, name, count| unsafe {
            rcl_count_clients(node, name, count)
        })
    }

    /// Returns the number of servers created for `service_name`.
    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble,
        r2r__ros__distro__iron
    )))]
    pub fn count_services(&self, service_name: &str) -> Result<usize> {
        self.count_graph_entities(service_name, |node, name, count| unsafe {
            rcl_count_services(node, name, count)
        })
    }

    fn count_graph_entities(
        &self, name: &str,
        query: impl FnOnce(*const rcl_node_t, *const std::os::raw::c_char, *mut usize) -> rcl_ret_t,
    ) -> Result<usize> {
        let name = CString::new(name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let mut count = 0;
        let ret = query(self.node_handle.as_ref(), name.as_ptr(), &mut count);
        check_rcl_ret(ret)?;
        Ok(count)
    }

    /// Returns publisher topic names and types associated with a remote node.
    pub fn get_publisher_names_and_types_by_node(
        &self, node_name: &str, node_namespace: &str, no_demangle: bool,
    ) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types_by_node(
            node_name,
            node_namespace,
            |node, allocator, name, namespace, names_and_types| unsafe {
                rcl_get_publisher_names_and_types_by_node(
                    node,
                    allocator,
                    no_demangle,
                    name,
                    namespace,
                    names_and_types,
                )
            },
        )
    }

    /// Returns subscription topic names and types associated with a remote node.
    pub fn get_subscription_names_and_types_by_node(
        &self, node_name: &str, node_namespace: &str, no_demangle: bool,
    ) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types_by_node(
            node_name,
            node_namespace,
            |node, allocator, name, namespace, names_and_types| unsafe {
                rcl_get_subscriber_names_and_types_by_node(
                    node,
                    allocator,
                    no_demangle,
                    name,
                    namespace,
                    names_and_types,
                )
            },
        )
    }

    /// Returns service names and types associated with a remote node.
    pub fn get_service_names_and_types_by_node(
        &self, node_name: &str, node_namespace: &str,
    ) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types_by_node(
            node_name,
            node_namespace,
            |node, allocator, name, namespace, names_and_types| unsafe {
                rcl_get_service_names_and_types_by_node(
                    node,
                    allocator,
                    name,
                    namespace,
                    names_and_types,
                )
            },
        )
    }

    /// Returns service client names and types associated with a remote node.
    pub fn get_client_names_and_types_by_node(
        &self, node_name: &str, node_namespace: &str,
    ) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types_by_node(
            node_name,
            node_namespace,
            |node, allocator, name, namespace, names_and_types| unsafe {
                rcl_get_client_names_and_types_by_node(
                    node,
                    allocator,
                    name,
                    namespace,
                    names_and_types,
                )
            },
        )
    }

    fn get_names_and_types_by_node(
        &self, node_name: &str, node_namespace: &str,
        query: impl FnOnce(
            *const rcl_node_t,
            *mut rcutils_allocator_t,
            *const std::os::raw::c_char,
            *const std::os::raw::c_char,
            *mut rcl_names_and_types_t,
        ) -> rcl_ret_t,
    ) -> Result<HashMap<String, Vec<String>>> {
        let node_name = CString::new(node_name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let node_namespace =
            CString::new(node_namespace).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        self.get_names_and_types(|node, allocator, names_and_types| {
            query(node, allocator, node_name.as_ptr(), node_namespace.as_ptr(), names_and_types)
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

struct StringArray(rcutils_string_array_t);

impl StringArray {
    fn new() -> Self {
        Self(unsafe { rcutils_get_zero_initialized_string_array() })
    }

    fn to_vec(&self) -> Result<Vec<String>> {
        if self.0.size == 0 {
            return Ok(Vec::new());
        }
        if self.0.data.is_null() {
            return Err(Error::RCL_RET_ERROR);
        }
        unsafe { std::slice::from_raw_parts(self.0.data, self.0.size) }
            .iter()
            .map(|value| c_string_from_ptr(*value))
            .collect()
    }
}

impl Drop for StringArray {
    fn drop(&mut self) {
        let ret = unsafe { rcutils_string_array_fini(&mut self.0) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize ROS graph string array: {ret}");
        }
    }
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

fn zip_graph_strings(names: Vec<String>, namespaces: Vec<String>) -> Result<Vec<(String, String)>> {
    if names.len() != namespaces.len() {
        return Err(Error::RCL_RET_ERROR);
    }
    Ok(names.into_iter().zip(namespaces).collect())
}

fn qualify_node_name(name: &str, namespace: &str) -> String {
    match namespace {
        "" | "/" => format!("/{name}"),
        namespace if namespace.ends_with('/') => format!("{namespace}{name}"),
        namespace => format!("{namespace}/{name}"),
    }
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

    fn handle_incoming(&mut self) -> Result<bool> {
        let mut is_ready = false;
        let ret = unsafe { rcl_timer_is_ready(self.get_handle(), &mut is_ready) };
        check_rcl_ret(ret)?;
        if !is_ready {
            return Ok(false);
        }

        let mut nanos = 0i64;
        let ret = unsafe { rcl_timer_get_time_since_last_call(self.get_handle(), &mut nanos) };
        check_rcl_ret(ret)?;
        let elapsed = duration_from_nonnegative_nanos(nanos)?;

        let ret = unsafe { rcl_timer_call(self.get_handle_mut()) };
        check_rcl_ret(ret)?;

        if let Err(e) = self.sender.try_send(elapsed) {
            if e.is_disconnected() {
                // client dropped the timer handle, let's drop our timer as well.
                return Ok(true);
            }
            if e.is_full() {
                log::debug!("Warning: timer tick not handled in time - no wakeup will occur");
            }
        }
        Ok(false)
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

pub(crate) async fn future_with_timeout<T>(
    future: impl Future<Output = Result<T>>, mut timer: Timer,
) -> Result<T> {
    futures::pin_mut!(future);
    let timeout = timer.tick();
    futures::pin_mut!(timeout);
    match future::select(future, timeout).await {
        future::Either::Left((result, _)) => result,
        future::Either::Right((timer_result, _)) => {
            timer_result?;
            Err(Error::RCL_RET_TIMEOUT)
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

    fn subtract(&mut self, other: Self) {
        self.subscriptions -= other.subscriptions;
        self.guard_conditions -= other.guard_conditions;
        self.timers -= other.timers;
        self.clients -= other.clients;
        self.services -= other.services;
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

fn duration_from_nonnegative_nanos(nanos: i64) -> Result<Duration> {
    let nanos = u64::try_from(nanos).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
    Ok(Duration::from_nanos(nanos))
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
            qos_profile: QosProfile::try_from(&info.qos_profile)?,
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
        assert_eq!(duration_from_nonnegative_nanos(42).unwrap(), Duration::from_nanos(42));
        assert!(duration_from_nonnegative_nanos(-1).is_err());
    }

    #[test]
    fn empty_node_spin_honors_timeout_without_an_rcl_error() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "empty_spin_test", "").unwrap();
        let timeout = Duration::from_millis(5);
        let started = std::time::Instant::now();

        node.try_spin_once(timeout).unwrap();

        assert!(started.elapsed() >= timeout);
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

        total.subtract(WaitSetCapacities {
            subscriptions: 5,
            guard_conditions: 4,
            timers: 3,
            clients: 2,
            services: 1,
        });
        assert_eq!(
            total,
            WaitSetCapacities {
                subscriptions: 1,
                guard_conditions: 2,
                timers: 3,
                clients: 4,
                services: 5,
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
    fn node_options_apply_programmatic_parameter_overrides() {
        let context = Context::create().unwrap();
        let node = Node::create_with_options(
            context,
            "node_options_test",
            "",
            NodeOptions::default()
                .use_global_arguments(false)
                .parameter_override("answer", ParameterValue::Integer(42))
                .parameter_overrides([
                    ("answer".to_owned(), ParameterValue::Integer(43)),
                    ("enabled".to_owned(), ParameterValue::Bool(true)),
                ]),
        )
        .unwrap();

        assert_eq!(node.get_parameter::<i64>("answer").unwrap(), 43);
        assert!(node.get_parameter::<bool>("enabled").unwrap());
        let native_options = unsafe { rcl_node_get_options(node.node_handle.as_ref()) };
        assert!(!native_options.is_null());
        assert!(!unsafe { (*native_options).use_global_arguments });
    }

    #[test]
    fn node_options_control_global_parameter_arguments() {
        const CHILD_ENV: &str = "R2R_NODE_OPTIONS_CHILD";
        if let Ok(mode) = std::env::var(CHILD_ENV) {
            let context = Context::create().unwrap();
            let options = match mode.as_str() {
                "global" => NodeOptions::default(),
                "isolated" => NodeOptions::default().use_global_arguments(false),
                "override" => {
                    NodeOptions::default().parameter_override("answer", ParameterValue::Integer(43))
                }
                _ => unreachable!(),
            };
            let node =
                Node::create_with_options(context, "node_options_cli_test", "", options).unwrap();
            match mode.as_str() {
                "global" => assert_eq!(node.get_parameter::<i64>("answer").unwrap(), 42),
                "isolated" => assert!(node.get_parameter::<i64>("answer").is_err()),
                "override" => assert_eq!(node.get_parameter::<i64>("answer").unwrap(), 43),
                _ => unreachable!(),
            }
            return;
        }

        let test_binary = std::env::current_exe().unwrap();
        for mode in ["global", "isolated", "override"] {
            let status = std::process::Command::new(&test_binary)
                .arg("nodes::tests::node_options_control_global_parameter_arguments")
                .arg("--exact")
                .arg("--nocapture")
                .arg("--test-threads=1")
                .arg("--")
                .arg("--ros-args")
                .arg("-p")
                .arg("answer:=42")
                .env(CHILD_ENV, mode)
                .status()
                .unwrap();
            assert!(status.success(), "child mode {mode} failed");
        }
    }

    #[test]
    fn node_options_control_rosout_and_its_qos() {
        let context = Context::create().unwrap();
        let disabled = Node::create_with_options(
            context.clone(),
            "rosout_disabled",
            "",
            NodeOptions::default().enable_rosout(false),
        )
        .unwrap();
        let configured = Node::create_with_options(
            context,
            "rosout_configured",
            "",
            NodeOptions::default().rosout_qos(QosProfile::sensor_data()),
        )
        .unwrap();

        let disabled_options = unsafe { rcl_node_get_options(disabled.node_handle.as_ref()) };
        let configured_options = unsafe { rcl_node_get_options(configured.node_handle.as_ref()) };
        assert!(!disabled_options.is_null());
        assert!(!configured_options.is_null());
        assert!(!unsafe { (*disabled_options).enable_rosout });
        assert!(unsafe { (*configured_options).enable_rosout });
        let configured_qos =
            QosProfile::try_from(unsafe { &(*configured_options).rosout_qos }).unwrap();
        assert_eq!(configured_qos.reliability, QosProfile::sensor_data().reliability);
        assert_eq!(configured_qos.depth, 5);
        drop(disabled);
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
    fn counts_topic_and_service_graph_entities() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "graph_count_test", "").unwrap();
        let topic = format!("/graph_count_topic_{}", uuid::Uuid::new_v4().simple());
        let service = format!("/graph_count_service_{}", uuid::Uuid::new_v4().simple());

        assert_eq!(node.count_publishers(&topic).unwrap(), 0);
        assert_eq!(node.count_subscribers(&topic).unwrap(), 0);
        #[cfg(not(any(
            r2r__ros__distro__foxy,
            r2r__ros__distro__galactic,
            r2r__ros__distro__humble,
            r2r__ros__distro__iron
        )))]
        {
            assert_eq!(node.count_clients(&service).unwrap(), 0);
            assert_eq!(node.count_services(&service).unwrap(), 0);
        }

        let _publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
            .unwrap();
        let _subscription = node
            .subscribe::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
            .unwrap();
        let _client = node
            .create_client::<AddTwoInts::Service>(&service, QosProfile::default())
            .unwrap();
        let _requests = node
            .create_service::<AddTwoInts::Service>(&service, QosProfile::default())
            .unwrap();

        assert!(node.count_publishers(&topic).unwrap() >= 1);
        assert!(node.count_subscribers(&topic).unwrap() >= 1);
        #[cfg(not(any(
            r2r__ros__distro__foxy,
            r2r__ros__distro__galactic,
            r2r__ros__distro__humble,
            r2r__ros__distro__iron
        )))]
        {
            assert!(node.count_clients(&service).unwrap() >= 1);
            assert!(node.count_services(&service).unwrap() >= 1);
        }

        assert!(matches!(
            node.count_publishers("bad\0topic"),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
    }

    #[test]
    fn reports_actual_publisher_and_subscription_qos() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "actual_qos_test", "").unwrap();
        let topic = format!("/actual_qos_{}", uuid::Uuid::new_v4().simple());
        let configured = QosProfile::default().best_effort().keep_last(7);
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(&topic, configured.clone())
            .unwrap();
        let subscription = node
            .subscribe::<crate::std_msgs::msg::String>(&topic, configured)
            .unwrap();

        let publisher_qos = publisher.get_actual_qos().unwrap();
        let subscription_qos = subscription.get_actual_qos(&node).unwrap();

        assert_eq!(publisher_qos.history, crate::qos::HistoryPolicy::KeepLast);
        assert_eq!(publisher_qos.depth, 7);
        assert_eq!(publisher_qos.reliability, crate::qos::ReliabilityPolicy::BestEffort);
        assert_eq!(subscription_qos.history, crate::qos::HistoryPolicy::KeepLast);
        assert_eq!(subscription_qos.depth, 7);
        assert_eq!(subscription_qos.reliability, crate::qos::ReliabilityPolicy::BestEffort);
    }

    #[test]
    fn publishers_support_manual_liveliness_assertions() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "publisher_controls_test", "").unwrap();
        let qos = QosProfile::default()
            .liveliness(crate::qos::LivelinessPolicy::ManualByTopic)
            .liveliness_lease_duration(Duration::from_secs(1));

        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/publisher_controls_typed",
                qos.clone(),
            )
            .unwrap();
        publisher.assert_liveliness().unwrap();

        let untyped = node
            .create_publisher_untyped("/publisher_controls_untyped", "std_msgs/msg/String", qos)
            .unwrap();
        untyped.assert_liveliness().unwrap();
    }

    #[test]
    fn publishers_report_resolved_topics_and_loan_support() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "publisher_introspection_test", "/scope").unwrap();

        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "relative_typed",
                QosProfile::default(),
            )
            .unwrap();
        assert_eq!(publisher.topic_name().unwrap(), "/scope/relative_typed");
        let _supports_typed_loans = publisher.can_loan_messages().unwrap();

        let untyped = node
            .create_publisher_untyped(
                "relative_untyped",
                "std_msgs/msg/String",
                QosProfile::default(),
            )
            .unwrap();
        assert_eq!(untyped.topic_name().unwrap(), "/scope/relative_untyped");
        let _supports_untyped_loans = untyped.can_loan_messages().unwrap();
    }

    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    #[test]
    fn publishers_wait_for_reliable_acknowledgements() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "publisher_ack_test", "").unwrap();
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/publisher_ack_typed",
                QosProfile::default(),
            )
            .unwrap();
        publisher.wait_for_all_acked(Duration::ZERO).unwrap();
        assert!(matches!(
            publisher.wait_for_all_acked(Duration::from_nanos(i64::MAX as u64 + 1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));

        let untyped = node
            .create_publisher_untyped(
                "/publisher_ack_untyped",
                "std_msgs/msg/String",
                QosProfile::default(),
            )
            .unwrap();
        untyped.wait_for_all_acked(Duration::ZERO).unwrap();
    }

    #[test]
    fn subscriptions_can_retain_rmw_message_info() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "message_info_test", "").unwrap();
        let topic = format!("/message_info_{}", uuid::Uuid::new_v4().simple());
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default())
            .unwrap();
        let mut typed = node
            .subscribe_with_info::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default())
            .unwrap();
        let mut native = node
            .subscribe_native_with_info::<crate::std_msgs::msg::Int32>(
                &topic,
                QosProfile::default(),
            )
            .unwrap();
        let mut untyped = node
            .subscribe_untyped_with_info(&topic, "std_msgs/msg/Int32", QosProfile::default())
            .unwrap();
        let mut raw = node
            .subscribe_raw_with_info(&topic, "std_msgs/msg/Int32", QosProfile::default())
            .unwrap();

        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if publisher.get_subscription_count().unwrap() >= 4 {
                break;
            }
        }
        publisher
            .publish(&crate::std_msgs::msg::Int32 { data: 42 })
            .unwrap();

        let mut typed_message = None;
        let mut native_message = None;
        let mut untyped_message = None;
        let mut raw_message = None;
        let (typed, native, untyped, raw) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            typed_message = typed_message.or_else(|| typed.next().now_or_never().flatten());
            native_message = native_message.or_else(|| native.next().now_or_never().flatten());
            untyped_message = untyped_message.or_else(|| untyped.next().now_or_never().flatten());
            raw_message = raw_message.or_else(|| raw.next().now_or_never().flatten());
            if typed_message.is_some()
                && native_message.is_some()
                && untyped_message.is_some()
                && raw_message.is_some()
            {
                break (
                    typed_message.take().unwrap(),
                    native_message.take().unwrap(),
                    untyped_message.take().unwrap(),
                    raw_message.take().unwrap(),
                );
            }
        };

        assert_eq!(typed.message.data, 42);
        assert_eq!(native.message.data, 42);
        assert_eq!(untyped.message.unwrap()["data"], 42);
        assert!(!raw.message.is_empty());
        assert!(!typed.info.publisher_gid.is_empty());
        assert_eq!(typed.info.publisher_gid, native.info.publisher_gid);
        assert_eq!(typed.info.publisher_gid, untyped.info.publisher_gid);
        assert_eq!(typed.info.publisher_gid, raw.info.publisher_gid);
        assert!(typed.info.received_timestamp >= 0);
    }

    #[test]
    fn subscription_options_control_rust_side_buffering() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "subscription_options_test", "").unwrap();
        let topic = format!("/subscription_options_{}", uuid::Uuid::new_v4().simple());
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default())
            .unwrap();
        let mut subscription = node
            .subscribe_with_options::<crate::std_msgs::msg::Int32>(
                &topic,
                QosProfile::default(),
                SubscriptionOptions::default().channel_capacity(0),
            )
            .unwrap();

        let wait = publisher
            .wait_for_subscribers_with_timeout(&mut node, Duration::from_secs(1))
            .unwrap();
        futures::pin_mut!(wait);
        loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if wait.as_mut().now_or_never().is_some() {
                break;
            }
        }

        for value in 1..=3 {
            publisher
                .publish(&crate::std_msgs::msg::Int32 { data: value })
                .unwrap();
        }
        for _ in 0..3 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
        }

        assert_eq!(subscription.next().now_or_never().flatten().unwrap().data, 1);
        assert!(subscription.next().now_or_never().is_none());
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        assert_eq!(subscription.next().now_or_never().flatten().unwrap().data, 2);
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        assert_eq!(subscription.next().now_or_never().flatten().unwrap().data, 3);

        let invalid = node.subscribe_with_info_and_options::<crate::std_msgs::msg::Int32>(
            "/invalid_subscription_capacity",
            QosProfile::default(),
            SubscriptionOptions::default().channel_capacity(usize::MAX),
        );
        assert!(matches!(invalid, Err(Error::RCL_RET_INVALID_ARGUMENT)));
    }

    #[test]
    fn subscription_options_can_ignore_local_publications() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "ignore_local_publications_test", "").unwrap();
        let topic = format!("/ignore_local_{}", uuid::Uuid::new_v4().simple());
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default())
            .unwrap();
        let mut control = node
            .subscribe::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default())
            .unwrap();
        let mut ignored = node
            .subscribe_with_info_and_options::<crate::std_msgs::msg::Int32>(
                &topic,
                QosProfile::default(),
                SubscriptionOptions::default().ignore_local_publications(true),
            )
            .unwrap();

        let wait = publisher
            .wait_for_subscribers_with_timeout(&mut node, Duration::from_secs(1))
            .unwrap();
        futures::pin_mut!(wait);
        loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if wait.as_mut().now_or_never().is_some() {
                break;
            }
        }
        publisher
            .publish(&crate::std_msgs::msg::Int32 { data: 42 })
            .unwrap();

        let control = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(message) = control.next().now_or_never().flatten() {
                break message;
            }
        };
        assert_eq!(control.data, 42);
        for _ in 0..5 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
        }
        assert!(ignored.next().now_or_never().is_none());
    }

    #[test]
    fn publisher_and_subscription_support_unique_network_flow_options() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "unique_network_flow_test", "").unwrap();
        let topic = format!("/unique_flow_{}", uuid::Uuid::new_v4().simple());
        let publisher = node
            .create_publisher_with_options::<crate::std_msgs::msg::Int32>(
                &topic,
                QosProfile::default(),
                PublisherOptions::default()
                    .unique_network_flow(UniqueNetworkFlowRequirement::SystemDefault),
            )
            .unwrap();
        let mut subscription = node
            .subscribe_with_info_and_options::<crate::std_msgs::msg::Int32>(
                &topic,
                QosProfile::default(),
                SubscriptionOptions::default()
                    .unique_network_flow(UniqueNetworkFlowRequirement::SystemDefault),
            )
            .unwrap();
        let _untyped = node
            .create_publisher_untyped_with_options(
                "/unique_flow_untyped",
                "std_msgs/msg/Int32",
                QosProfile::default(),
                PublisherOptions::default()
                    .unique_network_flow(UniqueNetworkFlowRequirement::SystemDefault),
            )
            .unwrap();

        let wait = publisher
            .wait_for_subscribers_with_timeout(&mut node, Duration::from_secs(1))
            .unwrap();
        futures::pin_mut!(wait);
        loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if wait.as_mut().now_or_never().is_some() {
                break;
            }
        }
        publisher
            .publish(&crate::std_msgs::msg::Int32 { data: 42 })
            .unwrap();
        let received = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(message) = subscription.next().now_or_never().flatten() {
                break message;
            }
        };
        assert_eq!(received.message.data, 42);
    }

    #[test]
    fn reports_actual_client_qos() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "actual_client_qos_test", "").unwrap();
        let configured = QosProfile::default().keep_last(7);
        let client = node
            .create_client::<AddTwoInts::Service>("/actual_client_qos", configured.clone())
            .unwrap();
        let untyped = node
            .create_client_untyped(
                "/actual_untyped_client_qos",
                "example_interfaces/srv/AddTwoInts",
                configured,
            )
            .unwrap();

        let request_qos = client.get_request_publisher_actual_qos().unwrap();
        let response_qos = client.get_response_subscription_actual_qos().unwrap();
        assert_eq!(request_qos.depth, 7);
        assert_eq!(response_qos.depth, 7);
        assert_eq!(untyped.get_request_publisher_actual_qos().unwrap().depth, 7);
        assert_eq!(
            untyped
                .get_response_subscription_actual_qos()
                .unwrap()
                .depth,
            7
        );
    }

    #[test]
    fn endpoints_report_resolved_topic_and_service_names() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node =
            Node::create(context.clone(), "resolved_endpoint_names_test", "/scope").unwrap();
        let other = Node::create(context, "resolved_endpoint_names_other", "").unwrap();
        let subscription = node
            .subscribe::<crate::std_msgs::msg::String>("relative_topic", QosProfile::default())
            .unwrap();
        assert_eq!(subscription.topic_name(&node).unwrap(), "/scope/relative_topic");
        assert!(matches!(
            subscription.topic_name(&other),
            Err(Error::RCL_RET_SUBSCRIPTION_INVALID)
        ));

        let service = node
            .create_service::<AddTwoInts::Service>("relative_service", QosProfile::default())
            .unwrap();
        let client = node
            .create_client::<AddTwoInts::Service>("relative_service", QosProfile::default())
            .unwrap();
        let untyped = node
            .create_client_untyped(
                "relative_service",
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();
        assert_eq!(service.service_name(&node).unwrap(), "/scope/relative_service");
        assert!(matches!(service.service_name(&other), Err(Error::RCL_RET_SERVICE_INVALID)));
        assert_eq!(client.service_name().unwrap(), "/scope/relative_service");
        assert_eq!(untyped.service_name().unwrap(), "/scope/relative_service");
    }

    #[test]
    fn reports_actual_service_qos() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "actual_service_qos_test", "").unwrap();
        let service = node
            .create_service::<AddTwoInts::Service>(
                "/actual_service_qos",
                QosProfile::default().keep_last(7),
            )
            .unwrap();

        assert_eq!(
            service
                .get_request_subscription_actual_qos(&node)
                .unwrap()
                .depth,
            7
        );
        assert_eq!(
            service
                .get_response_publisher_actual_qos(&node)
                .unwrap()
                .depth,
            7
        );

        drop(service);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.services.is_empty());
    }

    #[test]
    fn service_options_control_request_stream_buffering() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "service_options_test", "").unwrap();
        let service_name = "/service_options";
        let mut requests = node
            .create_service_with_options::<AddTwoInts::Service>(
                service_name,
                QosProfile::default(),
                ServiceOptions::default().request_channel_capacity(0),
            )
            .unwrap();
        let client = node
            .create_client::<AddTwoInts::Service>(service_name, QosProfile::default())
            .unwrap();

        let wait = node
            .is_available_with_timeout(&client, Duration::from_secs(1))
            .unwrap();
        futures::pin_mut!(wait);
        loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if wait.as_mut().now_or_never().is_some() {
                break;
            }
        }

        let _first_response = client.request(&AddTwoInts::Request { a: 1, b: 1 }).unwrap();
        let _second_response = client.request(&AddTwoInts::Request { a: 2, b: 2 }).unwrap();
        for _ in 0..2 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
        }

        assert_eq!(requests.next().now_or_never().flatten().unwrap().message.a, 1);
        assert!(requests.next().now_or_never().is_none());
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        assert_eq!(requests.next().now_or_never().flatten().unwrap().message.a, 2);

        let invalid = node.create_service_with_options::<AddTwoInts::Service>(
            "/invalid_service_capacity",
            QosProfile::default(),
            ServiceOptions::default().request_channel_capacity(usize::MAX),
        );
        assert!(matches!(invalid, Err(Error::RCL_RET_INVALID_ARGUMENT)));
    }

    #[test]
    fn discovers_names_and_types_for_a_specific_node() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut target = Node::create(context.clone(), "graph_target", "/graph_ns").unwrap();
        let observer = Node::create(context, "graph_observer", "/graph_ns").unwrap();
        let _publisher = target
            .create_publisher::<crate::std_msgs::msg::String>(
                "/graph_target_output",
                QosProfile::default(),
            )
            .unwrap();
        let _subscriber = target
            .subscribe::<crate::std_msgs::msg::Int32>("/graph_target_input", QosProfile::default())
            .unwrap();
        let _service = target
            .create_service::<AddTwoInts::Service>("/graph_target_service", QosProfile::default())
            .unwrap();
        let _client = target
            .create_client::<AddTwoInts::Service>("/graph_target_client", QosProfile::default())
            .unwrap();

        let publishers = observer
            .get_publisher_names_and_types_by_node("graph_target", "/graph_ns", false)
            .unwrap();
        let subscriptions = observer
            .get_subscription_names_and_types_by_node("graph_target", "/graph_ns", false)
            .unwrap();
        let services = observer
            .get_service_names_and_types_by_node("graph_target", "/graph_ns")
            .unwrap();
        let clients = observer
            .get_client_names_and_types_by_node("graph_target", "/graph_ns")
            .unwrap();

        assert_eq!(
            publishers.get("/graph_target_output"),
            Some(&vec!["std_msgs/msg/String".to_owned()])
        );
        assert_eq!(
            subscriptions.get("/graph_target_input"),
            Some(&vec!["std_msgs/msg/Int32".to_owned()])
        );
        assert_eq!(
            services.get("/graph_target_service"),
            Some(&vec!["example_interfaces/srv/AddTwoInts".to_owned()])
        );
        assert_eq!(
            clients.get("/graph_target_client"),
            Some(&vec!["example_interfaces/srv/AddTwoInts".to_owned()])
        );
        assert!(!publishers.contains_key("/graph_target_input"));
    }

    #[test]
    fn discovers_node_names_namespaces_and_enclaves() {
        let context = Context::create().unwrap();
        let target = Node::create(context.clone(), "node_graph_target", "/graph_ns").unwrap();
        let observer = Node::create(context, "node_graph_observer", "").unwrap();

        let names = observer.get_node_names().unwrap();
        let names_and_namespaces = observer.get_node_names_and_namespaces().unwrap();
        let with_enclaves = observer.get_node_names_with_enclaves().unwrap();

        assert!(names.contains(&"/graph_ns/node_graph_target".to_owned()));
        assert!(names.contains(&"/node_graph_observer".to_owned()));
        assert!(names_and_namespaces
            .contains(&("node_graph_target".to_owned(), "/graph_ns".to_owned())));
        assert!(with_enclaves.iter().any(|(name, namespace, _)| {
            name == "node_graph_target" && namespace == "/graph_ns"
        }));
        drop(target);
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

    #[test]
    fn parameter_service_publishes_standard_parameter_events() {
        use rcl_interfaces::{msg, srv::SetParameters};

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "parameter_event_test", "").unwrap();
        let mut events = node
            .subscribe_parameter_events(
                Some("/parameter_event_test".into()),
                Some("dynamic_value".into()),
            )
            .unwrap();
        let (handler, _local_events) = node.make_parameter_handler().unwrap();
        let client = node
            .create_client::<SetParameters::Service>(
                "parameter_event_test/set_parameters",
                QosProfile::default(),
            )
            .unwrap();
        let mut pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();

        for _ in 0..100 {
            if node
                .get_subscriptions_info_by_topic("/parameter_events", false)
                .unwrap()
                .iter()
                .any(|endpoint| endpoint.node_name == "parameter_event_test")
            {
                break;
            }
            node.try_spin_once(Duration::from_millis(10)).unwrap();
        }

        let values = [
            ParameterValue::Integer(1),
            ParameterValue::Integer(2),
            ParameterValue::NotSet,
        ];
        for (index, value) in values.into_iter().enumerate() {
            let request = SetParameters::Request {
                parameters: vec![msg::Parameter {
                    name: "dynamic_value".into(),
                    value: value.into_parameter_value_msg(),
                }],
            };
            let mut response = Box::pin(client.request(&request).unwrap());
            let response = loop {
                node.try_spin_once(Duration::from_millis(10)).unwrap();
                pool.run_until_stalled();
                if let Some(response) = response.as_mut().now_or_never() {
                    break response.unwrap();
                }
            };
            assert!(response.results[0].successful);

            let event = loop {
                node.try_spin_once(Duration::from_millis(10)).unwrap();
                if let Some(event) = events.next().now_or_never().flatten() {
                    break event;
                }
            };
            assert_eq!(event.node, "/parameter_event_test");
            assert!(event.stamp.sec > 0 || event.stamp.nanosec > 0);
            assert_eq!(event.name, "dynamic_value");
            assert_eq!(
                event.kind,
                match index {
                    0 => ParameterEventKind::New,
                    1 => ParameterEventKind::Changed,
                    _ => ParameterEventKind::Deleted,
                }
            );
        }
    }

    #[test]
    fn creates_action_endpoints_with_custom_options() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "action_options_test", "").unwrap();
        let _server = node
            .create_action_server_with_options::<Fibonacci::Action>(
                "/configured_action",
                ActionServerOptions::default()
                    .feedback_topic_qos(QosProfile::sensor_data())
                    .result_timeout(Duration::from_millis(250)),
            )
            .unwrap();
        let _client = node
            .create_action_client_with_options::<Fibonacci::Action>(
                "/configured_action",
                ActionClientOptions::default().feedback_topic_qos(QosProfile::sensor_data()),
            )
            .unwrap();
        let _untyped = node
            .create_action_client_untyped_with_options(
                "/configured_action",
                "example_interfaces/action/Fibonacci",
                ActionClientOptions::default().feedback_topic_qos(QosProfile::sensor_data()),
            )
            .unwrap();

        let feedback_topic = "/configured_action/_action/feedback";
        let publishers = node
            .get_publishers_info_by_topic(feedback_topic, false)
            .unwrap();
        let subscriptions = node
            .get_subscriptions_info_by_topic(feedback_topic, false)
            .unwrap();
        assert!(publishers.iter().any(|endpoint| {
            endpoint.qos_profile.reliability == QosProfile::sensor_data().reliability
        }));
        assert!(subscriptions.iter().all(|endpoint| {
            endpoint.qos_profile.reliability == QosProfile::sensor_data().reliability
        }));
    }

    #[test]
    fn action_options_control_stream_buffering() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "action_stream_options_test", "").unwrap();

        let invalid_client = node.create_action_client_with_options::<Fibonacci::Action>(
            "/invalid_action_client_capacity",
            ActionClientOptions::default().feedback_channel_capacity(usize::MAX),
        );
        assert!(matches!(invalid_client, Err(Error::RCL_RET_INVALID_ARGUMENT)));
        let invalid_server = node.create_action_server_with_options::<Fibonacci::Action>(
            "/invalid_action_server_capacity",
            ActionServerOptions::default().goal_request_channel_capacity(usize::MAX),
        );
        assert!(matches!(invalid_server, Err(Error::RCL_RET_INVALID_ARGUMENT)));
        let invalid_cancel = node.create_action_server_with_options::<Fibonacci::Action>(
            "/invalid_action_cancel_capacity",
            ActionServerOptions::default().cancel_request_channel_capacity(usize::MAX),
        );
        assert!(matches!(invalid_cancel, Err(Error::RCL_RET_INVALID_ARGUMENT)));
        let invalid_untyped = node.create_action_client_untyped_with_options(
            "/invalid_untyped_action_client_capacity",
            "example_interfaces/action/Fibonacci",
            ActionClientOptions::default().feedback_channel_capacity(usize::MAX),
        );
        assert!(matches!(invalid_untyped, Err(Error::RCL_RET_INVALID_ARGUMENT)));

        let action_name = "/buffered_action";
        let mut requests = node
            .create_action_server_with_options::<Fibonacci::Action>(
                action_name,
                ActionServerOptions::default()
                    .goal_request_channel_capacity(0)
                    .cancel_request_channel_capacity(0),
            )
            .unwrap();
        let client = node
            .create_action_client_with_options::<Fibonacci::Action>(
                action_name,
                ActionClientOptions::default().feedback_channel_capacity(0),
            )
            .unwrap();

        let first = client
            .send_goal_request(Fibonacci::Goal { order: 1 })
            .unwrap();
        let second = client
            .send_goal_request(Fibonacci::Goal { order: 2 })
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if requests.next().now_or_never().flatten().is_some() {
                break;
            }
        }
        assert!(requests.next().now_or_never().is_none());
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        assert!(requests.next().now_or_never().flatten().is_some());

        // Cancel the unanswered goal futures so client-side pending work is cleaned up.
        drop(first);
        drop(second);

        let flow_name = "/action_stream_flow";
        let mut flow_requests = node
            .create_action_server_with_options::<Fibonacci::Action>(
                flow_name,
                ActionServerOptions::default().cancel_request_channel_capacity(0),
            )
            .unwrap();
        let flow_client = node
            .create_action_client_with_options::<Fibonacci::Action>(
                flow_name,
                ActionClientOptions::default().feedback_channel_capacity(0),
            )
            .unwrap();
        let response = flow_client
            .send_goal_request(Fibonacci::Goal { order: 3 })
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let mut cancel_stream = None;
        let (client_goal, mut feedback) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_none() {
                if let Some(request) = flow_requests.next().now_or_never().flatten() {
                    let (goal, cancels) = request.accept().unwrap();
                    server_goal = Some(goal);
                    cancel_stream = Some(cancels);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                let (goal, _result, feedback) = response.unwrap();
                break (goal, feedback);
            }
        };
        let goal = server_goal.as_ref().unwrap();
        for value in 1..=3 {
            goal.publish_feedback(Fibonacci::Feedback {
                sequence: vec![value],
            })
            .unwrap();
        }
        for _ in 0..3 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
        }
        assert_eq!(feedback.next().now_or_never().flatten().unwrap().sequence, vec![1]);
        assert!(feedback.next().now_or_never().is_none());
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        assert_eq!(feedback.next().now_or_never().flatten().unwrap().sequence, vec![2]);
        assert!(feedback.next().now_or_never().is_none());
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        assert_eq!(feedback.next().now_or_never().flatten().unwrap().sequence, vec![3]);
        assert!(feedback.next().now_or_never().is_none());

        let first_cancel = client_goal.cancel().unwrap();
        for _ in 0..3 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
        }
        let cancel_stream = cancel_stream.as_mut().unwrap();
        let first_request = cancel_stream.next().now_or_never().flatten().unwrap();
        first_request.reject();
        assert!(cancel_stream.next().now_or_never().is_none());
        drop(first_cancel);

        let untyped_name = "/untyped_action_stream_flow";
        let mut untyped_requests = node
            .create_action_server::<Fibonacci::Action>(untyped_name)
            .unwrap();
        let untyped_client = node
            .create_action_client_untyped_with_options(
                untyped_name,
                "example_interfaces/action/Fibonacci",
                ActionClientOptions::default().feedback_channel_capacity(0),
            )
            .unwrap();
        let response = untyped_client
            .send_goal_request(serde_json::json!({ "order": 4 }))
            .unwrap();
        let mut response = Box::pin(response);
        let mut untyped_server_goal = None;
        let mut untyped_feedback = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if untyped_server_goal.is_none() {
                if let Some(request) = untyped_requests.next().now_or_never().flatten() {
                    let (goal, _cancels) = request.accept().unwrap();
                    untyped_server_goal = Some(goal);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                let (_goal, _result, feedback) = response.unwrap();
                break feedback;
            }
        };
        let goal = untyped_server_goal.as_ref().unwrap();
        for value in 1..=3 {
            goal.publish_feedback(Fibonacci::Feedback {
                sequence: vec![value],
            })
            .unwrap();
        }
        for _ in 0..3 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
        }
        for value in 1..=3 {
            let item = untyped_feedback
                .next()
                .now_or_never()
                .flatten()
                .unwrap()
                .unwrap();
            assert_eq!(item, serde_json::json!({ "sequence": [value] }));
            assert!(untyped_feedback.next().now_or_never().is_none());
            node.try_spin_once(Duration::from_millis(10)).unwrap();
        }
    }

    #[test]
    fn availability_waits_support_runtime_agnostic_timeouts() {
        use crate::example_interfaces::srv::AddTwoInts;

        fn drive<T>(node: &mut Node, future: impl Future<Output = Result<T>>) -> Result<T> {
            futures::pin_mut!(future);
            loop {
                node.try_spin_once(Duration::from_millis(10))?;
                if let Some(result) = future.as_mut().now_or_never() {
                    return result;
                }
            }
        }

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "availability_timeout_test", "").unwrap();
        let missing = node
            .create_client::<AddTwoInts::Service>("/missing_timeout_service", QosProfile::default())
            .unwrap();
        let timeout = node
            .is_available_with_timeout(&missing, Duration::from_millis(1))
            .unwrap();
        assert!(matches!(drive(&mut node, timeout), Err(Error::RCL_RET_TIMEOUT)));
        node.try_spin_once(Duration::ZERO).unwrap();

        let _service = node
            .create_service::<AddTwoInts::Service>(
                "/available_timeout_service",
                QosProfile::default(),
            )
            .unwrap();
        let available = node
            .create_client::<AddTwoInts::Service>(
                "/available_timeout_service",
                QosProfile::default(),
            )
            .unwrap();
        let wait = node
            .is_available_with_timeout(&available, Duration::from_secs(1))
            .unwrap();
        drive(&mut node, wait).unwrap();

        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/missing_subscription_timeout",
                QosProfile::default(),
            )
            .unwrap();
        let wait = publisher
            .wait_for_subscribers_with_timeout(&mut node, Duration::from_millis(1))
            .unwrap();
        assert!(matches!(drive(&mut node, wait), Err(Error::RCL_RET_TIMEOUT)));
        node.try_spin_once(Duration::ZERO).unwrap();

        let matched_publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/local_subscription_match",
                QosProfile::default(),
            )
            .unwrap();
        let _matched_subscriber = node
            .subscribe::<crate::std_msgs::msg::String>(
                "/local_subscription_match",
                QosProfile::default(),
            )
            .unwrap();
        let wait = matched_publisher
            .wait_for_subscribers_with_timeout(&mut node, Duration::from_secs(1))
            .unwrap();
        drive(&mut node, wait).unwrap();
        assert!(matched_publisher.get_subscription_count().unwrap() >= 1);
    }

    #[test]
    fn clients_can_check_availability_without_registering_waiters() {
        use crate::example_interfaces::{action::Fibonacci, srv::AddTwoInts};

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "immediate_availability_test", "").unwrap();

        let service_name = "/immediate_availability_service";
        let client = node
            .create_client::<AddTwoInts::Service>(service_name, QosProfile::default())
            .unwrap();
        let untyped_client = node
            .create_client_untyped(
                service_name,
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();
        assert!(!client.is_available(&node).unwrap());
        assert!(!untyped_client.is_available(&node).unwrap());
        let _service = node
            .create_service::<AddTwoInts::Service>(service_name, QosProfile::default())
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if client.is_available(&node).unwrap() && untyped_client.is_available(&node).unwrap() {
                break;
            }
        }
        assert!(client.is_available(&node).unwrap());
        assert!(untyped_client.is_available(&node).unwrap());

        let action_name = "/immediate_availability_action";
        let action_client = node
            .create_action_client::<Fibonacci::Action>(action_name)
            .unwrap();
        let untyped_action_client = node
            .create_action_client_untyped(action_name, "example_interfaces/action/Fibonacci")
            .unwrap();
        assert!(!action_client.is_available(&node).unwrap());
        assert!(!untyped_action_client.is_available(&node).unwrap());
        let _action_server = node
            .create_action_server::<Fibonacci::Action>(action_name)
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if action_client.is_available(&node).unwrap()
                && untyped_action_client.is_available(&node).unwrap()
            {
                break;
            }
        }
        assert!(action_client.is_available(&node).unwrap());
        assert!(untyped_action_client.is_available(&node).unwrap());
    }

    #[test]
    fn service_requests_support_runtime_agnostic_timeouts() {
        use crate::example_interfaces::srv::AddTwoInts;

        fn drive<T>(node: &mut Node, future: impl Future<Output = Result<T>>) -> Result<T> {
            futures::pin_mut!(future);
            loop {
                node.try_spin_once(Duration::from_millis(10))?;
                if let Some(result) = future.as_mut().now_or_never() {
                    return result;
                }
            }
        }

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "service_request_timeout_test", "").unwrap();
        let missing = node
            .create_client::<AddTwoInts::Service>(
                "/missing_request_timeout_service",
                QosProfile::default(),
            )
            .unwrap();
        let timeout = missing
            .request_with_timeout(
                &mut node,
                &AddTwoInts::Request { a: 1, b: 2 },
                Duration::from_millis(1),
            )
            .unwrap();
        assert!(matches!(drive(&mut node, timeout), Err(Error::RCL_RET_TIMEOUT)));
        drop(missing);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.clients.is_empty());

        let service_name = "/successful_request_timeout_service";
        let mut requests = node
            .create_service::<AddTwoInts::Service>(service_name, QosProfile::default())
            .unwrap();
        let client = node
            .create_client::<AddTwoInts::Service>(service_name, QosProfile::default())
            .unwrap();
        let response = client
            .request_with_timeout(
                &mut node,
                &AddTwoInts::Request { a: 20, b: 22 },
                Duration::from_secs(1),
            )
            .unwrap();
        let mut response = Box::pin(response);
        let result = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(request) = requests.next().now_or_never().flatten() {
                request.respond(AddTwoInts::Response { sum: 42 }).unwrap();
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert_eq!(result.sum, 42);

        let untyped = node
            .create_client_untyped(
                "/missing_untyped_request_timeout_service",
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();
        let timeout = untyped
            .request_with_timeout(
                &mut node,
                serde_json::json!({ "a": 1, "b": 2 }),
                Duration::from_millis(1),
            )
            .unwrap();
        assert!(matches!(drive(&mut node, timeout), Err(Error::RCL_RET_TIMEOUT)));
    }

    #[test]
    fn action_requests_support_runtime_agnostic_timeouts() {
        use crate::example_interfaces::action::Fibonacci;

        fn drive<T>(node: &mut Node, future: impl Future<Output = Result<T>>) -> Result<T> {
            futures::pin_mut!(future);
            loop {
                node.try_spin_once(Duration::from_millis(10))?;
                if let Some(result) = future.as_mut().now_or_never() {
                    return result;
                }
            }
        }

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "action_request_timeout_test", "").unwrap();
        let missing = node
            .create_action_client::<Fibonacci::Action>("/missing_action_request_timeout")
            .unwrap();
        let timeout = missing
            .send_goal_request_with_timeout(
                &mut node,
                Fibonacci::Goal { order: 1 },
                Duration::from_millis(1),
            )
            .unwrap();
        assert!(matches!(drive(&mut node, timeout), Err(Error::RCL_RET_TIMEOUT)));
        drop(missing);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());

        let missing_untyped = node
            .create_action_client_untyped(
                "/missing_untyped_action_request_timeout",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        let timeout = missing_untyped
            .send_goal_request_with_timeout(
                &mut node,
                serde_json::json!({ "order": 1 }),
                Duration::from_millis(1),
            )
            .unwrap();
        assert!(matches!(drive(&mut node, timeout), Err(Error::RCL_RET_TIMEOUT)));

        let action_name = "/cancel_request_timeout";
        let mut requests = node
            .create_action_server::<Fibonacci::Action>(action_name)
            .unwrap();
        let client = node
            .create_action_client::<Fibonacci::Action>(action_name)
            .unwrap();
        let goal_response = client
            .send_goal_request_with_timeout(
                &mut node,
                Fibonacci::Goal { order: 2 },
                Duration::from_secs(1),
            )
            .unwrap();
        let mut goal_response = Box::pin(goal_response);
        let mut cancel_requests = None;
        let mut server_goal = None;
        let (client_goal, result_future) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if cancel_requests.is_none() {
                if let Some(request) = requests.next().now_or_never().flatten() {
                    let (goal, requests) = request.accept().unwrap();
                    server_goal = Some(goal);
                    cancel_requests = Some(requests);
                }
            }
            if let Some(response) = goal_response.as_mut().now_or_never() {
                let (goal, result, _feedback) = response.unwrap();
                break (goal, result);
            }
        };

        let cancel = client_goal
            .cancel_with_timeout(&mut node, Duration::from_millis(1))
            .unwrap();
        assert!(matches!(drive(&mut node, cancel), Err(Error::RCL_RET_TIMEOUT)));

        let result = node
            .with_timeout(result_future, Duration::from_millis(1))
            .unwrap();
        assert!(matches!(drive(&mut node, result), Err(Error::RCL_RET_TIMEOUT)));
        drop(server_goal);
        drop(cancel_requests);
    }

    #[test]
    fn subscription_match_queries_waits_and_cleanup_work() {
        fn drive<T>(node: &mut Node, future: impl Future<Output = Result<T>>) -> Result<T> {
            futures::pin_mut!(future);
            loop {
                node.try_spin_once(Duration::from_millis(10))?;
                if let Some(result) = future.as_mut().now_or_never() {
                    return result;
                }
            }
        }

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "subscription_match_test", "").unwrap();
        let missing_topic = format!("/missing_publisher_{}", uuid::Uuid::new_v4().simple());
        let subscription = node
            .subscribe::<crate::std_msgs::msg::String>(&missing_topic, QosProfile::default())
            .unwrap();

        assert_eq!(subscription.get_publisher_count(&node).unwrap(), 0);
        let timeout = subscription
            .wait_for_publishers_with_timeout(&mut node, Duration::from_millis(1))
            .unwrap();
        assert!(matches!(drive(&mut node, timeout), Err(Error::RCL_RET_TIMEOUT)));
        node.try_spin_once(Duration::ZERO).unwrap();

        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(&missing_topic, QosProfile::default())
            .unwrap();
        let wait = subscription
            .wait_for_publishers_with_timeout(&mut node, Duration::from_secs(1))
            .unwrap();
        drive(&mut node, wait).unwrap();
        assert!(subscription.get_publisher_count(&node).unwrap() >= 1);

        node.destroy_publisher(publisher);
        let subscriptions_before_drop = node.subscribers.len();
        drop(subscription);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.subscribers.len(), subscriptions_before_drop - 1);
    }

    #[test]
    fn dropped_service_stream_is_removed_without_a_request() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "service_drop_cleanup_test", "").unwrap();
        // Keep an unrelated waitable alive so this test only exercises service cleanup. Empty
        // wait-set behavior is a separate Node concern.
        let _timer = node.create_wall_timer(Duration::from_secs(1)).unwrap();
        let requests = node
            .create_service::<AddTwoInts::Service>(
                "/idle_service_drop_cleanup",
                QosProfile::default(),
            )
            .unwrap();
        let services_before_drop = node.services.len();

        drop(requests);
        node.try_spin_once(Duration::ZERO).unwrap();

        assert_eq!(node.services.len(), services_before_drop - 1);
    }

    #[test]
    fn dropped_clients_are_removed_after_pending_work_is_gone() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "client_drop_cleanup_test", "").unwrap();
        let client = node
            .create_client::<AddTwoInts::Service>(
                "/missing_client_drop_cleanup",
                QosProfile::default(),
            )
            .unwrap();
        let availability = Node::is_available(&client).unwrap();

        drop(client);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.clients.len(), 1);

        drop(availability);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.clients.is_empty());

        let untyped = node
            .create_client_untyped(
                "/missing_untyped_client_drop_cleanup",
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();
        drop(untyped);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.clients.is_empty());

        let service_name = "/pending_request_client_drop_cleanup";
        let mut requests = node
            .create_service::<AddTwoInts::Service>(service_name, QosProfile::default())
            .unwrap();
        let client = node
            .create_client::<AddTwoInts::Service>(service_name, QosProfile::default())
            .unwrap();
        let response = client
            .request(&AddTwoInts::Request { a: 20, b: 22 })
            .unwrap();
        let mut response = Box::pin(response);
        drop(client);

        let request = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            assert_eq!(node.clients.len(), 1);
            if let Some(request) = requests.next().now_or_never().flatten() {
                break request;
            }
        };
        request.respond(AddTwoInts::Response { sum: 42 }).unwrap();

        let response = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert_eq!(response.sum, 42);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.clients.is_empty());
    }

    #[test]
    fn dropped_publishers_are_removed_after_pending_waits_finish() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "publisher_drop_cleanup_test", "").unwrap();
        let topic = "/publisher_drop_cleanup";
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(topic, QosProfile::default())
            .unwrap();
        let wait = publisher.wait_for_subscribers().unwrap();

        drop(publisher);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.pubs.len(), 1);

        drop(wait);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.pubs.is_empty());

        let subscription = node
            .subscribe::<crate::std_msgs::msg::String>(topic, QosProfile::default())
            .unwrap();
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(topic, QosProfile::default())
            .unwrap();
        let wait = publisher.wait_for_subscribers().unwrap();
        let mut wait = Box::pin(wait);
        drop(publisher);

        loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(result) = wait.as_mut().now_or_never() {
                result.unwrap();
                break;
            }
        }
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.pubs.is_empty());
        drop(subscription);

        let untyped = node
            .create_publisher_untyped(
                "/untyped_publisher_drop_cleanup",
                "std_msgs/msg/String",
                QosProfile::default(),
            )
            .unwrap();
        drop(untyped);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.pubs.is_empty());
    }

    #[test]
    fn publisher_can_only_be_destroyed_by_its_owning_node() {
        let context = Context::create().unwrap();
        let mut owner = Node::create(context.clone(), "publisher_owner_test", "").unwrap();
        let mut other = Node::create(context, "publisher_non_owner_test", "").unwrap();

        let publisher = owner
            .create_publisher::<crate::std_msgs::msg::String>(
                "/owned_typed_publisher",
                QosProfile::default(),
            )
            .unwrap();
        other.destroy_publisher(publisher.clone());
        publisher
            .publish(&crate::std_msgs::msg::String {
                data: "still valid".into(),
            })
            .unwrap();
        assert_eq!(owner.pubs.len(), 1);
        owner.destroy_publisher(publisher);
        assert!(owner.pubs.is_empty());

        let publisher = owner
            .create_publisher_untyped(
                "/owned_untyped_publisher",
                "std_msgs/msg/String",
                QosProfile::default(),
            )
            .unwrap();
        other.destroy_publisher_untyped(publisher.clone());
        publisher
            .publish(serde_json::json!({ "data": "still valid" }))
            .unwrap();
        assert_eq!(owner.pubs.len(), 1);
        owner.destroy_publisher_untyped(publisher);
        assert!(owner.pubs.is_empty());
    }

    #[test]
    fn dropped_timer_is_removed_before_its_deadline() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "timer_drop_cleanup_test", "").unwrap();
        let timer = node.create_wall_timer(Duration::from_secs(60)).unwrap();
        assert_eq!(node.timers.len(), 1);

        drop(timer);
        node.try_spin_once(Duration::ZERO).unwrap();

        assert!(node.timers.is_empty());
    }

    #[test]
    fn dropped_action_clients_are_removed_after_pending_waits_finish() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "action_client_drop_cleanup_test", "").unwrap();
        let client = node
            .create_action_client::<Fibonacci::Action>("/missing_action_drop_cleanup")
            .unwrap();
        let availability = Node::is_available(&client).unwrap();

        drop(client);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.action_clients.len(), 1);

        drop(availability);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());
        assert_eq!(node.action_wait_set_capacities, WaitSetCapacities::default());

        let client = node
            .create_action_client::<Fibonacci::Action>("/pending_goal_drop_cleanup")
            .unwrap();
        let goal = client
            .send_goal_request(Fibonacci::Goal { order: 5 })
            .unwrap();
        drop(client);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.action_clients.len(), 1);

        drop(goal);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());
        assert_eq!(node.action_wait_set_capacities, WaitSetCapacities::default());

        let untyped = node
            .create_action_client_untyped(
                "/missing_untyped_action_drop_cleanup",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        drop(untyped);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());
        assert_eq!(node.action_wait_set_capacities, WaitSetCapacities::default());
    }

    #[test]
    fn dropped_idle_action_server_is_removed_before_waiting() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "action_server_drop_cleanup_test", "").unwrap();
        let requests = node
            .create_action_server::<Fibonacci::Action>("/idle_action_server_drop_cleanup")
            .unwrap();
        assert_eq!(node.action_servers.len(), 1);

        drop(requests);
        node.try_spin_once(Duration::ZERO).unwrap();

        assert!(node.action_servers.is_empty());
        assert_eq!(node.action_wait_set_capacities, WaitSetCapacities::default());
    }
}
