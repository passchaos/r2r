//! Parameter service/event handler construction for [`Node`].
//!
//! The handler future drives the built-in parameter services and the
//! parameter-event publisher; it is spawned by the application exactly like
//! the futures returned by other entity factories. Extracted verbatim from
//! `nodes.rs` to keep node construction/entity bookkeeping separate.

use super::*;

impl Node {
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
                                                reason: err.to_string().into(),
                                            },
                                        );
                                        continue;
                                    }
                                };
                            let name = match p.name.clone().into_text("parameter name") {
                                Ok(name) => name,
                                Err(error) => {
                                    result
                                        .results
                                        .push(rcl_interfaces::msg::SetParametersResult {
                                            successful: false,
                                            reason: error.to_string().into(),
                                        });
                                    continue;
                                }
                            };
                            let update = vec![(name, val)];
                            let mut transaction =
                                match lock_parameter_transaction(&parameter_transaction) {
                                    Ok(transaction) => transaction,
                                    Err(err) => {
                                        result.results.push(
                                            rcl_interfaces::msg::SetParametersResult {
                                                successful: false,
                                                reason: err.to_string().into(),
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
                                            reason: Default::default(),
                                        });
                                }
                                Err(err) => {
                                    result
                                        .results
                                        .push(rcl_interfaces::msg::SetParametersResult {
                                            successful: false,
                                            reason: err.to_string().into(),
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
                                    parameter.name.clone().into_text("parameter name")?,
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
                                    format!("Can't set parameters atomically: {err}").into();
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
                                        format!("Can't set parameters atomically: {err}").into();
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
                                    format!("Can't set parameters atomically: {err}").into();
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
                let initialized = provider_state.parameter_names.set(parameter_names);
                debug_assert!(initialized.is_ok());
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
}
