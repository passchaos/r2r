use super::*;
use crate::{
    action_type_hash, message_type_hash, service_type_hash, std_msgs::msg::UInt64,
    type_description_hash, type_description_interfaces::srv::GetTypeDescription,
    FutureExecutionOptions,
};
use futures::StreamExt;

fn query(node: &mut Node, request: GetTypeDescription::Request) -> GetTypeDescription::Response {
    let name = node.type_description_service_name().unwrap().unwrap();
    let client = node
        .create_client::<GetTypeDescription::Service>(&name, QosProfile::services_default())
        .unwrap();
    let ready = client.wait_for_service().unwrap();
    node.spin_until_future_complete_with_options(
        ready,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let response = client.request(&request).unwrap();
    node.spin_until_future_complete_with_options(
        response,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap()
}

#[test]
fn type_description_service_returns_schema_sources_hash_and_releases_removed_types() {
    let mut node =
        Node::create(Context::create().unwrap(), "native_type_description", "/schema").unwrap();
    assert!(node.start_type_description_service().unwrap());
    assert!(!node.start_type_description_service().unwrap());
    assert_eq!(
        node.type_description_service_name().unwrap().as_deref(),
        Some("/schema/native_type_description/get_type_description")
    );
    let publisher = node
        .create_publisher::<UInt64>("values", QosProfile::default())
        .unwrap();
    let hash = message_type_hash::<UInt64>().unwrap();
    for sources in [true, false, true, false] {
        let response = query(
            &mut node,
            GetTypeDescription::Request {
                type_name: "std_msgs/msg/UInt64".into(),
                type_hash: hash.to_string().into(),
                include_type_sources: sources,
            },
        );
        assert!(response.successful, "{}", response.failure_reason);
        assert!(response.failure_reason.as_bytes().is_empty());
        assert_eq!(response.type_description.type_description.type_name, "std_msgs/msg/UInt64");
        assert_eq!(response.type_description.type_description.fields.len(), 1);
        assert_eq!(response.type_description.type_description.fields[0].name, "data");
        assert_eq!(type_description_hash(&response.type_description).unwrap(), hash);
        assert_eq!(!response.type_sources.is_empty(), sources);
        if sources {
            assert!(response
                .type_sources
                .iter()
                .any(|source| source.type_name == "std_msgs/msg/UInt64"
                    && !source.raw_file_contents.as_bytes().is_empty()));
        }
    }
    node.destroy_publisher(publisher);
    let response = query(
        &mut node,
        GetTypeDescription::Request {
            type_name: "std_msgs/msg/UInt64".into(),
            type_hash: hash.to_string().into(),
            include_type_sources: true,
        },
    );
    assert!(!response.successful);
    assert!(response
        .failure_reason
        .to_string()
        .contains("not currently in use"));
}

#[test]
fn type_description_service_shares_types_across_endpoints_and_covers_services_actions() {
    use crate::example_interfaces::{action::Fibonacci, srv::AddTwoInts};
    let mut node =
        Node::create(Context::create().unwrap(), "native_description_types", "").unwrap();
    node.start_type_description_service().unwrap();
    let publisher = node
        .create_publisher::<UInt64>("values", QosProfile::default())
        .unwrap();
    let subscription = node
        .subscribe::<UInt64>("values", QosProfile::default())
        .unwrap();
    node.destroy_publisher(publisher);
    let _service = node
        .create_service::<AddTwoInts::Service>("sum", QosProfile::services_default())
        .unwrap();
    let _action = node
        .create_action_server::<Fibonacci::Action>("fibonacci")
        .unwrap();
    for (name, hash) in [
        ("std_msgs/msg/UInt64", message_type_hash::<UInt64>().unwrap()),
        (
            "example_interfaces/srv/AddTwoInts",
            service_type_hash::<AddTwoInts::Service>().unwrap(),
        ),
        (
            "example_interfaces/action/Fibonacci",
            action_type_hash::<Fibonacci::Action>().unwrap(),
        ),
    ] {
        let response = query(
            &mut node,
            GetTypeDescription::Request {
                type_name: name.into(),
                type_hash: hash.to_string().into(),
                include_type_sources: true,
            },
        );
        assert!(response.successful, "{name}: {}", response.failure_reason);
        assert_eq!(response.type_description.type_description.type_name, name);
        assert_eq!(type_description_hash(&response.type_description).unwrap(), hash);
    }
    node.destroy_subscription(subscription);
}

#[test]
fn type_description_invalid_hash_and_missing_type_do_not_break_later_requests() {
    let mut node =
        Node::create(Context::create().unwrap(), "native_description_invalid", "").unwrap();
    node.start_type_description_service().unwrap();
    let _publisher = node
        .create_publisher::<UInt64>("values", QosProfile::default())
        .unwrap();
    let valid_hash = message_type_hash::<UInt64>().unwrap();
    for hash in [
        "invalid".to_owned(),
        crate::TopicTypeHash {
            version: 1,
            value: [0; 32],
        }
        .to_string(),
        valid_hash.to_string(),
    ] {
        let response = query(
            &mut node,
            GetTypeDescription::Request {
                type_name: "std_msgs/msg/UInt64".into(),
                type_hash: hash.clone().into(),
                include_type_sources: true,
            },
        );
        assert_eq!(response.successful, hash == valid_hash.to_string());
    }
}

#[test]
fn type_description_options_remapping_and_stop_restart_preserve_other_services() {
    let mut node = Node::create_with_options(
        Context::create().unwrap(),
        "native_description_remap",
        "/original",
        NodeOptions::default()
            .start_type_description_service(true)
            .arguments([
                "--ros-args",
                "-r",
                "~/get_type_description:=/remapped_description",
            ]),
    )
    .unwrap();
    assert_eq!(
        node.type_description_service_name().unwrap().as_deref(),
        Some("/remapped_description")
    );
    let ordinary = node
        .create_service::<crate::example_interfaces::srv::AddTwoInts::Service>(
            "sum",
            QosProfile::default(),
        )
        .unwrap();
    assert!(node.stop_type_description_service());
    assert_eq!(node.services.len(), 1);
    assert!(ordinary.service_name(&node).unwrap().ends_with("/sum"));
    assert!(node.start_type_description_service().unwrap());
    assert_eq!(node.services.len(), 2);
    let by_override = Node::create_with_options(
        node.context.clone(),
        "native_description_override",
        "",
        NodeOptions::default().parameter_override("start_type_description_service", true),
    )
    .unwrap();
    assert!(by_override
        .type_description_service_name()
        .unwrap()
        .is_some());
    let explicitly_disabled = Node::create_with_options(
        node.context.clone(),
        "native_description_disabled",
        "",
        NodeOptions::default()
            .parameter_override("start_type_description_service", true)
            .start_type_description_service(false),
    )
    .unwrap();
    assert!(explicitly_disabled
        .type_description_service_name()
        .unwrap()
        .is_none());
}

#[test]
fn type_description_client_validates_response_identity_and_timeout_owner_before_request() {
    let mut node =
        Node::create(Context::create().unwrap(), "native_description_client", "").unwrap();
    node.start_type_description_service().unwrap();
    let _publisher = node
        .create_publisher::<UInt64>("values", QosProfile::default())
        .unwrap();
    let client = node
        .create_type_description_client("/native_description_client")
        .unwrap();
    let ready = client.wait_for_service().unwrap();
    node.spin_until_future_complete_with_options(
        ready,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let hash = message_type_hash::<UInt64>().unwrap();
    let response = client.describe("std_msgs/msg/UInt64", &hash, true).unwrap();
    assert!(
        node.spin_until_future_complete_with_options(
            response,
            Duration::from_secs(3),
            FutureExecutionOptions::wake_driven()
        )
        .unwrap()
        .successful
    );
    let wrong_name = client.describe("std_msgs/msg/Int32", &hash, false).unwrap();
    assert!(matches!(
        node.spin_until_future_complete_with_options(
            wrong_name,
            Duration::from_secs(3),
            FutureExecutionOptions::wake_driven()
        ),
        Err(Error::TypeDescriptionIdentityMismatch { .. })
    ));
    let mut foreign = Node::create(node.context.clone(), "description_foreign", "").unwrap();
    assert!(matches!(
        client.describe_with_timeout(
            &mut foreign,
            "std_msgs/msg/UInt64",
            &hash,
            false,
            Duration::from_secs(1)
        ),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert!(client.describe("", &hash, false).is_err());
    let _missing = node.create_type_description_client("missing_peer").unwrap();
}

#[test]
fn type_description_client_rejects_a_forged_schema_with_the_expected_name() {
    let mut node = Node::create(Context::create().unwrap(), "description_hash_guard", "").unwrap();
    let mut forged = node
        .create_service::<GetTypeDescription::Service>(
            "/forged/get_type_description",
            QosProfile::services_default(),
        )
        .unwrap();
    let client = node.create_type_description_client("/forged").unwrap();
    let ready = client.wait_for_service().unwrap();
    let hash = message_type_hash::<UInt64>().unwrap();
    node.spin_until_future_complete_with_options(
        async {
            let server = async {
                let request = forged.next().await.unwrap();
                let mut response = GetTypeDescription::Response {
                    successful: true,
                    ..Default::default()
                };
                response.type_description.type_description.type_name = "std_msgs/msg/UInt64".into();
                // Deliberately omit the data field while keeping the requested type name.
                request.respond(response)
            };
            let application = async {
                ready.await?;
                let result = client.describe("std_msgs/msg/UInt64", &hash, false)?.await;
                assert!(matches!(result, Err(Error::TypeDescriptionIdentityMismatch { .. })));
                Ok::<_, Error>(())
            };
            futures::try_join!(server, application)?;
            Ok(())
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn type_description_native_batches_progress_on_a_shared_background_executor() {
    let context = Context::create().unwrap();
    let mut server = Node::create(context.clone(), "description_batch_server", "").unwrap();
    server
        .start_type_description_service_with_qos(QosProfile::services_default().keep_last(64))
        .unwrap();
    let _publisher = server
        .create_publisher::<UInt64>("values", QosProfile::default())
        .unwrap();
    let mut client_node = Node::create(context.clone(), "description_batch_client", "").unwrap();
    let client = client_node
        .create_type_description_client_with_qos(
            "/description_batch_server",
            QosProfile::services_default().keep_last(64),
        )
        .unwrap();
    let ready = client.wait_for_service().unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(server).unwrap();
    let id = executor.add_node(client_node).unwrap();
    let task = executor.spawn().unwrap();
    {
        tokio::time::timeout(Duration::from_secs(3), ready)
            .await
            .unwrap()
            .unwrap();
        let hash = message_type_hash::<UInt64>().unwrap();
        // More than the bounded sixteen-request native service dispatch pass.
        let replies = (0..40)
            .map(|index| {
                client
                    .describe("std_msgs/msg/UInt64", &hash, index % 2 == 0)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let results =
            tokio::time::timeout(Duration::from_secs(5), futures::future::try_join_all(replies))
                .await
                .unwrap()
                .unwrap();
        assert_eq!(results.len(), 40);
        for (index, reply) in results.iter().enumerate() {
            assert_eq!(!reply.type_sources.is_empty(), index % 2 == 0);
            assert_eq!(type_description_hash(&reply.type_description).unwrap(), hash);
        }
        let mut executor = task.stop().await.unwrap();
        executor.reset_cancel();
        let missing = executor
            .node_mut(id)
            .unwrap()
            .create_type_description_client("/missing_description")
            .unwrap();
        let wait = missing
            .wait_for_service_with_timeout(
                executor.node_mut(id).unwrap(),
                Duration::from_millis(20),
            )
            .unwrap();
        assert!(matches!(
            executor.spin_until_future_complete_with_options(
                wait,
                Duration::from_secs(1),
                FutureExecutionOptions::wake_driven()
            ),
            Err(Error::RCL_RET_TIMEOUT)
        ));
    }
}
