//! Independent typed/untyped service endpoint for the service-event interoperability oracle.

#[cfg(any(
    r2r__ros__distro__iron,
    r2r__ros__distro__jazzy,
    r2r__ros__distro__kilted,
    r2r__ros__distro__lyrical,
    r2r__ros__distro__rolling
))]
mod endpoint {
    use futures::{FutureExt, StreamExt};
    use r2r::{
        example_interfaces::srv::AddTwoInts, Client, ClientUntyped, Node, QosProfile, Service,
        ServiceIntrospectionState, ServiceUntyped, Subscription, SubscriptionOptions,
    };
    use serde_json::{json, Value};
    use std::{
        error::Error,
        io::{self, BufRead, Write},
        time::{Duration, Instant},
    };

    const REQUESTS: usize = 3;
    const CLOCK_NS: i64 = 12_000_000_034;
    const TIMEOUT: Duration = Duration::from_secs(10);
    type Outcome<T> = Result<T, Box<dyn Error>>;

    enum Endpoint {
        TypedServer(Service<AddTwoInts::Service>),
        UntypedServer(ServiceUntyped),
        TypedClient(Client<AddTwoInts::Service>),
        UntypedClient(ClientUntyped),
    }

    impl Endpoint {
        fn wait_for_service(&self, node: &mut Node) -> r2r::Result<()> {
            match self {
                Self::TypedClient(client) => {
                    node.spin_until_future_complete(client.wait_for_service()?, TIMEOUT)
                }
                Self::UntypedClient(client) => {
                    node.spin_until_future_complete(client.wait_for_service()?, TIMEOUT)
                }
                _ => Err(r2r::Error::RCL_RET_INVALID_ARGUMENT),
            }
        }

        fn configure(&self, node: &mut Node, state: ServiceIntrospectionState) -> r2r::Result<()> {
            let qos = event_qos();
            match self {
                Self::TypedServer(endpoint) => endpoint.configure_introspection(node, state, qos),
                Self::UntypedServer(endpoint) => endpoint.configure_introspection(node, state, qos),
                Self::TypedClient(endpoint) => endpoint.configure_introspection(node, state, qos),
                Self::UntypedClient(endpoint) => endpoint.configure_introspection(node, state, qos),
            }
        }

        fn call(&self, node: &mut Node, a: i64, b: i64) -> Outcome<Value> {
            let (sum, sequence, gid) = match self {
                Self::TypedClient(client) => {
                    let response = node.spin_until_future_complete(
                        client.request_with_info(&AddTwoInts::Request { a, b })?,
                        TIMEOUT,
                    )?;
                    (response.response.sum, response.info.sequence_number, client.get_gid()?.data)
                }
                Self::UntypedClient(client) => {
                    let response = node.spin_until_future_complete(
                        client.request_with_info(json!({"a": a, "b": b}))?,
                        TIMEOUT,
                    )?;
                    let sum = response.response?["sum"]
                        .as_i64()
                        .ok_or("invalid untyped sum")?;
                    (sum, response.info.sequence_number, client.get_gid()?.data)
                }
                _ => return Err("call on a server".into()),
            };
            if sum != a + b {
                return Err("response sum mismatch".into());
            }
            Ok(json!({"a": a, "b": b, "sum": sum, "sequence": sequence, "client_gid": gid}))
        }

        fn serve(&mut self) -> Outcome<Option<Value>> {
            let (a, b, info) = match self {
                Self::TypedServer(service) => {
                    let Some(request) = service.next().now_or_never().flatten() else {
                        return Ok(None);
                    };
                    let (a, b, info) =
                        (request.message.a, request.message.b, request.request_info());
                    request.respond(AddTwoInts::Response { sum: a + b })?;
                    (a, b, info)
                }
                Self::UntypedServer(service) => {
                    let Some(request) = service.next().now_or_never().flatten() else {
                        return Ok(None);
                    };
                    let message = request
                        .message
                        .as_ref()
                        .map_err(|error| error.to_string())?;
                    let a = message["a"].as_i64().ok_or("invalid untyped operand a")?;
                    let b = message["b"].as_i64().ok_or("invalid untyped operand b")?;
                    let info = request.request_info();
                    request.respond(json!({"sum": a + b}))?;
                    (a, b, info)
                }
                _ => return Err("serve on a client".into()),
            };
            Ok(Some(
                json!({"a": a, "b": b, "sum": a + b, "sequence": info.sequence_number, "writer_guid": info.writer_guid}),
            ))
        }
    }

    fn event_qos() -> QosProfile {
        QosProfile::default().keep_last(16).transient_local()
    }

    fn emit(value: Value) -> Outcome<()> {
        println!("{value}");
        io::stdout().flush()?;
        Ok(())
    }

    fn drain(events: &mut Subscription<AddTwoInts::Event>, output: &mut Vec<Value>) -> Outcome<()> {
        while let Some(event) = events.next().now_or_never().flatten() {
            output.push(serde_json::to_value(event)?);
        }
        Ok(())
    }

    pub fn run() -> Outcome<()> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args.len() != 3
            || !["client", "server"].contains(&args[0].as_str())
            || !["typed", "untyped"].contains(&args[1].as_str())
        {
            return Err("usage: service_introspection_interop <client|server> <typed|untyped> <service-name>; modes arrive on stdin".into());
        }
        let (role, representation, name) = (&args[0], &args[1], &args[2]);
        let context = r2r::Context::create()?;
        let mut node =
            Node::create(context, &format!("service_events_{role}_{}", std::process::id()), "")?;
        // Exercise the public managed-clock path, rather than exposing private clock handles.
        node.get_time_source().enable_sim_time(&mut node)?;
        let clock = node
            .create_publisher::<r2r::rosgraph_msgs::msg::Clock>("/clock", QosProfile::default())?;
        let rmw = clock.get_gid()?.implementation_identifier;
        let until = Instant::now() + TIMEOUT;
        while node.now_ns()? != CLOCK_NS {
            if Instant::now() >= until {
                return Err("simulated clock setup timed out".into());
            }
            clock.publish(&r2r::rosgraph_msgs::msg::Clock {
                clock: r2r::builtin_interfaces::msg::Time {
                    sec: 12,
                    nanosec: 34,
                },
            })?;
            node.try_spin_once(Duration::from_millis(10))?;
        }
        let topic = format!("{name}/_service_event");
        let mut events = node.subscribe_with_options::<AddTwoInts::Event>(
            &topic,
            event_qos(),
            SubscriptionOptions::default().channel_capacity(64),
        )?;
        let qos = QosProfile::services_default();
        let mut endpoint = match (role.as_str(), representation.as_str()) {
            ("server", "typed") => {
                Endpoint::TypedServer(node.create_service::<AddTwoInts::Service>(name, qos)?)
            }
            ("server", _) => Endpoint::UntypedServer(node.create_service_untyped(
                name,
                "example_interfaces/srv/AddTwoInts",
                qos,
            )?),
            ("client", "typed") => {
                Endpoint::TypedClient(node.create_client::<AddTwoInts::Service>(name, qos)?)
            }
            _ => Endpoint::UntypedClient(node.create_client_untyped(
                name,
                "example_interfaces/srv/AddTwoInts",
                qos,
            )?),
        };
        emit(json!({"kind": "started", "role": role, "representation": representation}))?;
        for (phase, line) in io::stdin().lock().lines().enumerate() {
            let mode = line?;
            let state = match mode.as_str() {
                "off" => ServiceIntrospectionState::Off,
                "metadata" => ServiceIntrospectionState::Metadata,
                "contents" => ServiceIntrospectionState::Contents,
                _ => return Err("unknown introspection mode".into()),
            };
            endpoint.configure(&mut node, state)?;
            emit(json!({"kind": "ready", "phase": phase, "mode": mode, "role": role}))?;
            let mut calls = Vec::new();
            if role == "client" {
                endpoint.wait_for_service(&mut node)?;
                for i in 0..REQUESTS {
                    calls.push(endpoint.call(&mut node, (phase * REQUESTS + i + 1) as i64, 23)?);
                }
            }
            let expected_events = if state == ServiceIntrospectionState::Off {
                0
            } else {
                REQUESTS * 4
            };
            let mut received = Vec::new();
            let deadline = Instant::now() + TIMEOUT;
            loop {
                if Instant::now() >= deadline {
                    return Err(format!(
                        "phase {phase} timed out: {} calls, {} events",
                        calls.len(),
                        received.len()
                    )
                    .into());
                }
                node.try_spin_once(Duration::from_millis(10))?;
                if role == "server" {
                    while let Some(call) = endpoint.serve()? {
                        calls.push(call);
                    }
                }
                drain(&mut events, &mut received)?;
                if calls.len() >= REQUESTS && received.len() >= expected_events {
                    break;
                }
            }
            // Keep spinning after completion to catch duplicate events and disabled publication.
            let quiet = Instant::now() + Duration::from_millis(200);
            while Instant::now() < quiet {
                node.try_spin_once(Duration::from_millis(10))?;
                drain(&mut events, &mut received)?;
            }
            if calls.len() != REQUESTS || received.len() != expected_events {
                return Err("unexpected request/event count".into());
            }
            let expected_publishers = if expected_events == 0 { 0 } else { 2 };
            while node.count_publishers(&topic)? != expected_publishers {
                if Instant::now() >= deadline {
                    return Err("event publisher graph did not converge".into());
                }
                node.try_spin_once(Duration::from_millis(10))?;
                drain(&mut events, &mut received)?;
            }
            emit(json!({"kind": "phase", "implementation": "r2r", "role": role,
                "representation": representation, "service": name, "phase": phase, "mode": mode,
                "rmw": rmw, "clock_ns": node.now_ns()?,
                "event_publishers": expected_publishers, "calls": calls, "events": received}))?;
        }
        Ok(())
    }
}

#[cfg(any(
    r2r__ros__distro__iron,
    r2r__ros__distro__jazzy,
    r2r__ros__distro__kilted,
    r2r__ros__distro__lyrical,
    r2r__ros__distro__rolling
))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    endpoint::run()
}

#[cfg(not(any(
    r2r__ros__distro__iron,
    r2r__ros__distro__jazzy,
    r2r__ros__distro__kilted,
    r2r__ros__distro__lyrical,
    r2r__ros__distro__rolling
)))]
fn main() -> std::process::ExitCode {
    eprintln!("Service introspection requires ROS 2 Iron or newer.");
    std::process::ExitCode::from(2)
}
