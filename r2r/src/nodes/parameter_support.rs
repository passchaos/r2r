//! Internal parameter support: RCL override parsing, parameter events,
//! transaction guards, callback registries and update/validation helpers.

use super::*;

thread_local! {
    static ACTIVE_PARAMETER_TRANSACTIONS: RefCell<HashSet<usize>> = RefCell::new(HashSet::new());
}

pub(crate) struct RclParams(pub(crate) *mut rcl_params_t);

impl Drop for RclParams {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { rcl_yaml_node_struct_fini(self.0) };
        }
    }
}

pub(crate) fn parameter_overrides_from_arguments(arguments: &rcl_arguments_t) -> Result<RclParams> {
    let mut params = RclParams(std::ptr::null_mut());
    let ret = unsafe { rcl_arguments_get_param_overrides(arguments, &mut params.0) };
    if ret == RCL_RET_OK as i32 {
        Ok(params)
    } else {
        Err(Error::from_rcl_error(ret))
    }
}

pub(crate) fn parameter_overrides(context: &Mutex<ContextHandle>) -> Result<RclParams> {
    let context = context.lock();
    parameter_overrides_from_arguments(&context.global_arguments)
}

pub(crate) fn parameter_rule_matches_node(rule: &str, fully_qualified_name: &str) -> bool {
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

#[derive(Default)]
pub(crate) struct ParameterEventChanges {
    pub(crate) new_parameters: Vec<rcl_interfaces::msg::Parameter>,
    pub(crate) changed_parameters: Vec<rcl_interfaces::msg::Parameter>,
    pub(crate) deleted_parameters: Vec<rcl_interfaces::msg::Parameter>,
}

impl ParameterEventChanges {
    pub(crate) fn record(
        &mut self, name: &str, previous: Option<&ParameterValue>, value: &ParameterValue,
    ) {
        if !parameter_value_changed(previous, value) {
            return;
        }
        let parameter = rcl_interfaces::msg::Parameter {
            name: name.to_owned().into(),
            value: value.clone().into_parameter_value_msg(),
        };
        match (previous, value) {
            (Some(ParameterValue::NotSet) | None, ParameterValue::NotSet) => {}
            (Some(_), ParameterValue::NotSet) => self.deleted_parameters.push(parameter),
            (Some(ParameterValue::NotSet) | None, _) => self.new_parameters.push(parameter),
            (Some(_), _) => self.changed_parameters.push(parameter),
        }
    }

    pub(crate) fn publish(
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
            node: node_name.to_owned().into(),
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
pub(crate) struct ParameterEventSink {
    pub(crate) sender: mpsc::UnboundedSender<LocalParameterEvent>,
    pub(crate) publisher: Option<Publisher<rcl_interfaces::msg::ParameterEvent>>,
    pub(crate) clock: Arc<Mutex<Clock>>,
    pub(crate) node_name: String,
}

type LocalParameterEvent = (String, ParameterValue);

pub(crate) fn deliver_local_parameter_events(
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
    pub(crate) fn notify(
        &self, changed: Vec<(String, ParameterValue)>, ros_events: ParameterEventChanges,
    ) {
        deliver_local_parameter_events(&self.sender, changed);
        if let Some(publisher) = &self.publisher {
            ros_events.publish(publisher, &self.clock, &self.node_name);
        }
    }
}

pub(crate) struct AppliedParameterChanges {
    pub(crate) updates: Vec<(String, ParameterValue)>,
    pub(crate) changed: Vec<(String, ParameterValue)>,
    pub(crate) ros_events: ParameterEventChanges,
    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    pub(crate) sim_time_update: Option<bool>,
}

pub(crate) struct PreparedParameterDeclaration {
    pub(crate) name: String,
    pub(crate) value: ParameterValue,
    pub(crate) descriptor: ParameterDescriptor,
    pub(crate) uses_existing_override: bool,
}

#[derive(Clone)]
pub(crate) struct ParameterProviderState {
    pub(crate) provider: Arc<Mutex<dyn RosParams + Send>>,
    pub(crate) parameter_names: Arc<OnceLock<HashSet<String>>>,
}

impl ParameterProviderState {
    pub(crate) fn new(provider: Arc<Mutex<dyn RosParams + Send>>) -> Self {
        Self {
            provider,
            parameter_names: Arc::new(OnceLock::new()),
        }
    }

    pub(crate) fn parameter_names(&self) -> &HashSet<String> {
        self.parameter_names
            .get()
            .expect("parameter provider names are initialized before use")
    }
}

pub(crate) struct ParameterTransactionGuard<'a> {
    pub(crate) guard: Option<MutexGuard<'a, ()>>,
    pub(crate) identity: usize,
}

impl ParameterTransactionGuard<'_> {
    pub(crate) fn unlock(&mut self) {
        self.guard.take();
    }
}

impl Drop for ParameterTransactionGuard<'_> {
    fn drop(&mut self) {
        ACTIVE_PARAMETER_TRANSACTIONS.with(|active| {
            let removed = active.borrow_mut().remove(&self.identity);
            debug_assert!(removed);
        });
    }
}

pub(crate) fn lock_parameter_transaction(
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

pub(crate) type ParameterValidationCallback =
    dyn Fn(&[(String, ParameterValue)]) -> Result<()> + Send + Sync + 'static;
pub(crate) type ParameterPostSetCallback =
    dyn Fn(&[(String, ParameterValue)]) + Send + Sync + 'static;
pub(crate) type ParameterPreSetCallback =
    dyn Fn(&mut Vec<(String, ParameterValue)>) + Send + Sync + 'static;

pub(crate) struct ParameterCallbackRegistry<C: ?Sized> {
    pub(crate) callbacks: BTreeMap<u64, Arc<C>>,
    pub(crate) next_id: u64,
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
    pub(crate) fn insert(&mut self, callback: Arc<C>) -> u64 {
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

    pub(crate) fn remove(&mut self, id: u64) {
        self.callbacks.remove(&id);
    }

    pub(crate) fn reverse_snapshot(&self) -> Vec<Arc<C>> {
        self.callbacks.values().rev().cloned().collect()
    }
}

pub(crate) fn transform_parameter_updates(
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

pub(crate) fn normalize_parameter_updates(
    updates: Vec<(String, ParameterValue)>,
) -> Vec<(String, ParameterValue)> {
    let mut normalized = IndexMap::with_capacity(updates.len());
    for (name, value) in updates {
        normalized.insert(name, value);
    }
    normalized.into_iter().collect()
}

pub(crate) fn validate_parameter_callbacks(
    updates: &[(String, ParameterValue)],
    callbacks: &Arc<Mutex<ParameterCallbackRegistry<ParameterValidationCallback>>>,
) -> Result<()> {
    let callbacks = callbacks.lock().reverse_snapshot();
    for callback in callbacks {
        callback(updates)?;
    }
    Ok(())
}

pub(crate) fn notify_parameter_post_set_callbacks(
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
    pub(crate) id: u64,
    pub(crate) callbacks:
        std::sync::Weak<Mutex<ParameterCallbackRegistry<ParameterValidationCallback>>>,
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
    pub(crate) id: u64,
    pub(crate) callbacks:
        std::sync::Weak<Mutex<ParameterCallbackRegistry<ParameterPostSetCallback>>>,
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
    pub(crate) id: u64,
    pub(crate) callbacks:
        std::sync::Weak<Mutex<ParameterCallbackRegistry<ParameterPreSetCallback>>>,
}

impl Drop for ParameterPreSetCallbackHandle {
    fn drop(&mut self) {
        if let Some(callbacks) = self.callbacks.upgrade() {
            callbacks.lock().remove(self.id);
        }
    }
}

pub(crate) fn parameter_value_changed(
    previous: Option<&ParameterValue>, value: &ParameterValue,
) -> bool {
    previous != Some(value) && !(previous.is_none() && matches!(value, ParameterValue::NotSet))
}

pub(crate) fn parameter_name_matches(
    name: &str, prefixes: &[impl AsRef<[u8]>], depth: u64,
) -> bool {
    let recursive = depth == rcl_interfaces::srv::ListParameters::Request::DEPTH_RECURSIVE as u64;
    let within_depth =
        |suffix: &[u8]| (suffix.iter().filter(|&&byte| byte == b'.').count() as u64) < depth;
    let name = name.as_bytes();
    (prefixes.is_empty() && (recursive || within_depth(name)))
        || prefixes.iter().any(|prefix| {
            let prefix = prefix.as_ref();
            name == prefix
                || name
                    .strip_prefix(prefix)
                    .and_then(|suffix| suffix.strip_prefix(b"."))
                    .is_some_and(|suffix| recursive || within_depth(suffix))
        })
}

pub(crate) fn require_declared_names(
    names: &[impl AsRef<[u8]>], params: &IndexMap<String, Parameter>,
) -> Result<()> {
    for name in names {
        let name = std::str::from_utf8(name.as_ref()).map_err(|error| Error::InvalidUtf8 {
            context: "parameter name",
            err: error.to_string(),
        })?;
        if !params.contains_key(name) {
            return Err(Error::ParameterNotDeclared {
                name: name.to_owned(),
            });
        }
    }
    Ok(())
}

pub(crate) fn read_parameter_values(
    names: &[impl AsRef<[u8]>], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    provider: Option<&ParameterProviderState>,
) -> Vec<ParameterValue> {
    read_parameter_values_impl(names, params, provider, true, false)
        .expect("permissive parameter reads map provider failures to unset values")
}

pub(crate) fn read_parameter_values_with_policy(
    names: &[impl AsRef<[u8]>], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    provider: Option<&ParameterProviderState>, allow_undeclared_parameters: bool,
) -> Result<Vec<ParameterValue>> {
    read_parameter_values_impl(names, params, provider, allow_undeclared_parameters, true)
}

pub(crate) fn read_parameter_values_impl(
    names: &[impl AsRef<[u8]>], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    provider: Option<&ParameterProviderState>, allow_undeclared_parameters: bool,
    propagate_provider_errors: bool,
) -> Result<Vec<ParameterValue>> {
    let mut values = {
        let params = params.lock();
        if !allow_undeclared_parameters {
            require_declared_names(names, &params)?;
        }
        names
            .iter()
            .map(|name| {
                std::str::from_utf8(name.as_ref())
                    .ok()
                    .and_then(|name| params.get(name))
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
        let Ok(name) = std::str::from_utf8(name.as_ref()) else {
            continue;
        };
        if !parameter_names.iter().any(|value| value == name) {
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

pub(crate) fn list_parameter_values(
    params: &IndexMap<String, Parameter>, prefixes: &[impl AsRef<[u8]>], depth: u64,
) -> rcl_interfaces::msg::ListParametersResult {
    let mut result = rcl_interfaces::msg::ListParametersResult::default();
    let mut seen_prefixes = HashSet::new();
    for name in params
        .keys()
        .filter(|name| parameter_name_matches(name, prefixes, depth))
    {
        result.names.push(name.clone().into());
        if let Some(last_separator) = name.rfind('.') {
            let prefix = &name[..last_separator];
            if seen_prefixes.insert(prefix) {
                result.prefixes.push(prefix.into());
            }
        }
    }
    result
}

pub(crate) fn read_list_parameter_values(
    params: &Arc<Mutex<IndexMap<String, Parameter>>>, prefixes: &[impl AsRef<[u8]>], depth: u64,
) -> rcl_interfaces::msg::ListParametersResult {
    let params = params.lock();
    list_parameter_values(&params, prefixes, depth)
}

pub(crate) fn describe_parameter_values(
    names: &[impl AsRef<[u8]>], params: &IndexMap<String, Parameter>,
    registered: &IndexMap<String, ParameterDescriptor>,
) -> Vec<ParameterDescriptor> {
    names
        .iter()
        .map(|bytes| {
            let name = std::str::from_utf8(bytes.as_ref()).ok();
            name.and_then(|name| registered.get(name))
                .cloned()
                .unwrap_or_else(|| {
                    name.and_then(|name| {
                        params
                            .get(name)
                            .map(|value| Node::default_parameter_descriptor(name, value, true))
                    })
                    .unwrap_or_else(|| ParameterDescriptor {
                        name: crate::RosString::from(bytes.as_ref()),
                        ..Default::default()
                    })
                })
        })
        .collect()
}

pub(crate) fn read_parameter_descriptors(
    names: &[impl AsRef<[u8]>], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    registered: &Arc<Mutex<IndexMap<String, ParameterDescriptor>>>,
) -> Vec<ParameterDescriptor> {
    read_parameter_descriptors_with_policy(names, params, registered, true)
        .expect("permissive parameter descriptions cannot reject an undeclared name")
}

pub(crate) fn read_parameter_descriptors_with_policy(
    names: &[impl AsRef<[u8]>], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    registered: &Arc<Mutex<IndexMap<String, ParameterDescriptor>>>,
    allow_undeclared_parameters: bool,
) -> Result<Vec<ParameterDescriptor>> {
    let params = params.lock();
    if !allow_undeclared_parameters {
        require_declared_names(names, &params)?;
    }
    let registered = registered.lock();
    Ok(describe_parameter_values(names, &params, &registered))
}

pub(crate) fn parameter_types(
    names: &[impl AsRef<[u8]>], params: &IndexMap<String, Parameter>,
) -> Vec<ParameterType> {
    names
        .iter()
        .map(|name| {
            std::str::from_utf8(name.as_ref())
                .ok()
                .and_then(|name| params.get(name))
                .map_or(ParameterType::NotSet, |parameter| parameter.value.parameter_type())
        })
        .collect()
}

pub(crate) fn read_parameter_types(
    names: &[impl AsRef<[u8]>], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
) -> Vec<ParameterType> {
    read_parameter_types_with_policy(names, params, true)
        .expect("permissive parameter type reads cannot reject an undeclared name")
}

pub(crate) fn read_parameter_types_with_policy(
    names: &[impl AsRef<[u8]>], params: &Arc<Mutex<IndexMap<String, Parameter>>>,
    allow_undeclared_parameters: bool,
) -> Result<Vec<ParameterType>> {
    let params = params.lock();
    if !allow_undeclared_parameters {
        require_declared_names(names, &params)?;
    }
    Ok(parameter_types(names, &params))
}

pub(crate) fn apply_parameter_updates(
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
