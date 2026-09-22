use crate::{
    rcl_interfaces::{msg, srv},
    Client, Error, Node, ParameterValue, QosProfile, Result,
};
use futures::{future::try_join_all, Future, FutureExt, TryFutureExt};
use std::time::Duration;

/// Runtime-agnostic client for the standard ROS parameter services of another node.
#[derive(Clone)]
pub struct ParameterClient {
    remote_node_name: String,
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
        Ok(Self {
            get_parameters: node
                .create_client(&format!("{remote_node_name}/get_parameters"), qos.clone())?,
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
        })
    }

    pub fn remote_node_name(&self) -> &str {
        &self.remote_node_name
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
        let request = srv::GetParameters::Request {
            names: strings(names),
        };
        let response = self.get_parameters.request(&request)?;
        Ok(response.and_then(|response| async move {
            response
                .values
                .into_iter()
                .map(ParameterValue::try_from_parameter_value_msg)
                .collect()
        }))
    }

    pub fn get_parameter_types<I, S>(
        &self, names: I,
    ) -> Result<impl Future<Output = Result<Vec<u8>>>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let request = srv::GetParameterTypes::Request {
            names: strings(names),
        };
        let response = self.get_parameter_types.request(&request)?;
        Ok(response.map(|response| response.map(|response| response.types)))
    }

    pub fn describe_parameters<I, S>(
        &self, names: I,
    ) -> Result<impl Future<Output = Result<Vec<msg::ParameterDescriptor>>>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let request = srv::DescribeParameters::Request {
            names: strings(names),
        };
        let response = self.describe_parameters.request(&request)?;
        Ok(response.map(|response| response.map(|response| response.descriptors)))
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

    pub fn set_parameters<I, S>(
        &self, parameters: I,
    ) -> Result<impl Future<Output = Result<Vec<msg::SetParametersResult>>>>
    where
        I: IntoIterator<Item = (S, ParameterValue)>,
        S: Into<String>,
    {
        let request = srv::SetParameters::Request {
            parameters: parameter_messages(parameters),
        };
        let response = self.set_parameters.request(&request)?;
        Ok(response.map(|response| response.map(|response| response.results)))
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

    pub fn delete_parameters<I, S>(
        &self, names: I,
    ) -> Result<impl Future<Output = Result<Vec<msg::SetParametersResult>>>>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.set_parameters(names.into_iter().map(|name| (name, ParameterValue::NotSet)))
    }
}

fn normalize_remote_node_name(name: &str) -> Result<String> {
    if name.is_empty() || name.as_bytes().contains(&0) {
        return Err(Error::RCL_RET_INVALID_ARGUMENT);
    }
    let normalized = name.trim_end_matches('/');
    if normalized.is_empty() {
        return Err(Error::RCL_RET_INVALID_ARGUMENT);
    }
    Ok(if normalized.starts_with('/') {
        normalized.to_owned()
    } else {
        format!("/{normalized}")
    })
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
        let mut future = Box::pin(future);
        loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            pool.run_until_stalled();
            if let Some(result) = future.as_mut().now_or_never() {
                return result.unwrap();
            }
        }
    }

    #[test]
    fn parameter_client_interoperates_with_standard_services() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "parameter_client_server", "").unwrap();
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
        assert_eq!(client.remote_node_name(), "/parameter_client_server");
        let mut pool = LocalPool::new();
        pool.spawner().spawn_local(handler).unwrap();

        drive(&mut node, &mut pool, client.wait_for_services().unwrap());
        assert!(client.services_ready(&node).unwrap());

        let names = ["answer", "missing"];
        assert_eq!(
            drive(&mut node, &mut pool, client.get_parameters(names).unwrap()),
            vec![ParameterValue::Integer(2), ParameterValue::NotSet]
        );
        assert_eq!(
            drive(&mut node, &mut pool, client.get_parameter_types(names).unwrap(),),
            vec![
                msg::ParameterType::PARAMETER_INTEGER as u8,
                msg::ParameterType::PARAMETER_NOT_SET as u8,
            ]
        );
        let descriptors = drive(&mut node, &mut pool, client.describe_parameters(names).unwrap());
        assert_eq!(descriptors[0].description, "even answer");
        assert_eq!(descriptors[1].type_, msg::ParameterType::PARAMETER_NOT_SET as u8);
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
    }

    #[test]
    fn parameter_client_rejects_invalid_remote_names() {
        assert!(normalize_remote_node_name("").is_err());
        assert!(normalize_remote_node_name("/").is_err());
        assert!(normalize_remote_node_name("bad\0node").is_err());
        assert_eq!(normalize_remote_node_name("node/").unwrap(), "/node");
    }

    #[test]
    fn parameter_client_is_cloneable() {
        fn assert_clone<T: Clone>() {}

        assert_clone::<ParameterClient>();
    }
}
