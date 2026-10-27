use std::{
    future::Future,
    time::{Duration, Instant},
};

use futures::{FutureExt, StreamExt};

use super::*;
use crate::{
    example_interfaces::srv::AddTwoInts, service_msgs::msg::ServiceEventInfo, Context, Subscription,
};

fn settle(node: &mut Node) {
    let until = Instant::now() + Duration::from_millis(100);
    while Instant::now() < until {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn observe_call(
    node: &mut Node, response: impl Future<Output = Result<AddTwoInts::Response>>,
    mut serve: impl FnMut() -> Option<[u8; 16]>, events: &mut Subscription<AddTwoInts::Event>,
    state: ServiceIntrospectionState, client_gid: [u8; 16],
) {
    futures::pin_mut!(response);
    let until = Instant::now() + Duration::from_secs(3);
    let mut result = None;
    let mut received = Vec::new();
    let mut writer_guid = None;
    loop {
        assert!(Instant::now() < until, "response={result:?}, events={received:?}");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(guid) = serve() {
            writer_guid = Some(guid);
        }
        if result.is_none() {
            result = response.as_mut().now_or_never();
        }
        while let Some(event) = events.next().now_or_never().flatten() {
            received.push(event);
        }
        if result.is_some() && (state == ServiceIntrospectionState::Off || received.len() >= 4) {
            break;
        }
    }
    assert_eq!(result.unwrap().unwrap().sum, 42);
    if state == ServiceIntrospectionState::Off {
        settle(node);
        while let Some(event) = events.next().now_or_never().flatten() {
            received.push(event);
        }
        assert!(received.is_empty(), "disabled introspection emitted {received:?}");
        return;
    }
    assert_eq!(received.len(), 4);
    let mut event_types = received
        .iter()
        .map(|event| event.info.event_type)
        .collect::<Vec<_>>();
    event_types.sort_unstable();
    assert_eq!(
        event_types,
        vec![
            ServiceEventInfo::REQUEST_SENT as u8,
            ServiceEventInfo::REQUEST_RECEIVED as u8,
            ServiceEventInfo::RESPONSE_SENT as u8,
            ServiceEventInfo::RESPONSE_RECEIVED as u8,
        ]
    );
    for event in &received {
        assert_eq!(event.info.stamp.sec, 12);
        assert_eq!(event.info.stamp.nanosec, 34);
        assert_eq!(event.info.sequence_number, received[0].info.sequence_number);
        // Some RMW implementations encode request writer identity differently from client GID.
        // Compare each native event against the identity of its originating endpoint.
        let expected_gid = if event.info.event_type == ServiceEventInfo::REQUEST_SENT as u8
            || event.info.event_type == ServiceEventInfo::RESPONSE_RECEIVED as u8
        {
            client_gid
        } else {
            writer_guid.unwrap()
        };
        assert_eq!(event.info.client_gid.as_slice(), expected_gid);
        assert!(event.info.client_gid.iter().any(|byte| *byte != 0));
        if state == ServiceIntrospectionState::Metadata {
            assert!(event.request.is_empty());
            assert!(event.response.is_empty());
        } else if event.info.event_type <= ServiceEventInfo::REQUEST_RECEIVED as u8 {
            assert_eq!(event.request, vec![AddTwoInts::Request { a: 19, b: 23 }]);
            assert!(event.response.is_empty());
        } else {
            assert_eq!(event.response, vec![AddTwoInts::Response { sum: 42 }]);
            assert!(event.request.is_empty());
        }
    }
}

#[test]
fn typed_service_introspection_transitions_and_simulated_timestamps() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "typed_service_introspection", "").unwrap();
    node.get_ros_clock()
        .lock()
        .enable_ros_time_override(12_000_000_034)
        .unwrap();
    let topic = "/typed_service_introspection/_service_event";
    let event_qos = QosProfile::default().keep_last(7).transient_local();
    let mut events = node
        .subscribe::<AddTwoInts::Event>(topic, event_qos.clone())
        .unwrap();
    let mut service = node
        .create_service::<AddTwoInts::Service>(
            "/typed_service_introspection",
            QosProfile::default(),
        )
        .unwrap();
    let client = node
        .create_client::<AddTwoInts::Service>("/typed_service_introspection", QosProfile::default())
        .unwrap();

    for state in [
        ServiceIntrospectionState::Off,
        ServiceIntrospectionState::Metadata,
        ServiceIntrospectionState::Contents,
        ServiceIntrospectionState::Off,
        ServiceIntrospectionState::Contents,
    ] {
        service
            .configure_introspection(&mut node, state, event_qos.clone())
            .unwrap();
        client
            .configure_introspection(&mut node, state, event_qos.clone())
            .unwrap();
        settle(&mut node);
        assert_eq!(
            node.count_publishers(topic).unwrap(),
            if state == ServiceIntrospectionState::Off {
                0
            } else {
                2
            }
        );
        for publisher in node.get_publishers_info_by_topic(topic, false).unwrap() {
            // Fast DDS graph discovery does not report history/depth. Retained history is
            // checked independently with a late subscriber below.
            if publisher.qos_profile.history == crate::qos::HistoryPolicy::KeepLast {
                assert_eq!(publisher.qos_profile.depth, event_qos.depth);
            }
            assert_eq!(publisher.qos_profile.reliability, event_qos.reliability);
            assert_eq!(publisher.qos_profile.durability, event_qos.durability);
        }
        let response = client
            .request(&AddTwoInts::Request { a: 19, b: 23 })
            .unwrap();
        observe_call(
            &mut node,
            response,
            || {
                if let Some(request) = service.next().now_or_never().flatten() {
                    let writer_guid = request.request_info().writer_guid;
                    let sum = request.message.a + request.message.b;
                    request.respond(AddTwoInts::Response { sum }).unwrap();
                    Some(writer_guid)
                } else {
                    None
                }
            },
            &mut events,
            state,
            client.get_gid().unwrap().data,
        );
    }
}

#[test]
fn event_qos_limits_retained_history_for_late_subscribers() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "service_event_history", "").unwrap();
    let mut service = node
        .create_service::<AddTwoInts::Service>("/service_event_history", QosProfile::default())
        .unwrap();
    let client = node
        .create_client::<AddTwoInts::Service>("/service_event_history", QosProfile::default())
        .unwrap();
    client
        .configure_introspection(
            &mut node,
            ServiceIntrospectionState::Contents,
            QosProfile::default().keep_last(1).transient_local(),
        )
        .unwrap();
    settle(&mut node);
    let mut last_sequence = 0;
    for a in [1, 19] {
        let call = client.request(&AddTwoInts::Request { a, b: 23 }).unwrap();
        last_sequence = node
            .spin_until_future_complete(
                async {
                    let serve = async {
                        let request = service.next().await.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
                        let sequence = request.request_info().sequence_number;
                        let sum = request.message.a + request.message.b;
                        request.respond(AddTwoInts::Response { sum })?;
                        Ok::<_, Error>(sequence)
                    };
                    let (sequence, response) = futures::try_join!(serve, call)?;
                    assert_eq!(response.sum, a + 23);
                    Ok(sequence)
                },
                Duration::from_secs(3),
            )
            .unwrap();
    }
    let mut late = node
        .subscribe::<AddTwoInts::Event>(
            "/service_event_history/_service_event",
            QosProfile::default().keep_last(10).transient_local(),
        )
        .unwrap();
    let event = node
        .spin_until_future_complete(
            async { late.next().await.ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID) },
            Duration::from_secs(3),
        )
        .unwrap();
    assert_eq!(event.info.event_type, ServiceEventInfo::RESPONSE_RECEIVED as u8);
    assert_eq!(event.info.sequence_number, last_sequence);
    assert_eq!(event.response, vec![AddTwoInts::Response { sum: 42 }]);
    settle(&mut node);
    assert!(late.next().now_or_never().is_none(), "publisher retained more than one event");
}

#[test]
fn untyped_service_introspection_publishes_typed_events() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "untyped_service_introspection", "").unwrap();
    node.get_ros_clock()
        .lock()
        .enable_ros_time_override(12_000_000_034)
        .unwrap();
    let mut events = node
        .subscribe::<AddTwoInts::Event>(
            "/untyped_service_introspection/_service_event",
            QosProfile::default(),
        )
        .unwrap();
    let mut service = node
        .create_service_untyped(
            "/untyped_service_introspection",
            "example_interfaces/srv/AddTwoInts",
            QosProfile::default(),
        )
        .unwrap();
    let client = node
        .create_client_untyped(
            "/untyped_service_introspection",
            "example_interfaces/srv/AddTwoInts",
            QosProfile::default(),
        )
        .unwrap();

    for state in [
        ServiceIntrospectionState::Off,
        ServiceIntrospectionState::Metadata,
        ServiceIntrospectionState::Contents,
        ServiceIntrospectionState::Off,
        ServiceIntrospectionState::Contents,
    ] {
        service
            .configure_introspection(&mut node, state, QosProfile::default())
            .unwrap();
        client
            .configure_introspection(&mut node, state, QosProfile::default())
            .unwrap();
        settle(&mut node);
        let response = client
            .request(serde_json::json!({ "a": 19, "b": 23 }))
            .unwrap()
            .map(|result| {
                result
                    .and_then(|json| json)
                    .map(|json| serde_json::from_value::<AddTwoInts::Response>(json).unwrap())
            });
        observe_call(
            &mut node,
            response,
            || {
                if let Some(request) = service.next().now_or_never().flatten() {
                    let writer_guid = request.request_info().writer_guid;
                    let message = request.message.as_ref().unwrap();
                    let sum = message["a"].as_i64().unwrap() + message["b"].as_i64().unwrap();
                    request.respond(serde_json::json!({ "sum": sum })).unwrap();
                    Some(writer_guid)
                } else {
                    None
                }
            },
            &mut events,
            state,
            client.get_gid().unwrap().data,
        );
    }
}

#[test]
fn service_introspection_rejects_foreign_nodes_and_destroyed_clients() {
    let context = Context::create().unwrap();
    let mut owner = Node::create(context.clone(), "introspection_owner", "").unwrap();
    let mut foreign = Node::create(context, "introspection_foreign", "").unwrap();
    let service = owner
        .create_service::<AddTwoInts::Service>("/introspection_owner", QosProfile::default())
        .unwrap();
    let untyped_service = owner
        .create_service_untyped(
            "/introspection_owner_untyped",
            "example_interfaces/srv/AddTwoInts",
            QosProfile::default(),
        )
        .unwrap();
    let client = owner
        .create_client::<AddTwoInts::Service>("/introspection_owner", QosProfile::default())
        .unwrap();
    let untyped_client = owner
        .create_client_untyped(
            "/introspection_owner_untyped",
            "example_interfaces/srv/AddTwoInts",
            QosProfile::default(),
        )
        .unwrap();
    let state = ServiceIntrospectionState::Contents;
    assert!(matches!(
        client.configure_introspection(&mut foreign, state, QosProfile::default()),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert!(matches!(
        untyped_client.configure_introspection(&mut foreign, state, QosProfile::default()),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert!(matches!(
        service.configure_introspection(&mut foreign, state, QosProfile::default()),
        Err(Error::RCL_RET_SERVICE_INVALID)
    ));
    assert!(matches!(
        untyped_service.configure_introspection(&mut foreign, state, QosProfile::default()),
        Err(Error::RCL_RET_SERVICE_INVALID)
    ));
    let stale = client.clone();
    owner.destroy_client(client);
    assert!(matches!(
        stale.configure_introspection(&mut owner, state, QosProfile::default()),
        Err(Error::RCL_RET_CLIENT_INVALID)
    ));
    let stale = untyped_client.clone();
    owner.destroy_client_untyped(untyped_client);
    assert!(matches!(
        stale.configure_introspection(&mut owner, state, QosProfile::default()),
        Err(Error::RCL_RET_CLIENT_INVALID)
    ));
    assert_eq!(
        foreign
            .count_publishers("/introspection_owner/_service_event")
            .unwrap(),
        0
    );
}
