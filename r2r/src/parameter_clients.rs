use crate::{
    rcl_interfaces::{msg, srv},
    Client, Error, Node, ParameterEventChange, ParameterType, ParameterValue, QosProfile, Result,
    SubscriptionOptions, WrongParameterType,
};
use futures::{future::try_join_all, Future, FutureExt, Stream, TryFutureExt};
use std::time::Duration;

/// Runtime-agnostic client for the standard ROS parameter services of another node.
#[derive(Clone)]
pub struct ParameterClient {
    remote_node_name: String,
    resolved_remote_node_name: String,
    get_parameters: Client<srv::GetParameters::Service>,
    get_parameter_types: Client<srv::GetParameterTypes::Service>,
    set_parameters: Client<srv::SetParameters::Service>,
    set_parameters_atomically: Client<srv::SetParametersAtomically::Service>,
    list_parameters: Client<srv::ListParameters::Service>,
    describe_parameters: Client<srv::DescribeParameters::Service>,
}

impl ParameterClient {
    pub(crate) fn new(node: &mut Node, remote_node_name: &str, qos: QosProfile) -> Result<Self> {
        let remote_node_name = normalize_remote_node_name(remote_node_name)?;
        let get_parameters =
            node.create_client(&format!("{remote_node_name}/get_parameters"), qos.clone())?;
        let get_parameters_service = get_parameters.service_name()?;
        let resolved_remote_node_name = get_parameters_service
            .strip_suffix("/get_parameters")
            .filter(|name| !name.is_empty())
            .ok_or(Error::RCL_RET_SERVICE_NAME_INVALID)?
            .to_owned();
        Ok(Self {
            get_parameters,
            get_parameter_types: node
                .create_client(&format!("{remote_node_name}/get_parameter_types"), qos.clone())?,
            set_parameters: node
                .create_client(&format!("{remote_node_name}/set_parameters"), qos.clone())?,
            set_parameters_atomically: node.create_client(
                &format!("{remote_node_name}/set_parameters_atomically"),
                qos.clone(),
            )?,
            list_parameters: node
                .create_client(&format!("{remote_node_name}/list_parameters"), qos.clone())?,
            describe_parameters: node
                .create_client(&format!("{remote_node_name}/describe_parameters"), qos)?,
            remote_node_name,
            resolved_remote_node_name,
        })
    }

    pub fn remote_node_name(&self) -> &str {
        &self.remote_node_name
    }

    /// Returns the fully resolved target node name used to filter parameter events.
    pub fn resolved_remote_node_name(&self) -> &str {
        &self.resolved_remote_node_name
    }

    /// Subscribes to standard parameter events from this client's target node.
    ///
    /// When `parameter` is set, changes for other parameters on the target node are filtered out.
    pub fn subscribe_parameter_events(
        &self, node: &mut Node, parameter: Option<String>,
    ) -> Result<impl Stream<Item = ParameterEventChange> + Unpin> {
        self.subscribe_parameter_events_with_options(
            node,
            parameter,
            QosProfile::parameter_events(),
            SubscriptionOptions::default(),
        )
    }

    /// Subscribes to target-node parameter events with explicit QoS and stream options.
    pub fn subscribe_parameter_events_with_options(
        &self, node: &mut Node, parameter: Option<String>, qos: QosProfile,
        options: SubscriptionOptions,
    ) -> Result<impl Stream<Item = ParameterEventChange> + Unpin> {
        node.subscribe_parameter_events_with_options(
            Some(self.resolved_remote_node_name.clone()),
            parameter,
            qos,
            options,
        )
    }

    /// Returns true only when all six standard parameter services are available.
    pub fn services_ready(&self, node: &Node) -> Result<bool> {
        Ok(self.get_parameters.is_available(node)?
            && self.get_parameter_types.is_available(node)?
            && self.set_parameters.is_available(node)?
            && self.set_parameters_atomically.is_available(node)?
            && self.list_parameters.is_available(node)?
            && self.describe_parameters.is_available(node)?)
    }

    /// Completes when all six standard parameter services are available. The node must be spun.
    pub fn wait_for_services(&self) -> Result<impl Future<Output = Result<()>>> {
        let waits = vec![
            Node::is_available(&self.get_parameters)?.boxed(),
            Node::is_available(&self.get_parameter_types)?.boxed(),
            Node::is_available(&self.set_parameters)?.boxed(),
            Node::is_available(&self.set_parameters_atomically)?.boxed(),
            Node::is_available(&self.list_parameters)?.boxed(),
            Node::is_available(&self.describe_parameters)?.boxed(),
        ];
        Ok(try_join_all(waits).map(|result| result.map(|_| ())))
    }

    pub fn wait_for_services_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        let wait = self.wait_for_services()?;
        node.with_timeout(wait, timeout)
    }

    pub fn get_parameters<I, S>(
        &self, names: I,
    ) -> Result<impl Future<Output = Result<Vec<ParameterValue>>>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names = strings(names);
        let expected = names.len();
        let request = srv::GetParameters::Request { names };
        let response = self.get_parameters.request(&request)?;
        Ok(response.and_then(move |response| async move {
            check_response_length("get_parameters", expected, response.values.len())?;
            response
                .values
                .into_iter()
                .map(ParameterValue::try_from_parameter_value_msg)
                .collect()
        }))
    }

    /// Gets parameter values, failing with [`Error::RCL_RET_TIMEOUT`] after `timeout`.
    pub fn get_parameters_with_timeout<I, S>(
        &self, node: &mut Node, names: I, timeout: Duration,
    ) -> Result<impl Future<Output = Result<Vec<ParameterValue>>>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let response = self.get_parameters(names)?;
        node.with_timeout(response, timeout)
    }

    /// Returns whether a parameter is declared on the target node.
    pub fn has_parameter(
        &self, name: impl Into<String>,
    ) -> Result<impl Future<Output = Result<bool>>> {
        let response = self.get_parameter_types([name.into()])?;
        Ok(response.map_ok(|types| {
            types
                .first()
                .is_some_and(|type_| *type_ != ParameterType::NotSet)
        }))
    }

    /// Checks whether a parameter is declared, failing after `timeout`.
    pub fn has_parameter_with_timeout(
        &self, node: &mut Node, name: impl Into<String>, timeout: Duration,
    ) -> Result<impl Future<Output = Result<bool>>> {
        let response = self.has_parameter(name)?;
        node.with_timeout(response, timeout)
    }

    /// Gets and converts one parameter from the target node.
    pub fn get_parameter<T>(
        &self, name: impl Into<String>,
    ) -> Result<impl Future<Output = Result<T>>>
    where
        ParameterValue: TryInto<T, Error = WrongParameterType>,
    {
        let name = name.into();
        let response = self.get_parameters([name.clone()])?;
        Ok(response.and_then(move |values| {
            let result = single_parameter_value(values).and_then(|value| {
                value
                    .try_into()
                    .map_err(|error: WrongParameterType| Error::ParameterWrongType {
                        name,
                        expected_type: error.expected_type_name,
                        actual_type: error.actual_type_name,
                    })
            });
            futures::future::ready(result)
        }))
    }

    /// Gets one parameter, failing after `timeout`.
    pub fn get_parameter_with_timeout<T>(
        &self, node: &mut Node, name: impl Into<String>, timeout: Duration,
    ) -> Result<impl Future<Output = Result<T>>>
    where
        ParameterValue: TryInto<T, Error = WrongParameterType>,
    {
        let response = self.get_parameter(name)?;
        node.with_timeout(response, timeout)
    }

    /// Gets one parameter or returns `alternative` when it is not declared.
    pub fn get_parameter_or<T>(
        &self, name: impl Into<String>, alternative: T,
    ) -> Result<impl Future<Output = Result<T>>>
    where
        ParameterValue: TryInto<T, Error = WrongParameterType>,
    {
        let name = name.into();
        let response = self.get_parameters([name.clone()])?;
        Ok(response.and_then(move |values| {
            let result = single_parameter_value(values).and_then(|value| {
                if matches!(value, ParameterValue::NotSet) {
                    Ok(alternative)
                } else {
                    value.try_into().map_err(|error: WrongParameterType| {
                        Error::ParameterWrongType {
                            name,
                            expected_type: error.expected_type_name,
                            actual_type: error.actual_type_name,
                        }
                    })
                }
            });
            futures::future::ready(result)
        }))
    }

    /// Gets one parameter or an alternative, failing after `timeout`.
    pub fn get_parameter_or_with_timeout<T>(
        &self, node: &mut Node, name: impl Into<String>, alternative: T, timeout: Duration,
    ) -> Result<impl Future<Output = Result<T>>>
    where
        ParameterValue: TryInto<T, Error = WrongParameterType>,
    {
        let response = self.get_parameter_or(name, alternative)?;
        node.with_timeout(response, timeout)
    }

    pub fn get_parameter_types<I, S>(
        &self, names: I,
    ) -> Result<impl Future<Output = Result<Vec<ParameterType>>>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names = strings(names);
        let expected = names.len();
        let request = srv::GetParameterTypes::Request { names };
        let response = self.get_parameter_types.request(&request)?;
        Ok(response.and_then(move |response| async move {
            check_response_length("get_parameter_types", expected, response.types.len())?;
            response
                .types
                .into_iter()
                .map(ParameterType::try_from)
                .collect()
        }))
    }

    /// Gets parameter types, failing with [`Error::RCL_RET_TIMEOUT`] after `timeout`.
    pub fn get_parameter_types_with_timeout<I, S>(
        &self, node: &mut Node, names: I, timeout: Duration,
    ) -> Result<impl Future<Output = Result<Vec<ParameterType>>>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let response = self.get_parameter_types(names)?;
        node.with_timeout(response, timeout)
    }

    /// Gets the type of one parameter. Undeclared parameters return [`ParameterType::NotSet`].
    pub fn get_parameter_type(
        &self, name: impl Into<String>,
    ) -> Result<impl Future<Output = Result<ParameterType>>> {
        let response = self.get_parameter_types([name.into()])?;
        Ok(response.and_then(|mut types| {
            futures::future::ready(types.pop().ok_or(Error::ParameterResponseLengthMismatch {
                operation: "get_parameter_types",
                expected: 1,
                actual: 0,
            }))
        }))
    }

    /// Gets one parameter type, failing after `timeout`.
    pub fn get_parameter_type_with_timeout(
        &self, node: &mut Node, name: impl Into<String>, timeout: Duration,
    ) -> Result<impl Future<Output = Result<ParameterType>>> {
        let response = self.get_parameter_type(name)?;
        node.with_timeout(response, timeout)
    }

    pub fn describe_parameters<I, S>(
        &self, names: I,
    ) -> Result<impl Future<Output = Result<Vec<msg::ParameterDescriptor>>>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names = strings(names);
        let expected = names.len();
        let request = srv::DescribeParameters::Request { names };
        let response = self.describe_parameters.request(&request)?;
        Ok(response.and_then(move |response| async move {
            check_response_length("describe_parameters", expected, response.descriptors.len())?;
            Ok(response.descriptors)
        }))
    }

    /// Describes parameters, failing with [`Error::RCL_RET_TIMEOUT`] after `timeout`.
    pub fn describe_parameters_with_timeout<I, S>(
        &self, node: &mut Node, names: I, timeout: Duration,
    ) -> Result<impl Future<Output = Result<Vec<msg::ParameterDescriptor>>>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let response = self.describe_parameters(names)?;
        node.with_timeout(response, timeout)
    }

    /// Describes one parameter on the target node.
    pub fn describe_parameter(
        &self, name: impl Into<String>,
    ) -> Result<impl Future<Output = Result<msg::ParameterDescriptor>>> {
        let response = self.describe_parameters([name.into()])?;
        Ok(response.and_then(|mut descriptors| {
            futures::future::ready(descriptors.pop().ok_or(
                Error::ParameterResponseLengthMismatch {
                    operation: "describe_parameters",
                    expected: 1,
                    actual: 0,
                },
            ))
        }))
    }

    /// Describes one parameter, failing after `timeout`.
    pub fn describe_parameter_with_timeout(
        &self, node: &mut Node, name: impl Into<String>, timeout: Duration,
    ) -> Result<impl Future<Output = Result<msg::ParameterDescriptor>>> {
        let response = self.describe_parameter(name)?;
        node.with_timeout(response, timeout)
    }

    pub fn list_parameters<I, S>(
        &self, prefixes: I, depth: u64,
    ) -> Result<impl Future<Output = Result<msg::ListParametersResult>>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let request = srv::ListParameters::Request {
            prefixes: strings(prefixes),
            depth,
        };
        let response = self.list_parameters.request(&request)?;
        Ok(response.map(|response| response.map(|response| response.result)))
    }

    /// Lists parameters, failing with [`Error::RCL_RET_TIMEOUT`] after `timeout`.
    pub fn list_parameters_with_timeout<I, S>(
        &self, node: &mut Node, prefixes: I, depth: u64, timeout: Duration,
    ) -> Result<impl Future<Output = Result<msg::ListParametersResult>>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let response = self.list_parameters(prefixes, depth)?;
        node.with_timeout(response, timeout)
    }

    pub fn set_parameters<I, S>(
        &self, parameters: I,
    ) -> Result<impl Future<Output = Result<Vec<msg::SetParametersResult>>>>
    where
        I: IntoIterator<Item = (S, ParameterValue)>,
        S: Into<String>,
    {
        let parameters = parameter_messages(parameters);
        let expected = parameters.len();
        let request = srv::SetParameters::Request { parameters };
        let response = self.set_parameters.request(&request)?;
        Ok(response.and_then(move |response| async move {
            check_response_length("set_parameters", expected, response.results.len())?;
            Ok(response.results)
        }))
    }

    /// Sets parameters, failing with [`Error::RCL_RET_TIMEOUT`] after `timeout`.
    pub fn set_parameters_with_timeout<I, S>(
        &self, node: &mut Node, parameters: I, timeout: Duration,
    ) -> Result<impl Future<Output = Result<Vec<msg::SetParametersResult>>>>
    where
        I: IntoIterator<Item = (S, ParameterValue)>,
        S: Into<String>,
    {
        let response = self.set_parameters(parameters)?;
        node.with_timeout(response, timeout)
    }

    /// Sets one parameter and returns the remote node's validation result.
    pub fn set_parameter(
        &self, name: impl Into<String>, value: ParameterValue,
    ) -> Result<impl Future<Output = Result<msg::SetParametersResult>>> {
        let response = self.set_parameters([(name.into(), value)])?;
        Ok(response.and_then(|mut results| {
            futures::future::ready(results.pop().ok_or(Error::ParameterResponseLengthMismatch {
                operation: "set_parameters",
                expected: 1,
                actual: 0,
            }))
        }))
    }

    /// Sets one parameter, failing after `timeout`.
    pub fn set_parameter_with_timeout(
        &self, node: &mut Node, name: impl Into<String>, value: ParameterValue, timeout: Duration,
    ) -> Result<impl Future<Output = Result<msg::SetParametersResult>>> {
        let response = self.set_parameter(name, value)?;
        node.with_timeout(response, timeout)
    }

    pub fn set_parameters_atomically<I, S>(
        &self, parameters: I,
    ) -> Result<impl Future<Output = Result<msg::SetParametersResult>>>
    where
        I: IntoIterator<Item = (S, ParameterValue)>,
        S: Into<String>,
    {
        let request = srv::SetParametersAtomically::Request {
            parameters: parameter_messages(parameters),
        };
        let response = self.set_parameters_atomically.request(&request)?;
        Ok(response.map(|response| response.map(|response| response.result)))
    }

    /// Atomically sets parameters, failing with [`Error::RCL_RET_TIMEOUT`] after `timeout`.
    pub fn set_parameters_atomically_with_timeout<I, S>(
        &self, node: &mut Node, parameters: I, timeout: Duration,
    ) -> Result<impl Future<Output = Result<msg::SetParametersResult>>>
    where
        I: IntoIterator<Item = (S, ParameterValue)>,
        S: Into<String>,
    {
        let response = self.set_parameters_atomically(parameters)?;
        node.with_timeout(response, timeout)
    }

    pub fn delete_parameters<I, S>(
        &self, names: I,
    ) -> Result<impl Future<Output = Result<Vec<msg::SetParametersResult>>>>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.set_parameters(names.into_iter().map(|name| (name, ParameterValue::NotSet)))
    }

    /// Deletes parameters, failing with [`Error::RCL_RET_TIMEOUT`] after `timeout`.
    pub fn delete_parameters_with_timeout<I, S>(
        &self, node: &mut Node, names: I, timeout: Duration,
    ) -> Result<impl Future<Output = Result<Vec<msg::SetParametersResult>>>>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let response = self.delete_parameters(names)?;
        node.with_timeout(response, timeout)
    }

    /// Deletes one dynamically typed parameter and returns the remote validation result.
    pub fn delete_parameter(
        &self, name: impl Into<String>,
    ) -> Result<impl Future<Output = Result<msg::SetParametersResult>>> {
        self.set_parameter(name, ParameterValue::NotSet)
    }

    /// Deletes one parameter, failing after `timeout`.
    pub fn delete_parameter_with_timeout(
        &self, node: &mut Node, name: impl Into<String>, timeout: Duration,
    ) -> Result<impl Future<Output = Result<msg::SetParametersResult>>> {
        let response = self.delete_parameter(name)?;
        node.with_timeout(response, timeout)
    }
}

fn check_response_length(operation: &'static str, expected: usize, actual: usize) -> Result<()> {
    if expected == actual {
        Ok(())
    } else {
        Err(Error::ParameterResponseLengthMismatch {
            operation,
            expected,
            actual,
        })
    }
}

fn single_parameter_value(mut values: Vec<ParameterValue>) -> Result<ParameterValue> {
    values.pop().ok_or(Error::ParameterResponseLengthMismatch {
        operation: "get_parameters",
        expected: 1,
        actual: 0,
    })
}

fn normalize_remote_node_name(name: &str) -> Result<String> {
    if name.is_empty() || name.as_bytes().contains(&0) {
        return Err(Error::RCL_RET_INVALID_ARGUMENT);
    }
    let normalized = name.trim_end_matches('/');
    if normalized.is_empty() {
        return Err(Error::RCL_RET_INVALID_ARGUMENT);
    }
    Ok(normalized.to_owned())
}

fn strings<I, S>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    values
        .into_iter()
        .map(|value| value.as_ref().to_owned())
        .collect()
}

fn parameter_messages<I, S>(parameters: I) -> Vec<msg::Parameter>
where
    I: IntoIterator<Item = (S, ParameterValue)>,
    S: Into<String>,
{
    parameters
        .into_iter()
        .map(|(name, value)| msg::Parameter {
            name: name.into(),
            value: value.into_parameter_value_msg(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Context, IntegerRange, ParameterDescriptor};
    use futures::{executor::LocalPool, task::LocalSpawnExt, FutureExt, StreamExt};

    fn drive<T>(
        node: &mut Node, pool: &mut LocalPool, future: impl Future<Output = Result<T>>,
    ) -> T {
        drive_result(node, pool, future).unwrap()
    }

    fn drive_result<T>(
        node: &mut Node, pool: &mut LocalPool, future: impl Future<Output = Result<T>>,
    ) -> Result<T> {
        let mut future = Box::pin(future);
        loop {
            node.try_spin_once(Duration::from_millis(10))?;
            pool.run_until_stalled();
            if let Some(result) = future.as_mut().now_or_never() {
                return result;
            }
        }
    }

    #[test]
    fn parameter_client_interoperates_with_standard_services() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "parameter_client_server", "/scope").unwrap();
        node.declare_parameter_with_descriptor(
            "answer",
            ParameterValue::Integer(2),
            ParameterDescriptor {
                description: "even answer".into(),
                integer_range: vec![IntegerRange {
                    from_value: 0,
                    to_value: 10,
                    step: 2,
                }],
                ..Default::default()
            },
        )
        .unwrap();
        node.declare_parameter("ratio", ParameterValue::Double(0.2))
            .unwrap();
        let (handler, _events) = node.make_parameter_handler().unwrap();
        let client = node
            .create_parameter_client("parameter_client_server/")
            .unwrap();
        assert_eq!(client.remote_node_name(), "parameter_client_server");
        assert_eq!(client.resolved_remote_node_name(), "/scope/parameter_client_server");
        let self_client = node.create_parameter_client_for_self().unwrap();
        assert_eq!(self_client.remote_node_name(), "/scope/parameter_client_server");
        assert_eq!(self_client.resolved_remote_node_name(), "/scope/parameter_client_server");
        let custom_qos_client = node
            .create_parameter_client_with_qos(
                "parameter_client_server",
                QosProfile::parameters().keep_last(7),
            )
            .unwrap();
        assert_eq!(
            custom_qos_client
                .get_parameters
                .get_request_publisher_actual_qos()
                .unwrap()
                .depth,
            7
        );
        assert_eq!(
            custom_qos_client
                .describe_parameters
                .get_response_subscription_actual_qos()
                .unwrap()
                .depth,
            7
        );
        let mut pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();

        drive(&mut node, &mut pool, client.wait_for_services().unwrap());
        assert!(client.services_ready(&node).unwrap());
        drive(&mut node, &mut pool, self_client.wait_for_services().unwrap());
        assert!(self_client.services_ready(&node).unwrap());

        let names = ["answer", "missing"];
        assert_eq!(
            drive(&mut node, &mut pool, client.get_parameters(names).unwrap()),
            vec![ParameterValue::Integer(2), ParameterValue::NotSet]
        );
        assert_eq!(
            drive(&mut node, &mut pool, self_client.get_parameters(["answer"]).unwrap()),
            vec![ParameterValue::Integer(2)]
        );
        assert!(drive(&mut node, &mut pool, client.has_parameter("answer").unwrap()));
        assert!(!drive(&mut node, &mut pool, client.has_parameter("missing").unwrap()));
        assert_eq!(drive(&mut node, &mut pool, client.get_parameter::<i64>("answer").unwrap()), 2);
        assert_eq!(
            drive(&mut node, &mut pool, client.get_parameter_or("missing", 42_i64).unwrap(),),
            42
        );
        let wrong_type = client.get_parameter::<String>("answer").unwrap();
        assert!(matches!(
            drive_result(&mut node, &mut pool, wrong_type),
            Err(Error::ParameterWrongType {
                name,
                expected_type: "string",
                actual_type: "integer",
            }) if name == "answer"
        ));
        assert_eq!(
            drive(&mut node, &mut pool, client.get_parameter_types(names).unwrap(),),
            vec![ParameterType::Integer, ParameterType::NotSet]
        );
        assert_eq!(
            drive(&mut node, &mut pool, client.get_parameter_type("answer").unwrap()),
            ParameterType::Integer
        );
        assert_eq!(
            drive(&mut node, &mut pool, client.get_parameter_type("missing").unwrap()),
            ParameterType::NotSet
        );
        let descriptors = drive(&mut node, &mut pool, client.describe_parameters(names).unwrap());
        assert_eq!(descriptors[0].description, "even answer");
        assert_eq!(descriptors[1].type_, msg::ParameterType::PARAMETER_NOT_SET as u8);
        let descriptor = drive(&mut node, &mut pool, client.describe_parameter("answer").unwrap());
        assert_eq!(descriptor.description, "even answer");
        let listed = drive(
            &mut node,
            &mut pool,
            client
                .list_parameters(
                    Vec::<String>::new(),
                    srv::ListParameters::Request::DEPTH_RECURSIVE as u64,
                )
                .unwrap(),
        );
        assert!(listed.names.iter().any(|name| name == "answer"));

        let set = drive(
            &mut node,
            &mut pool,
            client
                .set_parameter("answer", ParameterValue::Integer(4))
                .unwrap(),
        );
        assert!(set.successful);
        let rejected = drive(
            &mut node,
            &mut pool,
            client
                .set_parameter("answer", ParameterValue::Integer(3))
                .unwrap(),
        );
        assert!(!rejected.successful);
        assert_eq!(node.get_parameter::<i64>("answer").unwrap(), 4);

        let results = drive(
            &mut node,
            &mut pool,
            client
                .set_parameters([
                    ("answer", ParameterValue::Integer(4)),
                    ("answer", ParameterValue::Integer(3)),
                    ("temporary", ParameterValue::String("value".into())),
                ])
                .unwrap(),
        );
        assert!(results[0].successful);
        assert!(!results[1].successful);
        assert!(results[2].successful);
        assert_eq!(node.get_parameter::<i64>("answer").unwrap(), 4);

        let atomic = drive(
            &mut node,
            &mut pool,
            client
                .set_parameters_atomically([
                    ("answer", ParameterValue::Integer(6)),
                    ("ratio", ParameterValue::String("wrong".into())),
                ])
                .unwrap(),
        );
        assert!(!atomic.successful);
        assert_eq!(node.get_parameter::<i64>("answer").unwrap(), 4);
        assert_eq!(node.get_parameter::<f64>("ratio").unwrap(), 0.2);

        let deleted = drive(&mut node, &mut pool, client.delete_parameters(["temporary"]).unwrap());
        assert!(deleted[0].successful);
        assert!(!node.has_parameter("temporary"));
        let set = drive(
            &mut node,
            &mut pool,
            client
                .set_parameter("single_temporary", ParameterValue::Bool(true))
                .unwrap(),
        );
        assert!(set.successful);
        let deleted =
            drive(&mut node, &mut pool, client.delete_parameter("single_temporary").unwrap());
        assert!(deleted.successful);
        assert!(!node.has_parameter("single_temporary"));
    }

    #[test]
    fn parameter_client_filters_events_by_resolved_node_and_parameter() {
        use crate::ParameterEventKind;

        let context = Context::create().unwrap();
        let mut server = Node::create(context.clone(), "event_server", "/scope").unwrap();
        let mut client_node = Node::create(context, "event_client", "/scope").unwrap();
        let (handler, _local_events) = server.make_parameter_handler().unwrap();
        let client = client_node.create_parameter_client("event_server").unwrap();
        let mut events = client
            .subscribe_parameter_events_with_options(
                &mut client_node,
                Some("watched".into()),
                QosProfile::parameter_events().keep_last(3),
                SubscriptionOptions::default().channel_capacity(1),
            )
            .unwrap();
        assert!(client
            .subscribe_parameter_events_with_options(
                &mut client_node,
                None,
                QosProfile::parameter_events(),
                SubscriptionOptions::default().channel_capacity(usize::MAX),
            )
            .is_err());
        let mut pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();

        for _ in 0..100 {
            let discovered = server
                .get_subscriptions_info_by_topic("/parameter_events", false)
                .unwrap()
                .iter()
                .any(|endpoint| endpoint.node_name == "event_client");
            if discovered {
                break;
            }
            server.try_spin_once(Duration::from_millis(10)).unwrap();
            client_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
        }

        server
            .set_parameters_atomically([
                ("ignored", ParameterValue::Integer(1)),
                ("watched", ParameterValue::Integer(2)),
            ])
            .unwrap();
        let event = loop {
            server.try_spin_once(Duration::from_millis(10)).unwrap();
            client_node
                .try_spin_once(Duration::from_millis(10))
                .unwrap();
            pool.run_until_stalled();
            if let Some(event) = events.next().now_or_never().flatten() {
                break event;
            }
        };
        assert_eq!(event.node, "/scope/event_server");
        assert_eq!(event.name, "watched");
        assert_eq!(event.kind, ParameterEventKind::New);
        assert_eq!(event.value, ParameterValue::Integer(2));
        assert!(events.next().now_or_never().flatten().is_none());
        let endpoint = server
            .get_subscriptions_info_by_topic("/parameter_events", false)
            .unwrap()
            .into_iter()
            .find(|endpoint| endpoint.node_name == "event_client")
            .unwrap();
        assert_eq!(endpoint.qos_profile.depth, 3);
    }

    #[test]
    fn parameter_client_rejects_unknown_wire_types() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "malformed_parameter_client", "").unwrap();
        let mut requests = node
            .create_service::<srv::GetParameters::Service>(
                "/malformed_target/get_parameters",
                QosProfile::parameters(),
            )
            .unwrap();
        let mut type_requests = node
            .create_service::<srv::GetParameterTypes::Service>(
                "/malformed_target/get_parameter_types",
                QosProfile::parameters(),
            )
            .unwrap();
        let client = node.create_parameter_client("/malformed_target").unwrap();
        let mut response = Box::pin(client.get_parameters(["bad"]).unwrap());

        loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(request) = requests.next().now_or_never().flatten() {
                request
                    .respond(srv::GetParameters::Response {
                        values: vec![msg::ParameterValue {
                            type_: u8::MAX,
                            ..Default::default()
                        }],
                    })
                    .unwrap();
            }
            if let Some(result) = response.as_mut().now_or_never() {
                assert!(matches!(result, Err(Error::UnknownParameterType { type_: u8::MAX })));
                break;
            }
        }

        let mut response = Box::pin(client.get_parameter_types(["bad"]).unwrap());
        loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(request) = type_requests.next().now_or_never().flatten() {
                request
                    .respond(srv::GetParameterTypes::Response {
                        types: vec![u8::MAX],
                    })
                    .unwrap();
            }
            if let Some(result) = response.as_mut().now_or_never() {
                assert!(matches!(result, Err(Error::UnknownParameterType { type_: u8::MAX })));
                break;
            }
        }
    }

    #[test]
    fn parameter_client_rejects_invalid_remote_names() {
        assert!(normalize_remote_node_name("").is_err());
        assert!(normalize_remote_node_name("/").is_err());
        assert!(normalize_remote_node_name("bad\0node").is_err());
        assert_eq!(normalize_remote_node_name("node/").unwrap(), "node");
        assert_eq!(normalize_remote_node_name("/scope/node/").unwrap(), "/scope/node");
    }

    #[test]
    fn parameter_client_operations_support_timeouts() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "parameter_timeout_client", "").unwrap();
        let client = node
            .create_parameter_client("/missing_parameter_server")
            .unwrap();
        let mut pool = LocalPool::new();
        let timeout = Duration::from_millis(1);

        macro_rules! assert_times_out {
            ($future:expr) => {
                let future = $future;
                assert!(matches!(
                    drive_result(&mut node, &mut pool, future),
                    Err(Error::RCL_RET_TIMEOUT)
                ));
            };
        }

        assert_times_out!(client
            .get_parameters_with_timeout(&mut node, ["value"], timeout)
            .unwrap());
        assert_times_out!(client
            .has_parameter_with_timeout(&mut node, "value", timeout)
            .unwrap());
        assert_times_out!(client
            .get_parameter_with_timeout::<i64>(&mut node, "value", timeout)
            .unwrap());
        assert_times_out!(client
            .get_parameter_or_with_timeout(&mut node, "value", 42_i64, timeout)
            .unwrap());
        assert_times_out!(client
            .get_parameter_types_with_timeout(&mut node, ["value"], timeout)
            .unwrap());
        assert_times_out!(client
            .get_parameter_type_with_timeout(&mut node, "value", timeout)
            .unwrap());
        assert_times_out!(client
            .describe_parameters_with_timeout(&mut node, ["value"], timeout)
            .unwrap());
        assert_times_out!(client
            .describe_parameter_with_timeout(&mut node, "value", timeout)
            .unwrap());
        assert_times_out!(client
            .list_parameters_with_timeout(&mut node, ["prefix"], 1, timeout)
            .unwrap());
        assert_times_out!(client
            .set_parameters_with_timeout(
                &mut node,
                [("value", ParameterValue::Integer(1))],
                timeout,
            )
            .unwrap());
        assert_times_out!(client
            .set_parameter_with_timeout(&mut node, "value", ParameterValue::Integer(1), timeout,)
            .unwrap());
        assert_times_out!(client
            .set_parameters_atomically_with_timeout(
                &mut node,
                [("value", ParameterValue::Integer(1))],
                timeout,
            )
            .unwrap());
        assert_times_out!(client
            .delete_parameters_with_timeout(&mut node, ["value"], timeout)
            .unwrap());
        assert_times_out!(client
            .delete_parameter_with_timeout(&mut node, "value", timeout)
            .unwrap());
    }

    #[test]
    fn parameter_client_rejects_mismatched_response_lengths() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "short_parameter_server", "").unwrap();
        let get = node
            .create_service::<srv::GetParameters::Service>(
                "/short_parameter_server/get_parameters",
                QosProfile::parameters(),
            )
            .unwrap();
        let types = node
            .create_service::<srv::GetParameterTypes::Service>(
                "/short_parameter_server/get_parameter_types",
                QosProfile::parameters(),
            )
            .unwrap();
        let describe = node
            .create_service::<srv::DescribeParameters::Service>(
                "/short_parameter_server/describe_parameters",
                QosProfile::parameters(),
            )
            .unwrap();
        let set = node
            .create_service::<srv::SetParameters::Service>(
                "/short_parameter_server/set_parameters",
                QosProfile::parameters(),
            )
            .unwrap();
        let client = node
            .create_parameter_client("/short_parameter_server")
            .unwrap();
        let mut pool = LocalPool::new();
        pool.spawner()
            .spawn_local(get.for_each(|request| async move {
                request
                    .respond(srv::GetParameters::Response::default())
                    .unwrap();
            }))
            .unwrap();
        pool.spawner()
            .spawn_local(types.for_each(|request| async move {
                request
                    .respond(srv::GetParameterTypes::Response::default())
                    .unwrap();
            }))
            .unwrap();
        pool.spawner()
            .spawn_local(describe.for_each(|request| async move {
                request
                    .respond(srv::DescribeParameters::Response::default())
                    .unwrap();
            }))
            .unwrap();
        pool.spawner()
            .spawn_local(set.for_each(|request| async move {
                request
                    .respond(srv::SetParameters::Response::default())
                    .unwrap();
            }))
            .unwrap();

        let response = client.get_parameters(["first", "second"]).unwrap();
        assert!(matches!(
            drive_result(&mut node, &mut pool, response),
            Err(Error::ParameterResponseLengthMismatch {
                operation: "get_parameters",
                expected: 2,
                actual: 0,
            })
        ));
        let response = client.get_parameter_types(["first"]).unwrap();
        assert!(matches!(
            drive_result(&mut node, &mut pool, response),
            Err(Error::ParameterResponseLengthMismatch {
                operation: "get_parameter_types",
                expected: 1,
                actual: 0,
            })
        ));
        let response = client.describe_parameters(["first"]).unwrap();
        assert!(matches!(
            drive_result(&mut node, &mut pool, response),
            Err(Error::ParameterResponseLengthMismatch {
                operation: "describe_parameters",
                expected: 1,
                actual: 0,
            })
        ));
        let response = client
            .set_parameters([
                ("first", ParameterValue::Integer(1)),
                ("second", ParameterValue::Integer(2)),
            ])
            .unwrap();
        assert!(matches!(
            drive_result(&mut node, &mut pool, response),
            Err(Error::ParameterResponseLengthMismatch {
                operation: "set_parameters",
                expected: 2,
                actual: 0,
            })
        ));
    }

    #[test]
    fn parameter_client_is_cloneable() {
        fn assert_clone<T: Clone>() {}

        assert_clone::<ParameterClient>();
    }
}
