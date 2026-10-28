//! Declared-parameter API: declaration, lookup, mutation, descriptors
//! and parameter callback registration for [`Node`].

use super::*;

impl Node {
    /// Returns the initial CLI, YAML, and programmatic parameter overrides for this node.
    ///
    /// Programmatic overrides replace global overrides with the same name. The returned map is an
    /// immutable creation-time snapshot: declaring, setting, or undeclaring parameters does not
    /// change it.
    pub fn parameter_overrides(&self) -> &IndexMap<String, ParameterValue> {
        &self.parameter_overrides
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
            descriptor.name = name.clone().into();
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
                    descriptor.description = parameter.description.into();
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
        descriptor.name = name.clone().into();
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

    pub(in crate::nodes) fn default_parameter_descriptor(
        name: &str, parameter: &Parameter, dynamic_typing: bool,
    ) -> ParameterDescriptor {
        ParameterDescriptor {
            name: name.to_owned().into(),
            type_: parameter.value.parameter_type().into(),
            description: parameter.description.into(),
            dynamic_typing,
            ..Default::default()
        }
    }

    pub(in crate::nodes) fn initialize_parameter_descriptors(
        &self, dynamic_typing: bool,
    ) -> Result<()> {
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
            descriptor.name = name.clone().into();
            descriptor.type_ = parameter.value.parameter_type().into();
            if name == "use_sim_time" {
                descriptor.dynamic_typing = false;
            }
            if descriptor.description.is_empty() {
                descriptor.description = parameter.description.into();
            }
            validate_parameter_descriptor(name, descriptor)?;
            validate_parameter_update(name, &parameter.value, descriptor, false)?;
        }
        Ok(())
    }

    pub(in crate::nodes) fn record_successful_parameter_descriptor(
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
                    name: name.to_owned().into(),
                    type_: value.parameter_type().into(),
                    dynamic_typing,
                    ..Default::default()
                },
            );
        }
    }

    pub(in crate::nodes) fn store_dynamic_parameter(
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

    pub(in crate::nodes) fn handle_list_parameters(
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

    pub(in crate::nodes) fn handle_desc_parameters(
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
}
