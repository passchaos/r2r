use futures::{
    channel::{mpsc, oneshot},
    future::{self, join_all, FutureExt, TryFutureExt},
    stream::{Stream, StreamExt},
};
use indexmap::IndexMap;
use parking_lot::{Mutex, MutexGuard};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    ffi::{CStr, CString},
    future::Future,
    marker::PhantomPinned,
    pin::Pin,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
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
    availability::AvailabilityWaiters,
    clients::*,
    clocks::*,
    context::*,
    error::*,
    msg_types::{generated_msgs::rcl_interfaces, *},
    parameter_clients::ParameterClient,
    parameters::*,
    publishers::*,
    qos::QosProfile,
    services::*,
    subscribers::*,
};

const SPIN_UNTIL_FUTURE_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Options controlling native timer startup and Rust-side tick buffering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerOptions {
    /// Whether the timer starts running immediately after creation.
    pub autostart: bool,
    /// Number of pending ticks retained for the returned [`Timer`].
    pub channel_capacity: usize,
}

/// Timing metadata captured atomically when a timer is called.
///
/// The absolute timestamps use the clock domain reported by [`Self::clock_type`].
#[cfg(not(any(
    r2r__ros__distro__foxy,
    r2r__ros__distro__galactic,
    r2r__ros__distro__humble,
    r2r__ros__distro__iron
)))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerCallInfo {
    /// Time elapsed since the timer's previous call.
    pub elapsed: Duration,
    /// Scheduled call time in nanoseconds.
    pub expected_call_time_ns: i64,
    /// Actual call time in nanoseconds.
    pub actual_call_time_ns: i64,
    /// Clock domain used by the timer.
    pub clock_type: ClockType,
}

impl Default for TimerOptions {
    fn default() -> Self {
        Self {
            autostart: true,
            channel_capacity: 1,
        }
    }
}

impl TimerOptions {
    /// Selects whether the native timer starts immediately.
    pub const fn autostart(mut self, autostart: bool) -> Self {
        self.autostart = autostart;
        self
    }

    /// Sets the Rust-side tick stream capacity.
    pub const fn channel_capacity(mut self, channel_capacity: usize) -> Self {
        self.channel_capacity = channel_capacity;
        self
    }

    fn validate(self) -> Result<Self> {
        if self.channel_capacity >= usize::MAX >> 2 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        Ok(self)
    }
}

thread_local! {
    static ACTIVE_PARAMETER_TRANSACTIONS: RefCell<HashSet<usize>> = RefCell::new(HashSet::new());
}

struct RclParams(*mut rcl_params_t);

impl Drop for RclParams {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { rcl_yaml_node_struct_fini(self.0) };
        }
    }
}

fn parameter_overrides_from_arguments(arguments: &rcl_arguments_t) -> Result<RclParams> {
    let mut params = RclParams(std::ptr::null_mut());
    let ret = unsafe { rcl_arguments_get_param_overrides(arguments, &mut params.0) };
    if ret == RCL_RET_OK as i32 {
        Ok(params)
    } else {
        Err(Error::from_rcl_error(ret))
    }
}

fn parameter_overrides(context: &Mutex<ContextHandle>) -> Result<RclParams> {
    let context = context.lock();
    parameter_overrides_from_arguments(&context.global_arguments)
}

fn parameter_rule_matches_node(rule: &str, fully_qualified_name: &str) -> bool {
    let rule = rule.strip_prefix('/').unwrap_or(rule);
    let name = fully_qualified_name
        .strip_prefix('/')
        .unwrap_or(fully_qualified_name);
    let rule = rule.split('/').collect::<Vec<_>>();
    let name = name.split('/').collect::<Vec<_>>();

    fn matches(rule: &[&str], name: &[&str]) -> bool {
        match rule.split_first() {
            None => name.is_empty(),
            Some((&"**", rest)) => {
                matches(rest, name)
                    || name
                        .split_first()
                        .is_some_and(|(_, tail)| matches(rule, tail))
            }
            Some((&"*", rest)) => name
                .split_first()
                .is_some_and(|(_, tail)| matches(rest, tail)),
            Some((segment, rest)) => name
                .split_first()
                .is_some_and(|(candidate, tail)| segment == candidate && matches(rest, tail)),
        }
    }

    matches(&rule, &name)
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
        let arguments = config
            .arguments
            .iter()
            .map(|argument| {
                CString::new(argument.as_str()).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)
            })
            .collect::<Result<Vec<_>>>()?;
        let argument_ptrs = arguments
            .iter()
            .map(|argument| argument.as_ptr())
            .collect::<Vec<_>>();
        let argc =
            i32::try_from(argument_ptrs.len()).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let argv = if argument_ptrs.is_empty() {
            std::ptr::null()
        } else {
            argument_ptrs.as_ptr()
        };
        let ret =
            unsafe { rcl_parse_arguments(argc, argv, options.allocator, &mut options.arguments) };
        if ret != RCL_RET_OK as i32 {
            unsafe { rcl_node_options_fini(&mut options) };
            return Err(Error::from_rcl_error(ret));
        }
        if unsafe { rcl_arguments_get_count_unparsed_ros(&options.arguments) } != 0 {
            unsafe { rcl_node_options_fini(&mut options) };
            return Err(Error::RCL_RET_INVALID_ROS_ARGS);
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
        if !parameter_value_changed(previous, value) {
            return;
        }
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

#[derive(Clone)]
struct ParameterEventSink {
    sender: mpsc::UnboundedSender<LocalParameterEvent>,
    publisher: Option<Publisher<rcl_interfaces::msg::ParameterEvent>>,
    clock: Arc<Mutex<Clock>>,
    node_name: String,
}

type LocalParameterEvent = (String, ParameterValue);

fn deliver_local_parameter_events(
    sender: &mpsc::UnboundedSender<LocalParameterEvent>, events: Vec<LocalParameterEvent>,
) {
    for event in events {
        if let Err(err) = sender.unbounded_send(event) {
            log::debug!("Warning: could not send parameter event ({err}).");
            break;
        }
    }
}

impl ParameterEventSink {
    fn notify(&self, changed: Vec<(String, ParameterValue)>, ros_events: ParameterEventChanges) {
        deliver_local_parameter_events(&self.sender, changed);
        if let Some(publisher) = &self.publisher {
            ros_events.publish(publisher, &self.clock, &self.node_name);
        }
    }
}

struct AppliedParameterChanges {
    updates: Vec<(String, ParameterValue)>,
    changed: Vec<(String, ParameterValue)>,
    ros_events: ParameterEventChanges,
    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    sim_time_update: Option<bool>,
}

struct PreparedParameterDeclaration {
    name: String,
    value: ParameterValue,
    descriptor: ParameterDescriptor,
    uses_existing_override: bool,
}

#[derive(Clone)]
struct ParameterProviderState {
    provider: Arc<Mutex<dyn RosParams + Send>>,
    parameter_names: Arc<OnceLock<HashSet<String>>>,
}

impl ParameterProviderState {
    fn new(provider: Arc<Mutex<dyn RosParams + Send>>) -> Self {
        Self {
            provider,
            parameter_names: Arc::new(OnceLock::new()),
        }
    }

    fn parameter_names(&self) -> &HashSet<String> {
        self.parameter_names
            .get()
            .expect("parameter provider names are initialized before use")
    }
}

struct ParameterTransactionGuard<'a> {
    guard: Option<MutexGuard<'a, ()>>,
    identity: usize,
}

impl ParameterTransactionGuard<'_> {
    fn unlock(&mut self) {
        self.guard.take();
    }
}

impl Drop for ParameterTransactionGuard<'_> {
    fn drop(&mut self) {
        ACTIVE_PARAMETER_TRANSACTIONS.with(|active| {
            debug_assert!(active.borrow_mut().remove(&self.identity));
        });
    }
}

fn lock_parameter_transaction(
    transaction: &Arc<Mutex<()>>,
) -> Result<ParameterTransactionGuard<'_>> {
    let identity = Arc::as_ptr(transaction) as usize;
    let inserted =
        ACTIVE_PARAMETER_TRANSACTIONS.with(|active| active.borrow_mut().insert(identity));
    if !inserted {
        return Err(Error::RecursiveParameterMutation);
    }
    Ok(ParameterTransactionGuard {
        guard: Some(transaction.lock()),
        identity,
    })
}

type ParameterValidationCallback =
    dyn Fn(&[(String, ParameterValue)]) -> Result<()> + Send + Sync + 'static;
type ParameterPostSetCallback = dyn Fn(&[(String, ParameterValue)]) + Send + Sync + 'static;
type ParameterPreSetCallback = dyn Fn(&mut Vec<(String, ParameterValue)>) + Send + Sync + 'static;

struct ParameterCallbackRegistry<C: ?Sized> {
    callbacks: BTreeMap<u64, Arc<C>>,
    next_id: u64,
}

impl<C: ?Sized> Default for ParameterCallbackRegistry<C> {
    fn default() -> Self {
        Self {
            callbacks: BTreeMap::new(),
            next_id: 0,
        }
    }
}

impl<C: ?Sized> ParameterCallbackRegistry<C> {
    fn insert(&mut self, callback: Arc<C>) -> u64 {
        let id = loop {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1);
            if !self.callbacks.contains_key(&id) {
                break id;
            }
        };
        self.callbacks.insert(id, callback);
        id
    }

    fn remove(&mut self, id: u64) {
        self.callbacks.remove(&id);
    }

    fn reverse_snapshot(&self) -> Vec<Arc<C>> {
        self.callbacks.values().rev().cloned().collect()
    }
}

fn transform_parameter_updates(
    updates: &mut Vec<(String, ParameterValue)>,
    callbacks: &Arc<Mutex<ParameterCallbackRegistry<ParameterPreSetCallback>>>,
) -> Result<()> {
    let callbacks = callbacks.lock().reverse_snapshot();
    let transformed = !callbacks.is_empty();
    for callback in callbacks {
        callback(updates);
    }
    if transformed && updates.is_empty() {
        return Err(Error::ParameterUpdateRejected {
            reason: "pre-set callbacks produced an empty parameter list".into(),
        });
    }
    Ok(())
}

fn normalize_parameter_updates(
    updates: Vec<(String, ParameterValue)>,
) -> Vec<(String, ParameterValue)> {
    let mut normalized = IndexMap::with_capacity(updates.len());
    for (name, value) in updates {
        normalized.insert(name, value);
    }
    normalized.into_iter().collect()
}

fn validate_parameter_callbacks(
    updates: &[(String, ParameterValue)],
    callbacks: &Arc<Mutex<ParameterCallbackRegistry<ParameterValidationCallback>>>,
) -> Result<()> {
    let callbacks = callbacks.lock().reverse_snapshot();
    for callback in callbacks {
        callback(updates)?;
    }
    Ok(())
}

fn notify_parameter_post_set_callbacks(
    updates: &[(String, ParameterValue)],
    callbacks: &Arc<Mutex<ParameterCallbackRegistry<ParameterPostSetCallback>>>,
) {
    let callbacks = callbacks.lock().reverse_snapshot();
    for callback in callbacks {
        callback(updates);
    }
}

/// Keeps a parameter validation callback registered until dropped.
pub struct ParameterCallbackHandle {
    id: u64,
    callbacks: std::sync::Weak<Mutex<ParameterCallbackRegistry<ParameterValidationCallback>>>,
}

impl Drop for ParameterCallbackHandle {
    fn drop(&mut self) {
        if let Some(callbacks) = self.callbacks.upgrade() {
            callbacks.lock().remove(self.id);
        }
    }
}

/// Keeps a post-set parameter callback registered until dropped.
pub struct ParameterPostSetCallbackHandle {
    id: u64,
    callbacks: std::sync::Weak<Mutex<ParameterCallbackRegistry<ParameterPostSetCallback>>>,
}

impl Drop for ParameterPostSetCallbackHandle {
    fn drop(&mut self) {
        if let Some(callbacks) = self.callbacks.upgrade() {
            callbacks.lock().remove(self.id);
        }
    }
}

/// Keeps a pre-set parameter callback registered until dropped.
pub struct ParameterPreSetCallbackHandle {
    id: u64,
    callbacks: std::sync::Weak<Mutex<ParameterCallbackRegistry<ParameterPreSetCallback>>>,
}

impl Drop for ParameterPreSetCallbackHandle {
    fn drop(&mut self) {
        if let Some(callbacks) = self.callbacks.upgrade() {
            callbacks.lock().remove(self.id);
        }
    }
}

fn parameter_value_changed(previous: Option<&ParameterValue>, value: &ParameterValue) -> bool {
    previous != Some(value) && !(previous.is_none() && matches!(value, ParameterValue::NotSet))
}

fn parameter_name_matches(name: &str, prefixes: &[String], depth: u64) -> bool {
    let recursive = depth == rcl_interfaces::srv::ListParameters::Request::DEPTH_RECURSIVE as u64;
    let within_depth = |suffix: &str| (suffix.matches('.').count() as u64) < depth;

    (prefixes.is_empty() && (recursive || within_depth(name)))
        || prefixes.iter().any(|prefix| {
            name == prefix
                || name
                    .strip_prefix(prefix)
                    .and_then(|suffix| suffix.strip_prefix('.'))
                    .is_some_and(|suffix| recursive || within_depth(suffix))
        })
}

fn read_parameter_values(
    names: &[String], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    provider: Option<&ParameterProviderState>,
) -> Vec<ParameterValue> {
    read_parameter_values_impl(names, params, provider, true, false)
        .expect("permissive parameter reads map provider failures to unset values")
}

fn read_parameter_values_with_policy(
    names: &[String], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    provider: Option<&ParameterProviderState>, allow_undeclared_parameters: bool,
) -> Result<Vec<ParameterValue>> {
    read_parameter_values_impl(names, params, provider, allow_undeclared_parameters, true)
}

fn read_parameter_values_impl(
    names: &[String], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    provider: Option<&ParameterProviderState>, allow_undeclared_parameters: bool,
    propagate_provider_errors: bool,
) -> Result<Vec<ParameterValue>> {
    let mut values = {
        let params = params.lock();
        if !allow_undeclared_parameters {
            if let Some(name) = names.iter().find(|name| !params.contains_key(*name)) {
                return Err(Error::ParameterNotDeclared {
                    name: name.to_owned(),
                });
            }
        }
        names
            .iter()
            .map(|name| {
                params
                    .get(name)
                    .map(|parameter| parameter.value.clone())
                    .unwrap_or(ParameterValue::NotSet)
            })
            .collect::<Vec<_>>()
    };

    let Some(provider) = provider else {
        return Ok(values);
    };
    let parameter_names = provider.parameter_names();
    let mut implementation = provider.provider.lock();
    for (index, name) in names.iter().enumerate() {
        if !parameter_names.contains(name) {
            continue;
        }
        match implementation.get_parameter(name) {
            Ok(value) => values[index] = value,
            Err(err) if propagate_provider_errors => return Err(err),
            Err(err) => {
                log::warn!("could not read parameter {name} from its provider: {err}");
                values[index] = ParameterValue::NotSet;
            }
        }
    }
    Ok(values)
}

fn list_parameter_values(
    params: &IndexMap<String, Parameter>, prefixes: &[String], depth: u64,
) -> rcl_interfaces::msg::ListParametersResult {
    let mut result = rcl_interfaces::msg::ListParametersResult::default();
    let mut seen_prefixes = HashSet::new();
    for name in params
        .keys()
        .filter(|name| parameter_name_matches(name, prefixes, depth))
    {
        result.names.push(name.clone());
        if let Some(last_separator) = name.rfind('.') {
            let prefix = &name[..last_separator];
            if seen_prefixes.insert(prefix) {
                result.prefixes.push(prefix.to_owned());
            }
        }
    }
    result
}

fn read_list_parameter_values(
    params: &Arc<Mutex<IndexMap<String, Parameter>>>, prefixes: &[String], depth: u64,
) -> rcl_interfaces::msg::ListParametersResult {
    let params = params.lock();
    list_parameter_values(&params, prefixes, depth)
}

fn describe_parameter_values(
    names: &[String], params: &IndexMap<String, Parameter>,
    registered: &IndexMap<String, ParameterDescriptor>,
) -> Vec<ParameterDescriptor> {
    names
        .iter()
        .map(|name| {
            registered.get(name).cloned().unwrap_or_else(|| {
                params.get(name).map_or_else(
                    || ParameterDescriptor {
                        name: name.clone(),
                        ..Default::default()
                    },
                    |parameter| Node::default_parameter_descriptor(name, parameter, true),
                )
            })
        })
        .collect()
}

fn read_parameter_descriptors(
    names: &[String], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    registered: &Arc<Mutex<IndexMap<String, ParameterDescriptor>>>,
) -> Vec<ParameterDescriptor> {
    read_parameter_descriptors_with_policy(names, params, registered, true)
        .expect("permissive parameter descriptions cannot reject an undeclared name")
}

fn read_parameter_descriptors_with_policy(
    names: &[String], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    registered: &Arc<Mutex<IndexMap<String, ParameterDescriptor>>>,
    allow_undeclared_parameters: bool,
) -> Result<Vec<ParameterDescriptor>> {
    let params = params.lock();
    if !allow_undeclared_parameters {
        if let Some(name) = names.iter().find(|name| !params.contains_key(*name)) {
            return Err(Error::ParameterNotDeclared {
                name: name.to_owned(),
            });
        }
    }
    let registered = registered.lock();
    Ok(describe_parameter_values(names, &params, &registered))
}

fn parameter_types(names: &[String], params: &IndexMap<String, Parameter>) -> Vec<ParameterType> {
    names
        .iter()
        .map(|name| {
            params
                .get(name)
                .map_or(ParameterType::NotSet, |parameter| parameter.value.parameter_type())
        })
        .collect()
}

fn read_parameter_types(
    names: &[String], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
) -> Vec<ParameterType> {
    read_parameter_types_with_policy(names, params, true)
        .expect("permissive parameter type reads cannot reject an undeclared name")
}

fn read_parameter_types_with_policy(
    names: &[String], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    allow_undeclared_parameters: bool,
) -> Result<Vec<ParameterType>> {
    let params = params.lock();
    if !allow_undeclared_parameters {
        if let Some(name) = names.iter().find(|name| !params.contains_key(*name)) {
            return Err(Error::ParameterNotDeclared {
                name: name.to_owned(),
            });
        }
    }
    Ok(parameter_types(names, &params))
}

fn apply_parameter_updates(
    mut updates: Vec<(String, ParameterValue)>, params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    descriptors: &Arc<Mutex<IndexMap<String, ParameterDescriptor>>>,
    provider: Option<&ParameterProviderState>,
    pre_callbacks: &Arc<Mutex<ParameterCallbackRegistry<ParameterPreSetCallback>>>,
    callbacks: &Arc<Mutex<ParameterCallbackRegistry<ParameterValidationCallback>>>,
    allow_undeclared_parameters: bool,
) -> Result<AppliedParameterChanges> {
    transform_parameter_updates(&mut updates, pre_callbacks)?;
    updates = normalize_parameter_updates(updates);
    {
        let params = params.lock();
        let descriptors = descriptors.lock();
        for (name, value) in &updates {
            if name.is_empty() {
                return Err(Error::InvalidParameterName { name: name.clone() });
            }
            if !allow_undeclared_parameters && !params.contains_key(name) {
                return Err(Error::ParameterNotDeclared { name: name.clone() });
            }
            if name == "use_sim_time" && !matches!(value, ParameterValue::Bool(_)) {
                return Err(Error::InvalidParameterType {
                    name: name.clone(),
                    ty: "boolean",
                });
            }
            if let Some(descriptor) = descriptors.get(name) {
                validate_parameter_update(name, value, descriptor, true)?;
            }
        }
    }

    validate_parameter_callbacks(&updates, callbacks)?;

    if let Some(provider) = provider {
        let derived_updates = updates
            .iter()
            .filter(|(name, _)| provider.parameter_names().contains(name))
            .cloned()
            .collect::<Vec<_>>();
        if !derived_updates.is_empty() {
            provider
                .provider
                .lock()
                .set_parameters_atomically(&derived_updates)?;
        }
    }

    let mut changed = Vec::new();
    let mut ros_events = ParameterEventChanges::default();
    let mut params = params.lock();
    let mut descriptors = descriptors.lock();
    for (name, value) in &updates {
        let previous = params.get(name).map(|parameter| parameter.value.clone());
        if parameter_value_changed(previous.as_ref(), value) {
            changed.push((name.clone(), value.clone()));
            ros_events.record(name, previous.as_ref(), value);
            Node::store_dynamic_parameter(&mut params, name, value);
            Node::record_successful_parameter_descriptor(&mut descriptors, name, value);
        }
    }
    drop(descriptors);
    drop(params);

    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    let sim_time_update =
        updates
            .iter()
            .rev()
            .find_map(|(name, value)| match (name.as_str(), value) {
                ("use_sim_time", ParameterValue::Bool(enabled)) => Some(*enabled),
                _ => None,
            });
    Ok(AppliedParameterChanges {
        updates,
        changed,
        ros_events,
        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        sim_time_update,
    })
}

/// A ROS Node.
///
/// This struct owns all subscribes, publishers, etc.  To get events
/// from the ROS network into your ros application, `spin_once` should
/// be called continously.
pub struct Node {
    /// ROS parameters.
    pub params: Arc<Mutex<IndexMap<String, Parameter>>>,
    parameter_overrides: IndexMap<String, ParameterValue>,
    automatically_declare_parameters_from_overrides: bool,
    allow_undeclared_parameters: bool,
    parameter_descriptors: Arc<Mutex<IndexMap<String, ParameterDescriptor>>>,
    parameter_provider: Option<ParameterProviderState>,
    parameter_transaction: Arc<Mutex<()>>,
    parameter_pre_set_callbacks: Arc<Mutex<ParameterCallbackRegistry<ParameterPreSetCallback>>>,
    parameter_validation_callbacks:
        Arc<Mutex<ParameterCallbackRegistry<ParameterValidationCallback>>>,
    parameter_post_set_callbacks: Arc<Mutex<ParameterCallbackRegistry<ParameterPostSetCallback>>>,
    parameter_event_sink: Option<ParameterEventSink>,
    parameter_handler_started: bool,
    pub(crate) node_handle: Box<rcl_node_t>,
    // the node owns the subscribers
    pub(crate) subscribers: Vec<Box<dyn Subscriber_>>,
    subscription_indexes: HashMap<uuid::Uuid, usize>,
    canceled_subscriptions: Arc<Mutex<VecDeque<uuid::Uuid>>>,
    subscriptions_with_active_waits: HashSet<uuid::Uuid>,
    // services,
    services: Vec<Arc<Mutex<dyn Service_>>>,
    service_indexes: HashMap<uuid::Uuid, usize>,
    canceled_services: Arc<Mutex<VecDeque<uuid::Uuid>>>,
    // service clients
    clients: Vec<Arc<Mutex<dyn Client_>>>,
    client_identities: HashMap<usize, usize>,
    dropped_clients: Arc<Mutex<VecDeque<usize>>>,
    pending_client_removals: HashSet<usize>,
    active_availability_clients: Arc<Mutex<VecDeque<usize>>>,
    clients_with_active_availability: HashSet<usize>,
    // action clients
    action_clients: Vec<Arc<Mutex<dyn ActionClient_>>>,
    action_client_identities: HashMap<usize, usize>,
    dropped_action_clients: Arc<Mutex<VecDeque<usize>>>,
    pending_action_client_removals: HashSet<usize>,
    active_availability_action_clients: Arc<Mutex<VecDeque<usize>>>,
    action_clients_with_active_availability: HashSet<usize>,
    active_maintenance_action_clients: Arc<Mutex<VecDeque<usize>>>,
    action_clients_with_pending_maintenance: HashSet<usize>,
    // action servers
    action_servers: Vec<Arc<Mutex<dyn ActionServer_>>>,
    action_server_indexes: HashMap<uuid::Uuid, usize>,
    dropped_action_servers: Arc<Mutex<VecDeque<uuid::Uuid>>>,
    pending_action_server_removals: HashSet<uuid::Uuid>,
    action_servers_with_pending_cancels: HashSet<uuid::Uuid>,
    action_wait_set_capacities: WaitSetCapacities,
    // timers,
    timers: Vec<Timer_>,
    timer_indexes: HashMap<uuid::Uuid, usize>,
    canceled_timers: Arc<Mutex<VecDeque<uuid::Uuid>>>,
    // and the publishers, whom we allow to be shared.. hmm.
    pub(crate) pubs: Vec<Arc<Publisher_>>,
    publisher_indexes: HashMap<uuid::Uuid, usize>,
    dropped_publishers: Arc<Mutex<VecDeque<uuid::Uuid>>>,
    pending_publisher_removals: HashSet<uuid::Uuid>,
    active_wait_publishers: Arc<Mutex<VecDeque<uuid::Uuid>>>,
    publishers_with_active_waits: HashSet<uuid::Uuid>,
    // RosTime clock used by all timers created by create_timer()
    ros_clock: Arc<Mutex<Clock>>,
    // time source that provides simulated time
    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    time_source: TimeSource,
    graph_change_waiters: AvailabilityWaiters,
    wait_set: WaitSet,
    // Keep the ROS context alive until all other node resources have been dropped.
    context: Context,
}

/// Options applied while creating a [`Node`].
#[derive(Debug, Clone)]
pub struct NodeOptions {
    /// ROS-specific arguments applied only to this node.
    pub arguments: Vec<String>,
    /// Whether the node consumes global ROS arguments from its [`Context`].
    pub use_global_arguments: bool,
    /// Programmatic parameter overrides. These take precedence over global CLI/YAML overrides.
    pub parameter_overrides: IndexMap<String, ParameterValue>,
    /// Whether parameter overrides are immediately exposed as declared parameters.
    pub automatically_declare_parameters_from_overrides: bool,
    /// Whether setting an undeclared parameter implicitly declares it.
    pub allow_undeclared_parameters: bool,
    /// Whether the node publishes logs on `/rosout`.
    pub enable_rosout: bool,
    /// Optional override for the `/rosout` publisher QoS.
    pub rosout_qos: Option<QosProfile>,
}

impl Default for NodeOptions {
    fn default() -> Self {
        Self {
            arguments: Vec::new(),
            use_global_arguments: true,
            parameter_overrides: IndexMap::new(),
            automatically_declare_parameters_from_overrides: true,
            allow_undeclared_parameters: true,
            enable_rosout: true,
            rosout_qos: None,
        }
    }
}

impl NodeOptions {
    /// Replaces the ROS arguments applied only to this node.
    ///
    /// Include an executable name as the first item when matching normal process argv semantics.
    pub fn arguments<I, S>(mut self, arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.arguments = arguments.into_iter().map(Into::into).collect();
        self
    }

    pub fn use_global_arguments(mut self, enabled: bool) -> Self {
        self.use_global_arguments = enabled;
        self
    }

    pub fn parameter_override(
        mut self, name: impl Into<String>, value: impl Into<ParameterValue>,
    ) -> Self {
        self.parameter_overrides.insert(name.into(), value.into());
        self
    }

    pub fn parameter_overrides<I, N, V>(mut self, overrides: I) -> Self
    where
        I: IntoIterator<Item = (N, V)>,
        N: Into<String>,
        V: Into<ParameterValue>,
    {
        self.parameter_overrides.extend(
            overrides
                .into_iter()
                .map(|(name, value)| (name.into(), value.into())),
        );
        self
    }

    /// Controls whether overrides are exposed as parameters during node creation.
    ///
    /// Disabling this keeps overrides available to later explicit declarations without making
    /// them visible through parameter lookup or listing beforehand. The default is `true` for
    /// compatibility with r2r's historical behavior.
    pub fn automatically_declare_parameters_from_overrides(mut self, enabled: bool) -> Self {
        self.automatically_declare_parameters_from_overrides = enabled;
        self
    }

    /// Controls whether setting an undeclared parameter implicitly declares it.
    ///
    /// Disable this to catch misspelled or missing declarations before callbacks, providers, or
    /// parameter state are changed. The default is `true` for compatibility with r2r's historical
    /// behavior.
    pub fn allow_undeclared_parameters(mut self, enabled: bool) -> Self {
        self.allow_undeclared_parameters = enabled;
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

/// Options for the standard parameter services and event publisher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterHandlerOptions {
    /// Whether to create the six standard parameter service endpoints.
    pub start_parameter_services: bool,
    /// Whether to publish parameter changes on `/parameter_events`.
    pub start_parameter_event_publisher: bool,
    /// QoS used by each standard parameter service.
    pub service_qos: QosProfile,
    /// QoS used by the `/parameter_events` publisher.
    pub parameter_event_qos: QosProfile,
}

impl Default for ParameterHandlerOptions {
    fn default() -> Self {
        Self {
            start_parameter_services: true,
            start_parameter_event_publisher: true,
            service_qos: QosProfile::default(),
            parameter_event_qos: QosProfile::parameter_events(),
        }
    }
}

impl ParameterHandlerOptions {
    pub const fn start_parameter_services(mut self, enabled: bool) -> Self {
        self.start_parameter_services = enabled;
        self
    }

    pub const fn start_parameter_event_publisher(mut self, enabled: bool) -> Self {
        self.start_parameter_event_publisher = enabled;
        self
    }

    pub fn service_qos(mut self, qos: QosProfile) -> Self {
        self.service_qos = qos;
        self
    }

    pub fn parameter_event_qos(mut self, qos: QosProfile) -> Self {
        self.parameter_event_qos = qos;
        self
    }
}

unsafe impl Send for Node {}

impl Node {
    pub(crate) fn add_subscriber(&mut self, subscriber: Box<dyn Subscriber_>) {
        let index = self.subscribers.len();
        if let Some(state) = subscriber.match_state() {
            debug_assert!(self.subscription_indexes.insert(state.id, index).is_none());
        }
        self.subscribers.push(subscriber);
    }

    fn rebuild_subscription_indexes(
        subscribers: &[Box<dyn Subscriber_>], indexes: &mut HashMap<uuid::Uuid, usize>,
    ) {
        indexes.clear();
        indexes.reserve(subscribers.len());
        for (index, subscriber) in subscribers.iter().enumerate() {
            if let Some(state) = subscriber.match_state() {
                debug_assert!(indexes.insert(state.id, index).is_none());
            }
        }
    }

    fn remove_subscription(
        subscribers: &mut Vec<Box<dyn Subscriber_>>,
        subscription_indexes: &mut HashMap<uuid::Uuid, usize>, id: uuid::Uuid,
        node_handle: &mut rcl_node_t,
    ) -> bool {
        let Some(index) = subscription_indexes.remove(&id) else {
            return false;
        };
        let mut removed = subscribers.swap_remove(index);
        debug_assert_eq!(removed.match_state().map(|state| state.id), Some(id));
        removed.destroy(node_handle);
        if let Some(moved) = subscribers
            .get(index)
            .and_then(|subscriber| subscriber.match_state())
        {
            subscription_indexes.insert(moved.id, index);
        }
        true
    }

    fn destroy_subscription_by_id(&mut self, id: uuid::Uuid) -> bool {
        let removed = Self::remove_subscription(
            &mut self.subscribers,
            &mut self.subscription_indexes,
            id,
            self.node_handle.as_mut(),
        );
        if removed {
            self.subscriptions_with_active_waits.remove(&id);
            self.canceled_subscriptions
                .lock()
                .retain(|queued| *queued != id);
        }
        removed
    }

    pub(crate) fn subscriber(&self, id: uuid::Uuid) -> Result<&dyn Subscriber_> {
        let index = self
            .subscription_indexes
            .get(&id)
            .copied()
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        self.subscribers
            .get(index)
            .map(|subscriber| &**subscriber)
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)
    }

    pub(crate) fn subscriber_mut(&mut self, id: uuid::Uuid) -> Result<&mut (dyn Subscriber_ + '_)> {
        let index = self
            .subscription_indexes
            .get(&id)
            .copied()
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        match self.subscribers.get_mut(index) {
            Some(subscriber) => Ok(&mut **subscriber),
            None => Err(Error::RCL_RET_SUBSCRIPTION_INVALID),
        }
    }

    fn add_service(&mut self, id: uuid::Uuid, service: Arc<Mutex<dyn Service_>>) {
        let index = self.services.len();
        debug_assert!(self.service_indexes.insert(id, index).is_none());
        self.services.push(service);
    }

    fn remove_service(
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

    fn rollback_parameter_handler_endpoints(
        &mut self, service_count: usize, publisher_count: usize,
        #[cfg(r2r__rosgraph_msgs__msg__Clock)] subscriber_count: usize,
    ) {
        let mut removed_service_ids = HashSet::new();
        while self.services.len() > service_count {
            let service = self.services.pop().expect("service count checked");
            let mut service = service.lock();
            if let Some(id) = service.id() {
                self.service_indexes.remove(&id);
                removed_service_ids.insert(id);
            }
            service.destroy(self.node_handle.as_mut());
        }
        self.canceled_services
            .lock()
            .retain(|id| !removed_service_ids.contains(id));

        let mut removed_publisher_ids = HashSet::new();
        while self.pubs.len() > publisher_count {
            let publisher = self.pubs.pop().expect("publisher count checked");
            let id = Self::publisher_identity(&publisher);
            self.publisher_indexes.remove(&id);
            removed_publisher_ids.insert(id);
            publisher.destroy(self.node_handle.as_mut());
        }
        self.dropped_publishers
            .lock()
            .retain(|id| !removed_publisher_ids.contains(id));

        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        if self.subscribers.len() > subscriber_count {
            while self.subscribers.len() > subscriber_count {
                let mut subscriber = self.subscribers.pop().expect("subscriber count checked");
                // A failed simulated-time activation marks its internal subscriber for removal.
                // Consume that state transition while rolling back the exact endpoint we added.
                let _ = subscriber.should_remove();
                subscriber.destroy(self.node_handle.as_mut());
            }
            Self::rebuild_subscription_indexes(&self.subscribers, &mut self.subscription_indexes);
        }
    }

    fn service(&self, id: uuid::Uuid) -> Result<&Arc<Mutex<dyn Service_>>> {
        let index = self
            .service_indexes
            .get(&id)
            .copied()
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        self.services
            .get(index)
            .ok_or(Error::RCL_RET_SERVICE_INVALID)
    }

    fn add_timer(&mut self, timer: Timer_) {
        let index = self.timers.len();
        debug_assert!(self.timer_indexes.insert(timer.id, index).is_none());
        self.timers.push(timer);
    }

    fn remove_timer(
        timers: &mut Vec<Timer_>, timer_indexes: &mut HashMap<uuid::Uuid, usize>, id: uuid::Uuid,
    ) -> Option<Timer_> {
        let index = timer_indexes.remove(&id)?;
        let removed = timers.swap_remove(index);
        debug_assert_eq!(removed.id, id);
        if let Some(moved) = timers.get(index) {
            timer_indexes.insert(moved.id, index);
        }
        Some(removed)
    }

    fn publisher_identity(publisher: &Arc<Publisher_>) -> uuid::Uuid {
        publisher.id
    }

    fn add_publisher(&mut self, publisher: Arc<Publisher_>) {
        let index = self.pubs.len();
        let identity = Self::publisher_identity(&publisher);
        debug_assert!(self.publisher_indexes.insert(identity, index).is_none());
        self.pubs.push(publisher);
    }

    fn remove_dropped_publisher(&mut self, identity: uuid::Uuid) -> bool {
        let Some(index) = self.publisher_indexes.get(&identity).copied() else {
            return true;
        };
        if self.pubs[index].has_pending_waits() {
            return false;
        }
        let removed = self.pubs.swap_remove(index);
        debug_assert_eq!(Self::publisher_identity(&removed), identity);
        self.publisher_indexes.remove(&identity);
        if let Some(moved) = self.pubs.get(index) {
            self.publisher_indexes
                .insert(Self::publisher_identity(moved), index);
        }
        removed.destroy(self.node_handle.as_mut());
        true
    }

    fn destroy_publisher_by_id(&mut self, identity: uuid::Uuid) -> bool {
        let Some(index) = self.publisher_indexes.remove(&identity) else {
            return false;
        };
        let removed = self.pubs.swap_remove(index);
        debug_assert_eq!(Self::publisher_identity(&removed), identity);
        if let Some(moved) = self.pubs.get(index) {
            self.publisher_indexes
                .insert(Self::publisher_identity(moved), index);
        }
        self.pending_publisher_removals.remove(&identity);
        self.publishers_with_active_waits.remove(&identity);
        self.dropped_publishers
            .lock()
            .retain(|queued| *queued != identity);
        self.active_wait_publishers
            .lock()
            .retain(|queued| *queued != identity);
        removed.destroy(self.node_handle.as_mut());
        true
    }

    fn client_identity<T: ?Sized>(client: &Arc<Mutex<T>>) -> usize {
        Arc::as_ptr(client).cast::<()>() as usize
    }

    fn remove_client(&mut self, identity: usize) -> bool {
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

    fn destroy_client_by_identity(&mut self, identity: usize) -> bool {
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

    fn rollback_clients(&mut self, client_count: usize) {
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

    fn action_client_identity<T: ?Sized>(client: &Arc<Mutex<T>>) -> usize {
        Arc::as_ptr(client).cast::<()>() as usize
    }

    fn remove_action_client(&mut self, identity: usize) -> Result<bool> {
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

    fn destroy_action_client_by_identity(&mut self, identity: usize) -> Result<bool> {
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

    fn add_action_server(
        &mut self, id: uuid::Uuid, server: Arc<Mutex<dyn ActionServer_>>,
        capacities: WaitSetCapacities,
    ) {
        let index = self.action_servers.len();
        debug_assert!(self.action_server_indexes.insert(id, index).is_none());
        self.action_servers.push(server);
        self.action_wait_set_capacities.add(capacities);
    }

    fn remove_action_server(&mut self, id: uuid::Uuid) -> Result<bool> {
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
        self.action_servers_with_pending_cancels.remove(&id);
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
        self.action_servers_with_pending_cancels.remove(&id);
        self.dropped_action_servers
            .lock()
            .retain(|queued| *queued != id);
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

    pub(crate) fn owns_publisher(&self, publisher: &Arc<Publisher_>) -> bool {
        self.publisher_indexes
            .contains_key(&Self::publisher_identity(publisher))
    }

    pub(crate) fn owns_client<T>(&self, client: &Arc<Mutex<T>>) -> bool
    where
        T: Client_ + ?Sized,
    {
        self.client_identities
            .contains_key(&Self::client_identity(client))
    }

    pub(crate) fn owns_action_client<T>(&self, client: &Arc<Mutex<T>>) -> bool
    where
        T: ActionClient_ + ?Sized,
    {
        self.action_client_identities
            .contains_key(&Self::action_client_identity(client))
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

    /// Returns the ROS domain ID used by this node.
    pub fn domain_id(&self) -> Result<usize> {
        let mut domain_id = 0;
        let ret = unsafe { rcl_node_get_domain_id(self.node_handle.as_ref(), &mut domain_id) };
        check_rcl_ret(ret)?;
        Ok(domain_id)
    }

    /// Expands and remaps a topic name using this node's namespace and ROS arguments.
    #[cfg(not(r2r__ros__distro__foxy))]
    pub fn resolve_topic_name(&self, name: &str) -> Result<String> {
        self.resolve_name(name, false, false)
    }

    /// Expands a topic name without applying remapping rules.
    #[cfg(not(r2r__ros__distro__foxy))]
    pub fn resolve_topic_name_without_remapping(&self, name: &str) -> Result<String> {
        self.resolve_name(name, false, true)
    }

    /// Expands and remaps a service name using this node's namespace and ROS arguments.
    #[cfg(not(r2r__ros__distro__foxy))]
    pub fn resolve_service_name(&self, name: &str) -> Result<String> {
        self.resolve_name(name, true, false)
    }

    /// Expands a service name without applying remapping rules.
    #[cfg(not(r2r__ros__distro__foxy))]
    pub fn resolve_service_name_without_remapping(&self, name: &str) -> Result<String> {
        self.resolve_name(name, true, true)
    }

    #[cfg(not(r2r__ros__distro__foxy))]
    fn resolve_name(&self, name: &str, is_service: bool, only_expand: bool) -> Result<String> {
        let name = CString::new(name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let allocator = unsafe { rcutils_get_default_allocator() };
        let mut output = std::ptr::null_mut();
        let ret = unsafe {
            rcl_node_resolve_name(
                self.node_handle.as_ref(),
                name.as_ptr(),
                allocator,
                is_service,
                only_expand,
                &mut output,
            )
        };
        check_rcl_ret(ret)?;
        let output = AllocatedCString { output, allocator };
        if output.output.is_null() {
            return Err(Error::RCL_RET_ERROR);
        }
        c_str_to_owned(unsafe { CStr::from_ptr(output.output) }, "resolved ROS name")
    }

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

    /// Returns the initial CLI, YAML, and programmatic parameter overrides for this node.
    ///
    /// Programmatic overrides replace global overrides with the same name. The returned map is an
    /// immutable creation-time snapshot: declaring, setting, or undeclaring parameters does not
    /// change it.
    pub fn parameter_overrides(&self) -> &IndexMap<String, ParameterValue> {
        &self.parameter_overrides
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
        self.make_parameter_handler_with_options(ParameterHandlerOptions::default())
    }

    /// Creates parameter handlers with explicit service and event-publisher options.
    pub fn make_parameter_handler_with_options(
        &mut self, options: ParameterHandlerOptions,
    ) -> Result<(impl Future<Output = ()> + Send, impl Stream<Item = (String, ParameterValue)>)>
    {
        self.make_parameter_handler_internal(None, options)
    }

    /// Declares a statically typed parameter, honoring any CLI, YAML, or programmatic override.
    pub fn declare_parameter(
        &self, name: impl Into<String>, default_value: impl Into<ParameterValue>,
    ) -> Result<ParameterValue> {
        self.declare_parameter_with_descriptor(name, default_value, ParameterDescriptor::default())
    }

    /// Declares a parameter with a ROS descriptor and returns its effective initial value.
    ///
    /// An override loaded while creating the node takes precedence over `default_value`, but must
    /// satisfy the declared type and constraints. The descriptor's `name` and `type_` fields are
    /// derived from the arguments, matching ROS 2's declaration semantics.
    pub fn declare_parameter_with_descriptor(
        &self, name: impl Into<String>, default_value: impl Into<ParameterValue>,
        descriptor: ParameterDescriptor,
    ) -> Result<ParameterValue> {
        self.declare_parameter_with_options(
            name,
            default_value,
            descriptor,
            ParameterDeclarationOptions::default(),
        )
    }

    /// Declares a parameter with explicit override behavior.
    ///
    /// Set [`ParameterDeclarationOptions::ignore_override`] to use `default_value` even when the
    /// node was created with a matching CLI, YAML, or programmatic override.
    pub fn declare_parameter_with_options(
        &self, name: impl Into<String>, default_value: impl Into<ParameterValue>,
        descriptor: ParameterDescriptor, options: ParameterDeclarationOptions,
    ) -> Result<ParameterValue> {
        self.declare_parameters_impl("", [(name.into(), default_value.into(), descriptor)], options)
            .and_then(|mut values| values.pop().ok_or(Error::RCL_RET_ERROR))
    }

    /// Declares several parameters under `namespace` as one transaction.
    ///
    /// An empty namespace leaves names unchanged; otherwise each key is declared as
    /// `namespace.key`. Overrides are honored for every parameter. The entire batch is validated
    /// before state is changed, so an invalid descriptor, duplicate name, or rejected value leaves
    /// every parameter in the batch undeclared.
    pub fn declare_parameters<I, N, V>(
        &self, namespace: &str, parameters: I,
    ) -> Result<Vec<ParameterValue>>
    where
        I: IntoIterator<Item = (N, V)>,
        N: Into<String>,
        V: Into<ParameterValue>,
    {
        self.declare_parameters_impl(
            namespace,
            parameters
                .into_iter()
                .map(|(name, value)| (name.into(), value.into(), ParameterDescriptor::default())),
            ParameterDeclarationOptions::default(),
        )
    }

    /// Declares several parameters under `namespace` with explicit override behavior.
    pub fn declare_parameters_with_options<I, N, V>(
        &self, namespace: &str, parameters: I, options: ParameterDeclarationOptions,
    ) -> Result<Vec<ParameterValue>>
    where
        I: IntoIterator<Item = (N, V)>,
        N: Into<String>,
        V: Into<ParameterValue>,
    {
        self.declare_parameters_impl(
            namespace,
            parameters
                .into_iter()
                .map(|(name, value)| (name.into(), value.into(), ParameterDescriptor::default())),
            options,
        )
    }

    /// Declares several parameters and descriptors under `namespace` as one transaction.
    ///
    /// Descriptor names and types are derived from the fully qualified parameter name and default
    /// value. See [`Node::declare_parameters`] for namespace, override, callback, and atomicity
    /// behavior.
    pub fn declare_parameters_with_descriptors<I, N, V>(
        &self, namespace: &str, parameters: I,
    ) -> Result<Vec<ParameterValue>>
    where
        I: IntoIterator<Item = (N, V, ParameterDescriptor)>,
        N: Into<String>,
        V: Into<ParameterValue>,
    {
        self.declare_parameters_impl(
            namespace,
            parameters
                .into_iter()
                .map(|(name, value, descriptor)| (name.into(), value.into(), descriptor)),
            ParameterDeclarationOptions::default(),
        )
    }

    /// Declares several parameters and descriptors with explicit override behavior.
    pub fn declare_parameters_with_descriptors_and_options<I, N, V>(
        &self, namespace: &str, parameters: I, options: ParameterDeclarationOptions,
    ) -> Result<Vec<ParameterValue>>
    where
        I: IntoIterator<Item = (N, V, ParameterDescriptor)>,
        N: Into<String>,
        V: Into<ParameterValue>,
    {
        self.declare_parameters_impl(
            namespace,
            parameters
                .into_iter()
                .map(|(name, value, descriptor)| (name.into(), value.into(), descriptor)),
            options,
        )
    }

    fn declare_parameters_impl(
        &self, namespace: &str,
        parameters: impl IntoIterator<Item = (String, ParameterValue, ParameterDescriptor)>,
        options: ParameterDeclarationOptions,
    ) -> Result<Vec<ParameterValue>> {
        let mut transaction = lock_parameter_transaction(&self.parameter_transaction)?;
        let prefix = if namespace.is_empty() {
            String::new()
        } else {
            format!("{namespace}.")
        };
        let defaults = parameters.into_iter().collect::<Vec<_>>();
        let mut names = HashSet::with_capacity(defaults.len());
        let params = self.params.lock();
        let descriptors = self.parameter_descriptors.lock();
        let mut prepared = Vec::with_capacity(defaults.len());
        for (name, default_value, mut descriptor) in defaults {
            let name = format!("{prefix}{name}");
            if name.is_empty() {
                return Err(Error::InvalidParameterName { name });
            }
            if !names.insert(name.clone()) || descriptors.contains_key(&name) {
                return Err(Error::ParameterAlreadyDeclared { name });
            }
            descriptor.name = name.clone();
            descriptor.type_ = default_value.parameter_type().into();
            validate_parameter_descriptor(&name, &descriptor)?;

            let existing_override = (!options.ignore_override)
                .then(|| params.get(&name))
                .flatten();
            let deferred_override = (!options.ignore_override && existing_override.is_none())
                .then(|| self.parameter_overrides.get(&name))
                .flatten();
            let uses_existing_override = existing_override.is_some();
            let value = existing_override
                .map(|parameter| parameter.value.clone())
                .or_else(|| deferred_override.cloned())
                .unwrap_or(default_value);
            validate_parameter_update(&name, &value, &descriptor, false)?;
            if descriptor.description.is_empty() {
                if let Some(parameter) = existing_override {
                    descriptor.description = parameter.description.to_owned();
                }
            }
            if descriptor.dynamic_typing {
                descriptor.type_ = value.parameter_type().into();
            }
            prepared.push(PreparedParameterDeclaration {
                name,
                value,
                descriptor,
                uses_existing_override,
            });
        }
        drop(descriptors);
        drop(params);

        for declaration in &prepared {
            validate_parameter_callbacks(
                &[(declaration.name.clone(), declaration.value.clone())],
                &self.parameter_validation_callbacks,
            )?;
        }

        let mut params = self.params.lock();
        let mut descriptors = self.parameter_descriptors.lock();
        for declaration in &prepared {
            descriptors.insert(declaration.name.clone(), declaration.descriptor.clone());
            if !declaration.uses_existing_override {
                params.insert(declaration.name.clone(), Parameter::new(declaration.value.clone()));
            }
        }
        drop(descriptors);
        drop(params);
        transaction.unlock();

        let declared = prepared
            .iter()
            .map(|declaration| (declaration.name.clone(), declaration.value.clone()))
            .collect::<Vec<_>>();
        if let Some(sink) = &self.parameter_event_sink {
            let mut ros_events = ParameterEventChanges::default();
            for (name, value) in &declared {
                ros_events.record(name, None, value);
            }
            sink.notify(declared.clone(), ros_events);
        }
        for declaration in &prepared {
            notify_parameter_post_set_callbacks(
                &[(declaration.name.clone(), declaration.value.clone())],
                &self.parameter_post_set_callbacks,
            );
        }
        Ok(prepared
            .into_iter()
            .map(|declaration| declaration.value)
            .collect())
    }

    /// Registers the ROS descriptor and constraints for an existing parameter.
    ///
    /// The descriptor's `name` and `type_` fields are derived from `name` and the parameter's
    /// current value. Other fields, including read-only, dynamic typing, and numeric ranges, are
    /// exposed through `DescribeParameters` and enforced by both parameter-setting services.
    pub fn set_parameter_descriptor(
        &self, name: impl Into<String>, mut descriptor: ParameterDescriptor,
    ) -> Result<()> {
        let _transaction = lock_parameter_transaction(&self.parameter_transaction)?;
        let name = name.into();
        let params = self.params.lock();
        let value = params
            .get(&name)
            .map(|parameter| parameter.value.clone())
            .ok_or_else(|| Error::InvalidParameterName { name: name.clone() })?;
        let mut descriptors = self.parameter_descriptors.lock();
        if descriptors
            .get(&name)
            .is_some_and(|descriptor| descriptor.read_only)
        {
            return Err(Error::ParameterConstraintViolation {
                name,
                reason: "a read-only parameter's descriptor cannot be replaced".into(),
            });
        }
        descriptor.name = name.clone();
        descriptor.type_ = value.parameter_type().into();
        validate_parameter_descriptor(&name, &descriptor)?;
        validate_parameter_update(&name, &value, &descriptor, false)?;
        descriptors.insert(name, descriptor);
        Ok(())
    }

    /// Returns the registered descriptor for a parameter.
    pub fn get_parameter_descriptor(&self, name: &str) -> Result<ParameterDescriptor> {
        if let Some(descriptor) = self.parameter_descriptors.lock().get(name) {
            return Ok(descriptor.clone());
        }
        let params = self.params.lock();
        let parameter = params
            .get(name)
            .ok_or_else(|| Error::InvalidParameterName { name: name.into() })?;
        Ok(Self::default_parameter_descriptor(name, parameter, true))
    }

    /// Returns whether a parameter is currently declared on this node.
    pub fn has_parameter(&self, name: &str) -> bool {
        self.params.lock().contains_key(name)
    }

    /// Removes a declared dynamic parameter and its descriptor.
    ///
    /// Read-only and statically typed parameters cannot be removed through this API because they
    /// may be backed by a [`RosParams`] field. Parameter service clients can perform the same
    /// operation by assigning [`ParameterValue::NotSet`] to a dynamic parameter.
    pub fn undeclare_parameter(&self, name: &str) -> Result<ParameterValue> {
        let (removed, ros_events) = {
            let _transaction = lock_parameter_transaction(&self.parameter_transaction)?;
            let mut params = self.params.lock();
            let mut descriptors = self.parameter_descriptors.lock();
            if let Some(descriptor) = descriptors.get(name) {
                if descriptor.read_only || !descriptor.dynamic_typing {
                    return Err(Error::ParameterConstraintViolation {
                        name: name.to_owned(),
                        reason: "an immutable or statically typed parameter cannot be undeclared"
                            .into(),
                    });
                }
            }

            let removed = params
                .shift_remove(name)
                .ok_or_else(|| Error::InvalidParameterName { name: name.into() })?;
            descriptors.shift_remove(name);
            let mut ros_events = ParameterEventChanges::default();
            ros_events.record(name, Some(&removed.value), &ParameterValue::NotSet);
            (removed.value, ros_events)
        };

        if let Some(sink) = &self.parameter_event_sink {
            sink.notify(vec![(name.to_owned(), ParameterValue::NotSet)], ros_events);
        }
        Ok(removed)
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
        self.make_derived_parameter_handler_with_options(
            params_struct,
            ParameterHandlerOptions::default(),
        )
    }

    /// Creates derived parameter handlers with explicit service and event-publisher options.
    pub fn make_derived_parameter_handler_with_options(
        &mut self, params_struct: Arc<Mutex<dyn RosParams + Send>>,
        options: ParameterHandlerOptions,
    ) -> Result<(impl Future<Output = ()> + Send, impl Stream<Item = (String, ParameterValue)>)>
    {
        self.make_parameter_handler_internal(Some(params_struct), options)
    }

    fn make_parameter_handler_internal(
        &mut self, params_struct: Option<Arc<Mutex<dyn RosParams + Send>>>,
        options: ParameterHandlerOptions,
    ) -> Result<(impl Future<Output = ()> + Send, impl Stream<Item = (String, ParameterValue)>)>
    {
        if self.parameter_handler_started {
            return Err(Error::ParameterHandlerAlreadyCreated);
        }
        let parameter_provider = params_struct.map(ParameterProviderState::new);
        let original_params = self
            .params
            .lock()
            .iter()
            .map(|(name, parameter)| {
                (
                    name.clone(),
                    Parameter {
                        value: parameter.value.clone(),
                        description: parameter.description,
                    },
                )
            })
            .collect::<IndexMap<_, _>>();
        let original_parameter_descriptors = self.parameter_descriptors.lock().clone();
        let service_count = self.services.len();
        let publisher_count = self.pubs.len();
        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        let subscriber_count = self.subscribers.len();
        let setup: Result<_> = (|| {
            let mut handlers: Vec<std::pin::Pin<Box<dyn Future<Output = ()> + Send>>> = Vec::new();

            let (set_event_tx, event_rx) = mpsc::unbounded::<(String, ParameterValue)>();

            let node_name = self.name()?;
            let fully_qualified_node_name = self.fully_qualified_name()?;
            let parameter_event_publisher = if options.start_parameter_event_publisher {
                Some(self.create_publisher::<rcl_interfaces::msg::ParameterEvent>(
                    "/parameter_events",
                    options.parameter_event_qos.clone(),
                )?)
            } else {
                None
            };
            let parameter_event_clock = self.ros_clock.clone();
            let parameter_event_sink = ParameterEventSink {
                sender: set_event_tx,
                publisher: parameter_event_publisher,
                clock: parameter_event_clock,
                node_name: fully_qualified_node_name,
            };

            if options.start_parameter_services {
                // rcl_interfaces/srv/SetParameters
                let set_params_request_stream = self
                    .create_service::<rcl_interfaces::srv::SetParameters::Service>(
                        &format!("{}/set_parameters", node_name),
                        options.service_qos.clone(),
                    )?;

                let params = self.params.clone();
                let parameter_descriptors = self.parameter_descriptors.clone();
                let parameter_transaction = self.parameter_transaction.clone();
                let parameter_pre_set_callbacks = self.parameter_pre_set_callbacks.clone();
                let parameter_validation_callbacks = self.parameter_validation_callbacks.clone();
                let parameter_post_set_callbacks = self.parameter_post_set_callbacks.clone();
                let parameter_provider_clone = parameter_provider.clone();
                let allow_undeclared_parameters = self.allow_undeclared_parameters;
                let set_parameter_event_sink = parameter_event_sink.clone();
                #[cfg(r2r__rosgraph_msgs__msg__Clock)]
                let set_params_time_source = self.time_source.clone();
                let set_params_future = set_params_request_stream.for_each(
                    move |req: ServiceRequest<rcl_interfaces::srv::SetParameters::Service>| {
                        let mut result = rcl_interfaces::srv::SetParameters::Response::default();
                        for p in &req.message.parameters {
                            let val =
                                match ParameterValue::try_from_parameter_value_msg(p.value.clone())
                                {
                                    Ok(value) => value,
                                    Err(err) => {
                                        result.results.push(
                                            rcl_interfaces::msg::SetParametersResult {
                                                successful: false,
                                                reason: err.to_string(),
                                            },
                                        );
                                        continue;
                                    }
                                };
                            let update = vec![(p.name.clone(), val)];
                            let mut transaction =
                                match lock_parameter_transaction(&parameter_transaction) {
                                    Ok(transaction) => transaction,
                                    Err(err) => {
                                        result.results.push(
                                            rcl_interfaces::msg::SetParametersResult {
                                                successful: false,
                                                reason: err.to_string(),
                                            },
                                        );
                                        continue;
                                    }
                                };
                            let applied = apply_parameter_updates(
                                update,
                                &params,
                                &parameter_descriptors,
                                parameter_provider_clone.as_ref(),
                                &parameter_pre_set_callbacks,
                                &parameter_validation_callbacks,
                                allow_undeclared_parameters,
                            );
                            transaction.unlock();
                            match applied {
                                Ok(applied) => {
                                    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
                                    if let Some(enabled) = applied.sim_time_update {
                                        set_params_time_source.request_sim_time(enabled);
                                    }
                                    set_parameter_event_sink
                                        .notify(applied.changed, applied.ros_events);
                                    notify_parameter_post_set_callbacks(
                                        &applied.updates,
                                        &parameter_post_set_callbacks,
                                    );
                                    result
                                        .results
                                        .push(rcl_interfaces::msg::SetParametersResult {
                                            successful: true,
                                            reason: String::new(),
                                        });
                                }
                                Err(err) => {
                                    result
                                        .results
                                        .push(rcl_interfaces::msg::SetParametersResult {
                                            successful: false,
                                            reason: err.to_string(),
                                        });
                                }
                            }
                            drop(transaction);
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
                        options.service_qos.clone(),
                    )?;

                let params = self.params.clone();
                let parameter_provider_clone = parameter_provider.clone();
                let get_params_future = get_params_request_stream.for_each(
                    move |req: ServiceRequest<rcl_interfaces::srv::GetParameters::Service>| {
                        let values = read_parameter_values(
                            &req.message.names,
                            &params,
                            parameter_provider_clone.as_ref(),
                        )
                        .into_iter()
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
                    options.service_qos.clone(),
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
                let desc_params_request_stream = self
                    .create_service::<DescribeParameters::Service>(
                        &format!("{node_name}/describe_parameters"),
                        options.service_qos.clone(),
                    )?;

                let params = self.params.clone();
                let parameter_descriptors = self.parameter_descriptors.clone();
                let desc_params_future = desc_params_request_stream.for_each(
                    move |req: ServiceRequest<DescribeParameters::Service>| {
                        Self::handle_desc_parameters(req, &params, &parameter_descriptors)
                    },
                );

                handlers.push(Box::pin(desc_params_future));

                // rcl_interfaces/srv/GetParameterTypes
                use rcl_interfaces::srv::GetParameterTypes;
                let get_param_types_request_stream = self
                    .create_service::<GetParameterTypes::Service>(
                        &format!("{node_name}/get_parameter_types"),
                        options.service_qos.clone(),
                    )?;

                let params = self.params.clone();
                let get_param_types_future = get_param_types_request_stream.for_each(
                    move |req: ServiceRequest<GetParameterTypes::Service>| {
                        let types = read_parameter_types(&req.message.names, &params)
                            .into_iter()
                            .map(Into::into)
                            .collect();
                        if let Err(err) = req.respond(GetParameterTypes::Response { types }) {
                            log::warn!(
                                "could not send reply to get parameter types request: {err}"
                            );
                        }
                        future::ready(())
                    },
                );

                handlers.push(Box::pin(get_param_types_future));

                // rcl_interfaces/srv/SetParametersAtomically
                let set_params_atomically_request_stream =
                    self.create_service::<rcl_interfaces::srv::SetParametersAtomically::Service>(
                        &format!("{}/set_parameters_atomically", node_name),
                        options.service_qos.clone(),
                    )?;

                let params = self.params.clone();
                let parameter_descriptors = self.parameter_descriptors.clone();
                let parameter_transaction = self.parameter_transaction.clone();
                let parameter_pre_set_callbacks = self.parameter_pre_set_callbacks.clone();
                let parameter_validation_callbacks = self.parameter_validation_callbacks.clone();
                let parameter_post_set_callbacks = self.parameter_post_set_callbacks.clone();
                let parameter_provider_clone = parameter_provider.clone();
                let allow_undeclared_parameters = self.allow_undeclared_parameters;
                let atomic_parameter_event_sink = parameter_event_sink.clone();
                #[cfg(r2r__rosgraph_msgs__msg__Clock)]
                let atomic_time_source = self.time_source.clone();
                let set_params_atomically_future = set_params_atomically_request_stream.for_each(
                    move |req: ServiceRequest<
                        rcl_interfaces::srv::SetParametersAtomically::Service,
                    >| {
                        let mut result =
                            rcl_interfaces::srv::SetParametersAtomically::Response::default();
                        let updates = req
                            .message
                            .parameters
                            .iter()
                            .map(|parameter| {
                                Ok((
                                    parameter.name.clone(),
                                    ParameterValue::try_from_parameter_value_msg(
                                        parameter.value.clone(),
                                    )?,
                                ))
                            })
                            .collect::<Result<Vec<_>>>();
                        let updates = match updates {
                            Ok(updates) => updates,
                            Err(err) => {
                                result.result.successful = false;
                                result.result.reason =
                                    format!("Can't set parameters atomically: {err}");
                                if let Err(err) = req.respond(result) {
                                    log::warn!(
                                    "could not send reply to atomic set parameter request: {err}"
                                );
                                }
                                return future::ready(());
                            }
                        };
                        let mut transaction =
                            match lock_parameter_transaction(&parameter_transaction) {
                                Ok(transaction) => transaction,
                                Err(err) => {
                                    result.result.successful = false;
                                    result.result.reason =
                                        format!("Can't set parameters atomically: {err}");
                                    if let Err(err) = req.respond(result) {
                                        log::warn!(
                                    "could not send reply to atomic set parameter request: {err}"
                                );
                                    }
                                    return future::ready(());
                                }
                            };
                        let update_result = apply_parameter_updates(
                            updates,
                            &params,
                            &parameter_descriptors,
                            parameter_provider_clone.as_ref(),
                            &parameter_pre_set_callbacks,
                            &parameter_validation_callbacks,
                            allow_undeclared_parameters,
                        );
                        transaction.unlock();

                        match update_result {
                            Ok(applied) => {
                                result.result.successful = true;
                                #[cfg(r2r__rosgraph_msgs__msg__Clock)]
                                if let Some(enabled) = applied.sim_time_update {
                                    atomic_time_source.request_sim_time(enabled);
                                }
                                atomic_parameter_event_sink
                                    .notify(applied.changed, applied.ros_events);
                                notify_parameter_post_set_callbacks(
                                    &applied.updates,
                                    &parameter_post_set_callbacks,
                                );
                            }
                            Err(err) => {
                                result.result.successful = false;
                                result.result.reason =
                                    format!("Can't set parameters atomically: {err}");
                            }
                        }
                        drop(transaction);
                        if let Err(err) = req.respond(result) {
                            log::warn!(
                                "could not send reply to atomic set parameter request: {err}"
                            );
                        }
                        future::ready(())
                    },
                );
                handlers.push(Box::pin(set_params_atomically_future));
            }

            if let Some(provider_state) = &parameter_provider {
                let mut provider = provider_state.provider.lock();
                let mut params = self.params.lock();
                let mut deferred_overrides = HashSet::new();
                if !self.automatically_declare_parameters_from_overrides {
                    for (name, value) in &self.parameter_overrides {
                        if !params.contains_key(name) {
                            params.insert(name.clone(), Parameter::new(value.clone()));
                            deferred_overrides.insert(name.clone());
                        }
                    }
                }
                provider.register_parameters("", None, &mut params)?;
                let candidates = params
                    .keys()
                    .filter(|name| name.as_str() != "use_sim_time")
                    .cloned()
                    .collect::<Vec<_>>();
                let mut parameter_names = HashSet::new();
                let mut unused_deferred_overrides = Vec::new();
                for name in candidates {
                    match provider.get_parameter(&name) {
                        Ok(_) => {
                            parameter_names.insert(name);
                        }
                        Err(Error::InvalidParameterName { .. }) => {
                            if deferred_overrides.contains(&name) {
                                unused_deferred_overrides.push(name);
                            }
                        }
                        Err(err) => return Err(err),
                    }
                }
                for name in unused_deferred_overrides {
                    params.shift_remove(&name);
                }
                debug_assert!(provider_state.parameter_names.set(parameter_names).is_ok());
            }
            #[cfg(r2r__rosgraph_msgs__msg__Clock)]
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
                        let enabled = match self.parameter_overrides.get("use_sim_time") {
                            Some(ParameterValue::Bool(enabled)) => *enabled,
                            Some(_) => {
                                return Err(Error::InvalidParameterType {
                                    name: "use_sim_time".into(),
                                    ty: "boolean",
                                });
                            }
                            None => false,
                        };
                        params.insert(
                            "use_sim_time".into(),
                            Parameter::new(ParameterValue::Bool(enabled)),
                        );
                        enabled
                    }
                }
            };
            self.initialize_parameter_descriptors(parameter_provider.is_none())?;

            #[cfg(r2r__rosgraph_msgs__msg__Clock)]
            if use_sim_time {
                let ts = self.time_source.clone();
                ts.enable_sim_time(self)?;
            }

            // we don't care about the result, the futures will not complete anyway.
            Ok((join_all(handlers).map(|_| ()), event_rx, parameter_event_sink))
        })();

        match setup {
            Ok((handler, event_rx, parameter_event_sink)) => {
                self.parameter_handler_started = true;
                self.parameter_provider = parameter_provider;
                self.parameter_event_sink = Some(parameter_event_sink);
                Ok((handler, event_rx))
            }
            Err(err) => {
                self.rollback_parameter_handler_endpoints(
                    service_count,
                    publisher_count,
                    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
                    subscriber_count,
                );
                *self.params.lock() = original_params;
                *self.parameter_descriptors.lock() = original_parameter_descriptors;
                Err(err)
            }
        }
    }

    fn default_parameter_descriptor(
        name: &str, parameter: &Parameter, dynamic_typing: bool,
    ) -> ParameterDescriptor {
        ParameterDescriptor {
            name: name.to_owned(),
            type_: parameter.value.parameter_type().into(),
            description: parameter.description.to_owned(),
            dynamic_typing,
            ..Default::default()
        }
    }

    fn initialize_parameter_descriptors(&self, dynamic_typing: bool) -> Result<()> {
        let params = self.params.lock();
        let mut descriptors = self.parameter_descriptors.lock();
        for (name, parameter) in params.iter() {
            let descriptor = descriptors.entry(name.clone()).or_insert_with(|| {
                Self::default_parameter_descriptor(
                    name,
                    parameter,
                    dynamic_typing && name != "use_sim_time",
                )
            });
            descriptor.name = name.clone();
            descriptor.type_ = parameter.value.parameter_type().into();
            if name == "use_sim_time" {
                descriptor.dynamic_typing = false;
            }
            if descriptor.description.is_empty() {
                descriptor.description = parameter.description.to_owned();
            }
            validate_parameter_descriptor(name, descriptor)?;
            validate_parameter_update(name, &parameter.value, descriptor, false)?;
        }
        Ok(())
    }

    fn record_successful_parameter_descriptor(
        descriptors: &mut IndexMap<String, ParameterDescriptor>, name: &str, value: &ParameterValue,
    ) {
        if matches!(value, ParameterValue::NotSet) {
            descriptors.shift_remove(name);
            return;
        }
        if let Some(descriptor) = descriptors.get_mut(name) {
            if descriptor.dynamic_typing {
                descriptor.type_ = value.parameter_type().into();
            }
        } else {
            let dynamic_typing = name != "use_sim_time";
            descriptors.insert(
                name.to_owned(),
                ParameterDescriptor {
                    name: name.to_owned(),
                    type_: value.parameter_type().into(),
                    dynamic_typing,
                    ..Default::default()
                },
            );
        }
    }

    fn store_dynamic_parameter(
        params: &mut IndexMap<String, Parameter>, name: &str, value: &ParameterValue,
    ) {
        if matches!(value, ParameterValue::NotSet) {
            params.shift_remove(name);
        } else {
            params
                .entry(name.to_owned())
                .and_modify(|parameter| parameter.value = value.clone())
                .or_insert_with(|| Parameter::new(value.clone()));
        }
    }

    fn handle_list_parameters(
        req: ServiceRequest<rcl_interfaces::srv::ListParameters::Service>,
        params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    ) -> future::Ready<()> {
        use rcl_interfaces::srv::ListParameters;

        let result = read_list_parameter_values(params, &req.message.prefixes, req.message.depth);
        if let Err(err) = req.respond(ListParameters::Response { result }) {
            log::warn!("could not send reply to list parameter request: {err}");
        }
        future::ready(())
    }

    fn handle_desc_parameters(
        req: ServiceRequest<rcl_interfaces::srv::DescribeParameters::Service>,
        params: &Arc<Mutex<IndexMap<String, Parameter>>>,
        parameter_descriptors: &Arc<Mutex<IndexMap<String, ParameterDescriptor>>>,
    ) -> future::Ready<()> {
        use rcl_interfaces::srv::DescribeParameters;
        let descriptors =
            read_parameter_descriptors(&req.message.names, params, parameter_descriptors);
        if let Err(err) = req.respond(DescribeParameters::Response { descriptors }) {
            log::warn!("could not send reply to describe parameters request: {err}");
        }
        future::ready(())
    }

    /// Returns raw parameter values in the same order as `names`.
    ///
    /// Undeclared names and provider read failures produce [`ParameterValue::NotSet`], matching
    /// the error-free response shape of the ROS `GetParameters` service. Use
    /// [`Node::try_get_parameters`] when provider errors must be propagated.
    pub fn get_parameters<I, S>(&self, names: I) -> Vec<ParameterValue>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names = names
            .into_iter()
            .map(|name| name.as_ref().to_owned())
            .collect::<Vec<_>>();
        read_parameter_values(&names, &self.params, self.parameter_provider.as_ref())
    }

    /// Returns raw parameter values while honoring the node's undeclared-parameter policy.
    ///
    /// Unlike [`Node::get_parameters`], this returns [`Error::ParameterNotDeclared`] for the
    /// first unknown name when `allow_undeclared_parameters` is disabled.
    pub fn try_get_parameters<I, S>(&self, names: I) -> Result<Vec<ParameterValue>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names = names
            .into_iter()
            .map(|name| name.as_ref().to_owned())
            .collect::<Vec<_>>();
        read_parameter_values_with_policy(
            &names,
            &self.params,
            self.parameter_provider.as_ref(),
            self.allow_undeclared_parameters,
        )
    }

    /// Returns parameters below `prefix`, keyed by their name relative to the prefix.
    pub fn get_parameters_by_prefix(&self, prefix: &str) -> IndexMap<String, ParameterValue> {
        self.get_parameters_by_prefix_impl(prefix, false)
            .expect("permissive prefix reads map provider failures to unset values")
    }

    /// Returns parameters below `prefix`, propagating errors from a derived parameter provider.
    ///
    /// Keys are relative to `prefix`, as in [`Node::get_parameters_by_prefix`]. This method is
    /// useful when treating a provider read failure as [`ParameterValue::NotSet`] would hide a
    /// stale or unavailable backing value.
    pub fn try_get_parameters_by_prefix(
        &self, prefix: &str,
    ) -> Result<IndexMap<String, ParameterValue>> {
        self.get_parameters_by_prefix_impl(prefix, true)
    }

    fn get_parameters_by_prefix_impl(
        &self, prefix: &str, propagate_provider_errors: bool,
    ) -> Result<IndexMap<String, ParameterValue>> {
        let qualified_prefix = (!prefix.is_empty()).then(|| format!("{prefix}."));
        let names = self
            .params
            .lock()
            .keys()
            .filter(|name| {
                qualified_prefix
                    .as_ref()
                    .is_none_or(|prefix| name.starts_with(prefix))
            })
            .cloned()
            .collect::<Vec<_>>();
        let values = read_parameter_values_impl(
            &names,
            &self.params,
            self.parameter_provider.as_ref(),
            true,
            propagate_provider_errors,
        )?;
        let trim = qualified_prefix.as_ref().map_or(0, String::len);
        Ok(names
            .into_iter()
            .map(|name| name[trim..].to_owned())
            .zip(values)
            .collect())
    }

    /// Lists declared parameter names and namespaces filtered by prefixes and relative depth.
    ///
    /// A depth of [`rcl_interfaces::srv::ListParameters::Request::DEPTH_RECURSIVE`] recursively
    /// lists every matching parameter.
    pub fn list_parameters<I, S>(
        &self, prefixes: I, depth: u64,
    ) -> rcl_interfaces::msg::ListParametersResult
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let prefixes = prefixes
            .into_iter()
            .map(|prefix| prefix.as_ref().to_owned())
            .collect::<Vec<_>>();
        read_list_parameter_values(&self.params, &prefixes, depth)
    }

    /// Returns full ROS descriptors in the same order as `names`.
    pub fn describe_parameters<I, S>(&self, names: I) -> Vec<ParameterDescriptor>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names = names
            .into_iter()
            .map(|name| name.as_ref().to_owned())
            .collect::<Vec<_>>();
        read_parameter_descriptors(&names, &self.params, &self.parameter_descriptors)
    }

    /// Returns parameter descriptors while honoring the node's undeclared-parameter policy.
    pub fn try_describe_parameters<I, S>(&self, names: I) -> Result<Vec<ParameterDescriptor>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names = names
            .into_iter()
            .map(|name| name.as_ref().to_owned())
            .collect::<Vec<_>>();
        read_parameter_descriptors_with_policy(
            &names,
            &self.params,
            &self.parameter_descriptors,
            self.allow_undeclared_parameters,
        )
    }

    /// Returns the descriptor for one parameter.
    pub fn describe_parameter(&self, name: &str) -> Result<ParameterDescriptor> {
        self.try_describe_parameters([name])?
            .pop()
            .ok_or(Error::RCL_RET_ERROR)
    }

    /// Returns parameter types in the same order as `names`.
    pub fn get_parameter_types<I, S>(&self, names: I) -> Vec<ParameterType>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names = names
            .into_iter()
            .map(|name| name.as_ref().to_owned())
            .collect::<Vec<_>>();
        read_parameter_types(&names, &self.params)
    }

    /// Returns parameter types while honoring the node's undeclared-parameter policy.
    pub fn try_get_parameter_types<I, S>(&self, names: I) -> Result<Vec<ParameterType>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names = names
            .into_iter()
            .map(|name| name.as_ref().to_owned())
            .collect::<Vec<_>>();
        read_parameter_types_with_policy(&names, &self.params, self.allow_undeclared_parameters)
    }

    /// Returns the type of one parameter.
    pub fn get_parameter_type(&self, name: &str) -> Result<ParameterType> {
        self.try_get_parameter_types([name])?
            .pop()
            .ok_or(Error::RCL_RET_ERROR)
    }

    /// Fetch a single ROS parameter.
    pub fn get_parameter<T>(&self, name: &str) -> Result<T>
    where
        ParameterValue: TryInto<T, Error = WrongParameterType>,
    {
        let value = self
            .try_get_parameters([name])?
            .pop()
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

    /// Fetches a parameter or returns `alternative` when it is not set.
    ///
    /// A declared value with the wrong type still returns [`Error::ParameterWrongType`].
    pub fn get_parameter_or<T>(&self, name: &str, alternative: T) -> Result<T>
    where
        ParameterValue: TryInto<T, Error = WrongParameterType>,
    {
        let value = read_parameter_values_impl(
            &[name.to_owned()],
            &self.params,
            self.parameter_provider.as_ref(),
            true,
            true,
        )?
        .pop()
        .unwrap_or(ParameterValue::NotSet);
        if matches!(value, ParameterValue::NotSet) {
            return Ok(alternative);
        }
        value
            .try_into()
            .map_err(|error: WrongParameterType| Error::ParameterWrongType {
                name: name.to_owned(),
                expected_type: error.expected_type_name,
                actual_type: error.actual_type_name,
            })
    }

    /// Sets one parameter synchronously.
    ///
    /// Descriptor constraints and a registered [`RosParams`] provider are applied before the
    /// cached value changes. Parameter events are emitted when a parameter handler is active.
    pub fn set_parameter(
        &self, name: impl Into<String>, value: impl Into<ParameterValue>,
    ) -> Result<()> {
        self.set_parameters_atomically([(name.into(), value.into())])
    }

    /// Sets parameters independently and returns one result for each input item.
    ///
    /// A failed item does not prevent later items from being attempted. Use
    /// [`Node::set_parameters_atomically`] when the whole batch must succeed or fail together.
    pub fn set_parameters<I, N, V>(&self, parameters: I) -> Vec<Result<()>>
    where
        I: IntoIterator<Item = (N, V)>,
        N: Into<String>,
        V: Into<ParameterValue>,
    {
        parameters
            .into_iter()
            .map(|(name, value)| self.set_parameter(name, value.into()))
            .collect()
    }

    /// Registers a callback that can reject parameter updates before any value is changed.
    ///
    /// Callbacks run in reverse registration order and stop at the first error. Keep the returned
    /// handle alive for as long as the callback should remain active; dropping it unregisters the
    /// callback. A callback may inspect parameter state. Attempting another parameter mutation on
    /// the same node from a callback returns [`Error::RecursiveParameterMutation`].
    pub fn add_on_set_parameters_callback<F>(&self, callback: F) -> ParameterCallbackHandle
    where
        F: Fn(&[(String, ParameterValue)]) -> Result<()> + Send + Sync + 'static,
    {
        let id = self
            .parameter_validation_callbacks
            .lock()
            .insert(Arc::new(callback));
        ParameterCallbackHandle {
            id,
            callbacks: Arc::downgrade(&self.parameter_validation_callbacks),
        }
    }

    /// Registers a callback that may transform a parameter batch before validation.
    ///
    /// Callbacks run in reverse registration order and each sees the batch produced by the
    /// previous callback. The final batch is then validated and committed atomically. Producing
    /// an empty batch rejects the update. Keep the returned handle alive to keep the callback
    /// registered. Parameter declaration and explicit undeclaration do not invoke this callback.
    /// Recursive parameter mutation on the same node returns
    /// [`Error::RecursiveParameterMutation`].
    pub fn add_pre_set_parameters_callback<F>(&self, callback: F) -> ParameterPreSetCallbackHandle
    where
        F: Fn(&mut Vec<(String, ParameterValue)>) + Send + Sync + 'static,
    {
        let id = self
            .parameter_pre_set_callbacks
            .lock()
            .insert(Arc::new(callback));
        ParameterPreSetCallbackHandle {
            id,
            callbacks: Arc::downgrade(&self.parameter_pre_set_callbacks),
        }
    }

    /// Registers a callback invoked after a parameter transaction succeeds.
    ///
    /// Callbacks run in reverse registration order. They receive the complete successful input
    /// batch and execute after all internal parameter locks are released, so they may inspect the
    /// committed node state. Recursive parameter mutation on the same node returns
    /// [`Error::RecursiveParameterMutation`]. Keep the returned handle alive to keep the callback
    /// registered.
    pub fn add_post_set_parameters_callback<F>(&self, callback: F) -> ParameterPostSetCallbackHandle
    where
        F: Fn(&[(String, ParameterValue)]) + Send + Sync + 'static,
    {
        let id = self
            .parameter_post_set_callbacks
            .lock()
            .insert(Arc::new(callback));
        ParameterPostSetCallbackHandle {
            id,
            callbacks: Arc::downgrade(&self.parameter_post_set_callbacks),
        }
    }

    /// Validates and sets a batch of parameters as one transaction.
    pub fn set_parameters_atomically<I, N, V>(&self, parameters: I) -> Result<()>
    where
        I: IntoIterator<Item = (N, V)>,
        N: Into<String>,
        V: Into<ParameterValue>,
    {
        let updates = parameters
            .into_iter()
            .map(|(name, value)| (name.into(), value.into()))
            .collect::<Vec<_>>();
        let mut transaction = lock_parameter_transaction(&self.parameter_transaction)?;
        let applied = {
            apply_parameter_updates(
                updates,
                &self.params,
                &self.parameter_descriptors,
                self.parameter_provider.as_ref(),
                &self.parameter_pre_set_callbacks,
                &self.parameter_validation_callbacks,
                self.allow_undeclared_parameters,
            )?
        };
        transaction.unlock();
        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        if let Some(enabled) = applied.sim_time_update {
            self.time_source.request_sim_time(enabled);
        }
        if let Some(sink) = &self.parameter_event_sink {
            sink.notify(applied.changed, applied.ros_events);
        }
        notify_parameter_post_set_callbacks(&applied.updates, &self.parameter_post_set_callbacks);
        Ok(())
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
            max_messages_per_spin: subscription_batch_size(options.channel_capacity),
            match_state: SubscriptionMatchState::new(id),
        };
        self.add_subscriber(Box::new(ws));
        Ok(Subscription::new(receiver, id, Arc::clone(&self.canceled_subscriptions)))
    }

    /// Subscribe to standard ROS parameter events, optionally filtering by node and parameter.
    ///
    /// Each parameter in a ROS event is emitted as an individual [`ParameterEventChange`].
    pub fn subscribe_parameter_events(
        &mut self, node: Option<String>, parameter: Option<String>,
    ) -> Result<impl Stream<Item = ParameterEventChange> + Unpin> {
        self.subscribe_parameter_events_with_options(
            node,
            parameter,
            QosProfile::parameter_events(),
            SubscriptionOptions::default(),
        )
    }

    /// Subscribes to standard ROS parameter events with explicit QoS and stream options.
    pub fn subscribe_parameter_events_with_options(
        &mut self, node: Option<String>, parameter: Option<String>, qos: QosProfile,
        options: SubscriptionOptions,
    ) -> Result<impl Stream<Item = ParameterEventChange> + Unpin> {
        let events = self.subscribe_with_options::<rcl_interfaces::msg::ParameterEvent>(
            "/parameter_events",
            qos,
            options,
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
            max_messages_per_spin: subscription_batch_size(options.channel_capacity),
            match_state: SubscriptionMatchState::new(id),
        };
        self.add_subscriber(Box::new(ws));
        Ok(Subscription::new(receiver, id, Arc::clone(&self.canceled_subscriptions)))
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
            max_messages_per_spin: subscription_batch_size(options.channel_capacity),
            match_state: SubscriptionMatchState::new(id),
        };
        self.add_subscriber(Box::new(ws));
        Ok(Subscription::new(receiver, id, Arc::clone(&self.canceled_subscriptions)))
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
            max_messages_per_spin: subscription_batch_size(options.channel_capacity),
            match_state: SubscriptionMatchState::new(id),
        };
        self.add_subscriber(Box::new(ws));
        Ok(Subscription::new(receiver, id, Arc::clone(&self.canceled_subscriptions)))
    }

    /// Immediately destroys a subscription owned by this node.
    ///
    /// Dropping a subscription schedules its native endpoint for removal during the next spin.
    /// This method is useful when deterministic teardown is required before another spin cycle.
    /// Passing a subscription created by another node leaves this node unchanged and schedules
    /// the subscription for normal cleanup by its owning node.
    pub fn destroy_subscription<T>(&mut self, subscription: Subscription<T>) {
        let id = subscription.id;
        drop(subscription);
        self.destroy_subscription_by_id(id);
    }

    pub(crate) fn get_subscription_publisher_count(&self, id: uuid::Uuid) -> Result<usize> {
        self.subscriber(id)?.publisher_count()
    }

    pub(crate) fn get_subscription_actual_qos(&self, id: uuid::Uuid) -> Result<QosProfile> {
        self.subscriber(id)?.actual_qos()
    }

    pub(crate) fn get_subscription_topic_name(&self, id: uuid::Uuid) -> Result<String> {
        self.subscriber(id)?.topic_name()
    }

    pub(crate) fn get_subscription_can_loan_messages(&self, id: uuid::Uuid) -> Result<bool> {
        Ok(self.subscriber(id)?.can_loan_messages())
    }

    pub(crate) fn get_subscription_network_flow_endpoints(
        &self, id: uuid::Uuid,
    ) -> Result<Vec<NetworkFlowEndpoint>> {
        self.subscriber(id)?.network_flow_endpoints()
    }

    pub(crate) fn get_subscription_content_filter_enabled(&self, id: uuid::Uuid) -> Result<bool> {
        Ok(self.subscriber(id)?.is_content_filter_enabled())
    }

    pub(crate) fn set_subscription_content_filter(
        &mut self, id: uuid::Uuid, options: &ContentFilterOptions,
    ) -> Result<()> {
        self.subscriber(id)?.set_content_filter(options)
    }

    pub(crate) fn get_subscription_content_filter(
        &self, id: uuid::Uuid,
    ) -> Result<ContentFilterOptions> {
        self.subscriber(id)?.get_content_filter()
    }

    pub(crate) fn take_subscription_requested_incompatible_qos(
        &mut self, id: uuid::Uuid,
    ) -> Result<Option<RequestedIncompatibleQosStatus>> {
        self.subscriber_mut(id)?.take_requested_incompatible_qos()
    }

    pub(crate) fn take_subscription_requested_deadline_missed(
        &mut self, id: uuid::Uuid,
    ) -> Result<Option<RequestedDeadlineMissedStatus>> {
        self.subscriber_mut(id)?.take_requested_deadline_missed()
    }

    pub(crate) fn take_subscription_liveliness_changed(
        &mut self, id: uuid::Uuid,
    ) -> Result<Option<LivelinessChangedStatus>> {
        self.subscriber_mut(id)?.take_liveliness_changed()
    }

    pub(crate) fn take_subscription_message_lost(
        &mut self, id: uuid::Uuid,
    ) -> Result<Option<MessageLostStatus>> {
        self.subscriber_mut(id)?.take_message_lost()
    }

    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble
    )))]
    pub(crate) fn take_subscription_matched(
        &mut self, id: uuid::Uuid,
    ) -> Result<Option<MatchedStatus>> {
        self.subscriber_mut(id)?.take_matched()
    }

    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble
    )))]
    pub(crate) fn take_subscription_incompatible_type(
        &mut self, id: uuid::Uuid,
    ) -> Result<Option<IncompatibleTypeStatus>> {
        self.subscriber_mut(id)?.take_incompatible_type()
    }

    pub(crate) fn register_subscription_publisher_wait(
        &mut self, id: uuid::Uuid,
    ) -> Result<oneshot::Receiver<()>> {
        let state = self
            .subscriber_mut(id)?
            .match_state_mut()
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        let (sender, receiver) = oneshot::channel();
        state.waiters.register(sender);
        self.subscriptions_with_active_waits.insert(id);
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
        let closed_error = client.availability_closed_error();
        let (sender, receiver) = oneshot::channel();
        client.register_poll_available(sender)?;
        Ok(receiver.map_err(move |_| closed_error))
    }

    /// Wait for a service or action server to become available, up to `timeout`.
    ///
    /// This future is runtime-agnostic and advances while the node is spun.
    pub fn is_available_with_timeout(
        &mut self, client: &dyn IsAvailablePollable, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        client.validate_availability_owner(self)?;
        Self::validate_timeout(timeout)?;
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

    pub(crate) fn validate_timeout(timeout: Duration) -> Result<()> {
        duration_to_rcl_nanos(timeout).map(|_| ())
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
            cancel_request_channel_capacity,
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
        let id = uuid::Uuid::new_v4();
        let publisher_handle = create_publisher_helper(
            self.node_handle.as_mut(),
            topic,
            T::get_ts(),
            qos_profile,
            options,
            id,
            Arc::clone(&self.active_wait_publishers),
        )?;
        let arc = Arc::new(publisher_handle);
        let p = make_publisher(Arc::downgrade(&arc), id, Arc::clone(&self.dropped_publishers));
        self.add_publisher(arc);
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
        let id = uuid::Uuid::new_v4();
        let publisher_handle = create_publisher_helper(
            self.node_handle.as_mut(),
            topic,
            dummy.ts,
            qos_profile,
            options,
            id,
            Arc::clone(&self.active_wait_publishers),
        )?;
        let arc = Arc::new(publisher_handle);
        let p = make_publisher_untyped(
            Arc::downgrade(&arc),
            id,
            Arc::clone(&self.dropped_publishers),
            topic_type.to_owned(),
        );
        self.add_publisher(arc);
        Ok(p)
    }

    /// Destroy a ROS publisher.
    pub fn destroy_publisher<T: WrappedTypesupport>(&mut self, p: Publisher<T>) {
        let identity = p.identity();
        if self.destroy_publisher_by_id(identity) {
            p.mark_explicitly_destroyed();
        }
    }

    /// Destroy a ROS publisher.
    pub fn destroy_publisher_untyped(&mut self, p: PublisherUntyped) {
        let identity = p.identity();
        if self.destroy_publisher_by_id(identity) {
            p.mark_explicitly_destroyed();
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

    /// Spins this node until `future` completes or `timeout` elapses.
    ///
    /// This is a runtime-independent counterpart to repeatedly calling [`Node::try_spin_once`]
    /// and polling a service, action, timer, graph, or matching future. A future that is already
    /// ready is returned even when `timeout` is zero. Futures are polled periodically even when
    /// they do not wake themselves, which supports availability checks whose state may change
    /// without a corresponding wait-set event. The timeout uses steady wall-clock time, so it is
    /// not affected by ROS simulated time.
    pub fn spin_until_future_complete<T>(
        &mut self, future: impl Future<Output = Result<T>>, timeout: Duration,
    ) -> Result<T> {
        futures::pin_mut!(future);
        let started = Instant::now();
        loop {
            if let Some(result) = future.as_mut().now_or_never() {
                return result;
            }

            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(Error::RCL_RET_TIMEOUT);
            }
            self.try_spin_once(remaining.min(SPIN_UNTIL_FUTURE_POLL_INTERVAL))?;
        }
    }

    /// Spin the ROS node and report errors from the underlying RCL wait set.
    pub fn try_spin_once(&mut self, timeout: Duration) -> Result<()> {
        if !self.context.is_valid() {
            return Err(Error::RCL_RET_NOT_INIT);
        }

        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        {
            let time_source = self.time_source.clone();
            time_source.apply_requested_sim_time(self)?;
        }

        self.graph_change_waiters.prune_canceled();

        let canceled_subscriptions = {
            let mut canceled_subscriptions = self.canceled_subscriptions.lock();
            canceled_subscriptions.drain(..).collect::<Vec<_>>()
        };
        let node_handle = self.node_handle.as_mut();
        for id in canceled_subscriptions {
            Self::remove_subscription(
                &mut self.subscribers,
                &mut self.subscription_indexes,
                id,
                node_handle,
            );
        }

        // Internal subscriptions, such as `/clock`, have no public stream whose Drop can report
        // an ID. Only inspect them when their owning subsystem has requested a state transition.
        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        if self.time_source.has_pending_subscriber_removal() {
            let previous_subscriber_count = self.subscribers.len();
            self.subscribers.retain_mut(|subscriber| {
                if subscriber.match_state().is_none() && subscriber.should_remove() {
                    subscriber.destroy(node_handle);
                    false
                } else {
                    true
                }
            });
            if self.subscribers.len() != previous_subscriber_count {
                Self::rebuild_subscription_indexes(
                    &self.subscribers,
                    &mut self.subscription_indexes,
                );
            }
        }

        let canceled_services = {
            let mut canceled_services = self.canceled_services.lock();
            canceled_services.drain(..).collect::<Vec<_>>()
        };
        let node_handle = self.node_handle.as_mut();
        for id in canceled_services {
            Self::remove_service(&mut self.services, &mut self.service_indexes, id, node_handle);
        }

        {
            let mut dropped = self.dropped_clients.lock();
            self.pending_client_removals.extend(dropped.drain(..));
        }
        let pending_client_removals = std::mem::take(&mut self.pending_client_removals);
        for identity in pending_client_removals {
            if !self.remove_client(identity) {
                self.pending_client_removals.insert(identity);
            }
        }

        {
            let mut dropped = self.dropped_publishers.lock();
            self.pending_publisher_removals.extend(dropped.drain(..));
        }
        let pending_publisher_removals = std::mem::take(&mut self.pending_publisher_removals);
        for identity in pending_publisher_removals {
            if !self.remove_dropped_publisher(identity) {
                self.pending_publisher_removals.insert(identity);
            }
        }

        // Do not wait for a long-period timer to become ready before noticing that its public
        // handle has gone away. Timer::drop reports the exact UUID, avoiding a full timer scan.
        let canceled_timers = {
            let mut canceled_timers = self.canceled_timers.lock();
            canceled_timers.drain(..).collect::<Vec<_>>()
        };
        for id in canceled_timers {
            Self::remove_timer(&mut self.timers, &mut self.timer_indexes, id);
        }

        {
            let mut dropped = self.dropped_action_clients.lock();
            self.pending_action_client_removals
                .extend(dropped.drain(..));
        }
        let pending_action_client_removals =
            std::mem::take(&mut self.pending_action_client_removals);
        for identity in pending_action_client_removals {
            match self.remove_action_client(identity) {
                Ok(true) => {}
                Ok(false) => {
                    self.pending_action_client_removals.insert(identity);
                }
                Err(err) => {
                    log::warn!("could not inspect dropped action client: {err}");
                    self.pending_action_client_removals.insert(identity);
                }
            }
        }

        // Only action servers with an in-flight cancel delivery or decision need maintenance
        // before waiting. Keep them registered until every response has been sent.
        let action_servers_with_pending_cancels = self
            .action_servers_with_pending_cancels
            .iter()
            .copied()
            .collect::<Vec<_>>();
        for id in action_servers_with_pending_cancels {
            let Some(index) = self.action_server_indexes.get(&id).copied() else {
                self.action_servers_with_pending_cancels.remove(&id);
                continue;
            };
            let action_server = &self.action_servers[index];
            record_action_server_maintenance_poll();
            let maintenance_result =
                complete_action_server_operation(action_server, |server, completions| {
                    completions
                        .extend(server.take_pending_cancel_delivery(Arc::clone(action_server)));
                    server
                        .send_completed_cancel_requests(Arc::clone(action_server), completions)?;
                    Ok(())
                });
            if !action_server.lock().has_pending_cancel_work() {
                self.action_servers_with_pending_cancels.remove(&id);
            }
            maintenance_result?;
        }

        {
            let mut dropped = self.dropped_action_servers.lock();
            self.pending_action_server_removals
                .extend(dropped.drain(..));
        }
        let pending_action_server_removals =
            std::mem::take(&mut self.pending_action_server_removals);
        for id in pending_action_server_removals {
            match self.remove_action_server(id) {
                Ok(true) => {}
                Ok(false) => {
                    self.pending_action_server_removals.insert(id);
                }
                Err(err) => {
                    log::warn!("could not inspect dropped action server: {err}");
                    self.pending_action_server_removals.insert(id);
                }
            }
        }

        {
            let mut active = self.active_availability_clients.lock();
            self.clients_with_active_availability
                .extend(active.drain(..));
        }
        let clients_with_active_availability =
            std::mem::take(&mut self.clients_with_active_availability);
        for identity in clients_with_active_availability {
            let Some(index) = self.client_identities.get(&identity).copied() else {
                continue;
            };
            let client = &self.clients[index];
            complete_client_operation(client, |client| {
                Ok(client.poll_available(self.node_handle.as_mut()))
            })?;
            if client.lock().has_pending_availability() {
                self.clients_with_active_availability.insert(identity);
            }
        }

        {
            let mut active = self.active_maintenance_action_clients.lock();
            self.action_clients_with_pending_maintenance
                .extend(active.drain(..));
        }
        let action_clients_with_pending_maintenance =
            std::mem::take(&mut self.action_clients_with_pending_maintenance);
        for identity in action_clients_with_pending_maintenance {
            let Some(index) = self.action_client_identities.get(&identity).copied() else {
                continue;
            };
            let client = &self.action_clients[index];
            record_action_client_maintenance_poll();
            let maintenance_result = complete_action_client_operation(
                client,
                |client, completions| {
                    completions.extend(client.poll_maintenance());
                    Ok(())
                },
                |client, uuid| client.forget_goal(uuid),
            );
            if client.lock().has_pending_maintenance() {
                self.action_clients_with_pending_maintenance
                    .insert(identity);
            }
            maintenance_result?;
        }

        {
            let mut active = self.active_availability_action_clients.lock();
            self.action_clients_with_active_availability
                .extend(active.drain(..));
        }
        let active_action_clients =
            std::mem::take(&mut self.action_clients_with_active_availability);
        for identity in active_action_clients {
            let Some(index) = self.action_client_identities.get(&identity).copied() else {
                continue;
            };
            let client = &self.action_clients[index];
            complete_action_client_operation(
                client,
                |client, completions| {
                    completions.extend(client.poll_available(self.node_handle.as_mut()));
                    Ok(())
                },
                |client, uuid| client.forget_goal(uuid),
            )?;
            if client.lock().has_pending_availability() {
                self.action_clients_with_active_availability
                    .insert(identity);
            }
        }

        {
            let mut active = self.active_wait_publishers.lock();
            self.publishers_with_active_waits.extend(active.drain(..));
        }
        let publishers_with_active_waits = std::mem::take(&mut self.publishers_with_active_waits);
        for id in publishers_with_active_waits {
            let Some(index) = self.publisher_indexes.get(&id).copied() else {
                continue;
            };
            if self.pubs[index].poll_waits() {
                self.publishers_with_active_waits.insert(id);
            }
        }
        let subscriptions_with_active_waits =
            std::mem::take(&mut self.subscriptions_with_active_waits);
        for id in subscriptions_with_active_waits {
            let Some(index) = self.subscription_indexes.get(&id).copied() else {
                continue;
            };
            let subscriber = &mut self.subscribers[index];
            subscriber.poll_has_publishers();
            if subscriber.has_pending_publisher_waits() {
                self.subscriptions_with_active_waits.insert(id);
            }
        }

        let timeout_nanos = duration_to_rcl_nanos(timeout)?;

        // #[doc = "* This function is thread-safe for unique wait sets with unique contents."]
        // #[doc = "* This function cannot operate on the same wait set in multiple threads, and"]
        // #[doc = "* the wait sets may not share content."]
        // #[doc = "* For example, calling rcl_wait() in two threads on two different wait sets"]
        // #[doc = "* that both contain a single, shared guard condition is undefined behavior."]

        let capacities = WaitSetCapacities {
            subscriptions: self.subscribers.len() + self.action_wait_set_capacities.subscriptions,
            guard_conditions: self.action_wait_set_capacities.guard_conditions
                + usize::from(!self.graph_change_waiters.is_empty()),
            timers: self.timers.len() + self.action_wait_set_capacities.timers,
            clients: self.clients.len() + self.action_wait_set_capacities.clients,
            services: self.services.len() + self.action_wait_set_capacities.services,
        };
        if capacities == WaitSetCapacities::default() {
            // RCL rejects an empty wait set instead of treating it as an idle wait. Honor the
            // documented spin timeout while still allowing context shutdown to wake the spin.
            return self.context.wait_for_shutdown_or_timeout(timeout);
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

        let graph_guard_index = if self.graph_change_waiters.is_empty() {
            None
        } else {
            let graph_guard =
                unsafe { rcl_node_get_graph_guard_condition(self.node_handle.as_ref()) };
            if graph_guard.is_null() {
                return Err(Error::RCL_RET_NODE_INVALID);
            }
            let mut index = 0;
            check_rcl_ret(unsafe {
                rcl_wait_set_add_guard_condition(ws, graph_guard, &mut index)
            })?;
            Some(index)
        };

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

        if let Some(index) = graph_guard_index {
            let graph_changed = !ws.guard_conditions.is_null()
                && unsafe { !(*ws.guard_conditions.add(index)).is_null() };
            if graph_changed {
                for waiter in self.graph_change_waiters.take_all() {
                    let _ = waiter.send(());
                }
            }
        }

        let mut subs_to_remove = HashSet::new();
        let mut subscriber_error = None;
        if !ws.subscriptions.is_null() {
            let ws_subs =
                unsafe { std::slice::from_raw_parts(ws.subscriptions, self.subscribers.len()) };
            for (s, ws_s) in self.subscribers.iter_mut().zip(ws_subs) {
                if ws_s != &std::ptr::null() {
                    match s.handle_ready() {
                        Ok(true) => {
                            subs_to_remove.insert(s.handle() as *const rcl_subscription_t);
                        }
                        Ok(false) => {}
                        Err(err) => {
                            subscriber_error.get_or_insert(err);
                        }
                    }
                }
            }
        }
        if !subs_to_remove.is_empty() {
            let node_handle = self.node_handle.as_mut();
            self.subscribers.retain_mut(|subscriber| {
                if subs_to_remove.contains(&(subscriber.handle() as *const rcl_subscription_t)) {
                    subscriber.destroy(node_handle);
                    false
                } else {
                    true
                }
            });
            Self::rebuild_subscription_indexes(&self.subscribers, &mut self.subscription_indexes);
        }
        if let Some(err) = subscriber_error {
            return Err(err);
        }

        let mut timers_to_remove = Vec::new();
        let mut timer_error = None;
        if !ws.timers.is_null() {
            let ws_timers = unsafe { std::slice::from_raw_parts(ws.timers, self.timers.len()) };
            for (s, ws_s) in self.timers.iter_mut().zip(ws_timers) {
                if ws_s != &std::ptr::null() {
                    // TODO: move this to impl Timer
                    match s.handle_incoming() {
                        Ok(true) => {
                            timers_to_remove.push(s.id);
                        }
                        Ok(false) => {}
                        Err(err) => {
                            timer_error.get_or_insert(err);
                        }
                    }
                }
            }
        }
        for id in timers_to_remove {
            Self::remove_timer(&mut self.timers, &mut self.timer_indexes, id);
        }
        if let Some(err) = timer_error {
            return Err(err);
        }

        if !ws.clients.is_null() {
            let ws_clients = unsafe { std::slice::from_raw_parts(ws.clients, self.clients.len()) };
            for (s, ws_s) in self.clients.iter_mut().zip(ws_clients) {
                if ws_s != &std::ptr::null() {
                    complete_client_responses(s, Client_::handle_response)?;
                }
            }
        }

        let mut services_to_remove = Vec::new();
        let mut service_error = None;
        if !ws.services.is_null() {
            let ws_services =
                unsafe { std::slice::from_raw_parts(ws.services, self.services.len()) };
            for (s, ws_s) in self.services.iter_mut().zip(ws_services) {
                if ws_s != &std::ptr::null() {
                    match complete_ready_service(s) {
                        Ok(ServiceHandleResult::Remove) => {
                            if let Some(id) = s.lock().id() {
                                services_to_remove.push(id);
                            }
                        }
                        Ok(ServiceHandleResult::Taken | ServiceHandleResult::Idle) => {}
                        Err(err) => {
                            service_error.get_or_insert(err);
                        }
                    }
                }
            }
        }
        let node_handle = self.node_handle.as_mut();
        for id in services_to_remove {
            Self::remove_service(&mut self.services, &mut self.service_indexes, id, node_handle);
        }
        if let Some(err) = service_error {
            return Err(err);
        }

        for ac in &self.action_clients {
            let (
                is_feedback_ready,
                is_status_ready,
                is_goal_response_ready,
                is_cancel_response_ready,
                is_result_response_ready,
            ) = {
                let ac = ac.lock();
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
                (
                    is_feedback_ready,
                    is_status_ready,
                    is_goal_response_ready,
                    is_cancel_response_ready,
                    is_result_response_ready,
                )
            };

            let ac_ref = Arc::clone(ac);
            if is_feedback_ready {
                complete_action_feedback_batch(
                    ac,
                    |client, completions| client.handle_feedback_msg(ac_ref.clone(), completions),
                    |client, uuid| client.forget_goal(uuid),
                )?;
            }
            complete_action_client_operation(
                ac,
                |ac, completions| {
                    if is_status_ready {
                        completions.extend(ac.handle_status_msg(ac_ref.clone())?);
                    }
                    if is_goal_response_ready {
                        collect_action_client_responses(ac, completions, |ac, completions| {
                            ac.handle_goal_response(ac_ref.clone(), completions)
                        })?;
                    }
                    if is_cancel_response_ready {
                        collect_action_client_responses(ac, completions, |ac, completions| {
                            ac.handle_cancel_response(completions)
                        })?;
                    }
                    if is_result_response_ready {
                        collect_action_client_responses(ac, completions, |ac, completions| {
                            ac.handle_result_response(ac_ref.clone(), completions)
                        })?;
                    }
                    Ok(())
                },
                |client, uuid| client.forget_goal(uuid),
            )?;
        }

        for s in &self.action_servers {
            let (
                server_id,
                is_goal_request_ready,
                is_cancel_request_ready,
                is_result_request_ready,
                is_goal_expired,
            ) = {
                let server = s.lock();
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
                (
                    server.id(),
                    is_goal_request_ready,
                    is_cancel_request_ready,
                    is_result_request_ready,
                    is_goal_expired,
                )
            };

            if is_goal_request_ready {
                complete_action_server_request_batch(s, |server| {
                    server.handle_goal_request(s.clone())
                })?;
            }
            if is_cancel_request_ready {
                let cancel_result = complete_action_server_delivery_batch(
                    s,
                    |server| server.handle_cancel_request(s.clone()),
                    ActionServer_::has_pending_cancel_delivery,
                );
                if s.lock().has_pending_cancel_work() {
                    self.action_servers_with_pending_cancels.insert(server_id);
                } else {
                    self.action_servers_with_pending_cancels.remove(&server_id);
                }
                cancel_result?;
            }
            if is_result_request_ready {
                complete_action_server_take_batch(s, |server| {
                    server.handle_result_request(s.clone())
                })?;
            }
            complete_action_server_operation(s, |server, completions| {
                if is_goal_expired {
                    server.handle_goal_expired(completions)?;
                }
                Ok(())
            })?;
        }

        Ok(())
    }

    /// Returns a map of topic names and type names of the publishers
    /// visible to this node.
    pub fn get_topic_names_and_types(&self) -> Result<HashMap<String, Vec<String>>> {
        self.get_topic_names_and_types_impl(false)
    }

    /// Returns raw middleware topic names and type names without ROS demangling.
    ///
    /// This exposes non-ROS topics and their middleware-specific names for diagnostics and
    /// interoperability. Prefer [`Node::get_topic_names_and_types`] for normal ROS topic
    /// discovery.
    pub fn get_raw_topic_names_and_types(&self) -> Result<HashMap<String, Vec<String>>> {
        self.get_topic_names_and_types_impl(true)
    }

    fn get_topic_names_and_types_impl(
        &self, no_demangle: bool,
    ) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types(|node, allocator, names_and_types| unsafe {
            rcl_get_topic_names_and_types(node, allocator, no_demangle, names_and_types)
        })
    }

    /// Returns a future that completes after the ROS graph next changes.
    ///
    /// The node must be spun while awaiting. All waiters registered before the same graph change
    /// are completed together.
    pub fn wait_for_graph_change(&mut self) -> impl Future<Output = Result<()>> {
        let (sender, receiver) = oneshot::channel();
        self.graph_change_waiters.register(sender);
        receiver.map_err(|_| Error::RCL_RET_NODE_INVALID)
    }

    /// Waits for the next ROS graph change up to `timeout`. The node must be spun.
    pub fn wait_for_graph_change_with_timeout(
        &mut self, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        Self::validate_timeout(timeout)?;
        let changed = self.wait_for_graph_change();
        self.with_timeout(changed, timeout)
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
    ///
    /// Relative and private names are expanded in this node's namespace without applying remaps.
    pub fn count_publishers(&self, topic_name: &str) -> Result<usize> {
        self.count_graph_entities(topic_name, false, |node, name, count| unsafe {
            rcl_count_publishers(node, name, count)
        })
    }

    /// Returns the number of subscriptions created for `topic_name`.
    ///
    /// Relative and private names are expanded in this node's namespace without applying remaps.
    pub fn count_subscribers(&self, topic_name: &str) -> Result<usize> {
        self.count_graph_entities(topic_name, false, |node, name, count| unsafe {
            rcl_count_subscribers(node, name, count)
        })
    }

    /// Returns the number of clients created for `service_name`.
    ///
    /// Relative and private names are expanded in this node's namespace without applying remaps.
    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble,
        r2r__ros__distro__iron
    )))]
    pub fn count_clients(&self, service_name: &str) -> Result<usize> {
        self.count_graph_entities(service_name, true, |node, name, count| unsafe {
            rcl_count_clients(node, name, count)
        })
    }

    /// Returns the number of servers created for `service_name`.
    ///
    /// Relative and private names are expanded in this node's namespace without applying remaps.
    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble,
        r2r__ros__distro__iron
    )))]
    pub fn count_services(&self, service_name: &str) -> Result<usize> {
        self.count_graph_entities(service_name, true, |node, name, count| unsafe {
            rcl_count_services(node, name, count)
        })
    }

    fn count_graph_entities(
        &self, name: &str, is_service: bool,
        query: impl FnOnce(*const rcl_node_t, *const std::os::raw::c_char, *mut usize) -> rcl_ret_t,
    ) -> Result<usize> {
        #[cfg(not(r2r__ros__distro__foxy))]
        let name = self.resolve_name(name, is_service, true)?;
        #[cfg(r2r__ros__distro__foxy)]
        let _ = is_service;
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

    /// Returns publisher endpoint information for a topic.
    ///
    /// When `no_mangle` is false, relative and private names are expanded in this node's
    /// namespace without applying remaps. When it is true, `topic_name` is passed through as a
    /// middleware topic name.
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
    ///
    /// When `no_mangle` is false, relative and private names are expanded in this node's
    /// namespace without applying remaps. When it is true, `topic_name` is passed through as a
    /// middleware topic name.
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
        &self, topic_name: &str, no_mangle: bool,
        query: impl FnOnce(
            *const rcl_node_t,
            *mut rcutils_allocator_t,
            *const std::os::raw::c_char,
            *mut rcl_topic_endpoint_info_array_t,
        ) -> rcl_ret_t,
    ) -> Result<Vec<TopicEndpointInfo>> {
        #[cfg(not(r2r__ros__distro__foxy))]
        let topic_name = if no_mangle {
            topic_name.to_owned()
        } else {
            self.resolve_topic_name_without_remapping(topic_name)?
        };
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
        self.create_wall_timer_with_options(period, TimerOptions::default())
    }

    /// Creates a steady-clock timer with explicit startup and buffering options.
    pub fn create_wall_timer_with_options(
        &mut self, period: Duration, options: TimerOptions,
    ) -> Result<Timer> {
        let options = options.validate()?;
        let mut clock = Clock::create(ClockType::SteadyTime)?;
        let timer_handle = self.create_timer_helper(&mut clock, period, options.autostart)?;

        let (tx, rx) = mpsc::channel(options.channel_capacity);

        let timer = Timer_ {
            timer_handle,
            _clock: Some(clock), // The timer owns the clock.
            sender: tx,
            id: uuid::Uuid::new_v4(),
            #[cfg(not(any(
                r2r__ros__distro__foxy,
                r2r__ros__distro__galactic,
                r2r__ros__distro__humble,
                r2r__ros__distro__iron
            )))]
            clock_type: ClockType::SteadyTime,
        };
        let id = timer.id;
        self.add_timer(timer);

        let out_timer = Timer {
            receiver: rx,
            id,
            clock_type: ClockType::SteadyTime,
            canceled_timers: Arc::clone(&self.canceled_timers),
        };

        Ok(out_timer)
    }

    /// Create a ROS timer
    ///
    /// Create a ROS timer that is woken up by spin every `period`.
    ///
    /// This timer uses node's [`ClockType::RosTime`] clock.
    pub fn create_timer(&mut self, period: Duration) -> Result<Timer> {
        self.create_timer_with_options(period, TimerOptions::default())
    }

    /// Creates a ROS-clock timer with explicit startup and buffering options.
    pub fn create_timer_with_options(
        &mut self, period: Duration, options: TimerOptions,
    ) -> Result<Timer> {
        let options = options.validate()?;
        let timer_handle = {
            let mut clock = self.ros_clock.lock();
            self.create_timer_helper(&mut clock, period, options.autostart)?
        };

        let (tx, rx) = mpsc::channel(options.channel_capacity);

        let timer = Timer_ {
            timer_handle,
            _clock: None, // The timer does not own the clock (the node owns it).
            sender: tx,
            id: uuid::Uuid::new_v4(),
            #[cfg(not(any(
                r2r__ros__distro__foxy,
                r2r__ros__distro__galactic,
                r2r__ros__distro__humble,
                r2r__ros__distro__iron
            )))]
            clock_type: ClockType::RosTime,
        };
        let id = timer.id;
        self.add_timer(timer);

        let out_timer = Timer {
            receiver: rx,
            id,
            clock_type: ClockType::RosTime,
            canceled_timers: Arc::clone(&self.canceled_timers),
        };

        Ok(out_timer)
    }

    /// Immediately destroys a timer owned by this node.
    ///
    /// Dropping a timer schedules it for removal during the next spin. This method removes and
    /// finalizes it before returning. Passing a timer created by another node leaves this node
    /// unchanged and schedules normal cleanup by the owning node.
    pub fn destroy_timer(&mut self, timer: Timer) {
        let id = timer.id;
        drop(timer);
        if Self::remove_timer(&mut self.timers, &mut self.timer_indexes, id).is_some() {
            self.canceled_timers.lock().retain(|queued| *queued != id);
        }
    }

    fn create_timer_helper(
        &self, clock: &mut Clock, period: Duration, autostart: bool,
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
                autostart,
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

    /// Returns the current time from this node's ROS clock.
    ///
    /// This follows system time until simulated time is enabled, then reflects the latest
    /// `/clock` update. A negative ROS time is rejected because [`Duration`] cannot represent it.
    pub fn now(&self) -> Result<Duration> {
        self.ros_clock.lock().get_now()
    }

    /// Returns the signed current ROS clock value in nanoseconds.
    ///
    /// Unlike [`Node::now`], this preserves simulated clock values before zero.
    pub fn now_ns(&self) -> Result<i64> {
        self.ros_clock.lock().get_now_ns()
    }

    fn timer(&self, id: uuid::Uuid) -> Result<&Timer_> {
        self.timer_indexes
            .get(&id)
            .and_then(|index| self.timers.get(*index))
            .ok_or(Error::RCL_RET_TIMER_INVALID)
    }

    fn timer_mut(&mut self, id: uuid::Uuid) -> Result<&mut Timer_> {
        self.timer_indexes
            .get(&id)
            .and_then(|index| self.timers.get_mut(*index))
            .ok_or(Error::RCL_RET_TIMER_INVALID)
    }

    #[cfg(test)]
    pub(crate) fn timer_count(&self) -> usize {
        self.timers.len()
    }
}

struct NamesAndTypes(rmw_names_and_types_t);

#[cfg(not(r2r__ros__distro__foxy))]
struct AllocatedCString {
    output: *mut std::os::raw::c_char,
    allocator: rcutils_allocator_t,
}

#[cfg(not(r2r__ros__distro__foxy))]
impl Drop for AllocatedCString {
    fn drop(&mut self) {
        if let Some(deallocate) = self.allocator.deallocate {
            unsafe { deallocate(self.output.cast(), self.allocator.state) };
        }
    }
}

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
    callback: rcl_timer_callback_t, allocator: rcl_allocator_t, autostart: bool,
) -> rcl_ret_t {
    let ret = rcl_timer_init(timer, clock, context, period, callback, allocator);
    if ret != RCL_RET_OK as i32 || autostart {
        return ret;
    }
    let cancel_ret = rcl_timer_cancel(timer);
    if cancel_ret != RCL_RET_OK as i32 {
        rcl_timer_fini(timer);
    }
    cancel_ret
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
    callback: rcl_timer_callback_t, allocator: rcl_allocator_t, autostart: bool,
) -> rcl_ret_t {
    rcl_timer_init2(timer, clock, context, period, callback, allocator, autostart)
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
    sender: mpsc::Sender<TimerTick>,
    id: uuid::Uuid,
    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble,
        r2r__ros__distro__iron
    )))]
    clock_type: ClockType,
}

struct TimerTick {
    elapsed: Duration,
    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble,
        r2r__ros__distro__iron
    )))]
    expected_call_time_ns: i64,
    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble,
        r2r__ros__distro__iron
    )))]
    actual_call_time_ns: i64,
    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble,
        r2r__ros__distro__iron
    )))]
    clock_type: ClockType,
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

        #[cfg(any(
            r2r__ros__distro__foxy,
            r2r__ros__distro__galactic,
            r2r__ros__distro__humble,
            r2r__ros__distro__iron
        ))]
        let tick = {
            let ret = unsafe { rcl_timer_call(self.get_handle_mut()) };
            check_rcl_ret(ret)?;
            TimerTick { elapsed }
        };
        #[cfg(not(any(
            r2r__ros__distro__foxy,
            r2r__ros__distro__galactic,
            r2r__ros__distro__humble,
            r2r__ros__distro__iron
        )))]
        let tick = {
            let mut call_info = std::mem::MaybeUninit::<rcl_timer_call_info_t>::uninit();
            let ret =
                unsafe { rcl_timer_call_with_info(self.get_handle_mut(), call_info.as_mut_ptr()) };
            check_rcl_ret(ret)?;
            let call_info = unsafe { call_info.assume_init() };
            TimerTick {
                elapsed,
                expected_call_time_ns: call_info.expected_call_time,
                actual_call_time_ns: call_info.actual_call_time,
                clock_type: self.clock_type,
            }
        };

        if let Err(e) = self.sender.try_send(tick) {
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
    receiver: mpsc::Receiver<TimerTick>,
    id: uuid::Uuid,
    clock_type: ClockType,
    canceled_timers: Arc<Mutex<VecDeque<uuid::Uuid>>>,
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.receiver.close();
        self.canceled_timers.lock().push_back(self.id);
    }
}

impl Timer {
    /// Returns the type of clock driving this timer.
    pub const fn clock_type(&self) -> ClockType {
        self.clock_type
    }

    /// Returns whether this timer uses a monotonic steady clock.
    pub const fn is_steady(&self) -> bool {
        matches!(self.clock_type, ClockType::SteadyTime)
    }

    /// Completes when the next instant in the interval has been reached.
    ///
    /// Returns the time passed since the timer was last woken up.
    pub async fn tick(&mut self) -> Result<Duration> {
        self.receiver
            .next()
            .await
            .map(|tick| tick.elapsed)
            .ok_or(Error::RCL_RET_TIMER_INVALID)
    }

    /// Completes with timing metadata for the next timer call.
    ///
    /// The expected and actual timestamps are captured atomically by RCL. The owning node must be
    /// spun while awaiting.
    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble,
        r2r__ros__distro__iron
    )))]
    pub async fn tick_with_info(&mut self) -> Result<TimerCallInfo> {
        self.receiver
            .next()
            .await
            .map(|tick| TimerCallInfo {
                elapsed: tick.elapsed,
                expected_call_time_ns: tick.expected_call_time_ns,
                actual_call_time_ns: tick.actual_call_time_ns,
                clock_type: tick.clock_type,
            })
            .ok_or(Error::RCL_RET_TIMER_INVALID)
    }

    /// Pauses the timer until [`Timer::reset`] is called.
    pub fn cancel(&self, node: &mut Node) -> Result<()> {
        let timer = node.timer_mut(self.id)?;
        let ret = unsafe { rcl_timer_cancel(timer.get_handle_mut()) };
        check_rcl_ret(ret)
    }

    /// Restarts the timer from the current clock time, also resuming a canceled timer.
    pub fn reset(&self, node: &mut Node) -> Result<()> {
        let timer = node.timer_mut(self.id)?;
        let ret = unsafe { rcl_timer_reset(timer.get_handle_mut()) };
        check_rcl_ret(ret)
    }

    /// Returns whether the timer is currently canceled.
    pub fn is_canceled(&self, node: &Node) -> Result<bool> {
        let timer = node.timer(self.id)?;
        let mut canceled = false;
        let ret = unsafe { rcl_timer_is_canceled(timer.get_handle(), &mut canceled) };
        check_rcl_ret(ret)?;
        Ok(canceled)
    }

    /// Returns whether the timer is ready to trigger now.
    pub fn is_ready(&self, node: &Node) -> Result<bool> {
        let timer = node.timer(self.id)?;
        let mut ready = false;
        let ret = unsafe { rcl_timer_is_ready(timer.get_handle(), &mut ready) };
        check_rcl_ret(ret)?;
        Ok(ready)
    }

    /// Returns the remaining time until the next trigger.
    ///
    /// A timer that is ready or overdue returns [`Duration::ZERO`]. A canceled timer returns
    /// [`Error::RCL_RET_TIMER_CANCELED`].
    pub fn time_until_next_call(&self, node: &Node) -> Result<Duration> {
        Ok(Duration::from_nanos(
            self.time_until_next_call_ns(node)?.try_into().unwrap_or(0),
        ))
    }

    /// Returns the signed number of nanoseconds until the next trigger.
    ///
    /// Unlike [`Timer::time_until_next_call`], this preserves negative values, which indicate
    /// how far an active timer is overdue. A canceled timer returns
    /// [`Error::RCL_RET_TIMER_CANCELED`].
    pub fn time_until_next_call_ns(&self, node: &Node) -> Result<i64> {
        let timer = node.timer(self.id)?;
        let mut nanos = 0;
        let ret = unsafe { rcl_timer_get_time_until_next_call(timer.get_handle(), &mut nanos) };
        check_rcl_ret(ret)?;
        Ok(nanos)
    }

    /// Returns the signed number of nanoseconds since the previous timer call.
    ///
    /// The value may be negative when a ROS clock moves backwards.
    pub fn time_since_last_call_ns(&self, node: &Node) -> Result<i64> {
        let timer = node.timer(self.id)?;
        let mut nanos = 0;
        let ret = unsafe { rcl_timer_get_time_since_last_call(timer.get_handle(), &mut nanos) };
        check_rcl_ret(ret)?;
        Ok(nanos)
    }

    /// Returns the absolute next call time in nanoseconds in the timer's clock domain.
    ///
    /// A canceled timer returns [`Error::RCL_RET_TIMER_CANCELED`].
    pub fn next_call_time_ns(&self, node: &Node) -> Result<i64> {
        let timer = node.timer(self.id)?;
        let mut nanos = 0;
        let ret = unsafe { rcl_timer_get_next_call_time(timer.get_handle(), &mut nanos) };
        check_rcl_ret(ret)?;
        Ok(nanos)
    }

    /// Returns the timer period.
    pub fn period(&self, node: &Node) -> Result<Duration> {
        let timer = node.timer(self.id)?;
        let mut nanos = 0;
        let ret = unsafe { rcl_timer_get_period(timer.get_handle(), &mut nanos) };
        check_rcl_ret(ret)?;
        duration_from_nonnegative_nanos(nanos)
    }

    /// Changes the timer period and returns its previous value.
    pub fn set_period(&self, node: &mut Node, period: Duration) -> Result<Duration> {
        let period = duration_to_rcl_nanos(period)?;
        let timer = node.timer_mut(self.id)?;
        let mut previous = 0;
        let ret = unsafe { rcl_timer_exchange_period(timer.get_handle(), period, &mut previous) };
        check_rcl_ret(ret)?;
        duration_from_nonnegative_nanos(previous)
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
            let mut client = c.lock();
            client.disable_maintenance_notifications();
            client.destroy(&mut self.node_handle);
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
    fn validate_availability_owner(&self, _node: &Node) -> Result<()> {
        Ok(())
    }

    fn availability_closed_error(&self) -> Error {
        Error::RCL_RET_CLIENT_INVALID
    }
}

/// Whether graph endpoint information describes a publisher or a subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopicEndpointType {
    Invalid,
    Publisher,
    Subscription,
}

/// A ROS interface type hash as defined by REP-2011.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TopicTypeHash {
    /// Hash format version. Zero means that no hash was supplied by the middleware.
    pub version: u8,
    /// SHA-256 bytes associated with `version`.
    pub value: [u8; 32],
}

impl TopicTypeHash {
    pub fn is_unset(&self) -> bool {
        self.version == 0
    }
}

impl std::fmt::Display for TopicTypeHash {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "RIHS{:02x}_", self.version)?;
        for byte in self.value {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(not(any(
    r2r__ros__distro__foxy,
    r2r__ros__distro__galactic,
    r2r__ros__distro__humble
)))]
fn topic_type_hash_from_native(value: &rosidl_type_hash_t) -> TopicTypeHash {
    TopicTypeHash {
        version: value.version,
        value: value.value,
    }
}

impl TryFrom<rmw_endpoint_type_t> for TopicEndpointType {
    type Error = Error;

    fn try_from(value: rmw_endpoint_type_t) -> Result<Self> {
        match value {
            rmw_endpoint_type_t::RMW_ENDPOINT_INVALID => Ok(Self::Invalid),
            rmw_endpoint_type_t::RMW_ENDPOINT_PUBLISHER => Ok(Self::Publisher),
            rmw_endpoint_type_t::RMW_ENDPOINT_SUBSCRIPTION => Ok(Self::Subscription),
        }
    }
}

/// Metadata for one publisher or subscription discovered in the ROS graph.
#[derive(Debug, Clone, PartialEq)]
pub struct TopicEndpointInfo {
    pub node_name: String,
    pub node_namespace: String,
    pub topic_type: String,
    /// Topic schema hash when supported by the ROS distribution (Iron and newer).
    pub topic_type_hash: Option<TopicTypeHash>,
    pub endpoint_type: TopicEndpointType,
    pub endpoint_gid: [u8; RMW_GID_STORAGE_SIZE as usize],
    pub qos_profile: QosProfile,
}

impl TopicEndpointInfo {
    fn try_from_native(info: rmw_topic_endpoint_info_t) -> Result<Self> {
        Ok(Self {
            node_name: c_string_from_ptr(info.node_name)?,
            node_namespace: c_string_from_ptr(info.node_namespace)?,
            topic_type: c_string_from_ptr(info.topic_type)?,
            #[cfg(not(any(
                r2r__ros__distro__foxy,
                r2r__ros__distro__galactic,
                r2r__ros__distro__humble
            )))]
            topic_type_hash: Some(topic_type_hash_from_native(&info.topic_type_hash)),
            #[cfg(any(
                r2r__ros__distro__foxy,
                r2r__ros__distro__galactic,
                r2r__ros__distro__humble
            ))]
            topic_type_hash: None,
            endpoint_type: info.endpoint_type.try_into()?,
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
    use crate::{ActionGoalOptions, GoalStatus};
    use futures::{executor::LocalPool, task::LocalSpawnExt};
    use std::task::Poll;

    #[test]
    fn local_parameter_event_bursts_do_not_drop_events_at_the_old_capacity() {
        let (sender, mut receiver) = mpsc::unbounded();
        let events = (0..32)
            .map(|value| (format!("parameter_{value}"), ParameterValue::Integer(value)))
            .collect::<Vec<_>>();

        deliver_local_parameter_events(&sender, events.clone());

        for expected in events {
            assert_eq!(receiver.next().now_or_never().flatten(), Some(expected));
        }
        assert!(receiver.next().now_or_never().is_none());
    }

    #[derive(Debug)]
    struct LocalSetParams {
        count: i64,
        ratio: f64,
    }

    struct FailingReadParams {
        fail_reads: Arc<std::sync::atomic::AtomicBool>,
    }

    impl RosParams for FailingReadParams {
        fn register_parameters(
            &mut self, _prefix: &str, _parameter: Option<Parameter>,
            parameters: &mut IndexMap<String, Parameter>,
        ) -> Result<()> {
            parameters.insert("provider_value".into(), Parameter::new(7_i64));
            Ok(())
        }

        fn get_parameter(&mut self, name: &str) -> Result<ParameterValue> {
            if name != "provider_value" {
                return Err(Error::InvalidParameterName { name: name.into() });
            }
            if self.fail_reads.load(std::sync::atomic::Ordering::Acquire) {
                return Err(Error::ParameterValueConv {
                    name: name.into(),
                    msg: "injected provider read failure".into(),
                });
            }
            Ok(ParameterValue::Integer(7))
        }

        fn set_parameter(&mut self, name: &str, value: &ParameterValue) -> Result<()> {
            self.check_parameter(name, value)
        }

        fn check_parameter(&self, name: &str, value: &ParameterValue) -> Result<()> {
            match (name, value) {
                ("provider_value", ParameterValue::Integer(_)) => Ok(()),
                ("provider_value", _) => Err(Error::InvalidParameterType {
                    name: name.into(),
                    ty: "integer",
                }),
                _ => Err(Error::InvalidParameterName { name: name.into() }),
            }
        }
    }

    struct DerivedOverrideParams {
        count: i64,
    }

    struct HandlerSetupParams {
        registration_calls: usize,
        insert_invalid_name: bool,
    }

    impl RosParams for HandlerSetupParams {
        fn register_parameters(
            &mut self, _prefix: &str, _parameter: Option<Parameter>,
            parameters: &mut IndexMap<String, Parameter>,
        ) -> Result<()> {
            self.registration_calls += 1;
            let name = if self.insert_invalid_name {
                ""
            } else {
                "tracked"
            };
            parameters.insert(name.into(), Parameter::new(11_i64));
            Ok(())
        }

        fn get_parameter(&mut self, name: &str) -> Result<ParameterValue> {
            match name {
                "tracked" => Ok(ParameterValue::Integer(11)),
                _ => Err(Error::InvalidParameterName { name: name.into() }),
            }
        }

        fn set_parameter(&mut self, name: &str, value: &ParameterValue) -> Result<()> {
            self.check_parameter(name, value)
        }

        fn check_parameter(&self, name: &str, value: &ParameterValue) -> Result<()> {
            match (name, value) {
                ("tracked", ParameterValue::Integer(_)) => Ok(()),
                ("tracked", _) => Err(Error::InvalidParameterType {
                    name: name.into(),
                    ty: "integer",
                }),
                _ => Err(Error::InvalidParameterName { name: name.into() }),
            }
        }
    }

    impl RosParams for DerivedOverrideParams {
        fn register_parameters(
            &mut self, _prefix: &str, _parameter: Option<Parameter>,
            parameters: &mut IndexMap<String, Parameter>,
        ) -> Result<()> {
            self.count
                .register_parameters("count", Some(Parameter::empty()), parameters)
        }

        fn get_parameter(&mut self, name: &str) -> Result<ParameterValue> {
            match name {
                "count" => Ok(ParameterValue::Integer(self.count)),
                _ => Err(Error::InvalidParameterName { name: name.into() }),
            }
        }

        fn set_parameter(&mut self, name: &str, value: &ParameterValue) -> Result<()> {
            match (name, value) {
                ("count", ParameterValue::Integer(value)) => {
                    self.count = *value;
                    Ok(())
                }
                ("count", _) => Err(Error::InvalidParameterType {
                    name: name.into(),
                    ty: "integer",
                }),
                _ => Err(Error::InvalidParameterName { name: name.into() }),
            }
        }

        fn check_parameter(&self, name: &str, value: &ParameterValue) -> Result<()> {
            match (name, value) {
                ("count", ParameterValue::Integer(_)) => Ok(()),
                ("count", _) => Err(Error::InvalidParameterType {
                    name: name.into(),
                    ty: "integer",
                }),
                _ => Err(Error::InvalidParameterName { name: name.into() }),
            }
        }
    }

    impl RosParams for LocalSetParams {
        fn register_parameters(
            &mut self, prefix: &str, _parameter: Option<Parameter>,
            parameters: &mut IndexMap<String, Parameter>,
        ) -> Result<()> {
            let prefix = if prefix.is_empty() { "" } else { prefix };
            parameters.insert(
                format!("{prefix}count"),
                Parameter::new(ParameterValue::Integer(self.count)),
            );
            parameters.insert(
                format!("{prefix}ratio"),
                Parameter::new(ParameterValue::Double(self.ratio)),
            );
            Ok(())
        }

        fn get_parameter(&mut self, name: &str) -> Result<ParameterValue> {
            match name {
                "count" => Ok(ParameterValue::Integer(self.count)),
                "ratio" => Ok(ParameterValue::Double(self.ratio)),
                _ => Err(Error::InvalidParameterName { name: name.into() }),
            }
        }

        fn set_parameter(&mut self, name: &str, value: &ParameterValue) -> Result<()> {
            self.check_parameter(name, value)?;
            match (name, value) {
                ("count", ParameterValue::Integer(value)) => self.count = *value,
                ("ratio", ParameterValue::Double(value)) => self.ratio = *value,
                _ => unreachable!("check_parameter accepted an invalid update"),
            }
            Ok(())
        }

        fn check_parameter(&self, name: &str, value: &ParameterValue) -> Result<()> {
            match (name, value) {
                ("count", ParameterValue::Integer(_)) | ("ratio", ParameterValue::Double(_)) => {
                    Ok(())
                }
                ("count", _) => Err(Error::InvalidParameterType {
                    name: name.into(),
                    ty: "integer",
                }),
                ("ratio", _) => Err(Error::InvalidParameterType {
                    name: name.into(),
                    ty: "double",
                }),
                _ => Err(Error::InvalidParameterName { name: name.into() }),
            }
        }
    }

    #[test]
    fn converts_rcl_durations_without_wrapping() {
        assert_eq!(duration_to_rcl_nanos(Duration::from_nanos(i64::MAX as u64)).unwrap(), i64::MAX);
        assert!(duration_to_rcl_nanos(Duration::from_nanos(i64::MAX as u64 + 1)).is_err());
        assert_eq!(duration_from_nonnegative_nanos(42).unwrap(), Duration::from_nanos(42));
        assert!(duration_from_nonnegative_nanos(-1).is_err());
    }

    #[test]
    fn absent_parameter_unset_is_not_reported_as_a_change() {
        assert!(!parameter_value_changed(None, &ParameterValue::NotSet));
        assert!(parameter_value_changed(None, &ParameterValue::Integer(1)));
        assert!(parameter_value_changed(
            Some(&ParameterValue::Integer(1)),
            &ParameterValue::NotSet
        ));
        assert!(!parameter_value_changed(
            Some(&ParameterValue::Integer(1)),
            &ParameterValue::Integer(1)
        ));
    }

    #[test]
    fn parameter_prefix_depth_is_relative_to_the_prefix() {
        let prefix = vec!["robot".to_owned()];
        assert!(parameter_name_matches("robot", &prefix, 1));
        assert!(parameter_name_matches("robot.speed", &prefix, 1));
        assert!(!parameter_name_matches("robot.motor.speed", &prefix, 1));
        assert!(parameter_name_matches("robot.motor.speed", &prefix, 2));
        assert!(parameter_name_matches("robot.motor.speed", &prefix, 0));
        assert!(!parameter_name_matches("robotic.speed", &prefix, 0));

        assert!(parameter_name_matches("top", &[], 1));
        assert!(!parameter_name_matches("top.child", &[], 1));
        assert!(parameter_name_matches("top.child", &[], 0));
    }

    #[test]
    fn parameter_node_rules_match_ros_wildcard_segments() {
        for (rule, name) in [
            ("/**", "/node"),
            ("/**", "/a/b/node"),
            ("/*", "/node"),
            ("/ns/*", "/ns/node"),
            ("/**/foo/*/bar", "/a/b/foo/c/bar"),
            ("node", "/node"),
            ("ns/node", "/ns/node"),
        ] {
            assert!(parameter_rule_matches_node(rule, name), "{rule} should match {name}");
        }
        for (rule, name) in [
            ("/*", "/ns/node"),
            ("/ns/*", "/ns/a/node"),
            ("/**/foo/*/bar", "/a/b/foo/bar"),
            ("node", "/ns/node"),
            ("/node", "/ns/node"),
        ] {
            assert!(!parameter_rule_matches_node(rule, name), "{rule} should not match {name}");
        }
    }

    #[test]
    fn parameter_prefixes_are_deduplicated_in_first_seen_order() {
        let params = IndexMap::from([
            ("robot.motor.speed".to_owned(), Parameter::new(ParameterValue::Integer(1))),
            ("robot.motor.torque".to_owned(), Parameter::new(ParameterValue::Integer(2))),
            ("camera.exposure".to_owned(), Parameter::new(ParameterValue::Integer(3))),
            ("robot.sensor.range".to_owned(), Parameter::new(ParameterValue::Integer(4))),
        ]);

        let result = list_parameter_values(&params, &[], 0);

        assert_eq!(result.prefixes, vec!["robot.motor", "camera", "robot.sensor"]);
        assert_eq!(result.names, params.keys().cloned().collect::<Vec<_>>());
    }

    #[test]
    fn duplicate_parameter_updates_keep_first_order_and_last_value() {
        let updates = vec![
            ("first".into(), ParameterValue::Integer(1)),
            ("second".into(), ParameterValue::Integer(2)),
            ("first".into(), ParameterValue::Integer(3)),
        ];

        assert_eq!(
            normalize_parameter_updates(updates),
            [
                ("first".into(), ParameterValue::Integer(3)),
                ("second".into(), ParameterValue::Integer(2)),
            ]
        );
    }

    #[test]
    fn parameter_read_snapshots_release_their_locks() {
        let params = Arc::new(Mutex::new(IndexMap::from([(
            "robot.speed".to_owned(),
            Parameter::new(ParameterValue::Integer(4)),
        )])));
        let descriptors = Arc::new(Mutex::new(IndexMap::from([(
            "robot.speed".to_owned(),
            ParameterDescriptor {
                name: "robot.speed".into(),
                description: "wheel speed".into(),
                ..Default::default()
            },
        )])));
        let names = vec!["robot.speed".to_owned()];

        assert_eq!(read_parameter_types(&names, &params), vec![ParameterType::Integer]);
        assert_eq!(read_list_parameter_values(&params, &["robot".into()], 1).names, names);
        assert_eq!(
            read_parameter_descriptors(&names, &params, &descriptors)[0].description,
            "wheel speed"
        );

        assert!(params.try_lock().is_some());
        assert!(descriptors.try_lock().is_some());
    }

    #[test]
    fn parameter_override_snapshot_releases_the_context_lock() {
        let context = Context::create().unwrap();

        let overrides = parameter_overrides(&context.context_handle).unwrap();

        assert!(context.context_handle.try_lock().is_some());
        drop(overrides);
    }

    #[test]
    fn list_parameters_service_uses_relative_prefix_depth() {
        use rcl_interfaces::srv::ListParameters;

        fn drive(
            node: &mut Node, pool: &mut LocalPool,
            future: impl Future<Output = Result<ListParameters::Response>>,
        ) -> ListParameters::Response {
            let mut future = Box::pin(future);
            loop {
                node.try_spin_once(Duration::from_millis(10)).unwrap();
                pool.run_until_stalled();
                if let Some(result) = future.as_mut().now_or_never() {
                    return result.unwrap();
                }
            }
        }

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "list_parameter_depth_test", "").unwrap();
        {
            let mut params = node.params.lock();
            for name in ["robot", "robot.speed", "robot.motor.speed", "robotic.speed"] {
                params.insert(name.into(), Parameter::new(ParameterValue::Integer(1)));
            }
        }
        let (handler, _events) = node.make_parameter_handler().unwrap();
        let client = node
            .create_client::<ListParameters::Service>(
                "list_parameter_depth_test/list_parameters",
                QosProfile::default(),
            )
            .unwrap();
        let mut pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();

        let request = |depth| ListParameters::Request {
            prefixes: vec!["robot".into()],
            depth,
        };
        let depth_one = drive(&mut node, &mut pool, client.request(&request(1)).unwrap());
        assert_eq!(depth_one.result.names, vec!["robot", "robot.speed"]);

        let depth_two = drive(&mut node, &mut pool, client.request(&request(2)).unwrap());
        assert_eq!(depth_two.result.names, vec!["robot", "robot.speed", "robot.motor.speed"]);
    }

    #[test]
    fn local_parameter_introspection_matches_services() {
        use rcl_interfaces::srv::{
            DescribeParameters, GetParameterTypes, GetParameters, ListParameters,
        };

        fn drive<T>(
            node: &mut Node, pool: &mut LocalPool, future: impl Future<Output = Result<T>>,
        ) -> T {
            let mut future = Box::pin(future);
            loop {
                node.try_spin_once(Duration::from_millis(10)).unwrap();
                pool.run_until_stalled();
                if let Some(result) = future.as_mut().now_or_never() {
                    return result.unwrap();
                }
            }
        }

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "local_parameter_introspection_test", "").unwrap();
        node.declare_parameter_with_descriptor(
            "robot.speed",
            ParameterValue::Integer(4),
            ParameterDescriptor {
                description: "wheel speed".into(),
                integer_range: vec![IntegerRange {
                    from_value: 0,
                    to_value: 10,
                    step: 2,
                }],
                ..Default::default()
            },
        )
        .unwrap();
        node.declare_parameter("robot.motor.speed", ParameterValue::Integer(6))
            .unwrap();
        node.declare_parameter("label", ParameterValue::String("drive".into()))
            .unwrap();

        let names = vec!["robot.speed".to_owned(), "missing".to_owned()];
        let local_values = node.get_parameters(&names);
        let local_types = node.get_parameter_types(&names);
        let local_descriptors = node.describe_parameters(&names);
        let local_list = node.list_parameters(["robot"], 1);
        let local_prefix = node.get_parameters_by_prefix("robot");
        assert_eq!(local_values, vec![ParameterValue::Integer(4), ParameterValue::NotSet]);
        assert_eq!(node.get_parameter_or("missing", 99_i64).unwrap(), 99);
        assert!(node.get_parameter_or("label", 99_i64).is_err());
        assert_eq!(
            local_prefix.keys().map(String::as_str).collect::<Vec<_>>(),
            vec!["speed", "motor.speed"]
        );
        assert_eq!(
            local_prefix.values().cloned().collect::<Vec<_>>(),
            vec![ParameterValue::Integer(4), ParameterValue::Integer(6)]
        );

        let (handler, _events) = node.make_parameter_handler().unwrap();
        let get = node
            .create_client::<GetParameters::Service>(
                "local_parameter_introspection_test/get_parameters",
                QosProfile::default(),
            )
            .unwrap();
        let get_types = node
            .create_client::<GetParameterTypes::Service>(
                "local_parameter_introspection_test/get_parameter_types",
                QosProfile::default(),
            )
            .unwrap();
        let describe = node
            .create_client::<DescribeParameters::Service>(
                "local_parameter_introspection_test/describe_parameters",
                QosProfile::default(),
            )
            .unwrap();
        let list = node
            .create_client::<ListParameters::Service>(
                "local_parameter_introspection_test/list_parameters",
                QosProfile::default(),
            )
            .unwrap();
        let mut pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();

        let remote_values = drive(
            &mut node,
            &mut pool,
            get.request(&GetParameters::Request {
                names: names.clone(),
            })
            .unwrap(),
        )
        .values
        .into_iter()
        .map(ParameterValue::try_from_parameter_value_msg)
        .collect::<Result<Vec<_>>>()
        .unwrap();
        let remote_types = drive(
            &mut node,
            &mut pool,
            get_types
                .request(&GetParameterTypes::Request {
                    names: names.clone(),
                })
                .unwrap(),
        )
        .types
        .into_iter()
        .map(ParameterType::try_from)
        .collect::<Result<Vec<_>>>()
        .unwrap();
        let remote_descriptors = drive(
            &mut node,
            &mut pool,
            describe
                .request(&DescribeParameters::Request {
                    names: names.clone(),
                })
                .unwrap(),
        )
        .descriptors;
        let remote_list = drive(
            &mut node,
            &mut pool,
            list.request(&ListParameters::Request {
                prefixes: vec!["robot".into()],
                depth: 1,
            })
            .unwrap(),
        )
        .result;

        assert_eq!(remote_values, local_values);
        assert_eq!(remote_types, local_types);
        assert_eq!(remote_descriptors, local_descriptors);
        assert_eq!(remote_list, local_list);
    }

    #[test]
    fn local_parameter_updates_share_service_validation_and_events() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "local_parameter_update_test", "").unwrap();
        let provider = Arc::new(Mutex::new(LocalSetParams {
            count: 2,
            ratio: 0.2,
        }));
        let provider_for_handler: Arc<Mutex<dyn RosParams + Send>> = provider.clone();
        let (handler, mut local_events) = node
            .make_derived_parameter_handler(provider_for_handler)
            .unwrap();
        node.set_parameter_descriptor(
            "count",
            ParameterDescriptor {
                integer_range: vec![IntegerRange {
                    from_value: 0,
                    to_value: 10,
                    step: 2,
                }],
                ..Default::default()
            },
        )
        .unwrap();
        let mut ros_events = node
            .subscribe_parameter_events(
                Some("/local_parameter_update_test".into()),
                Some("count".into()),
            )
            .unwrap();
        let pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();
        for _ in 0..100 {
            if node
                .get_subscriptions_info_by_topic("/parameter_events", false)
                .unwrap()
                .iter()
                .any(|endpoint| endpoint.node_name == "local_parameter_update_test")
            {
                break;
            }
            node.try_spin_once(Duration::from_millis(10)).unwrap();
        }

        node.set_parameter("count", ParameterValue::Integer(4))
            .unwrap();
        assert_eq!(provider.lock().count, 4);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 4);
        assert_eq!(
            local_events.next().now_or_never().flatten(),
            Some(("count".into(), ParameterValue::Integer(4)))
        );
        let event = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(event) = ros_events.next().now_or_never().flatten() {
                break event;
            }
        };
        assert_eq!(event.kind, ParameterEventKind::Changed);
        assert_eq!(event.value, ParameterValue::Integer(4));

        let results = node.set_parameters([
            ("count", ParameterValue::Integer(3)),
            ("ratio", ParameterValue::Double(0.4)),
        ]);
        assert!(results[0].is_err());
        assert!(results[1].is_ok());
        assert_eq!(provider.lock().count, 4);
        assert_eq!(provider.lock().ratio, 0.4);

        assert!(node
            .set_parameters_atomically([
                ("count", ParameterValue::Integer(6)),
                ("ratio", ParameterValue::String("wrong".into())),
            ])
            .is_err());
        assert_eq!(provider.lock().count, 4);
        assert_eq!(provider.lock().ratio, 0.4);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 4);
        assert_eq!(node.get_parameter::<f64>("ratio").unwrap(), 0.4);

        node.set_parameters_atomically([
            ("count", ParameterValue::Integer(6)),
            ("ratio", ParameterValue::Double(0.6)),
        ])
        .unwrap();
        assert_eq!(provider.lock().count, 6);
        assert_eq!(provider.lock().ratio, 0.6);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 6);
        assert_eq!(node.get_parameter::<f64>("ratio").unwrap(), 0.6);
    }

    #[test]
    fn derived_and_runtime_parameters_can_be_updated_together() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "mixed_parameter_provider_test", "").unwrap();
        let provider = Arc::new(Mutex::new(LocalSetParams {
            count: 2,
            ratio: 0.2,
        }));
        let provider_for_handler: Arc<Mutex<dyn RosParams + Send>> = provider.clone();
        let (_handler, mut events) = node
            .make_derived_parameter_handler_with_options(
                provider_for_handler,
                ParameterHandlerOptions::default()
                    .start_parameter_services(false)
                    .start_parameter_event_publisher(false),
            )
            .unwrap();
        node.declare_parameter("runtime", 1_i64).unwrap();
        assert_eq!(
            events.next().now_or_never().flatten(),
            Some(("runtime".into(), ParameterValue::Integer(1)))
        );

        node.set_parameters_atomically([
            ("count", ParameterValue::Integer(4)),
            ("runtime", ParameterValue::Integer(3)),
        ])
        .unwrap();

        assert_eq!(provider.lock().count, 4);
        assert_eq!(node.get_parameter::<i64>("runtime").unwrap(), 3);
        assert_eq!(
            events.next().now_or_never().flatten(),
            Some(("count".into(), ParameterValue::Integer(4)))
        );
        assert_eq!(
            events.next().now_or_never().flatten(),
            Some(("runtime".into(), ParameterValue::Integer(3)))
        );
    }

    #[test]
    fn parameter_validation_callbacks_are_ordered_scoped_and_shared_with_services() {
        use rcl_interfaces::{msg, srv::SetParameters};

        fn drive<T>(
            node: &mut Node, pool: &mut LocalPool, future: impl Future<Output = Result<T>>,
        ) -> T {
            let mut future = Box::pin(future);
            loop {
                node.try_spin_once(Duration::from_millis(10)).unwrap();
                pool.run_until_stalled();
                if let Some(result) = future.as_mut().now_or_never() {
                    return result.unwrap();
                }
            }
        }

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "parameter_callback_test", "").unwrap();
        node.params
            .lock()
            .insert("count".into(), Parameter::new(ParameterValue::Integer(2)));
        let (handler, mut events) = node.make_parameter_handler().unwrap();
        let client = node
            .create_client::<SetParameters::Service>(
                "parameter_callback_test/set_parameters",
                QosProfile::default(),
            )
            .unwrap();
        let mut pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();

        let calls = Arc::new(Mutex::new(Vec::new()));
        let first_calls = calls.clone();
        let first = node.add_on_set_parameters_callback(move |_| {
            first_calls.lock().push("first");
            Ok(())
        });
        let second_calls = calls.clone();
        let second = node.add_on_set_parameters_callback(move |updates| {
            second_calls.lock().push("second");
            if updates
                .iter()
                .any(|(name, value)| name == "count" && value == &ParameterValue::Integer(3))
            {
                Err(Error::ParameterConstraintViolation {
                    name: "count".into(),
                    reason: "three is reserved".into(),
                })
            } else {
                Ok(())
            }
        });

        assert!(node
            .set_parameter("count", ParameterValue::Integer(3))
            .is_err());
        assert_eq!(*calls.lock(), vec!["second"]);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 2);
        assert!(events.next().now_or_never().is_none());

        calls.lock().clear();
        node.set_parameter("count", ParameterValue::Integer(4))
            .unwrap();
        assert_eq!(*calls.lock(), vec!["second", "first"]);
        assert_eq!(
            events.next().now_or_never().flatten(),
            Some(("count".into(), ParameterValue::Integer(4)))
        );

        drop(second);
        calls.lock().clear();
        let response = drive(
            &mut node,
            &mut pool,
            client
                .request(&SetParameters::Request {
                    parameters: vec![msg::Parameter {
                        name: "count".into(),
                        value: ParameterValue::Integer(6).into_parameter_value_msg(),
                    }],
                })
                .unwrap(),
        );
        assert!(response.results[0].successful);
        assert_eq!(*calls.lock(), vec!["first"]);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 6);

        calls.lock().clear();
        assert!(node
            .declare_parameter("declared_by_callback", ParameterValue::Integer(5))
            .is_ok());
        assert_eq!(*calls.lock(), vec!["first"]);
        let reject_declaration = node.add_on_set_parameters_callback(|updates| {
            if updates
                .iter()
                .any(|(name, _)| name == "rejected_declaration")
            {
                Err(Error::ParameterConstraintViolation {
                    name: "rejected_declaration".into(),
                    reason: "declaration rejected".into(),
                })
            } else {
                Ok(())
            }
        });
        assert!(node
            .declare_parameter("rejected_declaration", ParameterValue::Integer(1))
            .is_err());
        assert!(!node.has_parameter("rejected_declaration"));
        drop(reject_declaration);

        drop(first);
        calls.lock().clear();
        node.set_parameter("count", ParameterValue::Integer(8))
            .unwrap();
        assert!(calls.lock().is_empty());
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 8);
    }

    #[test]
    fn parameter_callback_registry_removes_by_token_and_preserves_reverse_order() {
        let mut callbacks = ParameterCallbackRegistry::<usize>::default();
        let first = callbacks.insert(Arc::new(1));
        let middle = callbacks.insert(Arc::new(2));
        let last = callbacks.insert(Arc::new(3));

        callbacks.remove(middle);

        assert_eq!(first, 0);
        assert_eq!(last, 2);
        assert_eq!(
            callbacks
                .reverse_snapshot()
                .into_iter()
                .map(|value| *value)
                .collect::<Vec<_>>(),
            [3, 1]
        );
    }

    #[test]
    fn parameter_transaction_guard_rejects_reentry_until_the_operation_finishes() {
        let transaction = Arc::new(Mutex::new(()));
        let mut outer = lock_parameter_transaction(&transaction).unwrap();

        assert!(matches!(
            lock_parameter_transaction(&transaction),
            Err(Error::RecursiveParameterMutation)
        ));
        outer.unlock();
        assert!(matches!(
            lock_parameter_transaction(&transaction),
            Err(Error::RecursiveParameterMutation)
        ));
        drop(outer);

        assert!(lock_parameter_transaction(&transaction).is_ok());
    }

    #[test]
    fn parameter_callback_observes_recursive_mutation_error_without_deadlocking() {
        let context = Context::create().unwrap();
        let node = Node::create(context, "recursive_parameter_callback_test", "").unwrap();
        node.declare_parameter("count", 1_i64).unwrap();
        let node_ptr = (&node as *const Node) as usize;
        let observed = Arc::new(Mutex::new(Vec::new()));
        let validation_observed = Arc::clone(&observed);
        let _callback = node.add_on_set_parameters_callback(move |_| {
            let node = unsafe { &*(node_ptr as *const Node) };
            if matches!(node.set_parameter("nested", 1_i64), Err(Error::RecursiveParameterMutation))
            {
                validation_observed.lock().push("validation");
            }
            Ok(())
        });
        let post_observed = Arc::clone(&observed);
        let _post = node.add_post_set_parameters_callback(move |_| {
            let node = unsafe { &*(node_ptr as *const Node) };
            if matches!(node.set_parameter("nested", 1_i64), Err(Error::RecursiveParameterMutation))
            {
                post_observed.lock().push("post");
            }
        });

        node.set_parameter("count", 2_i64).unwrap();

        assert_eq!(*observed.lock(), ["validation", "post"]);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 2);
        assert!(!node.has_parameter("nested"));
        node.set_parameter("count", 3_i64).unwrap();
    }

    #[test]
    fn post_set_callbacks_observe_only_successful_committed_batches() {
        use rcl_interfaces::{msg, srv::SetParametersAtomically};

        fn drive<T>(
            node: &mut Node, pool: &mut LocalPool, future: impl Future<Output = Result<T>>,
        ) -> T {
            let mut future = Box::pin(future);
            loop {
                node.try_spin_once(Duration::from_millis(10)).unwrap();
                pool.run_until_stalled();
                if let Some(result) = future.as_mut().now_or_never() {
                    return result.unwrap();
                }
            }
        }

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "post_parameter_callback_test", "").unwrap();
        let observed = Arc::new(Mutex::new(Vec::new()));
        let first_observed = observed.clone();
        let first = node.add_post_set_parameters_callback(move |updates| {
            first_observed.lock().push(("first", updates.to_vec()));
        });
        let second_observed = observed.clone();
        let second = node.add_post_set_parameters_callback(move |updates| {
            second_observed.lock().push(("second", updates.to_vec()));
        });

        node.declare_parameter("count", ParameterValue::Integer(1))
            .unwrap();
        assert_eq!(
            observed.lock().as_slice(),
            &[
                ("second", vec![("count".into(), ParameterValue::Integer(1))]),
                ("first", vec![("count".into(), ParameterValue::Integer(1))]),
            ]
        );

        let reject = node.add_on_set_parameters_callback(|updates| {
            if updates
                .iter()
                .any(|(_, value)| value == &ParameterValue::Integer(3))
            {
                Err(Error::ParameterConstraintViolation {
                    name: "count".into(),
                    reason: "rejected".into(),
                })
            } else {
                Ok(())
            }
        });
        observed.lock().clear();
        assert!(node
            .set_parameter("count", ParameterValue::Integer(3))
            .is_err());
        assert!(observed.lock().is_empty());

        node.set_parameters_atomically([
            ("count", ParameterValue::Integer(2)),
            ("label", ParameterValue::String("ok".into())),
        ])
        .unwrap();
        assert_eq!(observed.lock().len(), 2);
        assert_eq!(observed.lock()[0].0, "second");
        assert_eq!(observed.lock()[0].1.len(), 2);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 2);

        let (handler, _events) = node.make_parameter_handler().unwrap();
        let client = node
            .create_client::<SetParametersAtomically::Service>(
                "post_parameter_callback_test/set_parameters_atomically",
                QosProfile::default(),
            )
            .unwrap();
        let mut pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();
        observed.lock().clear();
        let response = drive(
            &mut node,
            &mut pool,
            client
                .request(&SetParametersAtomically::Request {
                    parameters: vec![msg::Parameter {
                        name: "count".into(),
                        value: ParameterValue::Integer(4).into_parameter_value_msg(),
                    }],
                })
                .unwrap(),
        );
        assert!(response.result.successful);
        assert_eq!(observed.lock().len(), 2);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 4);

        drop(second);
        observed.lock().clear();
        node.set_parameter("count", ParameterValue::Integer(6))
            .unwrap();
        assert_eq!(
            observed.lock().as_slice(),
            &[("first", vec![("count".into(), ParameterValue::Integer(6))])]
        );
        drop(first);
        drop(reject);
    }

    #[test]
    fn atomic_parameter_updates_expose_only_the_last_duplicate_value() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "duplicate_parameter_update_test", "").unwrap();
        let (handler, mut events) = node.make_parameter_handler().unwrap();
        let pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();
        let validated = Arc::new(Mutex::new(Vec::new()));
        let validated_by_callback = Arc::clone(&validated);
        let _validation = node.add_on_set_parameters_callback(move |updates| {
            *validated_by_callback.lock() = updates.to_vec();
            Ok(())
        });
        let posted = Arc::new(Mutex::new(Vec::new()));
        let posted_by_callback = Arc::clone(&posted);
        let _post = node.add_post_set_parameters_callback(move |updates| {
            *posted_by_callback.lock() = updates.to_vec();
        });

        node.set_parameters_atomically([
            ("value", ParameterValue::Integer(1)),
            ("other", ParameterValue::Integer(2)),
            ("value", ParameterValue::Integer(3)),
        ])
        .unwrap();

        let expected = vec![
            ("value".into(), ParameterValue::Integer(3)),
            ("other".into(), ParameterValue::Integer(2)),
        ];
        assert_eq!(*validated.lock(), expected);
        assert_eq!(*posted.lock(), expected);
        assert_eq!(node.get_parameter::<i64>("value").unwrap(), 3);
        assert_eq!(events.next().now_or_never().flatten(), Some(expected[0].clone()));
        assert_eq!(events.next().now_or_never().flatten(), Some(expected[1].clone()));
        assert!(events.next().now_or_never().is_none());
    }

    #[test]
    fn pre_set_callbacks_transform_the_final_transaction_batch() {
        use rcl_interfaces::{msg, srv::SetParametersAtomically};

        fn drive<T>(
            node: &mut Node, pool: &mut LocalPool, future: impl Future<Output = Result<T>>,
        ) -> T {
            let mut future = Box::pin(future);
            loop {
                node.try_spin_once(Duration::from_millis(10)).unwrap();
                pool.run_until_stalled();
                if let Some(result) = future.as_mut().now_or_never() {
                    return result.unwrap();
                }
            }
        }

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "pre_parameter_callback_test", "").unwrap();
        node.params
            .lock()
            .insert("source".into(), Parameter::new(ParameterValue::Integer(0)));
        node.params
            .lock()
            .insert("derived".into(), Parameter::new(ParameterValue::Integer(0)));
        let (handler, mut events) = node.make_parameter_handler().unwrap();
        let client = node
            .create_client::<SetParametersAtomically::Service>(
                "pre_parameter_callback_test/set_parameters_atomically",
                QosProfile::default(),
            )
            .unwrap();
        let mut pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();

        let order = Arc::new(Mutex::new(Vec::new()));
        let first_order = order.clone();
        let first = node.add_pre_set_parameters_callback(move |updates| {
            first_order.lock().push("first");
            if let Some((_, value)) = updates.iter_mut().find(|(name, _)| name == "derived") {
                *value = ParameterValue::Integer(20);
            }
        });
        let second_order = order.clone();
        let second = node.add_pre_set_parameters_callback(move |updates| {
            second_order.lock().push("second");
            updates.push(("derived".into(), ParameterValue::Integer(10)));
        });
        let validated = Arc::new(Mutex::new(Vec::new()));
        let validated_callback = validated.clone();
        let validation = node.add_on_set_parameters_callback(move |updates| {
            *validated_callback.lock() = updates.to_vec();
            Ok(())
        });
        let posted = Arc::new(Mutex::new(Vec::new()));
        let posted_callback = posted.clone();
        let post = node.add_post_set_parameters_callback(move |updates| {
            *posted_callback.lock() = updates.to_vec();
        });

        node.set_parameter("source", ParameterValue::Integer(1))
            .unwrap();
        assert_eq!(*order.lock(), vec!["second", "first"]);
        let expected = vec![
            ("source".into(), ParameterValue::Integer(1)),
            ("derived".into(), ParameterValue::Integer(20)),
        ];
        assert_eq!(*validated.lock(), expected);
        assert_eq!(*posted.lock(), expected);
        assert_eq!(node.get_parameter::<i64>("source").unwrap(), 1);
        assert_eq!(node.get_parameter::<i64>("derived").unwrap(), 20);
        assert_eq!(events.next().now_or_never().flatten(), Some(expected[0].clone()));
        assert_eq!(events.next().now_or_never().flatten(), Some(expected[1].clone()));

        order.lock().clear();
        let response = drive(
            &mut node,
            &mut pool,
            client
                .request(&SetParametersAtomically::Request {
                    parameters: vec![msg::Parameter {
                        name: "source".into(),
                        value: ParameterValue::Integer(2).into_parameter_value_msg(),
                    }],
                })
                .unwrap(),
        );
        assert!(response.result.successful);
        assert_eq!(*order.lock(), vec!["second", "first"]);
        assert_eq!(node.get_parameter::<i64>("source").unwrap(), 2);
        assert_eq!(node.get_parameter::<i64>("derived").unwrap(), 20);

        drop(second);
        drop(first);
        order.lock().clear();
        node.set_parameters_atomically(Vec::<(String, ParameterValue)>::new())
            .unwrap();
        assert!(order.lock().is_empty());

        let empty = node.add_pre_set_parameters_callback(Vec::clear);
        assert!(matches!(
            node.set_parameter("source", ParameterValue::Integer(3)),
            Err(Error::ParameterUpdateRejected { .. })
        ));
        assert_eq!(node.get_parameter::<i64>("source").unwrap(), 2);
        drop(empty);

        let declaration_calls = Arc::new(Mutex::new(0));
        let calls = declaration_calls.clone();
        let declaration_guard = node.add_pre_set_parameters_callback(move |_| *calls.lock() += 1);
        node.declare_parameter("declared", ParameterValue::Bool(true))
            .unwrap();
        node.undeclare_parameter("declared").unwrap_err();
        assert_eq!(*declaration_calls.lock(), 0);
        drop(declaration_guard);
        drop(validation);
        drop(post);
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
    fn spin_until_future_complete_handles_ready_timeout_and_ros_waits() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "spin_until_future_test", "").unwrap();

        assert_eq!(
            node.spin_until_future_complete(future::ready(Ok(42)), Duration::ZERO)
                .unwrap(),
            42
        );
        assert!(matches!(
            node.spin_until_future_complete(
                future::pending::<Result<()>>(),
                Duration::from_millis(1),
            ),
            Err(Error::RCL_RET_TIMEOUT)
        ));

        let polls = std::cell::Cell::new(0);
        let started = Instant::now();
        let result = node
            .spin_until_future_complete(
                future::poll_fn(|_| {
                    let poll = polls.get() + 1;
                    polls.set(poll);
                    if poll == 1 {
                        Poll::Pending
                    } else {
                        Poll::Ready(Ok(7))
                    }
                }),
                Duration::from_millis(200),
            )
            .unwrap();
        assert_eq!(result, 7);
        assert_eq!(polls.get(), 2);
        assert!(started.elapsed() < Duration::from_millis(150));

        let service = node
            .create_service::<AddTwoInts::Service>(
                "/spin_until_future_service",
                QosProfile::default(),
            )
            .unwrap();
        let client = node
            .create_client::<AddTwoInts::Service>(
                "/spin_until_future_service",
                QosProfile::default(),
            )
            .unwrap();
        let available = Node::is_available(&client).unwrap();
        node.spin_until_future_complete(available, Duration::from_secs(1))
            .unwrap();
        drop(service);
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
    fn reports_domain_id_and_resolves_names() {
        let context = Context::create().unwrap();
        let node = Node::create(context, "name_resolution_test", "/scope").unwrap();

        let mut default_domain_id = 0;
        assert_eq!(unsafe { rcl_get_default_domain_id(&mut default_domain_id) }, RCL_RET_OK as i32);
        assert_eq!(node.domain_id().unwrap(), default_domain_id);
        #[cfg(not(r2r__ros__distro__foxy))]
        {
            assert_eq!(node.resolve_topic_name("relative").unwrap(), "/scope/relative");
            assert_eq!(
                node.resolve_topic_name("~/private").unwrap(),
                "/scope/name_resolution_test/private"
            );
            assert_eq!(
                node.resolve_topic_name_without_remapping("relative")
                    .unwrap(),
                "/scope/relative"
            );
            assert_eq!(node.resolve_service_name("service").unwrap(), "/scope/service");
            assert_eq!(
                node.resolve_service_name_without_remapping("service")
                    .unwrap(),
                "/scope/service"
            );
            assert!(matches!(
                node.resolve_topic_name("bad\0name"),
                Err(Error::RCL_RET_INVALID_ARGUMENT)
            ));
        }
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
                .parameter_override("answer", 42_i64)
                .parameter_overrides([("answer", 43_i64)])
                .parameter_override("enabled", true),
        )
        .unwrap();

        assert_eq!(node.get_parameter::<i64>("answer").unwrap(), 43);
        assert!(node.get_parameter::<bool>("enabled").unwrap());
        assert_eq!(
            node.parameter_overrides()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["answer", "enabled"]
        );
        assert_eq!(node.parameter_overrides().get("answer"), Some(&ParameterValue::Integer(43)));
        let native_options = unsafe { rcl_node_get_options(node.node_handle.as_ref()) };
        assert!(!native_options.is_null());
        assert!(!unsafe { (*native_options).use_global_arguments });
    }

    #[test]
    fn parameter_mutation_accepts_native_rust_values() {
        let context = Context::create().unwrap();
        let node = Node::create(context, "native_parameter_value_test", "").unwrap();

        assert_eq!(node.declare_parameter("count", 1_i64).unwrap(), ParameterValue::Integer(1));
        assert_eq!(
            node.declare_parameter("label", "initial").unwrap(),
            ParameterValue::String("initial".into())
        );
        node.set_parameter("count", 2_i32).unwrap();
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 2);
        assert!(node
            .set_parameters([("count", 3_i64), ("other", 4_i64)])
            .into_iter()
            .all(|result| result.is_ok()));
        node.set_parameters_atomically([("count", 5_i64), ("other", 6_i64)])
            .unwrap();
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 5);
        assert_eq!(node.get_parameter::<i64>("other").unwrap(), 6);
    }

    #[test]
    fn strict_parameter_mode_rejects_undeclared_updates() {
        let context = Context::create().unwrap();
        let node = Node::create_with_options(
            context,
            "strict_parameter_test",
            "",
            NodeOptions::default()
                .use_global_arguments(false)
                .allow_undeclared_parameters(false),
        )
        .unwrap();
        node.declare_parameter("declared", 1_i64).unwrap();
        let validation_calls = Arc::new(Mutex::new(0));
        let calls = validation_calls.clone();
        let _validation = node.add_on_set_parameters_callback(move |_| {
            *calls.lock() += 1;
            Ok(())
        });

        node.set_parameter("declared", 2_i64).unwrap();
        assert!(matches!(
            node.set_parameter("misspelled", 3_i64),
            Err(Error::ParameterNotDeclared { name }) if name == "misspelled"
        ));
        assert!(matches!(
            node.set_parameters_atomically([("declared", 4_i64), ("missing", 5_i64)]),
            Err(Error::ParameterNotDeclared { name }) if name == "missing"
        ));
        assert_eq!(node.get_parameter::<i64>("declared").unwrap(), 2);
        assert!(!node.has_parameter("misspelled"));
        assert!(!node.has_parameter("missing"));
        assert_eq!(*validation_calls.lock(), 1);
    }

    #[test]
    fn strict_parameter_queries_reject_undeclared_names() {
        let context = Context::create().unwrap();
        let strict = Node::create_with_options(
            context.clone(),
            "strict_parameter_query_test",
            "",
            NodeOptions::default()
                .use_global_arguments(false)
                .allow_undeclared_parameters(false),
        )
        .unwrap();
        strict.declare_parameter("declared", 1_i64).unwrap();

        assert!(matches!(
            strict.get_parameter::<i64>("missing"),
            Err(Error::ParameterNotDeclared { name }) if name == "missing"
        ));
        assert!(matches!(
            strict.try_get_parameters(["declared", "missing"]),
            Err(Error::ParameterNotDeclared { name }) if name == "missing"
        ));
        assert!(matches!(
            strict.try_get_parameter_types(["declared", "missing"]),
            Err(Error::ParameterNotDeclared { name }) if name == "missing"
        ));
        assert!(matches!(
            strict.try_describe_parameters(["declared", "missing"]),
            Err(Error::ParameterNotDeclared { name }) if name == "missing"
        ));
        assert_eq!(strict.get_parameter_type("declared").unwrap(), ParameterType::Integer);
        assert_eq!(strict.describe_parameter("declared").unwrap().name, "declared");

        let permissive = Node::create_with_options(
            context,
            "permissive_parameter_query_test",
            "",
            NodeOptions::default().use_global_arguments(false),
        )
        .unwrap();
        assert_eq!(permissive.try_get_parameters(["missing"]).unwrap(), [ParameterValue::NotSet]);
        assert_eq!(permissive.get_parameter_type("missing").unwrap(), ParameterType::NotSet);
        assert_eq!(permissive.describe_parameter("missing").unwrap().name, "missing");
    }

    #[test]
    fn strict_parameter_queries_read_live_provider_values() {
        let context = Context::create().unwrap();
        let mut node = Node::create_with_options(
            context,
            "strict_provider_query_test",
            "",
            NodeOptions::default()
                .use_global_arguments(false)
                .allow_undeclared_parameters(false),
        )
        .unwrap();
        let provider = Arc::new(Mutex::new(LocalSetParams {
            count: 1,
            ratio: 0.5,
        }));
        let provider_for_handler: Arc<Mutex<dyn RosParams + Send>> = provider.clone();
        let (_handler, _events) = node
            .make_derived_parameter_handler(provider_for_handler)
            .unwrap();
        provider.lock().count = 42;

        assert_eq!(node.try_get_parameters(["count"]).unwrap(), [ParameterValue::Integer(42)]);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 42);
        assert_eq!(node.get_parameter_type("count").unwrap(), ParameterType::Integer);
        assert_eq!(node.describe_parameter("count").unwrap().name, "count");
    }

    #[test]
    fn strict_parameter_queries_propagate_provider_read_failures() {
        let context = Context::create().unwrap();
        let mut node = Node::create_with_options(
            context,
            "provider_read_failure_test",
            "",
            NodeOptions::default()
                .use_global_arguments(false)
                .allow_undeclared_parameters(false),
        )
        .unwrap();
        let fail_reads = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let provider: Arc<Mutex<dyn RosParams + Send>> = Arc::new(Mutex::new(FailingReadParams {
            fail_reads: Arc::clone(&fail_reads),
        }));
        let (handler, _events) = node.make_derived_parameter_handler(provider).unwrap();
        let client = node
            .create_client::<rcl_interfaces::srv::GetParameters::Service>(
                "provider_read_failure_test/get_parameters",
                QosProfile::default(),
            )
            .unwrap();
        let mut pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();

        assert_eq!(
            node.try_get_parameters(["provider_value"]).unwrap(),
            [ParameterValue::Integer(7)]
        );
        fail_reads.store(true, std::sync::atomic::Ordering::Release);

        assert!(matches!(
            node.try_get_parameters(["provider_value"]),
            Err(Error::ParameterValueConv { name, .. }) if name == "provider_value"
        ));
        assert!(matches!(
            node.get_parameter::<i64>("provider_value"),
            Err(Error::ParameterValueConv { name, .. }) if name == "provider_value"
        ));
        assert!(matches!(
            node.get_parameter_or("provider_value", 99_i64),
            Err(Error::ParameterValueConv { name, .. }) if name == "provider_value"
        ));
        assert_eq!(
            node.get_parameters_by_prefix("").get("provider_value"),
            Some(&ParameterValue::NotSet)
        );
        assert!(matches!(
            node.try_get_parameters_by_prefix(""),
            Err(Error::ParameterValueConv { name, .. }) if name == "provider_value"
        ));
        assert_eq!(node.get_parameter_or("missing", 99_i64).unwrap(), 99);
        assert_eq!(node.get_parameters(["provider_value"]), [ParameterValue::NotSet]);

        fail_reads.store(false, std::sync::atomic::Ordering::Release);
        assert_eq!(
            node.try_get_parameters_by_prefix("")
                .unwrap()
                .get("provider_value"),
            Some(&ParameterValue::Integer(7))
        );
        assert_eq!(node.params.lock()["provider_value"].value, ParameterValue::Integer(7));
        fail_reads.store(true, std::sync::atomic::Ordering::Release);

        let response = client
            .request(&rcl_interfaces::srv::GetParameters::Request {
                names: vec!["provider_value".into()],
            })
            .unwrap();
        let mut response = Box::pin(response);
        let response = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            pool.run_until_stalled();
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert_eq!(response.values.len(), 1);
        assert_eq!(
            response.values[0].type_,
            rcl_interfaces::msg::ParameterType::PARAMETER_NOT_SET as u8
        );
    }

    #[test]
    fn strict_parameter_mode_checks_names_added_by_pre_set_callbacks() {
        let context = Context::create().unwrap();
        let node = Node::create_with_options(
            context,
            "strict_pre_set_parameter_test",
            "",
            NodeOptions::default()
                .use_global_arguments(false)
                .allow_undeclared_parameters(false),
        )
        .unwrap();
        node.declare_parameter("declared", 1_i64).unwrap();
        let _pre = node.add_pre_set_parameters_callback(|updates| {
            updates.push(("injected".into(), ParameterValue::Integer(2)));
        });

        assert!(matches!(
            node.set_parameter("declared", 3_i64),
            Err(Error::ParameterNotDeclared { name }) if name == "injected"
        ));
        assert_eq!(node.get_parameter::<i64>("declared").unwrap(), 1);
        assert!(!node.has_parameter("injected"));
    }

    #[test]
    fn parameter_updates_reject_empty_names_after_pre_set_callbacks() {
        let context = Context::create().unwrap();
        let node = Node::create_with_options(
            context,
            "parameter_name_validation_test",
            "",
            NodeOptions::default().use_global_arguments(false),
        )
        .unwrap();

        assert!(matches!(
            node.set_parameter("", 1_i64),
            Err(Error::InvalidParameterName { name }) if name.is_empty()
        ));
        let _pre = node.add_pre_set_parameters_callback(|updates| {
            updates.push((String::new(), ParameterValue::Integer(2)));
        });
        assert!(matches!(
            node.set_parameter("valid", 1_i64),
            Err(Error::InvalidParameterName { name }) if name.is_empty()
        ));
        assert!(!node.has_parameter(""));
        assert!(!node.has_parameter("valid"));
    }

    #[test]
    fn parameter_declaration_validates_overrides_and_prevents_duplicates() {
        let context = Context::create().unwrap();
        let node = Node::create_with_options(
            context,
            "parameter_declaration_test",
            "",
            NodeOptions::default()
                .use_global_arguments(false)
                .parameter_override("count", ParameterValue::Integer(4))
                .parameter_override("invalid", ParameterValue::Integer(3)),
        )
        .unwrap();
        let range = IntegerRange {
            from_value: 0,
            to_value: 10,
            step: 2,
        };

        let effective = node
            .declare_parameter_with_descriptor(
                "count",
                ParameterValue::Integer(2),
                ParameterDescriptor {
                    description: "even count".into(),
                    integer_range: vec![range.clone()],
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(effective, ParameterValue::Integer(4));
        let descriptor = node.get_parameter_descriptor("count").unwrap();
        assert_eq!(descriptor.name, "count");
        assert_eq!(descriptor.type_, rcl_interfaces::msg::ParameterType::PARAMETER_INTEGER as u8);
        assert_eq!(descriptor.description, "even count");
        assert_eq!(descriptor.integer_range, vec![range.clone()]);

        assert!(matches!(
            node.declare_parameter("count", ParameterValue::Integer(2)),
            Err(Error::ParameterAlreadyDeclared { .. })
        ));
        assert!(matches!(
            node.declare_parameter_with_descriptor(
                "invalid",
                ParameterValue::Integer(2),
                ParameterDescriptor {
                    integer_range: vec![range],
                    ..Default::default()
                },
            ),
            Err(Error::ParameterConstraintViolation { .. })
        ));
        assert!(node.get_parameter_descriptor("missing").is_err());
    }

    #[test]
    fn batch_parameter_declaration_applies_namespace_overrides_and_descriptors() {
        let context = Context::create().unwrap();
        let node = Node::create_with_options(
            context,
            "batch_parameter_declaration_test",
            "",
            NodeOptions::default()
                .use_global_arguments(false)
                .parameter_override("robot.speed", 4_i64),
        )
        .unwrap();
        let range = IntegerRange {
            from_value: 0,
            to_value: 10,
            step: 2,
        };
        let post_calls = Arc::new(Mutex::new(Vec::new()));
        let post_calls_for_callback = Arc::clone(&post_calls);
        let _post = node.add_post_set_parameters_callback(move |updates| {
            post_calls_for_callback.lock().push(updates.to_vec());
        });

        let values = node
            .declare_parameters_with_descriptors(
                "robot",
                [
                    (
                        "speed",
                        ParameterValue::Integer(2),
                        ParameterDescriptor {
                            description: "even speed".into(),
                            integer_range: vec![range.clone()],
                            ..Default::default()
                        },
                    ),
                    ("limit", ParameterValue::Integer(8), ParameterDescriptor::default()),
                ],
            )
            .unwrap();

        assert_eq!(values, [ParameterValue::Integer(4), ParameterValue::Integer(8)]);
        assert_eq!(node.get_parameter::<i64>("robot.speed").unwrap(), 4);
        assert_eq!(node.get_parameter::<i64>("robot.limit").unwrap(), 8);
        let speed = node.get_parameter_descriptor("robot.speed").unwrap();
        assert_eq!(speed.name, "robot.speed");
        assert_eq!(speed.integer_range, [range]);
        assert_eq!(speed.description, "even speed");
        assert_eq!(post_calls.lock().len(), 2);
        assert_eq!(post_calls.lock()[0][0].0, "robot.speed");
        assert_eq!(post_calls.lock()[1][0].0, "robot.limit");
    }

    #[test]
    fn parameter_declaration_options_can_ignore_overrides() {
        let context = Context::create().unwrap();
        let node = Node::create_with_options(
            context,
            "ignore_parameter_override_test",
            "",
            NodeOptions::default()
                .use_global_arguments(false)
                .parameter_override("single", 41_i64)
                .parameter_override("group.first", 42_i64)
                .parameter_override("group.even", 3_i64),
        )
        .unwrap();
        let ignore = ParameterDeclarationOptions::default().ignore_override(true);

        assert_eq!(
            node.declare_parameter_with_options(
                "single",
                1_i64,
                ParameterDescriptor::default(),
                ignore,
            )
            .unwrap(),
            ParameterValue::Integer(1)
        );
        assert_eq!(
            node.declare_parameters_with_options(
                "group",
                [("first", 2_i64), ("second", 4_i64)],
                ignore,
            )
            .unwrap(),
            [ParameterValue::Integer(2), ParameterValue::Integer(4)]
        );
        assert_eq!(
            node.declare_parameters_with_descriptors_and_options(
                "group",
                [(
                    "even",
                    6_i64,
                    ParameterDescriptor {
                        integer_range: vec![IntegerRange {
                            from_value: 0,
                            to_value: 10,
                            step: 2,
                        }],
                        ..Default::default()
                    },
                )],
                ignore,
            )
            .unwrap(),
            [ParameterValue::Integer(6)]
        );
        assert_eq!(node.get_parameter::<i64>("single").unwrap(), 1);
        assert_eq!(node.get_parameter::<i64>("group.first").unwrap(), 2);
        assert_eq!(node.get_parameter::<i64>("group.even").unwrap(), 6);
    }

    #[test]
    fn parameter_override_snapshot_survives_runtime_mutations() {
        let context = Context::create().unwrap();
        let node = Node::create_with_options(
            context,
            "parameter_override_snapshot_test",
            "",
            NodeOptions::default()
                .use_global_arguments(false)
                .parameter_override("mutable", 41_i64)
                .parameter_override("ignored", 42_i64),
        )
        .unwrap();

        node.declare_parameter_with_descriptor(
            "mutable",
            1_i64,
            ParameterDescriptor {
                dynamic_typing: true,
                ..Default::default()
            },
        )
        .unwrap();
        node.set_parameter("mutable", 43_i64).unwrap();
        node.undeclare_parameter("mutable").unwrap();
        node.declare_parameter_with_options(
            "ignored",
            2_i64,
            ParameterDescriptor::default(),
            ParameterDeclarationOptions::default().ignore_override(true),
        )
        .unwrap();

        assert_eq!(
            node.parameter_overrides(),
            &IndexMap::from([
                ("mutable".into(), ParameterValue::Integer(41)),
                ("ignored".into(), ParameterValue::Integer(42)),
            ])
        );
        assert!(!node.has_parameter("mutable"));
        assert_eq!(node.get_parameter::<i64>("ignored").unwrap(), 2);
    }

    #[test]
    fn parameter_overrides_can_wait_for_explicit_declaration() {
        let context = Context::create().unwrap();
        let node = Node::create_with_options(
            context,
            "deferred_parameter_override_test",
            "",
            NodeOptions::default()
                .use_global_arguments(false)
                .parameter_override("pending", 41_i64)
                .parameter_override("ignored", 42_i64)
                .automatically_declare_parameters_from_overrides(false),
        )
        .unwrap();

        assert!(!node.has_parameter("pending"));
        assert!(!node
            .list_parameters(
                std::iter::empty::<&str>(),
                rcl_interfaces::srv::ListParameters::Request::DEPTH_RECURSIVE as u64,
            )
            .names
            .contains(&"pending".to_owned()));
        assert_eq!(node.parameter_overrides().get("pending"), Some(&ParameterValue::Integer(41)));

        assert_eq!(node.declare_parameter("pending", 1_i64).unwrap(), ParameterValue::Integer(41));
        assert_eq!(
            node.declare_parameter_with_options(
                "ignored",
                2_i64,
                ParameterDescriptor::default(),
                ParameterDeclarationOptions::default().ignore_override(true),
            )
            .unwrap(),
            ParameterValue::Integer(2)
        );
        assert_eq!(node.get_parameter::<i64>("pending").unwrap(), 41);
        assert_eq!(node.get_parameter::<i64>("ignored").unwrap(), 2);
    }

    #[test]
    fn deferred_overrides_initialize_only_derived_parameters() {
        let context = Context::create().unwrap();
        let mut node = Node::create_with_options(
            context,
            "deferred_derived_parameter_override_test",
            "",
            NodeOptions::default()
                .use_global_arguments(false)
                .parameter_override("count", 41_i64)
                .parameter_override("unclaimed", 42_i64)
                .automatically_declare_parameters_from_overrides(false),
        )
        .unwrap();
        let provider = Arc::new(Mutex::new(DerivedOverrideParams { count: 1 }));
        let provider_for_handler: Arc<Mutex<dyn RosParams + Send>> = provider.clone();

        let (_handler, _events) = node
            .make_derived_parameter_handler(provider_for_handler)
            .unwrap();

        assert_eq!(provider.lock().count, 41);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 41);
        assert!(!node.has_parameter("unclaimed"));
        assert_eq!(node.parameter_overrides().get("unclaimed"), Some(&ParameterValue::Integer(42)));
    }

    #[test]
    fn batch_parameter_declaration_is_atomic_on_validation_failures() {
        let context = Context::create().unwrap();
        let node = Node::create(context, "atomic_batch_parameter_declaration_test", "").unwrap();
        let post_calls = Arc::new(Mutex::new(0));
        let post_calls_for_callback = Arc::clone(&post_calls);
        let _post = node.add_post_set_parameters_callback(move |_| {
            *post_calls_for_callback.lock() += 1;
        });

        let result = node.declare_parameters_with_descriptors(
            "invalid",
            [
                ("first", ParameterValue::Integer(2), ParameterDescriptor::default()),
                (
                    "second",
                    ParameterValue::Integer(3),
                    ParameterDescriptor {
                        integer_range: vec![IntegerRange {
                            from_value: 0,
                            to_value: 10,
                            step: 2,
                        }],
                        ..Default::default()
                    },
                ),
            ],
        );

        assert!(matches!(result, Err(Error::ParameterConstraintViolation { .. })));
        assert!(!node.has_parameter("invalid.first"));
        assert!(!node.has_parameter("invalid.second"));
        assert_eq!(*post_calls.lock(), 0);

        let _reject = node.add_on_set_parameters_callback(|updates| {
            if updates[0].0 == "callback.second" {
                Err(Error::ParameterUpdateRejected {
                    reason: "rejected second declaration".into(),
                })
            } else {
                Ok(())
            }
        });
        let result = node.declare_parameters("callback", [("first", 1_i64), ("second", 2_i64)]);
        assert!(matches!(result, Err(Error::ParameterUpdateRejected { .. })));
        assert!(!node.has_parameter("callback.first"));
        assert!(!node.has_parameter("callback.second"));
        assert_eq!(*post_calls.lock(), 0);
    }

    #[test]
    fn batch_parameter_declaration_rejects_duplicate_and_existing_names_atomically() {
        let context = Context::create().unwrap();
        let node = Node::create(context, "duplicate_batch_parameter_test", "").unwrap();

        assert!(matches!(
            node.declare_parameters("batch", [("same", 1_i64), ("same", 2_i64)]),
            Err(Error::ParameterAlreadyDeclared { name }) if name == "batch.same"
        ));
        assert!(!node.has_parameter("batch.same"));

        node.declare_parameter("batch.existing", 3_i64).unwrap();
        assert!(matches!(
            node.declare_parameters(
                "batch",
                [("fresh", 4_i64), ("existing", 5_i64)],
            ),
            Err(Error::ParameterAlreadyDeclared { name }) if name == "batch.existing"
        ));
        assert!(!node.has_parameter("batch.fresh"));
        assert_eq!(node.get_parameter::<i64>("batch.existing").unwrap(), 3);
    }

    #[test]
    fn local_parameter_undeclare_removes_only_dynamic_parameters() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "local_parameter_undeclare_test", "").unwrap();
        node.declare_parameter_with_descriptor(
            "dynamic",
            ParameterValue::Integer(1),
            ParameterDescriptor {
                dynamic_typing: true,
                ..Default::default()
            },
        )
        .unwrap();
        node.declare_parameter("static", ParameterValue::Integer(2))
            .unwrap();
        node.declare_parameter_with_descriptor(
            "read_only",
            ParameterValue::Integer(3),
            ParameterDescriptor {
                read_only: true,
                ..Default::default()
            },
        )
        .unwrap();
        let (_handler, mut events) = node.make_parameter_handler().unwrap();
        let mut ros_events = node
            .subscribe_parameter_events(
                Some("/local_parameter_undeclare_test".into()),
                Some("dynamic".into()),
            )
            .unwrap();
        for _ in 0..100 {
            if node
                .get_subscriptions_info_by_topic("/parameter_events", false)
                .unwrap()
                .iter()
                .any(|endpoint| endpoint.node_name == "local_parameter_undeclare_test")
            {
                break;
            }
            node.try_spin_once(Duration::from_millis(10)).unwrap();
        }
        let observed = Arc::new(Mutex::new(Vec::new()));
        let observed_by_callback = Arc::clone(&observed);
        let _callback = node.add_post_set_parameters_callback(move |updates| {
            observed_by_callback.lock().extend_from_slice(updates);
        });

        assert!(node.has_parameter("dynamic"));
        assert_eq!(node.undeclare_parameter("dynamic").unwrap(), ParameterValue::Integer(1));
        assert_eq!(
            events.next().now_or_never().flatten(),
            Some(("dynamic".into(), ParameterValue::NotSet))
        );
        assert!(observed.lock().is_empty());
        let event = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(event) = ros_events.next().now_or_never().flatten() {
                break event;
            }
        };
        assert_eq!(event.kind, ParameterEventKind::Deleted);
        assert_eq!(event.name, "dynamic");
        assert_eq!(event.value, ParameterValue::NotSet);
        assert!(!node.has_parameter("dynamic"));
        assert!(node.get_parameter_descriptor("dynamic").is_err());
        assert!(node.undeclare_parameter("dynamic").is_err());
        assert!(node.undeclare_parameter("static").is_err());
        assert!(node.undeclare_parameter("read_only").is_err());
        assert!(node.has_parameter("static"));
        assert!(node.has_parameter("read_only"));
    }

    #[test]
    fn runtime_parameter_declarations_publish_local_and_ros_events() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "parameter_declaration_event_test", "").unwrap();
        let (handler, mut local_events) = node.make_parameter_handler().unwrap();
        let mut ros_events = node
            .subscribe_parameter_events(Some("/parameter_declaration_event_test".into()), None)
            .unwrap();
        let pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();
        for _ in 0..100 {
            if node
                .get_subscriptions_info_by_topic("/parameter_events", false)
                .unwrap()
                .iter()
                .any(|endpoint| endpoint.node_name == "parameter_declaration_event_test")
            {
                break;
            }
            node.try_spin_once(Duration::from_millis(10)).unwrap();
        }

        assert_eq!(
            node.declare_parameters("runtime", [("first", 1_i64), ("second", 2_i64)])
                .unwrap(),
            [ParameterValue::Integer(1), ParameterValue::Integer(2)]
        );
        assert_eq!(
            local_events.next().now_or_never().flatten(),
            Some(("runtime.first".into(), ParameterValue::Integer(1)))
        );
        assert_eq!(
            local_events.next().now_or_never().flatten(),
            Some(("runtime.second".into(), ParameterValue::Integer(2)))
        );
        assert!(local_events.next().now_or_never().is_none());

        let mut received = Vec::new();
        while received.len() < 2 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            while let Some(event) = ros_events.next().now_or_never().flatten() {
                received.push(event);
            }
        }
        assert_eq!(
            received
                .iter()
                .map(|event| (event.kind, event.name.as_str(), event.value.clone()))
                .collect::<Vec<_>>(),
            [
                (ParameterEventKind::New, "runtime.first", ParameterValue::Integer(1)),
                (ParameterEventKind::New, "runtime.second", ParameterValue::Integer(2)),
            ]
        );
        assert!(received
            .iter()
            .all(|event| event.node == "/parameter_declaration_event_test"));
        assert!(received
            .iter()
            .all(|event| event.stamp.sec > 0 || event.stamp.nanosec > 0));
    }

    #[test]
    fn primitive_derived_parameter_handler_returns_an_error_instead_of_panicking() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "primitive_parameter_handler_test", "").unwrap();
        let params: Arc<Mutex<dyn RosParams + Send>> = Arc::new(Mutex::new(42_i64));

        assert!(matches!(
            node.make_derived_parameter_handler(params),
            Err(Error::InvalidParameterName { name }) if name.is_empty()
        ));
    }

    #[test]
    fn parameter_handler_can_only_be_created_once() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "single_parameter_handler_test", "").unwrap();
        let (handler, _events) = node.make_parameter_handler().unwrap();
        let service_count = node.services.len();
        let publisher_count = node.pubs.len();

        assert!(matches!(
            node.make_parameter_handler(),
            Err(Error::ParameterHandlerAlreadyCreated)
        ));
        assert_eq!(node.services.len(), service_count);
        assert_eq!(node.pubs.len(), publisher_count);
        drop(handler);
    }

    #[test]
    fn parameter_handler_options_can_disable_network_endpoints() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "local_only_parameter_handler_test", "").unwrap();
        let options = ParameterHandlerOptions::default()
            .start_parameter_services(false)
            .start_parameter_event_publisher(false);

        let (handler, mut events) = node.make_parameter_handler_with_options(options).unwrap();

        assert!(handler.now_or_never().is_some());
        assert!(node.services.is_empty());
        assert!(node.pubs.is_empty());
        node.set_parameter("local_only", 42_i64).unwrap();
        assert_eq!(
            events.next().now_or_never().flatten(),
            Some(("local_only".into(), ParameterValue::Integer(42)))
        );
    }

    #[test]
    fn parameter_handler_options_control_endpoint_qos_independently() {
        let context = Context::create().unwrap();
        let mut event_node = Node::create(context.clone(), "parameter_event_qos_test", "").unwrap();
        let (event_handler, _events) = event_node
            .make_parameter_handler_with_options(
                ParameterHandlerOptions::default()
                    .start_parameter_services(false)
                    .parameter_event_qos(QosProfile::parameter_events().keep_last(7)),
            )
            .unwrap();
        assert!(event_handler.now_or_never().is_some());
        assert!(event_node.services.is_empty());
        assert_eq!(event_node.pubs.len(), 1);
        assert_eq!(
            event_node
                .parameter_event_sink
                .as_ref()
                .unwrap()
                .publisher
                .as_ref()
                .unwrap()
                .get_actual_qos()
                .unwrap()
                .depth,
            7
        );

        let mut service_node = Node::create(context, "parameter_service_qos_test", "").unwrap();
        let (service_handler, mut events) = service_node
            .make_parameter_handler_with_options(
                ParameterHandlerOptions::default()
                    .start_parameter_event_publisher(false)
                    .service_qos(QosProfile::default().keep_last(6)),
            )
            .unwrap();
        assert!(service_handler.now_or_never().is_none());
        assert_eq!(service_node.services.len(), 6);
        assert!(service_node.pubs.is_empty());
        assert!(service_node.services.iter().all(|service| {
            service
                .lock()
                .request_subscription_actual_qos()
                .unwrap()
                .depth
                == 6
                && service
                    .lock()
                    .response_publisher_actual_qos()
                    .unwrap()
                    .depth
                    == 6
        }));
        service_node.set_parameter("local_event", true).unwrap();
        assert_eq!(
            events.next().now_or_never().flatten(),
            Some(("local_event".into(), ParameterValue::Bool(true)))
        );
    }

    #[test]
    fn failed_parameter_handler_validation_can_be_retried_before_creating_endpoints() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "failed_parameter_handler_test", "").unwrap();
        let params: Arc<Mutex<dyn RosParams + Send>> = Arc::new(Mutex::new(42_i64));

        assert!(matches!(
            node.make_derived_parameter_handler(params),
            Err(Error::InvalidParameterName { .. })
        ));
        assert!(node.services.is_empty());
        assert!(node.pubs.is_empty());

        let (handler, _events) = node.make_parameter_handler().unwrap();
        assert!(node.parameter_handler_started);
        assert!(!node.services.is_empty());
        assert!(!node.pubs.is_empty());
        drop(handler);
    }

    #[test]
    fn failed_parameter_handler_endpoint_creation_rolls_back_and_can_be_retried() {
        let context = Context::create().unwrap();
        // The first five parameter service names fit within the ROS topic-name limit, while the
        // longer `set_parameters_atomically` endpoint does not. This exercises a real failure
        // after both the event publisher and multiple services have already been created.
        let node_name = "n".repeat(230);
        let mut node = Node::create(context, &node_name, "").unwrap();
        let unrelated_service = node
            .create_service::<crate::example_interfaces::srv::AddTwoInts::Service>(
                "/unrelated_service",
                QosProfile::default(),
            )
            .unwrap();
        let unrelated_publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/unrelated_publisher",
                QosProfile::default(),
            )
            .unwrap();

        assert!(node.make_parameter_handler().is_err());
        assert!(!node.parameter_handler_started);
        assert_eq!(node.services.len(), 1);
        assert_eq!(node.service_indexes.len(), 1);
        assert!(node.canceled_services.lock().is_empty());
        assert_eq!(unrelated_service.service_name(&node).unwrap(), "/unrelated_service");
        assert_eq!(node.pubs.len(), 1);
        assert_eq!(node.publisher_indexes.len(), 1);
        assert!(node.dropped_publishers.lock().is_empty());
        assert_eq!(unrelated_publisher.get_actual_qos().unwrap().depth, 10);

        let (handler, mut events) = node
            .make_parameter_handler_with_options(
                ParameterHandlerOptions::default()
                    .start_parameter_services(false)
                    .start_parameter_event_publisher(false),
            )
            .unwrap();
        assert!(handler.now_or_never().is_some());
        assert_eq!(node.services.len(), 1);
        assert_eq!(node.pubs.len(), 1);
        node.set_parameter("after_retry", 7_i64).unwrap();
        assert_eq!(
            events.next().now_or_never().flatten(),
            Some(("after_retry".into(), ParameterValue::Integer(7)))
        );
    }

    #[test]
    fn endpoint_failure_precedes_derived_parameter_registration() {
        let context = Context::create().unwrap();
        let node_name = "n".repeat(230);
        let mut node = Node::create(context, &node_name, "").unwrap();
        let provider = Arc::new(Mutex::new(HandlerSetupParams {
            registration_calls: 0,
            insert_invalid_name: false,
        }));
        let provider_for_handler: Arc<Mutex<dyn RosParams + Send>> = provider.clone();

        assert!(node
            .make_derived_parameter_handler(provider_for_handler)
            .is_err());
        assert_eq!(provider.lock().registration_calls, 0);
        assert!(!node.has_parameter("tracked"));

        let provider_for_handler: Arc<Mutex<dyn RosParams + Send>> = provider.clone();
        let (handler, _events) = node
            .make_derived_parameter_handler_with_options(
                provider_for_handler,
                ParameterHandlerOptions::default()
                    .start_parameter_services(false)
                    .start_parameter_event_publisher(false),
            )
            .unwrap();
        assert!(handler.now_or_never().is_some());
        assert_eq!(provider.lock().registration_calls, 1);
        assert_eq!(node.get_parameter::<i64>("tracked").unwrap(), 11);
    }

    #[test]
    fn failed_parameter_handler_initialization_restores_parameter_state() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "parameter_state_rollback_test", "").unwrap();
        node.declare_parameter("existing", 5_i64).unwrap();
        let original_descriptor = node.get_parameter_descriptor("existing").unwrap();
        let provider: Arc<Mutex<dyn RosParams + Send>> = Arc::new(Mutex::new(HandlerSetupParams {
            registration_calls: 0,
            insert_invalid_name: true,
        }));

        assert!(matches!(
            node.make_derived_parameter_handler_with_options(
                provider,
                ParameterHandlerOptions::default()
                    .start_parameter_services(false)
                    .start_parameter_event_publisher(false),
            ),
            Err(Error::InvalidParameterDescriptor { name, .. }) if name.is_empty()
        ));
        assert!(!node.parameter_handler_started);
        assert!(!node.has_parameter(""));
        assert!(!node.has_parameter("use_sim_time"));
        assert_eq!(node.get_parameter::<i64>("existing").unwrap(), 5);
        assert_eq!(node.get_parameter_descriptor("existing").unwrap(), original_descriptor);

        let (handler, _events) = node
            .make_parameter_handler_with_options(
                ParameterHandlerOptions::default()
                    .start_parameter_services(false)
                    .start_parameter_event_publisher(false),
            )
            .unwrap();
        assert!(handler.now_or_never().is_some());
    }

    #[test]
    fn provider_ownership_probe_propagates_read_failures_and_can_retry() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "provider_ownership_failure_test", "").unwrap();
        node.declare_parameter("runtime_value", 3_i64).unwrap();
        let fail_reads = Arc::new(std::sync::atomic::AtomicBool::new(true));
        let provider = Arc::new(Mutex::new(FailingReadParams {
            fail_reads: Arc::clone(&fail_reads),
        }));
        let provider_for_handler: Arc<Mutex<dyn RosParams + Send>> = provider.clone();

        assert!(matches!(
            node.make_derived_parameter_handler(provider_for_handler),
            Err(Error::ParameterValueConv { name, .. }) if name == "provider_value"
        ));
        assert!(!node.parameter_handler_started);
        assert!(!node.has_parameter("provider_value"));
        assert_eq!(node.get_parameter::<i64>("runtime_value").unwrap(), 3);
        assert!(node.services.is_empty());
        assert!(node.pubs.is_empty());

        fail_reads.store(false, std::sync::atomic::Ordering::Release);
        let provider_for_handler: Arc<Mutex<dyn RosParams + Send>> = provider;
        let (handler, _events) = node
            .make_derived_parameter_handler(provider_for_handler)
            .unwrap();
        assert!(handler.now_or_never().is_none());
        assert_eq!(node.get_parameter::<i64>("provider_value").unwrap(), 7);
        assert_eq!(node.get_parameter::<i64>("runtime_value").unwrap(), 3);
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
                "local" => {
                    NodeOptions::default().arguments(["r2r-node", "--ros-args", "-p", "answer:=43"])
                }
                "local_isolated" => NodeOptions::default()
                    .use_global_arguments(false)
                    .arguments(["r2r-node", "--ros-args", "-p", "answer:=44"]),
                "programmatic" => NodeOptions::default()
                    .arguments(["r2r-node", "--ros-args", "-p", "answer:=43"])
                    .parameter_override("answer", ParameterValue::Integer(44)),
                "deferred" => {
                    NodeOptions::default().automatically_declare_parameters_from_overrides(false)
                }
                _ => unreachable!(),
            };
            let node =
                Node::create_with_options(context, "node_options_cli_test", "", options).unwrap();
            match mode.as_str() {
                "global" => {
                    assert_eq!(node.get_parameter::<i64>("answer").unwrap(), 42);
                    assert_eq!(
                        node.parameter_overrides().get("answer"),
                        Some(&ParameterValue::Integer(42))
                    );
                }
                "isolated" => {
                    assert!(node.get_parameter::<i64>("answer").is_err());
                    assert!(node.parameter_overrides().is_empty());
                }
                "override" => {
                    assert_eq!(node.get_parameter::<i64>("answer").unwrap(), 43);
                    assert_eq!(
                        node.parameter_overrides().get("answer"),
                        Some(&ParameterValue::Integer(43))
                    );
                }
                "local" => {
                    assert_eq!(node.get_parameter::<i64>("answer").unwrap(), 43);
                    assert_eq!(
                        node.parameter_overrides().get("answer"),
                        Some(&ParameterValue::Integer(43))
                    );
                }
                "local_isolated" => {
                    assert_eq!(node.get_parameter::<i64>("answer").unwrap(), 44);
                    assert_eq!(
                        node.parameter_overrides().get("answer"),
                        Some(&ParameterValue::Integer(44))
                    );
                }
                "programmatic" => {
                    assert_eq!(node.get_parameter::<i64>("answer").unwrap(), 44);
                    assert_eq!(
                        node.parameter_overrides().get("answer"),
                        Some(&ParameterValue::Integer(44))
                    );
                }
                "deferred" => {
                    assert!(!node.has_parameter("answer"));
                    assert_eq!(
                        node.parameter_overrides().get("answer"),
                        Some(&ParameterValue::Integer(42))
                    );
                    assert_eq!(
                        node.declare_parameter("answer", 1_i64).unwrap(),
                        ParameterValue::Integer(42)
                    );
                }
                _ => unreachable!(),
            }
            return;
        }

        let test_binary = std::env::current_exe().unwrap();
        for mode in [
            "global",
            "isolated",
            "override",
            "local",
            "local_isolated",
            "programmatic",
            "deferred",
        ] {
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
    fn node_options_apply_local_name_and_namespace_remaps() {
        let context = Context::create().unwrap();
        let node = Node::create_with_options(
            context,
            "original_name",
            "/original_namespace",
            NodeOptions::default()
                .use_global_arguments(false)
                .arguments([
                    "r2r-node",
                    "--ros-args",
                    "-r",
                    "__node:=remapped_name",
                    "-r",
                    "__ns:=/remapped_namespace",
                ]),
        )
        .unwrap();

        assert_eq!(node.name().unwrap(), "remapped_name");
        assert_eq!(node.namespace().unwrap(), "/remapped_namespace");
        assert_eq!(node.fully_qualified_name().unwrap(), "/remapped_namespace/remapped_name");
    }

    #[test]
    fn parameter_files_apply_segment_wildcards_to_the_fully_qualified_node_name() {
        let path = std::env::temp_dir()
            .join(format!("r2r_parameter_wildcards_{}.yaml", uuid::Uuid::new_v4().simple()));
        std::fs::write(
            &path,
            r#"
/**:
  ros__parameters:
    global_match: true
/graph_ns/*:
  ros__parameters:
    one_level_match: 1
/**/graph_ns/**:
  ros__parameters:
    recursive_match: 2
/other_ns/graph_target:
  ros__parameters:
    should_not_match: true
/graph_target:
  ros__parameters:
    basename_must_not_match: true
"#,
        )
        .unwrap();
        let arguments = vec![
            "r2r-node".to_owned(),
            "--ros-args".to_owned(),
            "--params-file".to_owned(),
            path.to_string_lossy().into_owned(),
        ];
        let context = Context::create().unwrap();

        let node = Node::create_with_options(
            context,
            "graph_target",
            "/graph_ns",
            NodeOptions::default()
                .use_global_arguments(false)
                .arguments(arguments),
        )
        .unwrap();
        std::fs::remove_file(path).unwrap();

        assert!(node.get_parameter::<bool>("global_match").unwrap());
        assert_eq!(node.get_parameter::<i64>("one_level_match").unwrap(), 1);
        assert_eq!(node.get_parameter::<i64>("recursive_match").unwrap(), 2);
        assert!(!node.has_parameter("should_not_match"));
        assert!(!node.has_parameter("basename_must_not_match"));
    }

    #[test]
    fn node_options_reject_invalid_local_ros_arguments() {
        let context = Context::create().unwrap();

        assert!(matches!(
            Node::create_with_options(
                context.clone(),
                "invalid_local_argument_test",
                "",
                NodeOptions::default().arguments(["r2r-node", "--ros-args", "--unknown"]),
            ),
            Err(Error::RCL_RET_INVALID_ROS_ARGS)
        ));
        assert!(matches!(
            Node::create_with_options(
                context,
                "nul_local_argument_test",
                "",
                NodeOptions::default().arguments(["r2r-node", "bad\0argument"]),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
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
        let raw_topics = node.get_raw_topic_names_and_types().unwrap();
        let publishers = node.get_publishers_info_by_topic(&topic, false).unwrap();
        let subscriptions = node.get_subscriptions_info_by_topic(&topic, false).unwrap();

        assert_eq!(topics.get(&topic), Some(&vec!["std_msgs/msg/String".to_owned()]));
        assert_eq!(
            raw_topics.get(&format!("rt{}", topic)),
            Some(&vec!["std_msgs::msg::dds_::String_".to_owned()])
        );
        assert!(publishers.iter().any(|publisher| {
            publisher.topic_type == "std_msgs/msg/String"
                && publisher.endpoint_type == TopicEndpointType::Publisher
        }));
        assert!(subscriptions.iter().any(|subscription| {
            subscription.topic_type == "std_msgs/msg/String"
                && subscription.endpoint_type == TopicEndpointType::Subscription
                && subscription.qos_profile.reliability
                    == QosProfile::default().best_effort().reliability
        }));
        #[cfg(not(any(
            r2r__ros__distro__foxy,
            r2r__ros__distro__galactic,
            r2r__ros__distro__humble
        )))]
        {
            let publisher_hash = publishers
                .iter()
                .find(|endpoint| endpoint.topic_type == "std_msgs/msg/String")
                .and_then(|endpoint| endpoint.topic_type_hash);
            let subscription_hash = subscriptions
                .iter()
                .find(|endpoint| endpoint.topic_type == "std_msgs/msg/String")
                .and_then(|endpoint| endpoint.topic_type_hash);
            assert_eq!(publisher_hash, subscription_hash);
            if let Some(hash) = publisher_hash {
                if !hash.is_unset() {
                    assert!(hash.to_string().starts_with("RIHS"));
                    assert_eq!(hash.to_string().len(), 71);
                }
            }
        }
    }

    #[test]
    fn publisher_gids_correlate_received_messages() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "publisher_gid_test", "").unwrap();
        let typed_topic = format!("/publisher_gid_typed_{}", uuid::Uuid::new_v4().simple());
        let untyped_topic = format!("/publisher_gid_untyped_{}", uuid::Uuid::new_v4().simple());
        let typed = node
            .create_publisher::<crate::std_msgs::msg::String>(&typed_topic, QosProfile::default())
            .unwrap();
        let mut subscription = node
            .subscribe_with_info::<crate::std_msgs::msg::String>(
                &typed_topic,
                QosProfile::default(),
            )
            .unwrap();
        let untyped = node
            .create_publisher_untyped(&untyped_topic, "std_msgs/msg/String", QosProfile::default())
            .unwrap();

        let typed_gid = typed.get_gid().unwrap();
        assert_eq!(typed.get_gid().unwrap(), typed_gid);
        let untyped_gid = untyped.get_gid().unwrap();
        assert!(!typed_gid.implementation_identifier.is_empty());
        assert_eq!(typed_gid.implementation_identifier, untyped_gid.implementation_identifier);
        assert!(typed_gid.data.iter().any(|byte| *byte != 0));
        assert_ne!(typed_gid.data, untyped_gid.data);

        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if typed.get_subscription_count().unwrap() > 0 {
                break;
            }
        }
        typed
            .publish(&crate::std_msgs::msg::String { data: "gid".into() })
            .unwrap();
        let mut received = None;
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            received = received.or_else(|| subscription.next().now_or_never().flatten());
            if received.is_some() {
                break;
            }
        }
        assert_eq!(received.unwrap().info.publisher_gid.as_slice(), typed_gid.data.as_slice());
    }

    #[test]
    fn maps_topic_endpoint_roles_without_loss() {
        assert_eq!(
            TopicEndpointType::try_from(rmw_endpoint_type_t::RMW_ENDPOINT_INVALID).unwrap(),
            TopicEndpointType::Invalid
        );
        assert_eq!(
            TopicEndpointType::try_from(rmw_endpoint_type_t::RMW_ENDPOINT_PUBLISHER).unwrap(),
            TopicEndpointType::Publisher
        );
        assert_eq!(
            TopicEndpointType::try_from(rmw_endpoint_type_t::RMW_ENDPOINT_SUBSCRIPTION).unwrap(),
            TopicEndpointType::Subscription
        );
    }

    #[test]
    fn formats_topic_type_hash_as_rep_2011_identifier() {
        let mut value = [0_u8; 32];
        for (index, byte) in value.iter_mut().enumerate() {
            *byte = index as u8;
        }
        let hash = TopicTypeHash { version: 1, value };
        assert!(!hash.is_unset());
        assert_eq!(
            hash.to_string(),
            "RIHS01_000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
        );
        assert!(TopicTypeHash {
            version: 0,
            value: [0; 32],
        }
        .is_unset());

        #[cfg(not(any(
            r2r__ros__distro__foxy,
            r2r__ros__distro__galactic,
            r2r__ros__distro__humble
        )))]
        {
            let native = rosidl_type_hash_t { version: 1, value };
            let allocator = unsafe { rcutils_get_default_allocator() };
            let mut output = std::ptr::null_mut();
            let ret = unsafe { rosidl_stringify_type_hash(&native, allocator, &mut output) };
            assert_eq!(ret, RCL_RET_OK as i32);
            let output = AllocatedCString { output, allocator };
            assert_eq!(c_string_from_ptr(output.output).unwrap(), hash.to_string());
        }
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

    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    #[test]
    fn publishers_wait_for_acknowledgements_without_blocking() {
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
        let mut node = Node::create(context, "publisher_async_ack_test", "").unwrap();
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/publisher_async_ack",
                QosProfile::default(),
            )
            .unwrap();
        let acked = publisher.wait_for_all_acked_async().unwrap();
        drive(&mut node, acked).unwrap();

        let untyped = node
            .create_publisher_untyped(
                "/publisher_async_ack_untyped",
                "std_msgs/msg/String",
                QosProfile::default(),
            )
            .unwrap();
        let acked = untyped
            .wait_for_all_acked_async_with_timeout(&mut node, Duration::from_secs(1))
            .unwrap();
        drive(&mut node, acked).unwrap();

        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/publisher_canceled_ack_wait",
                QosProfile::default(),
            )
            .unwrap();
        let publisher_count = node.pubs.len();
        let acked = publisher.wait_for_all_acked_async().unwrap();
        drop(publisher);
        node.try_spin_once(Duration::ZERO).unwrap();
        // Cleanup runs before ACK polling, so the waiter keeps the native publisher alive for
        // the spin that resolves it. The next spin may release it.
        assert_eq!(node.pubs.len(), publisher_count);
        drop(acked);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.pubs.len(), publisher_count - 1);
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
    fn subscription_drains_bursts_in_one_spin() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "subscription_batch_test", "").unwrap();
        let topic = format!("/subscription_batch_{}", uuid::Uuid::new_v4().simple());
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default())
            .unwrap();
        let mut subscription = node
            .subscribe_with_options::<crate::std_msgs::msg::Int32>(
                &topic,
                QosProfile::default(),
                SubscriptionOptions::default().channel_capacity(8),
            )
            .unwrap();

        let available = publisher
            .wait_for_subscribers_with_timeout(&mut node, Duration::from_secs(1))
            .unwrap();
        node.spin_until_future_complete(available, Duration::from_secs(1))
            .unwrap();

        for value in 1..=3 {
            publisher
                .publish(&crate::std_msgs::msg::Int32 { data: value })
                .unwrap();
        }
        std::thread::sleep(Duration::from_millis(20));
        node.try_spin_once(Duration::from_millis(100)).unwrap();

        let received = (0..3)
            .map(|_| subscription.next().now_or_never().flatten().unwrap().data)
            .collect::<Vec<_>>();
        assert_eq!(received, vec![1, 2, 3]);
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
        fn validate_endpoints(result: Result<Vec<NetworkFlowEndpoint>>) {
            match result {
                Ok(endpoints) => {
                    assert!(!endpoints.is_empty());
                    for endpoint in endpoints {
                        assert!(!endpoint.transport_protocol.is_empty());
                        assert!(!endpoint.internet_protocol.is_empty());
                        if endpoint.internet_protocol != "Unknown" {
                            endpoint
                                .internet_address
                                .parse::<std::net::IpAddr>()
                                .unwrap();
                        }
                    }
                }
                Err(Error::RCL_RET_UNSUPPORTED) => {}
                Err(err) => panic!("unexpected network flow endpoint error: {err}"),
            }
        }

        let context = Context::create().unwrap();
        let other = Node::create(context.clone(), "unique_network_flow_other", "").unwrap();
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
        let untyped_topic = format!("/unique_flow_untyped_{}", uuid::Uuid::new_v4().simple());
        let untyped = node
            .create_publisher_untyped_with_options(
                &untyped_topic,
                "std_msgs/msg/Int32",
                QosProfile::default(),
                PublisherOptions::default()
                    .unique_network_flow(UniqueNetworkFlowRequirement::SystemDefault),
            )
            .unwrap();
        let untyped_subscription = node
            .subscribe_untyped_with_options(
                &untyped_topic,
                "std_msgs/msg/Int32",
                QosProfile::default(),
                SubscriptionOptions::default()
                    .unique_network_flow(UniqueNetworkFlowRequirement::SystemDefault),
            )
            .unwrap();

        validate_endpoints(publisher.get_network_flow_endpoints());
        validate_endpoints(subscription.get_network_flow_endpoints(&node));
        validate_endpoints(untyped.get_network_flow_endpoints());
        validate_endpoints(untyped_subscription.get_network_flow_endpoints(&node));
        assert!(matches!(
            subscription.get_network_flow_endpoints(&other),
            Err(Error::RCL_RET_SUBSCRIPTION_INVALID)
        ));

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
        let _subscription_supports_loans = subscription.can_loan_messages(&node).unwrap();
        assert!(matches!(
            subscription.topic_name(&other),
            Err(Error::RCL_RET_SUBSCRIPTION_INVALID)
        ));
        assert!(matches!(
            subscription.can_loan_messages(&other),
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
    fn service_drains_request_bursts_in_one_spin() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "service_batch_test", "").unwrap();
        let service_name = format!("/service_batch_{}", uuid::Uuid::new_v4().simple());
        let mut service = node
            .create_service_with_options::<AddTwoInts::Service>(
                &service_name,
                QosProfile::default(),
                ServiceOptions::default().request_channel_capacity(8),
            )
            .unwrap();
        let client = node
            .create_client::<AddTwoInts::Service>(&service_name, QosProfile::default())
            .unwrap();
        let available = Node::is_available(&client).unwrap();
        node.spin_until_future_complete(available, Duration::from_secs(1))
            .unwrap();

        let responses = (1..=3)
            .map(|value| {
                client
                    .request(&AddTwoInts::Request { a: value, b: 0 })
                    .unwrap()
            })
            .collect::<Vec<_>>();
        std::thread::sleep(Duration::from_millis(20));
        node.try_spin_once(Duration::from_millis(100)).unwrap();

        let received = (0..3)
            .map(|_| {
                let request = service.next().now_or_never().flatten().unwrap();
                (request.message.a, request.request_info())
            })
            .collect::<Vec<_>>();
        assert_eq!(received.iter().map(|(value, _)| *value).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert!(received
            .windows(2)
            .all(|pair| pair[0].1.sequence_number != pair[1].1.sequence_number));
        assert!(received
            .windows(2)
            .all(|pair| pair[0].1.writer_guid == pair[1].1.writer_guid));
        assert!(received[0].1.writer_guid.iter().any(|byte| *byte != 0));
        assert!(received
            .iter()
            .all(|(_, info)| { info.source_timestamp >= 0 && info.received_timestamp >= 0 }));
        drop(responses);
    }

    #[test]
    fn client_drains_response_bursts_in_one_spin() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "client_response_batch_test", "").unwrap();
        let service_name = format!("/client_batch_{}", uuid::Uuid::new_v4().simple());
        let mut service = node
            .create_service_with_options::<AddTwoInts::Service>(
                &service_name,
                QosProfile::default().keep_last(8),
                ServiceOptions::default().request_channel_capacity(8),
            )
            .unwrap();
        let client = node
            .create_client::<AddTwoInts::Service>(&service_name, QosProfile::default().keep_last(8))
            .unwrap();
        let available = Node::is_available(&client).unwrap();
        node.spin_until_future_complete(available, Duration::from_secs(1))
            .unwrap();

        let mut responses = (1..=3)
            .map(|value| {
                Box::pin(
                    client
                        .request(&AddTwoInts::Request { a: value, b: 0 })
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        std::thread::sleep(Duration::from_millis(20));
        node.try_spin_once(Duration::from_millis(100)).unwrap();
        while let Some(request) = service.next().now_or_never().flatten() {
            let sum = request.message.a;
            request.respond(AddTwoInts::Response { sum }).unwrap();
        }
        std::thread::sleep(Duration::from_millis(20));
        node.try_spin_once(Duration::from_millis(100)).unwrap();

        let values = responses
            .iter_mut()
            .map(|response| response.as_mut().now_or_never().unwrap().unwrap().sum)
            .collect::<Vec<_>>();
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    #[test]
    fn client_gids_are_stable_and_unique() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "client_gid_test", "").unwrap();
        let typed = node
            .create_client::<AddTwoInts::Service>("/client_gid_typed", QosProfile::default())
            .unwrap();
        let untyped = node
            .create_client_untyped(
                "/client_gid_untyped",
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();

        let typed_gid = typed.get_gid().unwrap();
        assert_eq!(typed.get_gid().unwrap(), typed_gid);
        let untyped_gid = untyped.get_gid().unwrap();
        assert!(!typed_gid.implementation_identifier.is_empty());
        assert_eq!(typed_gid.implementation_identifier, untyped_gid.implementation_identifier);
        assert!(typed_gid.data.iter().any(|byte| *byte != 0));
        assert_ne!(typed_gid.data, untyped_gid.data);
    }

    #[test]
    fn service_clients_can_return_response_metadata() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "client_response_info_test", "").unwrap();
        let service_name = format!("/client_response_info_{}", uuid::Uuid::new_v4().simple());
        let mut service = node
            .create_service::<AddTwoInts::Service>(&service_name, QosProfile::default())
            .unwrap();
        let client = node
            .create_client::<AddTwoInts::Service>(&service_name, QosProfile::default())
            .unwrap();
        let untyped_client = node
            .create_client_untyped(
                &service_name,
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();
        let available = Node::is_available(&client).unwrap();
        node.spin_until_future_complete(available, Duration::from_secs(1))
            .unwrap();
        let available = Node::is_available(&untyped_client).unwrap();
        node.spin_until_future_complete(available, Duration::from_secs(1))
            .unwrap();

        let response = client
            .request_with_info_with_timeout(
                &mut node,
                &AddTwoInts::Request { a: 20, b: 22 },
                Duration::from_secs(1),
            )
            .unwrap();
        let mut response = Box::pin(response);
        let (response, request_info) = 'typed_response: loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(request) = service.next().now_or_never().flatten() {
                let request_info = request.request_info();
                request.respond(AddTwoInts::Response { sum: 42 }).unwrap();
                loop {
                    node.try_spin_once(Duration::from_millis(10)).unwrap();
                    if let Some(response) = response.as_mut().now_or_never() {
                        break 'typed_response (response.unwrap(), request_info);
                    }
                }
            }
        };
        assert_eq!(response.response.sum, 42);
        assert_eq!(response.info.sequence_number, request_info.sequence_number);
        assert!(response.info.writer_guid.iter().any(|byte| *byte != 0));
        assert!(response.info.source_timestamp >= 0);
        assert!(response.info.received_timestamp >= 0);

        let legacy_response = client.request(&AddTwoInts::Request { a: 1, b: 2 }).unwrap();
        let mut legacy_response = Box::pin(legacy_response);
        let legacy_response = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(request) = service.next().now_or_never().flatten() {
                request.respond(AddTwoInts::Response { sum: 3 }).unwrap();
            }
            if let Some(response) = legacy_response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert_eq!(legacy_response.sum, 3);

        let response = untyped_client
            .request_with_info_with_timeout(
                &mut node,
                serde_json::json!({ "a": 19, "b": 23 }),
                Duration::from_secs(1),
            )
            .unwrap();
        let mut response = Box::pin(response);
        let (response, request_info) = 'untyped_response: loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(request) = service.next().now_or_never().flatten() {
                let request_info = request.request_info();
                request.respond(AddTwoInts::Response { sum: 42 }).unwrap();
                loop {
                    node.try_spin_once(Duration::from_millis(10)).unwrap();
                    if let Some(response) = response.as_mut().now_or_never() {
                        break 'untyped_response (response.unwrap(), request_info);
                    }
                }
            }
        };
        assert_eq!(response.response.unwrap(), serde_json::json!({ "sum": 42 }));
        assert_eq!(response.info.sequence_number, request_info.sequence_number);
        assert!(response.info.writer_guid.iter().any(|byte| *byte != 0));
        assert!(response.info.source_timestamp >= 0);
        assert!(response.info.received_timestamp >= 0);
    }

    #[test]
    fn untyped_service_servers_interoperate_with_typed_clients() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "untyped_service_server_test", "/scope").unwrap();
        let mut requests = node
            .create_service_untyped_with_options(
                "add",
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default().keep_last(7),
                ServiceOptions::default().request_channel_capacity(0),
            )
            .unwrap();
        assert_eq!(requests.service_name(&node).unwrap(), "/scope/add");
        assert_eq!(
            requests
                .get_request_subscription_actual_qos(&node)
                .unwrap()
                .depth,
            7
        );
        assert_eq!(
            requests
                .get_response_publisher_actual_qos(&node)
                .unwrap()
                .depth,
            7
        );
        let client = node
            .create_client::<AddTwoInts::Service>("add", QosProfile::default().keep_last(7))
            .unwrap();

        let first = client.request(&AddTwoInts::Request { a: 1, b: 2 }).unwrap();
        let second = client
            .request(&AddTwoInts::Request { a: 20, b: 22 })
            .unwrap();
        let mut first = Box::pin(first);
        let mut second = Box::pin(second);
        for _ in 0..2 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
        }
        let request = requests.next().now_or_never().flatten().unwrap();
        let first_info = request.request_info();
        assert_eq!(request.message.as_ref().unwrap(), &serde_json::json!({ "a": 1, "b": 2 }));
        request.respond(serde_json::json!({ "sum": 3 })).unwrap();
        assert!(requests.next().now_or_never().is_none());
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        let request = requests.next().now_or_never().flatten().unwrap();
        let second_info = request.request_info();
        assert_eq!(request.message.as_ref().unwrap(), &serde_json::json!({ "a": 20, "b": 22 }));
        request.respond(serde_json::json!({ "sum": 42 })).unwrap();
        assert_ne!(first_info.sequence_number, second_info.sequence_number);
        assert_eq!(first_info.writer_guid, second_info.writer_guid);
        assert!(first_info.writer_guid.iter().any(|byte| *byte != 0));
        assert!(first_info.source_timestamp >= 0);
        assert!(first_info.received_timestamp >= 0);
        assert!(second_info.source_timestamp >= 0);
        assert!(second_info.received_timestamp >= 0);

        let mut first_result = None;
        let mut second_result = None;
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            first_result = first_result.or_else(|| first.as_mut().now_or_never());
            second_result = second_result.or_else(|| second.as_mut().now_or_never());
            if first_result.is_some() && second_result.is_some() {
                break;
            }
        }
        assert_eq!(first_result.unwrap().unwrap().sum, 3);
        assert_eq!(second_result.unwrap().unwrap().sum, 42);

        let invalid = node.create_service_untyped(
            "invalid",
            "missing_interfaces/srv/Nope",
            QosProfile::default(),
        );
        assert!(matches!(invalid, Err(Error::InvalidMessageType { .. })));
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
        assert!(matches!(
            observer.get_publisher_names_and_types_by_node(
                "missing_graph_target",
                "/graph_ns",
                false,
            ),
            Err(Error::RCL_RET_NODE_NAME_NON_EXISTENT)
        ));
    }

    #[cfg(not(r2r__ros__distro__foxy))]
    #[test]
    fn endpoint_info_queries_expand_relative_and_private_topic_names() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "endpoint_name_test", "/scope").unwrap();
        let _relative = node
            .create_publisher::<crate::std_msgs::msg::String>("relative", QosProfile::default())
            .unwrap();
        let _private = node
            .create_publisher::<crate::std_msgs::msg::String>("~/private", QosProfile::default())
            .unwrap();
        let _relative_subscription = node
            .subscribe::<crate::std_msgs::msg::String>("relative", QosProfile::default())
            .unwrap();
        let _private_subscription = node
            .subscribe::<crate::std_msgs::msg::String>("~/private", QosProfile::default())
            .unwrap();
        #[cfg(not(any(
            r2r__ros__distro__galactic,
            r2r__ros__distro__humble,
            r2r__ros__distro__iron
        )))]
        let _relative_service = node
            .create_service::<crate::example_interfaces::srv::AddTwoInts::Service>(
                "relative_service",
                QosProfile::default(),
            )
            .unwrap();
        #[cfg(not(any(
            r2r__ros__distro__galactic,
            r2r__ros__distro__humble,
            r2r__ros__distro__iron
        )))]
        let _relative_client = node
            .create_client::<crate::example_interfaces::srv::AddTwoInts::Service>(
                "relative_client",
                QosProfile::default(),
            )
            .unwrap();

        let relative = node
            .get_publishers_info_by_topic("relative", false)
            .unwrap();
        assert!(relative
            .iter()
            .any(|info| info.topic_type == "std_msgs/msg/String"));
        let private = node
            .get_publishers_info_by_topic("~/private", false)
            .unwrap();
        assert!(private
            .iter()
            .any(|info| info.topic_type == "std_msgs/msg/String"));
        let relative = node
            .get_subscriptions_info_by_topic("relative", false)
            .unwrap();
        assert!(relative
            .iter()
            .any(|info| info.topic_type == "std_msgs/msg/String"));
        let private = node
            .get_subscriptions_info_by_topic("~/private", false)
            .unwrap();
        assert!(private
            .iter()
            .any(|info| info.topic_type == "std_msgs/msg/String"));

        assert_eq!(node.count_publishers("relative").unwrap(), 1);
        assert_eq!(node.count_subscribers("relative").unwrap(), 1);
        #[cfg(not(any(
            r2r__ros__distro__galactic,
            r2r__ros__distro__humble,
            r2r__ros__distro__iron
        )))]
        {
            assert_eq!(node.count_services("relative_service").unwrap(), 1);
            assert_eq!(node.count_clients("relative_client").unwrap(), 1);
        }
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
    fn graph_change_waits_are_broadcast_and_cancel_safe() {
        let context = Context::create().unwrap();
        let mut observer = Node::create(context.clone(), "graph_wait_observer", "").unwrap();

        let first = observer.wait_for_graph_change();
        let second = observer.wait_for_graph_change();
        let mut first = Box::pin(first);
        let mut second = Box::pin(second);
        let _target = Node::create(context, "graph_wait_target", "").unwrap();
        let mut first_result = None;
        let mut second_result = None;
        for _ in 0..100 {
            observer.try_spin_once(Duration::from_millis(10)).unwrap();
            if first_result.is_none() {
                first_result = first.as_mut().now_or_never();
            }
            if second_result.is_none() {
                second_result = second.as_mut().now_or_never();
            }
            if first_result.is_some() && second_result.is_some() {
                break;
            }
        }
        assert!(matches!(first_result, Some(Ok(()))));
        assert!(matches!(second_result, Some(Ok(()))));

        let canceled = observer.wait_for_graph_change();
        assert_eq!(observer.graph_change_waiters.len(), 1);
        drop(canceled);
        observer.try_spin_once(Duration::ZERO).unwrap();
        assert!(observer.graph_change_waiters.is_empty());

        let canceled = (0..20)
            .map(|_| observer.wait_for_graph_change())
            .collect::<Vec<_>>();
        drop(canceled);
        observer.graph_change_waiters.prune_canceled();
        assert_eq!(observer.graph_change_waiters.len(), 4);
        observer.graph_change_waiters.prune_canceled();
        assert!(observer.graph_change_waiters.is_empty());

        // The timeout variant composes the graph waiter with the already tested Node timeout
        // mechanism. Do not assert a timeout here: discovery events may legitimately continue
        // arriving after the target node was created.
        let timed = observer
            .wait_for_graph_change_with_timeout(Duration::from_millis(1))
            .unwrap();
        drop(timed);
        observer.try_spin_once(Duration::ZERO).unwrap();
        assert!(observer.graph_change_waiters.is_empty());
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

    #[test]
    fn subscription_loan_is_received_or_reports_unsupported() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "subscription_loan_test", "").unwrap();
        let mut other = Node::create(context, "subscription_loan_other", "").unwrap();
        let topic = format!("/subscription_loan_{}", uuid::Uuid::new_v4().simple());
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default())
            .unwrap();
        let subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default())
            .unwrap();

        assert!(matches!(
            subscription.take_loaned_message(&mut other),
            Err(Error::RCL_RET_SUBSCRIPTION_INVALID)
        ));
        if !subscription.can_loan_messages(&node).unwrap() {
            assert!(matches!(
                subscription.take_loaned_message(&mut node),
                Err(Error::RCL_RET_UNSUPPORTED)
            ));
            return;
        }

        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if publisher.get_subscription_count().unwrap() > 0 {
                break;
            }
        }
        publisher
            .publish(&crate::std_msgs::msg::Int32 { data: 42 })
            .unwrap();
        for _ in 0..100 {
            if let Some(message) = subscription.take_loaned_message(&mut node).unwrap() {
                assert_eq!(message.data, 42);
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("subscription did not receive a loaned message");
    }

    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    #[test]
    fn subscriptions_support_runtime_content_filters_when_available() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "content_filter_test", "").unwrap();
        let mut other = Node::create(context, "content_filter_other", "").unwrap();
        let topic = format!("/content_filter_{}", uuid::Uuid::new_v4().simple());
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default())
            .unwrap();
        let mut subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default())
            .unwrap();
        let filter = ContentFilterOptions::new("data > %0").expression_parameters(["10"]);

        assert!(matches!(
            subscription.set_content_filter(&mut other, &filter),
            Err(Error::RCL_RET_SUBSCRIPTION_INVALID)
        ));
        match subscription.set_content_filter(&mut node, &filter) {
            Err(Error::RCL_RET_UNSUPPORTED) => {
                assert!(!subscription.is_content_filter_enabled(&node).unwrap());
                return;
            }
            Err(err) => panic!("unexpected content filter error: {err}"),
            Ok(()) => {}
        }

        assert!(subscription.is_content_filter_enabled(&node).unwrap());
        assert_eq!(subscription.get_content_filter(&node).unwrap(), filter);
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if publisher.get_subscription_count().unwrap() > 0 {
                break;
            }
        }
        publisher
            .publish(&crate::std_msgs::msg::Int32 { data: 1 })
            .unwrap();
        publisher
            .publish(&crate::std_msgs::msg::Int32 { data: 42 })
            .unwrap();
        let received = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(message) = subscription.next().now_or_never().flatten() {
                break message;
            }
        };
        assert_eq!(received.data, 42);
        subscription.clear_content_filter(&mut node).unwrap();
        assert!(!subscription.is_content_filter_enabled(&node).unwrap());
    }

    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble
    )))]
    #[test]
    fn publishers_and_subscriptions_report_matching_changes() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "matched_event_test", "").unwrap();

        let publisher_topic = format!("/publisher_matched_{}", uuid::Uuid::new_v4().simple());
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(
                &publisher_topic,
                QosProfile::default(),
            )
            .unwrap();
        match publisher.take_matched() {
            Ok(None) => {}
            Err(Error::RCL_RET_UNSUPPORTED) => return,
            result => panic!("unexpected initial publisher matched status: {result:?}"),
        }
        let subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(&publisher_topic, QosProfile::default())
            .unwrap();
        let connected = (0..100).find_map(|_| {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            publisher.take_matched().unwrap()
        });
        let connected = connected.expect("publisher did not report a matched subscription");
        assert!(connected.total_count >= 1);
        assert!(connected.total_count_change >= 1);
        assert!(connected.current_count >= 1);
        assert!(connected.current_count_change > 0);

        node.destroy_subscription(subscription);
        let disconnected = (0..100).find_map(|_| {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            publisher.take_matched().unwrap()
        });
        let disconnected = disconnected.expect("publisher did not report the disconnection");
        assert_eq!(disconnected.total_count_change, 0);
        assert_eq!(disconnected.current_count, 0);
        assert!(disconnected.current_count_change < 0);

        let subscription_topic = format!("/subscription_matched_{}", uuid::Uuid::new_v4().simple());
        let subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(&subscription_topic, QosProfile::default())
            .unwrap();
        match subscription.take_matched(&mut node) {
            Ok(None) => {}
            Err(Error::RCL_RET_UNSUPPORTED) => return,
            result => panic!("unexpected initial subscription matched status: {result:?}"),
        }
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(
                &subscription_topic,
                QosProfile::default(),
            )
            .unwrap();
        let connected = (0..100).find_map(|_| {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            subscription.take_matched(&mut node).unwrap()
        });
        let connected = connected.expect("subscription did not report a matched publisher");
        assert!(connected.total_count >= 1);
        assert!(connected.total_count_change >= 1);
        assert!(connected.current_count >= 1);
        assert!(connected.current_count_change > 0);

        node.destroy_publisher(publisher);
        let disconnected = (0..100).find_map(|_| {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            subscription.take_matched(&mut node).unwrap()
        });
        let disconnected = disconnected.expect("subscription did not report the disconnection");
        assert_eq!(disconnected.total_count_change, 0);
        assert_eq!(disconnected.current_count, 0);
        assert!(disconnected.current_count_change < 0);
    }

    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble
    )))]
    #[test]
    fn incompatible_type_event_queries_initialize_when_supported() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "incompatible_type_event_test", "").unwrap();

        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(
                "/publisher_incompatible_type_event",
                QosProfile::default(),
            )
            .unwrap();
        match publisher.take_incompatible_type() {
            Ok(None) => {}
            Err(Error::RCL_RET_UNSUPPORTED) => return,
            result => panic!("unexpected initial publisher incompatible-type status: {result:?}"),
        }
        assert!(publisher.take_incompatible_type().unwrap().is_none());

        let subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(
                "/subscription_incompatible_type_event",
                QosProfile::default(),
            )
            .unwrap();
        match subscription.take_incompatible_type(&mut node) {
            Ok(None) => {}
            Err(Error::RCL_RET_UNSUPPORTED) => return,
            result => {
                panic!("unexpected initial subscription incompatible-type status: {result:?}")
            }
        }
        assert!(subscription
            .take_incompatible_type(&mut node)
            .unwrap()
            .is_none());
    }

    #[test]
    fn subscriptions_report_requested_incompatible_qos() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "subscription_qos_event_test", "").unwrap();
        let mut other = Node::create(context, "subscription_qos_event_other", "").unwrap();
        let topic = format!("/subscription_qos_event_{}", uuid::Uuid::new_v4().simple());
        let subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default().reliable())
            .unwrap();

        assert!(matches!(
            subscription.take_requested_incompatible_qos(&mut other),
            Err(Error::RCL_RET_SUBSCRIPTION_INVALID)
        ));
        match subscription.take_requested_incompatible_qos(&mut node) {
            Err(Error::RCL_RET_UNSUPPORTED) => return,
            Err(err) => panic!("unexpected QoS event initialization error: {err}"),
            Ok(None) => {}
            Ok(Some(status)) => panic!("unexpected initial QoS event: {status:?}"),
        }

        let _publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(
                &topic,
                QosProfile::default().best_effort(),
            )
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(status) = subscription
                .take_requested_incompatible_qos(&mut node)
                .unwrap()
            {
                assert!(status.total_count >= 1);
                assert!(status.total_count_change >= 1);
                assert_eq!(status.last_policy_kind, crate::QosPolicyKind::Reliability);
                assert!(subscription
                    .take_requested_incompatible_qos(&mut node)
                    .unwrap()
                    .is_none());
                return;
            }
        }
        panic!("subscription did not report incompatible QoS");
    }

    #[test]
    fn subscriptions_report_requested_deadline_misses() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "subscription_deadline_event_test", "").unwrap();
        let topic = format!("/subscription_deadline_event_{}", uuid::Uuid::new_v4().simple());
        let qos = QosProfile::default().deadline(Duration::from_millis(20));
        let mut subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(&topic, qos.clone())
            .unwrap();
        match subscription.take_requested_deadline_missed(&mut node) {
            Err(Error::RCL_RET_UNSUPPORTED) => return,
            Err(err) => panic!("unexpected deadline event initialization error: {err}"),
            Ok(None) => {}
            Ok(Some(status)) => panic!("unexpected initial deadline event: {status:?}"),
        }
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(&topic, qos)
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if publisher.get_subscription_count().unwrap() > 0 {
                break;
            }
        }
        publisher
            .publish(&crate::std_msgs::msg::Int32 { data: 1 })
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if subscription.next().now_or_never().flatten().is_some() {
                break;
            }
        }
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(status) = subscription
                .take_requested_deadline_missed(&mut node)
                .unwrap()
            {
                assert!(status.total_count >= 1);
                assert!(status.total_count_change >= 1);
                return;
            }
        }
        panic!("subscription did not report a missed deadline");
    }

    #[test]
    fn subscriptions_report_liveliness_changes() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "subscription_liveliness_event_test", "").unwrap();
        let topic = format!("/subscription_liveliness_event_{}", uuid::Uuid::new_v4().simple());
        let qos = QosProfile::default()
            .liveliness(crate::qos::LivelinessPolicy::ManualByTopic)
            .liveliness_lease_duration(Duration::from_millis(50));
        let subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(&topic, qos.clone())
            .unwrap();
        match subscription.take_liveliness_changed(&mut node) {
            Err(Error::RCL_RET_UNSUPPORTED) => return,
            Err(err) => panic!("unexpected liveliness event initialization error: {err}"),
            Ok(None) => {}
            Ok(Some(status)) => panic!("unexpected initial liveliness event: {status:?}"),
        }
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(&topic, qos)
            .unwrap();
        publisher
            .publish(&crate::std_msgs::msg::Int32 { data: 1 })
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(status) = subscription.take_liveliness_changed(&mut node).unwrap() {
                assert!(status.alive_count >= 1);
                assert!(status.alive_count_change >= 1);
                assert!(subscription
                    .take_liveliness_changed(&mut node)
                    .unwrap()
                    .is_none());
                assert!(publisher.get_subscription_count().unwrap() >= 1);
                return;
            }
        }
        panic!("subscription did not report publisher liveliness");
    }

    #[test]
    fn subscription_message_lost_status_is_nonblocking() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "subscription_message_lost_test", "").unwrap();
        let subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(
                "/subscription_message_lost",
                QosProfile::default(),
            )
            .unwrap();

        match subscription.take_message_lost(&mut node) {
            Ok(None) | Err(Error::RCL_RET_UNSUPPORTED) => {}
            Ok(Some(status)) => panic!("unexpected initial message-lost event: {status:?}"),
            Err(err) => panic!("unexpected message-lost event initialization error: {err}"),
        }
    }

    #[test]
    fn publishers_report_offered_incompatible_qos() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "publisher_qos_event_test", "").unwrap();
        let topic = format!("/publisher_qos_event_{}", uuid::Uuid::new_v4().simple());
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(
                &topic,
                QosProfile::default().best_effort(),
            )
            .unwrap();

        match publisher.take_offered_incompatible_qos() {
            Err(Error::RCL_RET_UNSUPPORTED) => return,
            Err(err) => panic!("unexpected QoS event initialization error: {err}"),
            Ok(None) => {}
            Ok(Some(status)) => panic!("unexpected initial QoS event: {status:?}"),
        }

        let _subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default().reliable())
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(status) = publisher.take_offered_incompatible_qos().unwrap() {
                assert!(status.total_count >= 1);
                assert!(status.total_count_change >= 1);
                assert_eq!(status.last_policy_kind, crate::QosPolicyKind::Reliability);
                assert!(publisher.take_offered_incompatible_qos().unwrap().is_none());
                return;
            }
        }
        panic!("publisher did not report incompatible QoS");
    }

    #[test]
    fn untyped_publishers_report_offered_incompatible_qos() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "untyped_publisher_qos_event_test", "").unwrap();
        let topic = format!("/untyped_publisher_qos_event_{}", uuid::Uuid::new_v4().simple());
        let publisher = node
            .create_publisher_untyped_with_options(
                &topic,
                "std_msgs/msg/Int32",
                QosProfile::default().best_effort(),
                PublisherOptions::default(),
            )
            .unwrap();

        match publisher.take_offered_incompatible_qos() {
            Err(Error::RCL_RET_UNSUPPORTED) => return,
            Err(err) => panic!("unexpected QoS event initialization error: {err}"),
            Ok(None) => {}
            Ok(Some(status)) => panic!("unexpected initial QoS event: {status:?}"),
        }

        let _subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default().reliable())
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(status) = publisher.take_offered_incompatible_qos().unwrap() {
                assert!(status.total_count >= 1);
                assert!(status.total_count_change >= 1);
                assert_eq!(status.last_policy_kind, crate::QosPolicyKind::Reliability);
                return;
            }
        }
        panic!("untyped publisher did not report incompatible QoS");
    }

    #[test]
    fn publishers_report_offered_deadline_misses() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "publisher_deadline_event_test", "").unwrap();
        let topic = format!("/publisher_deadline_event_{}", uuid::Uuid::new_v4().simple());
        let qos = QosProfile::default().deadline(Duration::from_millis(20));
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(&topic, qos.clone())
            .unwrap();
        match publisher.take_offered_deadline_missed() {
            Err(Error::RCL_RET_UNSUPPORTED) => return,
            Err(err) => panic!("unexpected deadline event initialization error: {err}"),
            Ok(None) => {}
            Ok(Some(status)) => panic!("unexpected initial deadline event: {status:?}"),
        }
        let _subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(&topic, qos)
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if publisher.get_subscription_count().unwrap() > 0 {
                break;
            }
        }
        publisher
            .publish(&crate::std_msgs::msg::Int32 { data: 1 })
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(status) = publisher.take_offered_deadline_missed().unwrap() {
                assert!(status.total_count >= 1);
                assert!(status.total_count_change >= 1);
                assert!(publisher.take_offered_deadline_missed().unwrap().is_none());
                return;
            }
        }
        panic!("publisher did not report a missed deadline");
    }

    #[test]
    fn publishers_report_liveliness_lost() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "publisher_liveliness_event_test", "").unwrap();
        let topic = format!("/publisher_liveliness_event_{}", uuid::Uuid::new_v4().simple());
        let qos = QosProfile::default()
            .liveliness(crate::qos::LivelinessPolicy::ManualByTopic)
            .liveliness_lease_duration(Duration::from_millis(50));
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(&topic, qos.clone())
            .unwrap();
        match publisher.take_liveliness_lost() {
            Err(Error::RCL_RET_UNSUPPORTED) => return,
            Err(err) => panic!("unexpected liveliness event initialization error: {err}"),
            Ok(None) => {}
            Ok(Some(status)) => panic!("unexpected initial liveliness event: {status:?}"),
        }
        let _subscription = node
            .subscribe::<crate::std_msgs::msg::Int32>(&topic, qos)
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if publisher.get_subscription_count().unwrap() > 0 {
                break;
            }
        }
        publisher
            .publish(&crate::std_msgs::msg::Int32 { data: 1 })
            .unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(status) = publisher.take_liveliness_lost().unwrap() {
                assert!(status.total_count >= 1);
                assert!(status.total_count_change >= 1);
                assert!(publisher.take_liveliness_lost().unwrap().is_none());
                return;
            }
        }
        panic!("publisher did not report liveliness loss");
    }

    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    #[test]
    fn sim_time_remains_statically_typed_when_set_before_handler_creation() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "early_sim_time_test", "").unwrap();

        node.set_parameter("use_sim_time", ParameterValue::Bool(false))
            .unwrap();
        let descriptor = node.get_parameter_descriptor("use_sim_time").unwrap();
        assert_eq!(descriptor.type_, rcl_interfaces::msg::ParameterType::PARAMETER_BOOL as u8);
        assert!(!descriptor.dynamic_typing);

        let (_handler, _events) = node.make_parameter_handler().unwrap();
        assert!(node
            .set_parameter("use_sim_time", ParameterValue::NotSet)
            .is_err());
        assert!(node
            .set_parameter("use_sim_time", ParameterValue::String("false".into()))
            .is_err());
        assert!(!node.get_parameter::<bool>("use_sim_time").unwrap());
        assert!(
            !node
                .get_parameter_descriptor("use_sim_time")
                .unwrap()
                .dynamic_typing
        );
    }

    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    #[test]
    fn node_now_follows_its_managed_ros_clock() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "node_now_test", "").unwrap();

        assert!(node.now().unwrap() > Duration::ZERO);
        node.get_time_source().enable_sim_time(&mut node).unwrap();
        assert_eq!(node.now().unwrap(), Duration::ZERO);

        let expected = Duration::new(42, 123);
        node.ros_clock
            .lock()
            .set_ros_time_override(i64::try_from(expected.as_nanos()).unwrap())
            .unwrap();
        assert_eq!(node.now().unwrap(), expected);
        assert_eq!(node.now_ns().unwrap(), i64::try_from(expected.as_nanos()).unwrap());

        node.ros_clock.lock().set_ros_time_override(-1).unwrap();
        assert_eq!(node.now_ns().unwrap(), -1);
        assert!(matches!(node.now(), Err(Error::RCL_RET_INVALID_ARGUMENT)));
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
        use rcl_interfaces::{
            msg,
            srv::{DescribeParameters, ListParameters, SetParameters},
        };

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
        let list_client = node
            .create_client::<ListParameters::Service>(
                "parameter_event_test/list_parameters",
                QosProfile::default(),
            )
            .unwrap();
        let describe_client = node
            .create_client::<DescribeParameters::Service>(
                "parameter_event_test/describe_parameters",
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
        assert!(!node.has_parameter("dynamic_value"));
        assert!(node.get_parameter_descriptor("dynamic_value").is_err());

        let mut listed = Box::pin(
            list_client
                .request(&ListParameters::Request {
                    prefixes: vec![],
                    depth: ListParameters::Request::DEPTH_RECURSIVE as u64,
                })
                .unwrap(),
        );
        let listed = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            pool.run_until_stalled();
            if let Some(response) = listed.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert!(!listed
            .result
            .names
            .iter()
            .any(|name| name == "dynamic_value"));

        let mut described = Box::pin(
            describe_client
                .request(&DescribeParameters::Request {
                    names: vec!["dynamic_value".into()],
                })
                .unwrap(),
        );
        let described = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            pool.run_until_stalled();
            if let Some(response) = described.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert_eq!(described.descriptors[0].type_, msg::ParameterType::PARAMETER_NOT_SET as u8);
        assert!(!described.descriptors[0].dynamic_typing);
    }

    #[test]
    fn parameter_services_expose_and_enforce_descriptors() {
        use rcl_interfaces::{
            msg,
            srv::{DescribeParameters, SetParameters, SetParametersAtomically},
        };

        fn parameter(name: &str, value: ParameterValue) -> msg::Parameter {
            msg::Parameter {
                name: name.into(),
                value: value.into_parameter_value_msg(),
            }
        }

        fn drive<T>(
            node: &mut Node, pool: &mut LocalPool, future: impl Future<Output = Result<T>>,
        ) -> T {
            let mut future = Box::pin(future);
            loop {
                node.try_spin_once(Duration::from_millis(10)).unwrap();
                pool.run_until_stalled();
                if let Some(result) = future.as_mut().now_or_never() {
                    return result.unwrap();
                }
            }
        }

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "parameter_descriptor_test", "").unwrap();
        {
            let mut params = node.params.lock();
            params.insert("read_only".into(), Parameter::new(ParameterValue::Integer(7)));
            params.insert("count".into(), Parameter::new(ParameterValue::Integer(2)));
            params.insert("ratio".into(), Parameter::new(ParameterValue::Double(0.2)));
            params
                .insert("dynamic".into(), Parameter::new(ParameterValue::String("initial".into())));
        }
        node.set_parameter_descriptor(
            "read_only",
            ParameterDescriptor {
                description: "immutable value".into(),
                additional_constraints: "configured during startup".into(),
                read_only: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(matches!(
            node.set_parameter_descriptor(
                "read_only",
                ParameterDescriptor {
                    read_only: false,
                    dynamic_typing: true,
                    description: "replacement".into(),
                    ..Default::default()
                },
            ),
            Err(Error::ParameterConstraintViolation { .. })
        ));
        let read_only_descriptor = node.get_parameter_descriptor("read_only").unwrap();
        assert!(read_only_descriptor.read_only);
        assert!(!read_only_descriptor.dynamic_typing);
        assert_eq!(read_only_descriptor.description, "immutable value");
        node.set_parameter_descriptor(
            "count",
            ParameterDescriptor {
                integer_range: vec![IntegerRange {
                    from_value: 0,
                    to_value: 10,
                    step: 2,
                }],
                ..Default::default()
            },
        )
        .unwrap();
        node.set_parameter_descriptor(
            "ratio",
            ParameterDescriptor {
                floating_point_range: vec![FloatingPointRange {
                    from_value: 0.0,
                    to_value: 1.0,
                    step: 0.1,
                }],
                ..Default::default()
            },
        )
        .unwrap();
        node.set_parameter_descriptor(
            "dynamic",
            ParameterDescriptor {
                dynamic_typing: true,
                ..Default::default()
            },
        )
        .unwrap();

        let (handler, _events) = node.make_parameter_handler().unwrap();
        let describe = node
            .create_client::<DescribeParameters::Service>(
                "parameter_descriptor_test/describe_parameters",
                QosProfile::default(),
            )
            .unwrap();
        let set = node
            .create_client::<SetParameters::Service>(
                "parameter_descriptor_test/set_parameters",
                QosProfile::default(),
            )
            .unwrap();
        let set_atomically = node
            .create_client::<SetParametersAtomically::Service>(
                "parameter_descriptor_test/set_parameters_atomically",
                QosProfile::default(),
            )
            .unwrap();
        let mut pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();

        let response = drive(
            &mut node,
            &mut pool,
            describe
                .request(&DescribeParameters::Request {
                    names: vec!["read_only".into(), "count".into()],
                })
                .unwrap(),
        );
        assert_eq!(response.descriptors[0].name, "read_only");
        assert_eq!(response.descriptors[0].description, "immutable value");
        assert_eq!(response.descriptors[0].additional_constraints, "configured during startup");
        assert!(response.descriptors[0].read_only);
        assert_eq!(response.descriptors[1].integer_range[0].step, 2);

        let response = drive(
            &mut node,
            &mut pool,
            set.request(&SetParameters::Request {
                parameters: vec![
                    parameter("read_only", ParameterValue::Integer(8)),
                    parameter("count", ParameterValue::Integer(3)),
                    parameter("count", ParameterValue::String("wrong".into())),
                    parameter("count", ParameterValue::Integer(6)),
                    parameter("dynamic", ParameterValue::Integer(42)),
                ],
            })
            .unwrap(),
        );
        assert!(!response.results[0].successful);
        assert!(!response.results[1].successful);
        assert!(!response.results[2].successful);
        assert!(response.results[3].successful);
        assert!(response.results[4].successful);
        assert_eq!(node.get_parameter::<i64>("read_only").unwrap(), 7);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 6);
        assert_eq!(node.get_parameter::<i64>("dynamic").unwrap(), 42);
        assert_eq!(
            node.get_parameter_descriptor("dynamic").unwrap().type_,
            msg::ParameterType::PARAMETER_INTEGER as u8
        );

        let response = drive(
            &mut node,
            &mut pool,
            set_atomically
                .request(&SetParametersAtomically::Request {
                    parameters: vec![
                        parameter("count", ParameterValue::Integer(8)),
                        parameter("dynamic", ParameterValue::NotSet),
                    ],
                })
                .unwrap(),
        );
        assert!(response.result.successful);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 8);
        assert!(!node.has_parameter("dynamic"));
        assert!(node.get_parameter_descriptor("dynamic").is_err());

        let response = drive(
            &mut node,
            &mut pool,
            set.request(&SetParameters::Request {
                parameters: vec![parameter("dynamic", ParameterValue::Bool(true))],
            })
            .unwrap(),
        );
        assert!(response.results[0].successful);
        assert!(node.get_parameter::<bool>("dynamic").unwrap());
        let descriptor = node.get_parameter_descriptor("dynamic").unwrap();
        assert!(descriptor.dynamic_typing);
        assert_eq!(descriptor.type_, msg::ParameterType::PARAMETER_BOOL as u8);

        let malformed = msg::ParameterValue {
            type_: u8::MAX,
            integer_value: 10,
            ..Default::default()
        };
        let response = drive(
            &mut node,
            &mut pool,
            set.request(&SetParameters::Request {
                parameters: vec![msg::Parameter {
                    name: "count".into(),
                    value: malformed.clone(),
                }],
            })
            .unwrap(),
        );
        assert!(!response.results[0].successful);
        assert!(response.results[0]
            .reason
            .contains("Unknown ROS parameter type"));
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 8);

        let response = drive(
            &mut node,
            &mut pool,
            set_atomically
                .request(&SetParametersAtomically::Request {
                    parameters: vec![
                        parameter("count", ParameterValue::Integer(10)),
                        parameter("ratio", ParameterValue::Double(0.25)),
                    ],
                })
                .unwrap(),
        );
        assert!(!response.result.successful);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 8);
        assert_eq!(node.get_parameter::<f64>("ratio").unwrap(), 0.2);

        let response = drive(
            &mut node,
            &mut pool,
            set_atomically
                .request(&SetParametersAtomically::Request {
                    parameters: vec![
                        parameter("count", ParameterValue::Integer(10)),
                        msg::Parameter {
                            name: "ratio".into(),
                            value: malformed,
                        },
                    ],
                })
                .unwrap(),
        );
        assert!(!response.result.successful);
        assert!(response
            .result
            .reason
            .contains("Unknown ROS parameter type"));
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 8);
        assert_eq!(node.get_parameter::<f64>("ratio").unwrap(), 0.2);

        let response = drive(
            &mut node,
            &mut pool,
            set_atomically
                .request(&SetParametersAtomically::Request {
                    parameters: vec![
                        parameter("count", ParameterValue::Integer(10)),
                        parameter("ratio", ParameterValue::Double(0.4)),
                    ],
                })
                .unwrap(),
        );
        assert!(response.result.successful);
        assert_eq!(node.get_parameter::<i64>("count").unwrap(), 10);
        assert_eq!(node.get_parameter::<f64>("ratio").unwrap(), 0.4);
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
    fn untyped_action_servers_interoperate_with_typed_and_untyped_clients() {
        use crate::example_interfaces::action::Fibonacci;
        use crate::GoalStatus;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "untyped_action_server_test", "/scope").unwrap();
        let mut requests = node
            .create_action_server_untyped_with_options(
                "fibonacci",
                "example_interfaces/action/Fibonacci",
                ActionServerOptions::default().goal_request_channel_capacity(0),
            )
            .unwrap();
        let client = node
            .create_action_client::<Fibonacci::Action>("fibonacci")
            .unwrap();
        let response = client
            .send_goal_request(Fibonacci::Goal { order: 3 })
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let mut cancel_requests = None;
        let (client_goal, result, mut feedback) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_none() {
                if let Some(request) = requests.next().now_or_never().flatten() {
                    assert_eq!(request.action_name().unwrap(), "/scope/fibonacci");
                    assert_eq!(request.goal.as_ref().unwrap(), &serde_json::json!({ "order": 3 }));
                    let (goal, cancels) = request.accept_deferred().unwrap();
                    server_goal = Some(goal);
                    cancel_requests = Some(cancels);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };

        let mut goal = server_goal.unwrap();
        assert!(client_goal.accepted_at().nanosec < 1_000_000_000);
        assert_eq!(client_goal.clone().accepted_at(), client_goal.accepted_at());
        assert_eq!(goal.action_name().unwrap(), "/scope/fibonacci");
        assert_eq!(goal.goal.as_ref().unwrap(), &serde_json::json!({ "order": 3 }));
        assert_eq!(goal.get_status().unwrap(), GoalStatus::Accepted);
        assert!(matches!(
            goal.publish_feedback(serde_json::json!({ "sequence": "invalid" })),
            Err(Error::SerdeError { .. })
        ));
        assert!(matches!(
            goal.succeed(serde_json::json!({ "sequence": "invalid" })),
            Err(Error::SerdeError { .. })
        ));
        assert_eq!(goal.get_status().unwrap(), GoalStatus::Accepted);
        goal.execute().unwrap();
        goal.publish_feedback(serde_json::json!({ "sequence": [0, 1, 1] }))
            .unwrap();
        goal.succeed(serde_json::json!({ "sequence": [0, 1, 1, 2] }))
            .unwrap();

        let mut result = Box::pin(result);
        let mut received_feedback = None;
        let mut received_result = None;
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            received_feedback =
                received_feedback.or_else(|| feedback.next().now_or_never().flatten());
            received_result = received_result.or_else(|| result.as_mut().now_or_never());
            if received_feedback.is_some() && received_result.is_some() {
                break;
            }
        }
        assert_eq!(received_feedback.unwrap().sequence, vec![0, 1, 1]);
        let (status, result) = received_result.unwrap().unwrap();
        assert_eq!(status, GoalStatus::Succeeded);
        assert_eq!(result.sequence, vec![0, 1, 1, 2]);
        assert_eq!(client_goal.get_status().unwrap(), GoalStatus::Succeeded);
        drop(cancel_requests);

        let cancel_name = "cancel_fibonacci";
        let mut cancel_requests_stream = node
            .create_action_server_untyped(cancel_name, "example_interfaces/action/Fibonacci")
            .unwrap();
        let cancel_client = node
            .create_action_client_untyped(cancel_name, "example_interfaces/action/Fibonacci")
            .unwrap();
        let response = cancel_client
            .send_goal_request(serde_json::json!({ "order": 2 }))
            .unwrap();
        let mut response = Box::pin(response);
        let mut cancel_server_goal = None;
        let mut server_cancels = None;
        let (cancel_client_goal, cancel_result) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if cancel_server_goal.is_none() {
                if let Some(request) = cancel_requests_stream.next().now_or_never().flatten() {
                    let (goal, cancels) = request.accept().unwrap();
                    cancel_server_goal = Some(goal);
                    server_cancels = Some(cancels);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                let (goal, result, _feedback) = response.unwrap();
                break (goal, result);
            }
        };
        let cancel = cancel_client.cancel_all().unwrap();
        let mut cancel = Box::pin(cancel);
        let mut cancel_request = None;
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            cancel_request = cancel_request.or_else(|| {
                server_cancels
                    .as_mut()
                    .unwrap()
                    .next()
                    .now_or_never()
                    .flatten()
            });
            if cancel_request.is_some() {
                break;
            }
        }
        cancel_request.unwrap().accept();
        let mut cancel_server_goal = cancel_server_goal.unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if cancel_server_goal.is_cancelling().unwrap() {
                break;
            }
        }
        cancel_server_goal
            .cancel(serde_json::json!({ "sequence": [0, 1, 1] }))
            .unwrap();
        let mut cancel_result = Box::pin(cancel_result);
        let mut cancel_response = None;
        let mut result_response = None;
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            cancel_response = cancel_response.or_else(|| cancel.as_mut().now_or_never());
            result_response = result_response.or_else(|| cancel_result.as_mut().now_or_never());
            if cancel_response.is_some() && result_response.is_some() {
                break;
            }
        }
        assert_eq!(cancel_response.unwrap().unwrap(), vec![cancel_client_goal.uuid]);
        let (status, result) = result_response.unwrap().unwrap();
        assert_eq!(status, GoalStatus::Canceled);
        assert_eq!(result.unwrap(), serde_json::json!({ "sequence": [0, 1, 1] }));

        let invalid =
            node.create_action_server_untyped("invalid", "missing_interfaces/action/Nope");
        assert!(matches!(invalid, Err(Error::InvalidMessageType { .. })));
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
        assert_eq!(flow_client.action_name().unwrap(), flow_name);
        let response = flow_client
            .send_goal_request(Fibonacci::Goal { order: 3 })
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let mut cancel_stream = None;
        let (client_goal, result, mut feedback) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_none() {
                if let Some(request) = flow_requests.next().now_or_never().flatten() {
                    assert_eq!(request.action_name().unwrap(), flow_name);
                    let (goal, cancels) = request.accept().unwrap();
                    assert_eq!(goal.action_name().unwrap(), flow_name);
                    server_goal = Some(goal);
                    cancel_stream = Some(cancels);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert!(client_goal.is_feedback_aware().unwrap());
        assert!(client_goal.is_result_aware().unwrap());
        let client_goal_clone = client_goal.clone();
        drop(result);
        assert!(!client_goal.is_result_aware().unwrap());
        assert!(!client_goal_clone.is_result_aware().unwrap());
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
        drop(feedback);
        assert!(!client_goal.is_feedback_aware().unwrap());
        assert!(!client_goal_clone.is_feedback_aware().unwrap());

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
        assert_eq!(untyped_client.action_name().unwrap(), untyped_name);
        let response = untyped_client
            .send_goal_request(serde_json::json!({ "order": 4 }))
            .unwrap();
        let mut response = Box::pin(response);
        let mut untyped_server_goal = None;
        let (untyped_client_goal, untyped_result, mut untyped_feedback) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if untyped_server_goal.is_none() {
                if let Some(request) = untyped_requests.next().now_or_never().flatten() {
                    let (goal, _cancels) = request.accept().unwrap();
                    untyped_server_goal = Some(goal);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert!(untyped_client_goal.accepted_at().nanosec < 1_000_000_000);
        assert_eq!(untyped_client_goal.clone().accepted_at(), untyped_client_goal.accepted_at());
        assert!(untyped_client_goal.is_feedback_aware().unwrap());
        assert!(untyped_client_goal.is_result_aware().unwrap());
        let untyped_client_goal_clone = untyped_client_goal.clone();
        drop(untyped_result);
        assert!(!untyped_client_goal.is_result_aware().unwrap());
        assert!(!untyped_client_goal_clone.is_result_aware().unwrap());
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
        drop(untyped_feedback);
        assert!(!untyped_client_goal.is_feedback_aware().unwrap());
        assert!(!untyped_client_goal_clone.is_feedback_aware().unwrap());
    }

    #[test]
    fn invalid_action_names_report_action_name_errors() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "invalid_action_name_test", "").unwrap();

        assert!(matches!(
            node.create_action_client::<Fibonacci::Action>("invalid action name"),
            Err(Error::RCL_RET_ACTION_NAME_INVALID)
        ));
        assert!(matches!(
            node.create_action_server::<Fibonacci::Action>("invalid action name"),
            Err(Error::RCL_RET_ACTION_NAME_INVALID)
        ));
    }

    #[test]
    fn action_client_drains_goal_response_bursts_in_one_spin() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut server_node =
            Node::create(context.clone(), "action_goal_batch_server", "").unwrap();
        let mut client_node = Node::create(context, "action_goal_batch_client", "").unwrap();
        let action_name = format!("/action_goal_batch_{}", uuid::Uuid::new_v4().simple());
        let mut server = server_node
            .create_action_server_with_options::<Fibonacci::Action>(
                &action_name,
                ActionServerOptions::default().goal_request_channel_capacity(8),
            )
            .unwrap();
        let client = client_node
            .create_action_client::<Fibonacci::Action>(&action_name)
            .unwrap();
        let available = Node::is_available(&client).unwrap();
        client_node
            .spin_until_future_complete(available, Duration::from_secs(1))
            .unwrap();

        let mut responses = (1..=3)
            .map(|order| {
                Box::pin(
                    client
                        .send_goal_request_with_options(
                            Fibonacci::Goal { order },
                            ActionGoalOptions::fire_and_forget(),
                        )
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        std::thread::sleep(Duration::from_millis(20));
        let mut rejected = 0;
        while rejected < 3 {
            server_node.try_spin_once(Duration::ZERO).unwrap();
            while let Some(request) = server.next().now_or_never().flatten() {
                request.reject().unwrap();
                rejected += 1;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
        client_node
            .try_spin_once(Duration::from_millis(100))
            .unwrap();

        assert!(responses.iter_mut().all(|response| matches!(
            response.as_mut().now_or_never(),
            Some(Err(Error::RCL_RET_ACTION_GOAL_REJECTED))
        )));
    }

    #[test]
    fn action_server_drains_goal_request_bursts_in_one_spin() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut server_node =
            Node::create(context.clone(), "action_request_batch_server", "").unwrap();
        let mut client_node = Node::create(context, "action_request_batch_client", "").unwrap();
        let action_name = format!("/action_request_batch_{}", uuid::Uuid::new_v4().simple());
        let qos = QosProfile::default().keep_last(8);
        let mut server = server_node
            .create_action_server_with_options::<Fibonacci::Action>(
                &action_name,
                ActionServerOptions::default()
                    .goal_service_qos(qos.clone())
                    .goal_request_channel_capacity(8),
            )
            .unwrap();
        let client = client_node
            .create_action_client_with_options::<Fibonacci::Action>(
                &action_name,
                ActionClientOptions::default().goal_service_qos(qos),
            )
            .unwrap();
        for _ in 0..200 {
            server_node.try_spin_once(Duration::ZERO).unwrap();
            client_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            if client.is_available(&client_node).unwrap() {
                break;
            }
        }
        assert!(client.is_available(&client_node).unwrap());

        let responses = (1..=3)
            .map(|order| {
                client
                    .send_goal_request_with_options(
                        Fibonacci::Goal { order },
                        ActionGoalOptions::fire_and_forget(),
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        std::thread::sleep(Duration::from_millis(20));
        server_node
            .try_spin_once(Duration::from_millis(100))
            .unwrap();

        let orders = (0..3)
            .map(|_| server.next().now_or_never().flatten().unwrap().goal.order)
            .collect::<Vec<_>>();
        assert_eq!(orders, vec![1, 2, 3]);
        drop(responses);
    }

    #[test]
    fn action_client_drains_feedback_bursts_in_one_spin() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut server_node =
            Node::create(context.clone(), "action_feedback_batch_server", "").unwrap();
        let mut client_node = Node::create(context, "action_feedback_batch_client", "").unwrap();
        let action_name = format!("/action_feedback_batch_{}", uuid::Uuid::new_v4().simple());
        let qos = QosProfile::default().keep_last(8);
        let mut server = server_node
            .create_action_server_with_options::<Fibonacci::Action>(
                &action_name,
                ActionServerOptions::default().feedback_topic_qos(qos.clone()),
            )
            .unwrap();
        let client = client_node
            .create_action_client_with_options::<Fibonacci::Action>(
                &action_name,
                ActionClientOptions::default()
                    .feedback_topic_qos(qos)
                    .feedback_channel_capacity(8),
            )
            .unwrap();
        for _ in 0..200 {
            server_node.try_spin_once(Duration::ZERO).unwrap();
            client_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            if client.is_available(&client_node).unwrap() {
                break;
            }
        }
        assert!(client.is_available(&client_node).unwrap());

        let response = client
            .send_goal_request_with_options(
                Fibonacci::Goal { order: 3 },
                ActionGoalOptions::default().track_result(false),
            )
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let response = loop {
            server_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            if server_goal.is_none() {
                if let Some(request) = server.next().now_or_never().flatten() {
                    server_goal = Some(request.accept().unwrap().0);
                }
            }
            client_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        let goal = server_goal.unwrap();
        for value in 1..=3 {
            goal.publish_feedback(Fibonacci::Feedback {
                sequence: vec![value],
            })
            .unwrap();
        }
        std::thread::sleep(Duration::from_millis(20));
        client_node
            .try_spin_once(Duration::from_millis(100))
            .unwrap();

        let mut feedback = response.feedback.unwrap();
        let values = (0..3)
            .map(|_| feedback.next().now_or_never().flatten().unwrap().sequence[0])
            .collect::<Vec<_>>();
        assert_eq!(values, vec![1, 2, 3]);
    }

    #[test]
    fn action_server_drains_result_request_bursts_in_one_spin() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut server_node =
            Node::create(context.clone(), "action_result_batch_server", "").unwrap();
        let mut client_node = Node::create(context, "action_result_batch_client", "").unwrap();
        let action_name = format!("/action_result_batch_{}", uuid::Uuid::new_v4().simple());
        let qos = QosProfile::default().keep_last(8);
        let mut server = server_node
            .create_action_server_with_options::<Fibonacci::Action>(
                &action_name,
                ActionServerOptions::default()
                    .goal_service_qos(qos.clone())
                    .result_service_qos(qos.clone())
                    .goal_request_channel_capacity(8),
            )
            .unwrap();
        let client = client_node
            .create_action_client_with_options::<Fibonacci::Action>(
                &action_name,
                ActionClientOptions::default()
                    .goal_service_qos(qos.clone())
                    .result_service_qos(qos),
            )
            .unwrap();
        for _ in 0..200 {
            server_node.try_spin_once(Duration::ZERO).unwrap();
            client_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            if client.is_available(&client_node).unwrap() {
                break;
            }
        }

        let mut responses = (1..=3)
            .map(|order| {
                Box::pin(
                    client
                        .send_goal_request_with_options(
                            Fibonacci::Goal { order },
                            ActionGoalOptions::default().track_feedback(false),
                        )
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        std::thread::sleep(Duration::from_millis(20));
        let mut server_goals = Vec::new();
        while server_goals.len() < 3 {
            server_node.try_spin_once(Duration::ZERO).unwrap();
            while let Some(request) = server.next().now_or_never().flatten() {
                server_goals.push(request.accept().unwrap().0);
            }
        }
        std::thread::sleep(Duration::from_millis(20));
        client_node
            .try_spin_once(Duration::from_millis(100))
            .unwrap();
        let mut results = responses
            .iter_mut()
            .map(|response| {
                response
                    .as_mut()
                    .now_or_never()
                    .unwrap()
                    .unwrap()
                    .result
                    .unwrap()
            })
            .collect::<Vec<_>>();

        std::thread::sleep(Duration::from_millis(20));
        server_node
            .try_spin_once(Duration::from_millis(100))
            .unwrap();
        for (index, goal) in server_goals.iter_mut().enumerate() {
            goal.succeed(Fibonacci::Result {
                sequence: vec![index as i32],
            })
            .unwrap();
        }
        std::thread::sleep(Duration::from_millis(20));
        client_node
            .try_spin_once(Duration::from_millis(100))
            .unwrap();

        let values = results
            .iter_mut()
            .map(|result| result.now_or_never().unwrap().unwrap().1.sequence[0])
            .collect::<Vec<_>>();
        assert_eq!(values, vec![0, 1, 2]);
    }

    #[test]
    fn action_cancel_batches_stop_at_zero_capacity_backpressure() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut server_node =
            Node::create(context.clone(), "action_cancel_batch_server", "").unwrap();
        let mut client_node = Node::create(context, "action_cancel_batch_client", "").unwrap();
        let action_name = format!("/action_cancel_batch_{}", uuid::Uuid::new_v4().simple());
        let mut requests = server_node
            .create_action_server_with_options::<Fibonacci::Action>(
                &action_name,
                ActionServerOptions::default().cancel_request_channel_capacity(0),
            )
            .unwrap();
        let client = client_node
            .create_action_client::<Fibonacci::Action>(&action_name)
            .unwrap();
        for _ in 0..200 {
            server_node.try_spin_once(Duration::ZERO).unwrap();
            client_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            if client.is_available(&client_node).unwrap() {
                break;
            }
        }

        let response = client
            .send_goal_request_with_options(
                Fibonacci::Goal { order: 1 },
                ActionGoalOptions::default().track_feedback(false),
            )
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let mut cancel_stream = None;
        let mut client_goal = None;
        for _ in 0..200 {
            server_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            if server_goal.is_none() {
                if let Some(request) = requests.next().now_or_never().flatten() {
                    let (goal, cancels) = request.accept().unwrap();
                    server_goal = Some(goal);
                    cancel_stream = Some(cancels);
                }
            }
            client_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            if let Some(response) = response.as_mut().now_or_never() {
                client_goal = Some(response.unwrap().goal);
                break;
            }
        }
        let client_goal = client_goal.expect("goal response was not delivered");
        assert!(server_goal.is_some());
        assert!(cancel_stream.is_some());

        let first = client_goal.cancel().unwrap();
        let second = client_goal.cancel().unwrap();
        std::thread::sleep(Duration::from_millis(20));
        let mut cancel_stream = cancel_stream.unwrap();
        let waker = futures::task::noop_waker_ref();
        let mut context = std::task::Context::from_waker(waker);
        assert!(matches!(Pin::new(&mut cancel_stream).poll_next(&mut context), Poll::Pending));

        let mut first_request = None;
        for _ in 0..200 {
            server_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            first_request = first_request.or_else(|| cancel_stream.next().now_or_never().flatten());
            if first_request.is_some() {
                break;
            }
        }
        let first_request = first_request.expect("first cancel request was not delivered");
        assert!(cancel_stream.next().now_or_never().is_none());
        first_request.reject();

        let mut second_request = None;
        for _ in 0..200 {
            server_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            second_request =
                second_request.or_else(|| cancel_stream.next().now_or_never().flatten());
            if second_request.is_some() {
                break;
            }
        }
        let second_request = second_request.expect("second cancel request was not delivered");
        second_request.reject();
        drop(first);
        drop(second);
        drop(server_goal);
    }

    #[test]
    fn action_server_drains_cancel_request_bursts_in_one_spin() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut server_node =
            Node::create(context.clone(), "action_cancel_burst_server", "").unwrap();
        let mut client_node = Node::create(context, "action_cancel_burst_client", "").unwrap();
        let action_name = format!("/action_cancel_burst_{}", uuid::Uuid::new_v4().simple());
        let qos = QosProfile::default().keep_last(8);
        let mut requests = server_node
            .create_action_server_with_options::<Fibonacci::Action>(
                &action_name,
                ActionServerOptions::default()
                    .goal_service_qos(qos.clone())
                    .cancel_service_qos(qos.clone())
                    .cancel_request_channel_capacity(8),
            )
            .unwrap();
        let server_id = requests.id;
        let client = client_node
            .create_action_client_with_options::<Fibonacci::Action>(
                &action_name,
                ActionClientOptions::default()
                    .goal_service_qos(qos.clone())
                    .cancel_service_qos(qos),
            )
            .unwrap();
        for _ in 0..200 {
            server_node.try_spin_once(Duration::ZERO).unwrap();
            client_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            if client.is_available(&client_node).unwrap() {
                break;
            }
        }

        let response = client
            .send_goal_request_with_options(
                Fibonacci::Goal { order: 1 },
                ActionGoalOptions::default().track_feedback(false),
            )
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let mut cancel_stream = None;
        let client_goal = loop {
            server_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            if server_goal.is_none() {
                if let Some(request) = requests.next().now_or_never().flatten() {
                    let (goal, cancels) = request.accept().unwrap();
                    server_goal = Some(goal);
                    cancel_stream = Some(cancels);
                }
            }
            client_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap().goal;
            }
        };

        let cancels = [
            client_goal.cancel().unwrap(),
            client_goal.cancel().unwrap(),
            client_goal.cancel().unwrap(),
        ];
        std::thread::sleep(Duration::from_millis(20));
        server_node
            .try_spin_once(Duration::from_millis(100))
            .unwrap();
        assert!(server_node
            .action_servers_with_pending_cancels
            .contains(&server_id));

        let mut cancel_stream = cancel_stream.unwrap();
        for _ in 0..3 {
            cancel_stream
                .next()
                .now_or_never()
                .flatten()
                .unwrap()
                .reject();
        }
        server_node.try_spin_once(Duration::ZERO).unwrap();
        assert!(!server_node
            .action_servers_with_pending_cancels
            .contains(&server_id));
        drop(cancels);
        drop(server_goal);
    }

    #[test]
    fn availability_waits_support_runtime_agnostic_timeouts() {
        use crate::example_interfaces::{action::Fibonacci, srv::AddTwoInts};

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
        let timeout = missing
            .wait_for_service_with_timeout(&mut node, Duration::from_millis(1))
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
        let wait = available
            .wait_for_service_with_timeout(&mut node, Duration::from_secs(1))
            .unwrap();
        drive(&mut node, wait).unwrap();

        let untyped = node
            .create_client_untyped(
                "/available_timeout_service",
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();
        let wait = untyped.wait_for_service().unwrap();
        drive(&mut node, wait).unwrap();

        let _action_server = node
            .create_action_server::<Fibonacci::Action>("/available_timeout_action")
            .unwrap();
        let action_client = node
            .create_action_client::<Fibonacci::Action>("/available_timeout_action")
            .unwrap();
        let wait = action_client
            .wait_for_action_server_with_timeout(&mut node, Duration::from_secs(1))
            .unwrap();
        drive(&mut node, wait).unwrap();

        let untyped_action_client = node
            .create_action_client_untyped(
                "/available_timeout_action",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        let wait = untyped_action_client.wait_for_action_server().unwrap();
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
    fn publisher_timeout_helpers_reject_foreign_nodes_before_registering() {
        let context = Context::create().unwrap();
        let mut owner = Node::create(context.clone(), "publisher_timeout_owner", "").unwrap();
        let mut other = Node::create(context, "publisher_timeout_other", "").unwrap();
        let typed = owner
            .create_publisher::<crate::std_msgs::msg::String>(
                "/foreign_node_typed_publisher",
                QosProfile::default(),
            )
            .unwrap();
        let untyped = owner
            .create_publisher_untyped(
                "/foreign_node_untyped_publisher",
                "std_msgs/msg/String",
                QosProfile::default(),
            )
            .unwrap();
        let timer_count = other.timers.len();

        assert!(matches!(
            typed.wait_for_subscribers_with_timeout(&mut other, Duration::from_secs(1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            untyped.wait_for_subscribers_with_timeout(&mut other, Duration::from_secs(1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
        {
            assert!(matches!(
                typed.wait_for_all_acked_async_with_timeout(&mut other, Duration::from_secs(1),),
                Err(Error::RCL_RET_INVALID_ARGUMENT)
            ));
            assert!(matches!(
                untyped.wait_for_all_acked_async_with_timeout(&mut other, Duration::from_secs(1),),
                Err(Error::RCL_RET_INVALID_ARGUMENT)
            ));
        }

        assert_eq!(other.timers.len(), timer_count);
        assert!(!typed.handle.upgrade().unwrap().has_pending_waits());
        assert!(!untyped.handle.upgrade().unwrap().has_pending_waits());
        assert!(owner.active_wait_publishers.lock().is_empty());
    }

    #[test]
    fn oversized_wait_timeouts_do_not_register_waiters() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "oversized_wait_timeout_test", "").unwrap();
        let oversized = Duration::from_nanos(i64::MAX as u64 + 1);
        let client = node
            .create_client::<AddTwoInts::Service>(
                "/oversized_availability_timeout",
                QosProfile::default(),
            )
            .unwrap();
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/oversized_publisher_timeout",
                QosProfile::default(),
            )
            .unwrap();
        let subscription = node
            .subscribe::<crate::std_msgs::msg::String>(
                "/oversized_subscription_timeout",
                QosProfile::default(),
            )
            .unwrap();
        let timer_count = node.timer_count();

        assert!(matches!(
            node.is_available_with_timeout(&client, oversized),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            publisher.wait_for_subscribers_with_timeout(&mut node, oversized),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
        assert!(matches!(
            publisher.wait_for_all_acked_async_with_timeout(&mut node, oversized),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            subscription.wait_for_publishers_with_timeout(&mut node, oversized),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            node.wait_for_graph_change_with_timeout(oversized),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));

        assert_eq!(node.timer_count(), timer_count);
        assert!(!publisher.handle.upgrade().unwrap().has_pending_waits());
        assert!(node.active_wait_publishers.lock().is_empty());
        assert!(node.graph_change_waiters.is_empty());
        assert!(node
            .subscriber(subscription.id)
            .unwrap()
            .match_state()
            .unwrap()
            .waiters
            .is_empty());
    }

    #[test]
    fn clients_can_check_availability_without_registering_waiters() {
        use crate::example_interfaces::{action::Fibonacci, srv::AddTwoInts};

        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "immediate_availability_test", "").unwrap();
        let other = Node::create(context, "immediate_availability_other", "").unwrap();

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
        assert!(matches!(client.is_available(&other), Err(Error::RCL_RET_INVALID_ARGUMENT)));
        assert!(matches!(
            untyped_client.is_available(&other),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
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
        assert!(matches!(
            action_client.is_available(&other),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            untyped_action_client.is_available(&other),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
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
    fn service_availability_waiters_poll_only_the_active_client() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "client_sparse_availability_test", "").unwrap();
        let active = node
            .create_client::<AddTwoInts::Service>(
                "/active_client_availability",
                QosProfile::default(),
            )
            .unwrap();
        let _idle = node
            .create_client::<AddTwoInts::Service>(
                "/idle_client_availability",
                QosProfile::default(),
            )
            .unwrap();
        let active_state = active.client.upgrade().unwrap();
        let active_identity = Node::client_identity(&active_state);
        let active_index = node.client_identities[&active_identity];
        let idle_index = usize::from(active_index == 0);

        let wait = Node::is_available(&active).unwrap();
        let second_wait = Node::is_available(&active).unwrap();
        assert_eq!(
            node.active_availability_clients
                .lock()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [active_identity]
        );
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.clients_with_active_availability, HashSet::from([active_identity]));
        assert_eq!(node.clients[active_index].lock().availability_poll_count(), 1);
        assert_eq!(node.clients[idle_index].lock().availability_poll_count(), 0);

        drop(wait);
        drop(second_wait);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.clients_with_active_availability.is_empty());
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.clients[active_index].lock().availability_poll_count(), 2);
        assert_eq!(node.clients[idle_index].lock().availability_poll_count(), 0);

        let untyped = node
            .create_client_untyped(
                "/active_untyped_client_availability",
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();
        let untyped_identity = untyped.identity();
        let first_untyped_wait = Node::is_available(&untyped).unwrap();
        let second_untyped_wait = Node::is_available(&untyped).unwrap();
        assert_eq!(
            node.active_availability_clients
                .lock()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [untyped_identity]
        );
        drop(first_untyped_wait);
        drop(second_untyped_wait);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(!node
            .clients_with_active_availability
            .contains(&untyped_identity));
    }

    #[test]
    fn explicit_client_destruction_is_immediate_and_completes_waiters() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "destroy_client_test", "").unwrap();
        let mut other = Node::create(context, "destroy_client_other", "").unwrap();
        let first_name = format!("/destroy_client_first_{}", uuid::Uuid::new_v4().simple());
        let removed_name = format!("/destroy_client_removed_{}", uuid::Uuid::new_v4().simple());
        let last_name = format!("/destroy_client_last_{}", uuid::Uuid::new_v4().simple());
        let first = node
            .create_client::<AddTwoInts::Service>(&first_name, QosProfile::default())
            .unwrap();
        let removed = node
            .create_client::<AddTwoInts::Service>(&removed_name, QosProfile::default())
            .unwrap();
        let removed_clone = removed.clone();
        let last = node
            .create_client_untyped(
                &last_name,
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();
        let removed_state = removed.client.upgrade().unwrap();
        let removed_identity = Node::client_identity(&removed_state);
        drop(removed_state);
        let response = removed
            .request(&AddTwoInts::Request { a: 20, b: 22 })
            .unwrap();
        let availability = Node::is_available(&removed).unwrap();

        node.destroy_client(removed);

        assert_eq!(node.clients.len(), 2);
        assert_eq!(node.client_identities.len(), 2);
        assert!(!node.client_identities.contains_key(&removed_identity));
        assert!(!node.pending_client_removals.contains(&removed_identity));
        assert!(!node
            .clients_with_active_availability
            .contains(&removed_identity));
        assert!(node.dropped_clients.lock().is_empty());
        assert!(node.active_availability_clients.lock().is_empty());
        assert!(matches!(response.now_or_never(), Some(Err(Error::RCL_RET_CLIENT_INVALID))));
        assert!(matches!(availability.now_or_never(), Some(Err(Error::RCL_RET_CLIENT_INVALID))));
        assert!(matches!(removed_clone.service_name(), Err(Error::RCL_RET_CLIENT_INVALID)));
        assert_eq!(first.service_name().unwrap(), first_name);
        assert_eq!(last.service_name().unwrap(), last_name);
        drop(removed_clone);
        assert!(node.dropped_clients.lock().is_empty());

        let foreign_name = format!("/destroy_client_foreign_{}", uuid::Uuid::new_v4().simple());
        let foreign = node
            .create_client_untyped(
                &foreign_name,
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();
        other.destroy_client_untyped(foreign);
        assert_eq!(node.clients.len(), 3);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.clients.len(), 2);
        assert_eq!(first.service_name().unwrap(), first_name);
        assert_eq!(last.service_name().unwrap(), last_name);
    }

    #[test]
    fn failed_parameter_client_creation_does_not_leave_partial_clients() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "parameter_client_rollback_test", "").unwrap();
        let unrelated = node
            .create_client::<AddTwoInts::Service>(
                "/unrelated_parameter_client_service",
                QosProfile::default(),
            )
            .unwrap();
        let remote_name = format!("/{}", "n".repeat(230));

        assert!(node.create_parameter_client(&remote_name).is_err());
        assert_eq!(node.clients.len(), 1);
        assert_eq!(node.client_identities.len(), 1);
        assert!(node.dropped_clients.lock().is_empty());
        assert_eq!(unrelated.service_name().unwrap(), "/unrelated_parameter_client_service");
    }

    #[test]
    fn action_availability_waiters_skip_idle_clients() {
        use crate::{
            action_clients::take_action_availability_poll_count,
            example_interfaces::action::Fibonacci,
        };

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "action_sparse_availability_test", "").unwrap();
        let active = node
            .create_action_client::<Fibonacci::Action>("/active_action_availability")
            .unwrap();
        let _idle = node
            .create_action_client::<Fibonacci::Action>("/idle_action_availability")
            .unwrap();
        take_action_availability_poll_count();

        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(take_action_availability_poll_count(), 0);

        let wait = Node::is_available(&active).unwrap();
        let second_wait = Node::is_available(&active).unwrap();
        assert_eq!(
            node.active_availability_action_clients
                .lock()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [active.identity()]
        );
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(take_action_availability_poll_count(), 1);
        assert_eq!(node.action_clients_with_active_availability.len(), 1);

        drop(wait);
        drop(second_wait);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(take_action_availability_poll_count(), 0);
        assert!(node.action_clients_with_active_availability.is_empty());
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(take_action_availability_poll_count(), 0);

        let untyped = node
            .create_action_client_untyped(
                "/active_untyped_action_availability",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        let first_untyped_wait = Node::is_available(&untyped).unwrap();
        let second_untyped_wait = Node::is_available(&untyped).unwrap();
        assert_eq!(
            node.active_availability_action_clients
                .lock()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [untyped.identity()]
        );
        drop(first_untyped_wait);
        drop(second_untyped_wait);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(!node
            .action_clients_with_active_availability
            .contains(&untyped.identity()));
    }

    #[test]
    fn action_client_maintenance_skips_idle_clients() {
        use crate::{
            action_clients::take_action_client_maintenance_poll_count,
            example_interfaces::action::Fibonacci,
        };

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "action_sparse_maintenance_test", "").unwrap();
        let active = node
            .create_action_client::<Fibonacci::Action>("/active_action_maintenance")
            .unwrap();
        let _idle = node
            .create_action_client::<Fibonacci::Action>("/idle_action_maintenance")
            .unwrap();
        let active_identity = active.identity();
        take_action_client_maintenance_poll_count();

        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(take_action_client_maintenance_poll_count(), 0);

        let canceled = active.cancel_all().unwrap();
        std::thread::scope(|scope| {
            scope.spawn(move || drop(canceled)).join().unwrap();
        });
        assert_eq!(
            node.active_maintenance_action_clients
                .lock()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [active_identity]
        );
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(take_action_client_maintenance_poll_count(), 1);
        assert!(node.action_clients_with_pending_maintenance.is_empty());
        assert!(!node.action_clients[node.action_client_identities[&active_identity]]
            .lock()
            .has_pending_work());

        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(take_action_client_maintenance_poll_count(), 0);
    }

    #[test]
    fn action_server_cancel_maintenance_skips_idle_servers() {
        use crate::{
            action_servers::take_action_server_maintenance_poll_count,
            example_interfaces::action::Fibonacci,
        };

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "action_sparse_cancel_maintenance_test", "").unwrap();
        let active = node
            .create_action_server::<Fibonacci::Action>("/active_action_cancel_maintenance")
            .unwrap();
        let _idle = node
            .create_action_server::<Fibonacci::Action>("/idle_action_cancel_maintenance")
            .unwrap();
        take_action_server_maintenance_poll_count();

        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(take_action_server_maintenance_poll_count(), 0);

        node.action_servers_with_pending_cancels.insert(active.id);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(take_action_server_maintenance_poll_count(), 1);
        assert!(node.action_servers_with_pending_cancels.is_empty());

        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(take_action_server_maintenance_poll_count(), 0);
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
    fn service_client_timeout_helpers_reject_foreign_nodes_before_sending() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut owner = Node::create(context.clone(), "service_timeout_owner", "").unwrap();
        let mut other = Node::create(context, "service_timeout_other", "").unwrap();
        let typed = owner
            .create_client::<AddTwoInts::Service>(
                "/foreign_node_typed_timeout",
                QosProfile::default(),
            )
            .unwrap();
        let untyped = owner
            .create_client_untyped(
                "/foreign_node_untyped_timeout",
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();

        assert!(matches!(
            typed.request_with_timeout(
                &mut other,
                &AddTwoInts::Request { a: 1, b: 2 },
                Duration::from_secs(1),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            typed.request_with_info_with_timeout(
                &mut other,
                &AddTwoInts::Request { a: 1, b: 2 },
                Duration::from_secs(1),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            typed.wait_for_service_with_timeout(&mut other, Duration::from_secs(1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            other.is_available_with_timeout(&typed, Duration::from_secs(1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        let typed_state = typed.client.upgrade().unwrap();
        let typed_state = typed_state.lock();
        assert!(typed_state.response_channels.is_empty());
        assert!(typed_state.poll_available_channels.is_empty());
        drop(typed_state);

        assert!(matches!(
            untyped.request_with_timeout(
                &mut other,
                serde_json::json!({ "a": 1, "b": 2 }),
                Duration::from_secs(1),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            untyped.request_with_info_with_timeout(
                &mut other,
                serde_json::json!({ "a": 1, "b": 2 }),
                Duration::from_secs(1),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            untyped.wait_for_service_with_timeout(&mut other, Duration::from_secs(1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            other.is_available_with_timeout(&untyped, Duration::from_secs(1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        let untyped_state = untyped.client.upgrade().unwrap();
        let untyped_state = untyped_state.lock();
        assert!(untyped_state.response_channels.is_empty());
        assert!(untyped_state.poll_available_channels.is_empty());
    }

    #[test]
    fn oversized_service_timeouts_do_not_send_requests_or_register_waiters() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "oversized_service_timeout_test", "").unwrap();
        let service_name = format!("/oversized_service_timeout_{}", uuid::Uuid::new_v4().simple());
        let mut service = node
            .create_service::<AddTwoInts::Service>(&service_name, QosProfile::default())
            .unwrap();
        let client = node
            .create_client::<AddTwoInts::Service>(&service_name, QosProfile::default())
            .unwrap();
        let oversized = Duration::from_nanos(i64::MAX as u64 + 1);
        let timer_count = node.timer_count();

        assert!(matches!(
            client.request_with_timeout(&mut node, &AddTwoInts::Request { a: 1, b: 2 }, oversized,),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            client.wait_for_service_with_timeout(&mut node, oversized),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert_eq!(node.timer_count(), timer_count);
        let client_state = client.client.upgrade().unwrap();
        let client_state = client_state.lock();
        assert!(client_state.response_channels.is_empty());
        assert!(client_state.poll_available_channels.is_empty());
        drop(client_state);

        node.try_spin_once(Duration::from_millis(10)).unwrap();
        assert!(service.next().now_or_never().is_none());
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
        let availability = missing
            .wait_for_action_server_with_timeout(&mut node, Duration::from_millis(1))
            .unwrap();
        assert!(matches!(drive(&mut node, availability), Err(Error::RCL_RET_TIMEOUT)));
        let timeout = missing
            .send_goal_request_with_options_and_timeout(
                &mut node,
                Fibonacci::Goal { order: 1 },
                ActionGoalOptions::fire_and_forget(),
                Duration::from_millis(1),
            )
            .unwrap();
        assert!(matches!(drive(&mut node, timeout), Err(Error::RCL_RET_TIMEOUT)));
        let cancel_all = missing
            .cancel_all_with_timeout(&mut node, Duration::from_millis(1))
            .unwrap();
        assert!(matches!(drive(&mut node, cancel_all), Err(Error::RCL_RET_TIMEOUT)));
        let cancel_before = missing
            .cancel_before_with_timeout(
                &mut node,
                crate::builtin_interfaces::msg::Time {
                    sec: 42,
                    nanosec: 7,
                },
                Duration::from_millis(1),
            )
            .unwrap();
        assert!(matches!(drive(&mut node, cancel_before), Err(Error::RCL_RET_TIMEOUT)));
        drop(missing);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());

        let missing_untyped = node
            .create_action_client_untyped(
                "/missing_untyped_action_request_timeout",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        let availability = missing_untyped.wait_for_action_server().unwrap();
        let availability = node
            .with_timeout(availability, Duration::from_millis(1))
            .unwrap();
        assert!(matches!(drive(&mut node, availability), Err(Error::RCL_RET_TIMEOUT)));
        let timeout = missing_untyped
            .send_goal_request_with_options_and_timeout(
                &mut node,
                serde_json::json!({ "order": 1 }),
                ActionGoalOptions::fire_and_forget(),
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
    fn action_client_timeout_helpers_reject_foreign_nodes_before_sending() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut owner = Node::create(context.clone(), "action_timeout_owner", "").unwrap();
        let mut other = Node::create(context, "action_timeout_other", "").unwrap();
        let typed = owner
            .create_action_client::<Fibonacci::Action>("/foreign_node_typed_action")
            .unwrap();
        let untyped = owner
            .create_action_client_untyped(
                "/foreign_node_untyped_action",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();

        assert!(matches!(
            typed.send_goal_request_with_timeout(
                &mut other,
                Fibonacci::Goal { order: 1 },
                Duration::from_secs(1),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            typed.send_goal_request_with_options_and_timeout(
                &mut other,
                Fibonacci::Goal { order: 1 },
                ActionGoalOptions::fire_and_forget(),
                Duration::from_secs(1),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            typed.cancel_all_with_timeout(&mut other, Duration::from_secs(1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            typed.cancel_before_with_timeout(
                &mut other,
                crate::builtin_interfaces::msg::Time::default(),
                Duration::from_secs(1),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            typed.wait_for_action_server_with_timeout(&mut other, Duration::from_secs(1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            other.is_available_with_timeout(&typed, Duration::from_secs(1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));

        assert!(matches!(
            untyped.send_goal_request_with_timeout(
                &mut other,
                serde_json::json!({ "order": 1 }),
                Duration::from_secs(1),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            untyped.send_goal_request_with_options_and_timeout(
                &mut other,
                serde_json::json!({ "order": 1 }),
                ActionGoalOptions::fire_and_forget(),
                Duration::from_secs(1),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            untyped.cancel_all_with_timeout(&mut other, Duration::from_secs(1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            untyped.cancel_before_with_timeout(
                &mut other,
                crate::builtin_interfaces::msg::Time::default(),
                Duration::from_secs(1),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            untyped.wait_for_action_server_with_timeout(&mut other, Duration::from_secs(1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            other.is_available_with_timeout(&untyped, Duration::from_secs(1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));

        let typed_index = owner.action_client_identities[&typed.identity()];
        assert!(!owner.action_clients[typed_index].lock().has_pending_work());
        let untyped_index = owner.action_client_identities[&untyped.identity()];
        assert!(!owner.action_clients[untyped_index]
            .lock()
            .has_pending_work());
        assert!(owner.active_availability_action_clients.lock().is_empty());
    }

    #[test]
    fn oversized_action_timeouts_do_not_send_goals_or_register_waiters() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "oversized_action_timeout_test", "").unwrap();
        let action_name = format!("/oversized_action_timeout_{}", uuid::Uuid::new_v4().simple());
        let mut server = node
            .create_action_server::<Fibonacci::Action>(&action_name)
            .unwrap();
        let client = node
            .create_action_client::<Fibonacci::Action>(&action_name)
            .unwrap();
        let oversized = Duration::from_nanos(i64::MAX as u64 + 1);
        let timer_count = node.timer_count();

        assert!(matches!(
            client.send_goal_request_with_options_and_timeout(
                &mut node,
                Fibonacci::Goal { order: 1 },
                ActionGoalOptions::fire_and_forget(),
                oversized,
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert!(matches!(
            client.wait_for_action_server_with_timeout(&mut node, oversized),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert_eq!(node.timer_count(), timer_count);
        let client_index = node.action_client_identities[&client.identity()];
        assert!(!node.action_clients[client_index].lock().has_pending_work());

        node.try_spin_once(Duration::from_millis(10)).unwrap();
        assert!(server.next().now_or_never().is_none());
    }

    #[test]
    fn action_clients_cancel_goals_at_or_before_a_stamp() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "action_cancel_before_test", "").unwrap();
        let mut requests = node
            .create_action_server::<Fibonacci::Action>("/action_cancel_before")
            .unwrap();
        let client = node
            .create_action_client::<Fibonacci::Action>("/action_cancel_before")
            .unwrap();
        let response = client
            .send_goal_request(Fibonacci::Goal { order: 1 })
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let mut server_cancels = None;
        let (client_goal, _result, _feedback) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_none() {
                if let Some(request) = requests.next().now_or_never().flatten() {
                    let (goal, cancels) = request.accept().unwrap();
                    server_goal = Some(goal);
                    server_cancels = Some(cancels);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };

        let accepted_at = client_goal.accepted_at();
        let cutoff = crate::builtin_interfaces::msg::Time {
            sec: accepted_at.sec + 1,
            nanosec: accepted_at.nanosec,
        };
        let cancel = client.cancel_before(cutoff).unwrap();
        let mut cancel = Box::pin(cancel);
        let mut cancel_request = None;
        let canceled = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            cancel_request = cancel_request.or_else(|| {
                server_cancels
                    .as_mut()
                    .unwrap()
                    .next()
                    .now_or_never()
                    .flatten()
            });
            if let Some(request) = cancel_request.take() {
                request.accept();
            }
            if let Some(response) = cancel.as_mut().now_or_never() {
                break response.unwrap();
            }
        };

        assert_eq!(canceled, vec![client_goal.uuid]);
        let mut server_goal = server_goal.unwrap();
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_cancelling().unwrap() {
                break;
            }
        }
        assert!(server_goal.is_cancelling().unwrap());
        server_goal.cancel(Fibonacci::Result::default()).unwrap();
    }

    #[test]
    fn action_goals_can_skip_result_and_feedback_tracking() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "action_goal_tracking_test", "").unwrap();
        let mut typed_requests = node
            .create_action_server::<Fibonacci::Action>("/typed_fire_and_forget")
            .unwrap();
        let typed_client = node
            .create_action_client::<Fibonacci::Action>("/typed_fire_and_forget")
            .unwrap();
        let response = typed_client
            .send_goal_request_with_options_and_timeout(
                &mut node,
                Fibonacci::Goal { order: 1 },
                ActionGoalOptions::fire_and_forget(),
                Duration::from_secs(1),
            )
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let response = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_none() {
                if let Some(request) = typed_requests.next().now_or_never().flatten() {
                    server_goal = Some(request.accept().unwrap().0);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert!(response.result.is_none());
        assert!(response.feedback.is_none());
        assert!(!response.goal.is_result_aware().unwrap());
        assert!(!response.goal.is_feedback_aware().unwrap());
        assert_eq!(node.action_clients.len(), 1);
        drop(response);
        drop(typed_client);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());
        server_goal
            .unwrap()
            .succeed(Fibonacci::Result::default())
            .unwrap();

        let mut untyped_requests = node
            .create_action_server::<Fibonacci::Action>("/untyped_fire_and_forget")
            .unwrap();
        let untyped_client = node
            .create_action_client_untyped(
                "/untyped_fire_and_forget",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        let response = untyped_client
            .send_goal_request_with_options_and_timeout(
                &mut node,
                serde_json::json!({ "order": 1 }),
                ActionGoalOptions::fire_and_forget(),
                Duration::from_secs(1),
            )
            .unwrap();
        let mut response = Box::pin(response);
        let mut untyped_server_goal = None;
        let response = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if untyped_server_goal.is_none() {
                if let Some(request) = untyped_requests.next().now_or_never().flatten() {
                    untyped_server_goal = Some(request.accept().unwrap().0);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert!(response.result.is_none());
        assert!(response.feedback.is_none());
        assert!(!response.goal.is_result_aware().unwrap());
        assert!(!response.goal.is_feedback_aware().unwrap());
        assert_eq!(node.action_clients.len(), 1);
        drop(response);
        drop(untyped_client);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());
        untyped_server_goal
            .unwrap()
            .succeed(Fibonacci::Result::default())
            .unwrap();
    }

    #[test]
    fn feedback_only_action_goals_close_streams_at_terminal_status() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "feedback_only_action_goal_test", "").unwrap();
        let mut typed_requests = node
            .create_action_server::<Fibonacci::Action>("/typed_feedback_only")
            .unwrap();
        let typed_client = node
            .create_action_client::<Fibonacci::Action>("/typed_feedback_only")
            .unwrap();
        let response = typed_client
            .send_goal_request_with_options(
                Fibonacci::Goal { order: 1 },
                ActionGoalOptions::default().track_result(false),
            )
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let response = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_none() {
                if let Some(request) = typed_requests.next().now_or_never().flatten() {
                    server_goal = Some(request.accept().unwrap().0);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert!(response.result.is_none());
        assert!(response.goal.is_feedback_aware().unwrap());
        assert!(!response.goal.is_result_aware().unwrap());
        let client_goal = response.goal;
        let mut feedback = response.feedback.unwrap();
        server_goal
            .unwrap()
            .succeed(Fibonacci::Result::default())
            .unwrap();

        let mut feedback_closed = false;
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if matches!(feedback.next().now_or_never(), Some(None)) {
                feedback_closed = true;
                break;
            }
        }
        assert!(feedback_closed);
        assert_eq!(client_goal.get_status().unwrap(), GoalStatus::Succeeded);
        assert!(!client_goal.is_feedback_aware().unwrap());
        drop(feedback);
        drop(client_goal);
        drop(typed_client);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());

        let mut untyped_requests = node
            .create_action_server::<Fibonacci::Action>("/untyped_feedback_only")
            .unwrap();
        let untyped_client = node
            .create_action_client_untyped(
                "/untyped_feedback_only",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        let response = untyped_client
            .send_goal_request_with_options(
                serde_json::json!({ "order": 1 }),
                ActionGoalOptions::default().track_result(false),
            )
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let response = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_none() {
                if let Some(request) = untyped_requests.next().now_or_never().flatten() {
                    server_goal = Some(request.accept().unwrap().0);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert!(response.result.is_none());
        assert!(response.goal.is_feedback_aware().unwrap());
        assert!(!response.goal.is_result_aware().unwrap());
        let client_goal = response.goal;
        let mut feedback = response.feedback.unwrap();
        server_goal
            .unwrap()
            .succeed(Fibonacci::Result::default())
            .unwrap();

        let mut feedback_closed = false;
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if matches!(feedback.next().now_or_never(), Some(None)) {
                feedback_closed = true;
                break;
            }
        }
        assert!(feedback_closed);
        assert_eq!(client_goal.get_status().unwrap(), GoalStatus::Succeeded);
        assert!(!client_goal.is_feedback_aware().unwrap());
        drop(feedback);
        drop(client_goal);
        drop(untyped_client);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());
    }

    #[test]
    fn result_only_action_goals_complete_without_feedback_tracking() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "result_only_action_goal_test", "").unwrap();
        let mut typed_requests = node
            .create_action_server::<Fibonacci::Action>("/typed_result_only")
            .unwrap();
        let typed_client = node
            .create_action_client::<Fibonacci::Action>("/typed_result_only")
            .unwrap();
        let response = typed_client
            .send_goal_request_with_options(
                Fibonacci::Goal { order: 1 },
                ActionGoalOptions::default().track_feedback(false),
            )
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let response = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_none() {
                if let Some(request) = typed_requests.next().now_or_never().flatten() {
                    server_goal = Some(request.accept().unwrap().0);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert!(response.feedback.is_none());
        assert!(!response.goal.is_feedback_aware().unwrap());
        assert!(response.goal.is_result_aware().unwrap());
        server_goal
            .unwrap()
            .succeed(Fibonacci::Result { sequence: vec![1] })
            .unwrap();
        let mut result = Box::pin(response.result.unwrap());
        let (status, result) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(result) = result.as_mut().now_or_never() {
                break result.unwrap();
            }
        };
        assert_eq!(status, GoalStatus::Succeeded);
        assert_eq!(result.sequence, vec![1]);
        assert!(!response.goal.is_result_aware().unwrap());
        drop(response.goal);
        drop(typed_client);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());

        let mut untyped_requests = node
            .create_action_server::<Fibonacci::Action>("/untyped_result_only")
            .unwrap();
        let untyped_client = node
            .create_action_client_untyped(
                "/untyped_result_only",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        let response = untyped_client
            .send_goal_request_with_options(
                serde_json::json!({ "order": 1 }),
                ActionGoalOptions::default().track_feedback(false),
            )
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let response = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_none() {
                if let Some(request) = untyped_requests.next().now_or_never().flatten() {
                    server_goal = Some(request.accept().unwrap().0);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        assert!(response.feedback.is_none());
        assert!(!response.goal.is_feedback_aware().unwrap());
        assert!(response.goal.is_result_aware().unwrap());
        server_goal
            .unwrap()
            .succeed(Fibonacci::Result { sequence: vec![1] })
            .unwrap();
        let mut result = Box::pin(response.result.unwrap());
        let (status, result) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(result) = result.as_mut().now_or_never() {
                break result.unwrap();
            }
        };
        assert_eq!(status, GoalStatus::Succeeded);
        assert_eq!(result.unwrap(), serde_json::json!({ "sequence": [1] }));
        assert!(!response.goal.is_result_aware().unwrap());
        drop(response.goal);
        drop(untyped_client);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());
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
        assert!(node.subscriptions_with_active_waits.is_empty());
        let timeout = subscription
            .wait_for_publishers_with_timeout(&mut node, Duration::from_millis(1))
            .unwrap();
        assert_eq!(node.subscriptions_with_active_waits, HashSet::from([subscription.id]));
        assert!(matches!(drive(&mut node, timeout), Err(Error::RCL_RET_TIMEOUT)));
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.subscriptions_with_active_waits.is_empty());

        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(&missing_topic, QosProfile::default())
            .unwrap();
        let wait = subscription
            .wait_for_publishers_with_timeout(&mut node, Duration::from_secs(1))
            .unwrap();
        assert_eq!(node.subscriptions_with_active_waits, HashSet::from([subscription.id]));
        drive(&mut node, wait).unwrap();
        assert!(node.subscriptions_with_active_waits.is_empty());
        assert!(subscription.get_publisher_count(&node).unwrap() >= 1);

        node.destroy_publisher(publisher);
        let subscriptions_before_drop = node.subscribers.len();
        drop(subscription);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.subscribers.len(), subscriptions_before_drop - 1);
    }

    #[test]
    fn subscription_waiters_poll_only_the_active_subscription() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "subscription_sparse_wait_test", "").unwrap();
        let active = node
            .subscribe::<crate::std_msgs::msg::String>(
                "/subscription_sparse_wait",
                QosProfile::default(),
            )
            .unwrap();
        let _idle = node
            .subscribe::<crate::std_msgs::msg::String>(
                "/subscription_without_wait",
                QosProfile::default(),
            )
            .unwrap();
        let active_index = node.subscription_indexes[&active.id];
        let idle_index = usize::from(active_index == 0);

        let wait = active.wait_for_publishers(&mut node).unwrap();
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.subscribers[active_index].publisher_wait_poll_count(), 1);
        assert_eq!(node.subscribers[idle_index].publisher_wait_poll_count(), 0);
        assert_eq!(node.subscriptions_with_active_waits, HashSet::from([active.id]));

        drop(wait);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.subscriptions_with_active_waits.is_empty());
        assert_eq!(node.subscribers[active_index].publisher_wait_poll_count(), 2);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.subscribers[active_index].publisher_wait_poll_count(), 2);
        assert_eq!(node.subscribers[idle_index].publisher_wait_poll_count(), 0);
    }

    #[test]
    fn subscription_indexes_follow_removal_and_reordering() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "subscription_index_test", "").unwrap();
        let first_topic = format!("/indexed_first_{}", uuid::Uuid::new_v4().simple());
        let removed_topic = format!("/indexed_removed_{}", uuid::Uuid::new_v4().simple());
        let last_topic = format!("/indexed_last_{}", uuid::Uuid::new_v4().simple());
        let first = node
            .subscribe::<crate::std_msgs::msg::String>(&first_topic, QosProfile::default())
            .unwrap();
        let removed = node
            .subscribe::<crate::std_msgs::msg::String>(&removed_topic, QosProfile::default())
            .unwrap();
        let last = node
            .subscribe::<crate::std_msgs::msg::String>(&last_topic, QosProfile::default())
            .unwrap();
        let removed_id = *node
            .subscription_indexes
            .iter()
            .find_map(|(id, index)| {
                (node.subscribers[*index].topic_name().unwrap() == removed_topic).then_some(id)
            })
            .unwrap();

        assert_eq!(node.subscription_indexes.len(), 3);
        drop(removed);
        assert_eq!(
            node.canceled_subscriptions
                .lock()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [removed_id]
        );
        node.try_spin_once(Duration::ZERO).unwrap();

        assert_eq!(node.subscribers.len(), 2);
        assert_eq!(node.subscription_indexes.len(), 2);
        assert!(node.canceled_subscriptions.lock().is_empty());
        assert_eq!(first.topic_name(&node).unwrap(), first_topic);
        assert_eq!(last.topic_name(&node).unwrap(), last_topic);
        for (id, index) in &node.subscription_indexes {
            assert_eq!(node.subscribers[*index].match_state().unwrap().id, *id);
        }
    }

    #[test]
    fn explicit_subscription_destruction_is_immediate_and_targeted() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "destroy_subscription_test", "").unwrap();
        let mut other = Node::create(context, "destroy_subscription_other", "").unwrap();
        let first_topic = format!("/destroy_sub_first_{}", uuid::Uuid::new_v4().simple());
        let removed_topic = format!("/destroy_sub_removed_{}", uuid::Uuid::new_v4().simple());
        let last_topic = format!("/destroy_sub_last_{}", uuid::Uuid::new_v4().simple());
        let first = node
            .subscribe::<crate::std_msgs::msg::String>(&first_topic, QosProfile::default())
            .unwrap();
        let removed = node
            .subscribe::<crate::std_msgs::msg::String>(&removed_topic, QosProfile::default())
            .unwrap();
        let last = node
            .subscribe::<crate::std_msgs::msg::String>(&last_topic, QosProfile::default())
            .unwrap();
        let wait = removed.wait_for_publishers(&mut node).unwrap();
        let removed_id = removed.id;

        node.destroy_subscription(removed);

        assert_eq!(node.subscribers.len(), 2);
        assert_eq!(node.subscription_indexes.len(), 2);
        assert!(!node.subscription_indexes.contains_key(&removed_id));
        assert!(!node.subscriptions_with_active_waits.contains(&removed_id));
        assert!(node.canceled_subscriptions.lock().is_empty());
        assert!(matches!(wait.now_or_never(), Some(Err(_))));
        assert_eq!(first.topic_name(&node).unwrap(), first_topic);
        assert_eq!(last.topic_name(&node).unwrap(), last_topic);

        let foreign_topic = format!("/destroy_sub_foreign_{}", uuid::Uuid::new_v4().simple());
        let foreign = node
            .subscribe::<crate::std_msgs::msg::String>(&foreign_topic, QosProfile::default())
            .unwrap();
        other.destroy_subscription(foreign);
        assert_eq!(node.subscribers.len(), 3);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.subscribers.len(), 2);
        assert_eq!(first.topic_name(&node).unwrap(), first_topic);
        assert_eq!(last.topic_name(&node).unwrap(), last_topic);
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
    fn service_indexes_follow_removal_and_reordering() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "service_index_test", "").unwrap();
        let first_name = format!("/indexed_first_{}", uuid::Uuid::new_v4().simple());
        let removed_name = format!("/indexed_removed_{}", uuid::Uuid::new_v4().simple());
        let last_name = format!("/indexed_last_{}", uuid::Uuid::new_v4().simple());
        let first = node
            .create_service::<AddTwoInts::Service>(&first_name, QosProfile::default())
            .unwrap();
        let removed = node
            .create_service::<AddTwoInts::Service>(&removed_name, QosProfile::default())
            .unwrap();
        let last = node
            .create_service::<AddTwoInts::Service>(&last_name, QosProfile::default())
            .unwrap();
        let removed_id = *node
            .service_indexes
            .iter()
            .find_map(|(id, index)| {
                (node.services[*index].lock().service_name().unwrap() == removed_name).then_some(id)
            })
            .unwrap();

        assert_eq!(node.service_indexes.len(), 3);
        drop(removed);
        assert_eq!(
            node.canceled_services
                .lock()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [removed_id]
        );
        node.try_spin_once(Duration::ZERO).unwrap();

        assert_eq!(node.services.len(), 2);
        assert_eq!(node.service_indexes.len(), 2);
        assert!(node.canceled_services.lock().is_empty());
        assert_eq!(first.service_name(&node).unwrap(), first_name);
        assert_eq!(last.service_name(&node).unwrap(), last_name);
        for (id, index) in &node.service_indexes {
            assert_eq!(node.services[*index].lock().id(), Some(*id));
        }
    }

    #[test]
    fn explicit_service_destruction_is_immediate_and_targeted() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "destroy_service_test", "").unwrap();
        let mut other = Node::create(context, "destroy_service_other", "").unwrap();
        let first_name = format!("/destroy_srv_first_{}", uuid::Uuid::new_v4().simple());
        let removed_name = format!("/destroy_srv_removed_{}", uuid::Uuid::new_v4().simple());
        let last_name = format!("/destroy_srv_last_{}", uuid::Uuid::new_v4().simple());
        let first = node
            .create_service::<AddTwoInts::Service>(&first_name, QosProfile::default())
            .unwrap();
        let removed = node
            .create_service::<AddTwoInts::Service>(&removed_name, QosProfile::default())
            .unwrap();
        let last = node
            .create_service_untyped(
                &last_name,
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();
        let removed_id = removed.id;

        node.destroy_service(removed);

        assert_eq!(node.services.len(), 2);
        assert_eq!(node.service_indexes.len(), 2);
        assert!(!node.service_indexes.contains_key(&removed_id));
        assert!(node.canceled_services.lock().is_empty());
        assert_eq!(first.service_name(&node).unwrap(), first_name);
        assert_eq!(last.service_name(&node).unwrap(), last_name);

        let foreign_name = format!("/destroy_srv_foreign_{}", uuid::Uuid::new_v4().simple());
        let foreign = node
            .create_service_untyped(
                &foreign_name,
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();
        other.destroy_service_untyped(foreign);
        assert_eq!(node.services.len(), 3);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.services.len(), 2);
        assert_eq!(first.service_name(&node).unwrap(), first_name);
        assert_eq!(last.service_name(&node).unwrap(), last_name);
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
        assert_eq!(node.dropped_clients.lock().len(), 1);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.clients.len(), 1);
        assert_eq!(node.pending_client_removals.len(), 1);

        drop(availability);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.clients.is_empty());
        assert!(node.client_identities.is_empty());
        assert!(node.pending_client_removals.is_empty());

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
        assert!(node.client_identities.is_empty());

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
        assert!(node.client_identities.is_empty());
    }

    #[test]
    fn client_identities_follow_middle_removal() {
        use crate::example_interfaces::srv::AddTwoInts;

        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "client_identity_test", "").unwrap();
        let other = Node::create(context, "client_identity_other", "").unwrap();
        let first = node
            .create_client::<AddTwoInts::Service>("/indexed_first_client", QosProfile::default())
            .unwrap();
        let removed = node
            .create_client::<AddTwoInts::Service>("/indexed_removed_client", QosProfile::default())
            .unwrap();
        let last = node
            .create_client_untyped(
                "/indexed_last_client",
                "example_interfaces/srv/AddTwoInts",
                QosProfile::default(),
            )
            .unwrap();

        assert_eq!(node.client_identities.len(), 3);
        drop(removed);
        node.try_spin_once(Duration::ZERO).unwrap();

        assert_eq!(node.clients.len(), 2);
        assert_eq!(node.client_identities.len(), 2);
        for (identity, index) in &node.client_identities {
            assert_eq!(Node::client_identity(&node.clients[*index]), *identity);
        }
        assert!(!first.is_available(&node).unwrap());
        assert!(!last.is_available(&node).unwrap());
        assert!(matches!(last.is_available(&other), Err(Error::RCL_RET_INVALID_ARGUMENT)));
    }

    #[test]
    fn action_client_identities_follow_middle_removal() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "action_client_identity_test", "").unwrap();
        let other = Node::create(context, "action_client_identity_other", "").unwrap();
        let first = node
            .create_action_client::<Fibonacci::Action>("/indexed_first_action_client")
            .unwrap();
        let removed = node
            .create_action_client::<Fibonacci::Action>("/indexed_removed_action_client")
            .unwrap();
        let last = node
            .create_action_client_untyped(
                "/indexed_last_action_client",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();

        assert_eq!(node.action_client_identities.len(), 3);
        drop(removed);
        node.try_spin_once(Duration::ZERO).unwrap();

        assert_eq!(node.action_clients.len(), 2);
        assert_eq!(node.action_client_identities.len(), 2);
        for (identity, index) in &node.action_client_identities {
            assert_eq!(Node::action_client_identity(&node.action_clients[*index]), *identity);
        }
        assert!(!first.is_available(&node).unwrap());
        assert!(!last.is_available(&node).unwrap());
        assert!(matches!(last.is_available(&other), Err(Error::RCL_RET_INVALID_ARGUMENT)));
    }

    #[test]
    fn explicit_action_client_destruction_is_immediate_and_updates_capacity() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "destroy_action_client_test", "").unwrap();
        let mut other = Node::create(context, "destroy_action_client_other", "").unwrap();
        let first = node
            .create_action_client::<Fibonacci::Action>("/destroy_action_client_first")
            .unwrap();
        let removed = node
            .create_action_client::<Fibonacci::Action>("/destroy_action_client_removed")
            .unwrap();
        let removed_clone = removed.clone();
        let last = node
            .create_action_client_untyped(
                "/destroy_action_client_last",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        let removed_identity = removed.identity();
        let removed_index = node.action_client_identities[&removed_identity];
        let removed_capacities =
            action_client_wait_set_capacities(node.action_clients[removed_index].lock().handle())
                .unwrap();
        let expected_capacities = {
            let mut capacities = node.action_wait_set_capacities;
            capacities.subtract(removed_capacities);
            capacities
        };
        let goal = removed
            .send_goal_request(Fibonacci::Goal { order: 1 })
            .unwrap();
        let availability = Node::is_available(&removed).unwrap();
        let canceled = removed.cancel_all().unwrap();

        node.destroy_action_client(removed).unwrap();

        assert_eq!(node.action_clients.len(), 2);
        assert_eq!(node.action_client_identities.len(), 2);
        assert!(!node
            .action_client_identities
            .contains_key(&removed_identity));
        assert!(!node
            .pending_action_client_removals
            .contains(&removed_identity));
        assert!(!node
            .action_clients_with_active_availability
            .contains(&removed_identity));
        assert!(node.dropped_action_clients.lock().is_empty());
        assert!(node.active_availability_action_clients.lock().is_empty());
        assert!(node.active_maintenance_action_clients.lock().is_empty());
        assert!(!node
            .action_clients_with_pending_maintenance
            .contains(&removed_identity));
        assert_eq!(node.action_wait_set_capacities, expected_capacities);
        assert!(matches!(goal.now_or_never(), Some(Err(Error::RCL_RET_ACTION_CLIENT_INVALID))));
        assert!(matches!(
            availability.now_or_never(),
            Some(Err(Error::RCL_RET_ACTION_CLIENT_INVALID))
        ));
        assert!(matches!(removed_clone.action_name(), Err(Error::RCL_RET_ACTION_CLIENT_INVALID)));
        drop(canceled);
        assert!(node.active_maintenance_action_clients.lock().is_empty());
        assert_eq!(first.action_name().unwrap(), "/destroy_action_client_first");
        assert_eq!(last.action_name().unwrap(), "/destroy_action_client_last");
        drop(removed_clone);
        assert!(node.dropped_action_clients.lock().is_empty());

        let foreign = node
            .create_action_client_untyped(
                "/destroy_action_client_foreign",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        other.destroy_action_client_untyped(foreign).unwrap();
        assert_eq!(node.action_clients.len(), 3);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.action_clients.len(), 2);
        assert_eq!(first.action_name().unwrap(), "/destroy_action_client_first");
        assert_eq!(last.action_name().unwrap(), "/destroy_action_client_last");

        let removed_untyped = node
            .create_action_client_untyped(
                "/destroy_untyped_action_client",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        let removed_untyped_identity = removed_untyped.identity();
        let canceled_untyped = removed_untyped.cancel_all().unwrap();
        let availability_untyped = removed_untyped.wait_for_action_server().unwrap();
        node.destroy_action_client_untyped(removed_untyped).unwrap();
        drop(canceled_untyped);
        assert!(matches!(
            availability_untyped.now_or_never(),
            Some(Err(Error::RCL_RET_ACTION_CLIENT_INVALID))
        ));
        assert!(!node
            .action_client_identities
            .contains_key(&removed_untyped_identity));
        assert!(node.active_maintenance_action_clients.lock().is_empty());
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
        assert_eq!(node.dropped_publishers.lock().len(), 1);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.pubs.len(), 1);
        assert_eq!(node.pending_publisher_removals.len(), 1);

        drop(wait);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.pubs.is_empty());
        assert!(node.publisher_indexes.is_empty());
        assert!(node.pending_publisher_removals.is_empty());

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
    fn publisher_waiters_activate_and_leave_the_sparse_poll_set() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "publisher_sparse_wait_test", "").unwrap();
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/publisher_sparse_wait",
                QosProfile::default(),
            )
            .unwrap();
        let id = publisher.handle.upgrade().unwrap().id;
        let idle = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/publisher_without_wait",
                QosProfile::default(),
            )
            .unwrap();
        let active_state = publisher.handle.upgrade().unwrap();
        let idle_state = idle.handle.upgrade().unwrap();

        assert!(node.active_wait_publishers.lock().is_empty());
        assert!(node.publishers_with_active_waits.is_empty());
        let wait = publisher.wait_for_subscribers().unwrap();
        let second_wait = publisher.wait_for_subscribers().unwrap();
        assert_eq!(
            node.active_wait_publishers
                .lock()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [id]
        );

        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.publishers_with_active_waits, HashSet::from([id]));
        assert_eq!(
            active_state
                .wait_poll_count
                .load(std::sync::atomic::Ordering::Relaxed),
            1
        );
        assert_eq!(
            idle_state
                .wait_poll_count
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        drop(wait);
        drop(second_wait);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.publishers_with_active_waits.is_empty());
        assert_eq!(
            active_state
                .wait_poll_count
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(
            active_state
                .wait_poll_count
                .load(std::sync::atomic::Ordering::Relaxed),
            2
        );
        assert_eq!(
            idle_state
                .wait_poll_count
                .load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        assert_eq!(node.pubs.len(), 2);
    }

    #[test]
    fn publisher_indexes_follow_explicit_and_automatic_removal() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "publisher_index_test", "").unwrap();
        let first = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/indexed_first_publisher",
                QosProfile::default(),
            )
            .unwrap();
        let removed = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/indexed_removed_publisher",
                QosProfile::default(),
            )
            .unwrap();
        let last = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/indexed_last_publisher",
                QosProfile::default(),
            )
            .unwrap();

        assert_eq!(node.publisher_indexes.len(), 3);
        node.destroy_publisher(removed);
        assert_eq!(node.pubs.len(), 2);
        assert_eq!(node.publisher_indexes.len(), 2);
        assert!(node.owns_publisher(&first.handle.upgrade().unwrap()));
        assert!(node.owns_publisher(&last.handle.upgrade().unwrap()));

        drop(first);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.pubs.len(), 1);
        assert_eq!(node.publisher_indexes.len(), 1);
        assert!(node.owns_publisher(&last.handle.upgrade().unwrap()));
        for (identity, index) in &node.publisher_indexes {
            assert_eq!(Node::publisher_identity(&node.pubs[*index]), *identity);
        }
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
    fn explicit_publisher_destruction_clears_waits_and_suppresses_stale_drop_notifications() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "publisher_destroy_cleanup_test", "").unwrap();
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::String>(
                "/publisher_destroy_cleanup",
                QosProfile::default(),
            )
            .unwrap();
        let clone = publisher.clone();
        let id = publisher.identity();
        let wait = publisher.wait_for_subscribers().unwrap();
        node.publishers_with_active_waits.insert(id);

        node.destroy_publisher(publisher);

        assert!(node.pubs.is_empty());
        assert!(node.publisher_indexes.is_empty());
        assert!(!node.publishers_with_active_waits.contains(&id));
        assert!(node.pending_publisher_removals.is_empty());
        assert!(node.dropped_publishers.lock().is_empty());
        assert!(node.active_wait_publishers.lock().is_empty());
        assert!(matches!(wait.now_or_never(), Some(Err(Error::RCL_RET_PUBLISHER_INVALID))));
        assert!(matches!(
            clone.publish(&crate::std_msgs::msg::String::default()),
            Err(Error::RCL_RET_PUBLISHER_INVALID)
        ));

        drop(clone);
        assert!(node.dropped_publishers.lock().is_empty());
    }

    #[test]
    fn dropped_timer_is_removed_before_its_deadline() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "timer_drop_cleanup_test", "").unwrap();
        let timer = node.create_wall_timer(Duration::from_secs(60)).unwrap();
        let id = timer.id;
        assert_eq!(node.timers.len(), 1);

        drop(timer);
        assert_eq!(
            node.canceled_timers
                .lock()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [id]
        );
        node.try_spin_once(Duration::ZERO).unwrap();

        assert!(node.timers.is_empty());
        assert!(node.timer_indexes.is_empty());
        assert!(node.canceled_timers.lock().is_empty());
    }

    #[test]
    fn timer_indexes_follow_removal_and_reordering() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "timer_index_test", "").unwrap();
        let first = node.create_wall_timer(Duration::from_secs(60)).unwrap();
        let removed = node.create_wall_timer(Duration::from_secs(61)).unwrap();
        let last = node.create_wall_timer(Duration::from_secs(62)).unwrap();

        assert_eq!(node.timer_indexes.len(), 3);
        drop(removed);
        node.try_spin_once(Duration::ZERO).unwrap();

        assert_eq!(node.timers.len(), 2);
        assert_eq!(node.timer_indexes.len(), 2);
        assert_eq!(first.period(&node).unwrap(), Duration::from_secs(60));
        assert_eq!(last.period(&node).unwrap(), Duration::from_secs(62));
        last.cancel(&mut node).unwrap();
        assert!(last.is_canceled(&node).unwrap());
        for (id, index) in &node.timer_indexes {
            assert_eq!(node.timers[*index].id, *id);
        }
    }

    #[test]
    fn explicit_timer_destruction_is_immediate_and_targeted() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "destroy_timer_test", "").unwrap();
        let mut other = Node::create(context, "destroy_timer_other", "").unwrap();
        let first = node.create_wall_timer(Duration::from_secs(60)).unwrap();
        let removed = node.create_wall_timer(Duration::from_secs(61)).unwrap();
        let last = node.create_timer(Duration::from_secs(62)).unwrap();
        let removed_id = removed.id;

        node.destroy_timer(removed);

        assert_eq!(node.timers.len(), 2);
        assert_eq!(node.timer_indexes.len(), 2);
        assert!(!node.timer_indexes.contains_key(&removed_id));
        assert!(node.canceled_timers.lock().is_empty());
        assert_eq!(first.period(&node).unwrap(), Duration::from_secs(60));
        assert_eq!(last.period(&node).unwrap(), Duration::from_secs(62));

        let foreign = node.create_wall_timer(Duration::from_secs(63)).unwrap();
        other.destroy_timer(foreign);
        assert_eq!(node.timers.len(), 3);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.timers.len(), 2);
        assert_eq!(first.period(&node).unwrap(), Duration::from_secs(60));
        assert_eq!(last.period(&node).unwrap(), Duration::from_secs(62));
    }

    #[test]
    fn timers_support_pause_reset_and_period_changes() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "timer_controls_test", "").unwrap();
        let other = Node::create(context, "timer_controls_other", "").unwrap();
        let timer = node.create_wall_timer(Duration::from_millis(100)).unwrap();

        assert_eq!(timer.clock_type(), ClockType::SteadyTime);
        assert!(timer.is_steady());
        assert_eq!(timer.period(&node).unwrap(), Duration::from_millis(100));
        assert!(!timer.is_canceled(&node).unwrap());
        assert!(!timer.is_ready(&node).unwrap());
        assert!(timer.time_until_next_call(&node).unwrap() <= Duration::from_millis(100));
        assert!(timer.time_until_next_call_ns(&node).unwrap() > 0);
        assert!(timer.time_since_last_call_ns(&node).unwrap() >= 0);
        assert!(timer.next_call_time_ns(&node).unwrap() > 0);
        timer.cancel(&mut node).unwrap();
        assert!(timer.is_canceled(&node).unwrap());
        assert!(!timer.is_ready(&node).unwrap());
        assert!(matches!(timer.time_until_next_call(&node), Err(Error::RCL_RET_TIMER_CANCELED)));
        assert!(matches!(
            timer.time_until_next_call_ns(&node),
            Err(Error::RCL_RET_TIMER_CANCELED)
        ));
        assert!(matches!(timer.next_call_time_ns(&node), Err(Error::RCL_RET_TIMER_CANCELED)));
        assert!(matches!(timer.period(&other), Err(Error::RCL_RET_TIMER_INVALID)));
        assert!(matches!(timer.is_ready(&other), Err(Error::RCL_RET_TIMER_INVALID)));
        assert!(matches!(
            timer.time_since_last_call_ns(&other),
            Err(Error::RCL_RET_TIMER_INVALID)
        ));

        let previous = timer
            .set_period(&mut node, Duration::from_millis(25))
            .unwrap();
        assert_eq!(previous, Duration::from_millis(100));
        assert_eq!(timer.period(&node).unwrap(), Duration::from_millis(25));
        assert!(matches!(
            timer.set_period(&mut node, Duration::from_nanos(i64::MAX as u64 + 1)),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));

        timer.reset(&mut node).unwrap();
        assert!(!timer.is_canceled(&node).unwrap());
        assert!(timer.time_since_last_call_ns(&node).unwrap() >= 0);

        timer.set_period(&mut node, Duration::ZERO).unwrap();
        timer.reset(&mut node).unwrap();
        assert!(timer.is_ready(&node).unwrap());
        assert_eq!(timer.time_until_next_call(&node).unwrap(), Duration::ZERO);
        assert!(timer.time_until_next_call_ns(&node).unwrap() <= 0);

        let ros_timer = node.create_timer(Duration::from_secs(1)).unwrap();
        assert_eq!(ros_timer.clock_type(), ClockType::RosTime);
        assert!(!ros_timer.is_steady());
    }

    #[cfg(not(any(
        r2r__ros__distro__foxy,
        r2r__ros__distro__galactic,
        r2r__ros__distro__humble,
        r2r__ros__distro__iron
    )))]
    #[test]
    fn timer_ticks_expose_native_call_timing_metadata() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "timer_call_info_test", "").unwrap();
        let mut timer = node.create_wall_timer(Duration::from_millis(1)).unwrap();

        node.try_spin_once(Duration::from_secs(1)).unwrap();
        let info = timer.tick_with_info().now_or_never().unwrap().unwrap();

        assert_eq!(info.clock_type, ClockType::SteadyTime);
        assert!(info.expected_call_time_ns > 0);
        assert!(info.actual_call_time_ns >= info.expected_call_time_ns);
        assert!(info.elapsed >= Duration::from_millis(1));

        node.try_spin_once(Duration::from_secs(1)).unwrap();
        assert!(timer.tick().now_or_never().unwrap().unwrap() > Duration::ZERO);
    }

    #[test]
    fn timer_options_control_startup_and_tick_buffering() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "timer_options_test", "").unwrap();
        assert_eq!(
            TimerOptions::default(),
            TimerOptions {
                autostart: true,
                channel_capacity: 1,
            }
        );

        let timer_count = node.timers.len();
        assert!(matches!(
            node.create_wall_timer_with_options(
                Duration::from_millis(1),
                TimerOptions::default().channel_capacity(usize::MAX),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert_eq!(node.timers.len(), timer_count);

        let mut wall = node
            .create_wall_timer_with_options(
                Duration::ZERO,
                TimerOptions::default().autostart(false).channel_capacity(0),
            )
            .unwrap();
        let ros = node
            .create_timer_with_options(
                Duration::from_secs(1),
                TimerOptions::default().autostart(false),
            )
            .unwrap();
        assert!(wall.is_canceled(&node).unwrap());
        assert!(ros.is_canceled(&node).unwrap());
        assert!(wall.tick().now_or_never().is_none());

        wall.reset(&mut node).unwrap();
        node.try_spin_once(Duration::ZERO).unwrap();
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(matches!(wall.tick().now_or_never(), Some(Ok(_))));
        assert!(wall.tick().now_or_never().is_none());

        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(matches!(wall.tick().now_or_never(), Some(Ok(_))));
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
        assert_eq!(node.dropped_action_clients.lock().len(), 1);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.action_clients.len(), 1);
        assert_eq!(node.pending_action_client_removals.len(), 1);

        drop(availability);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());
        assert!(node.action_client_identities.is_empty());
        assert!(node.pending_action_client_removals.is_empty());
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
        assert!(node.action_client_identities.is_empty());
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
        assert!(node.action_client_identities.is_empty());
        assert_eq!(node.action_wait_set_capacities, WaitSetCapacities::default());
    }

    #[test]
    fn completed_results_do_not_restore_dropped_action_goal_status() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "completed_action_cleanup_test", "").unwrap();
        let mut typed_requests = node
            .create_action_server::<Fibonacci::Action>("/typed_completed_action_cleanup")
            .unwrap();
        let typed_client = node
            .create_action_client::<Fibonacci::Action>("/typed_completed_action_cleanup")
            .unwrap();
        let response = typed_client
            .send_goal_request(Fibonacci::Goal { order: 1 })
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let (client_goal, result, feedback) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_none() {
                if let Some(request) = typed_requests.next().now_or_never().flatten() {
                    server_goal = Some(request.accept().unwrap().0);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        drop(response);
        drop(client_goal);
        drop(feedback);
        server_goal
            .unwrap()
            .succeed(Fibonacci::Result { sequence: vec![1] })
            .unwrap();
        let mut result = Box::pin(result);
        loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if result.as_mut().now_or_never().is_some() {
                break;
            }
        }
        drop(typed_client);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());

        let mut untyped_requests = node
            .create_action_server::<Fibonacci::Action>("/untyped_completed_action_cleanup")
            .unwrap();
        let untyped_client = node
            .create_action_client_untyped(
                "/untyped_completed_action_cleanup",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        let response = untyped_client
            .send_goal_request(serde_json::json!({ "order": 1 }))
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let (client_goal, result, feedback) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_none() {
                if let Some(request) = untyped_requests.next().now_or_never().flatten() {
                    server_goal = Some(request.accept().unwrap().0);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        drop(response);
        drop(client_goal);
        drop(feedback);
        server_goal
            .unwrap()
            .succeed(Fibonacci::Result { sequence: vec![1] })
            .unwrap();
        let mut result = Box::pin(result);
        loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if result.as_mut().now_or_never().is_some() {
                break;
            }
        }
        drop(untyped_client);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_clients.is_empty());
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
        assert_eq!(
            node.dropped_action_servers
                .lock()
                .iter()
                .copied()
                .collect::<Vec<_>>()
                .len(),
            1
        );
        node.try_spin_once(Duration::ZERO).unwrap();

        assert!(node.action_servers.is_empty());
        assert!(node.action_server_indexes.is_empty());
        assert!(node.pending_action_server_removals.is_empty());
        assert_eq!(node.action_wait_set_capacities, WaitSetCapacities::default());
    }

    #[test]
    fn dropped_action_server_removal_updates_swapped_index() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context, "action_server_index_cleanup_test", "").unwrap();
        let first = node
            .create_action_server::<Fibonacci::Action>("/first_action_server_cleanup")
            .unwrap();
        let middle = node
            .create_action_server::<Fibonacci::Action>("/middle_action_server_cleanup")
            .unwrap();
        let last = node
            .create_action_server::<Fibonacci::Action>("/last_action_server_cleanup")
            .unwrap();
        let ids = node
            .action_servers
            .iter()
            .map(|server| server.lock().id())
            .collect::<Vec<_>>();

        drop(middle);
        node.try_spin_once(Duration::ZERO).unwrap();

        assert_eq!(node.action_servers.len(), 2);
        assert!(!node.action_server_indexes.contains_key(&ids[1]));
        for (index, server) in node.action_servers.iter().enumerate() {
            assert_eq!(node.action_server_indexes.get(&server.lock().id()), Some(&index));
        }

        drop(first);
        drop(last);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!(node.action_servers.is_empty());
        assert!(node.action_server_indexes.is_empty());
        assert_eq!(node.action_wait_set_capacities, WaitSetCapacities::default());
    }

    #[test]
    fn explicit_action_server_destruction_is_immediate_and_updates_capacity() {
        use crate::example_interfaces::action::Fibonacci;

        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "destroy_action_server_test", "").unwrap();
        let mut other = Node::create(context, "destroy_action_server_other", "").unwrap();
        let first = node
            .create_action_server::<Fibonacci::Action>("/destroy_action_server_first")
            .unwrap();
        let removed = node
            .create_action_server::<Fibonacci::Action>("/destroy_action_server_removed")
            .unwrap();
        let last = node
            .create_action_server_untyped(
                "/destroy_action_server_last",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        let removed_id = removed.id;
        let removed_index = node.action_server_indexes[&removed_id];
        let removed_capacities =
            action_server_wait_set_capacities(node.action_servers[removed_index].lock().handle())
                .unwrap();
        let expected_capacities = {
            let mut capacities = node.action_wait_set_capacities;
            capacities.subtract(removed_capacities);
            capacities
        };

        node.destroy_action_server(removed).unwrap();

        assert_eq!(node.action_servers.len(), 2);
        assert_eq!(node.action_server_indexes.len(), 2);
        assert!(!node.action_server_indexes.contains_key(&removed_id));
        assert!(node.pending_action_server_removals.is_empty());
        assert!(!node
            .action_servers_with_pending_cancels
            .contains(&removed_id));
        assert!(node.dropped_action_servers.lock().is_empty());
        assert_eq!(node.action_wait_set_capacities, expected_capacities);
        assert_eq!(first.action_name(&node).unwrap(), "/destroy_action_server_first");
        assert_eq!(last.action_name(&node).unwrap(), "/destroy_action_server_last");

        let foreign = node
            .create_action_server_untyped(
                "/destroy_action_server_foreign",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        other.destroy_action_server_untyped(foreign).unwrap();
        assert_eq!(node.action_servers.len(), 3);
        node.try_spin_once(Duration::ZERO).unwrap();
        assert_eq!(node.action_servers.len(), 2);
        assert_eq!(first.action_name(&node).unwrap(), "/destroy_action_server_first");
        assert_eq!(last.action_name(&node).unwrap(), "/destroy_action_server_last");
    }
}
