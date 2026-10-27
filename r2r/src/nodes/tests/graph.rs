use super::*;

fn wait_for_graph_condition(node: &mut Node, mut present: impl FnMut(&Node) -> bool) {
    let started = Instant::now();
    loop {
        // Register before querying: native graph caches update asynchronously, including
        // local endpoints, and a change between the query and the wait must be retained.
        let changed = node.wait_for_graph_change();
        if present(node) {
            drop(changed);
            return;
        }
        let remaining = Duration::from_secs(2).saturating_sub(started.elapsed());
        assert!(!remaining.is_zero(), "created endpoints are missing from the graph cache");
        node.spin_until_future_complete_with_options(
            changed,
            remaining,
            crate::FutureExecutionOptions::wake_driven(),
        )
        .expect("created endpoints did not become visible in the graph cache");
    }
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

    wait_for_graph_condition(&mut node, |node| {
        node.get_topic_names_and_types()
            .unwrap()
            .contains_key(&topic)
            && node
                .get_raw_topic_names_and_types()
                .unwrap()
                .contains_key(&format!("rt{}", topic))
            && !node
                .get_publishers_info_by_topic(&topic, false)
                .unwrap()
                .is_empty()
            && !node
                .get_subscriptions_info_by_topic(&topic, false)
                .unwrap()
                .is_empty()
    });
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
    #[cfg(not(r2r__ros__distro__humble))]
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

    wait_for_graph_condition(&mut node, |node| {
        node.get_service_names_and_types()
            .unwrap()
            .contains_key(&service_name)
    });
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
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
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

    wait_for_graph_condition(&mut node, |node| {
        let present = node.count_publishers(&topic).unwrap() >= 1
            && node.count_subscribers(&topic).unwrap() >= 1;
        #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
        let present = present
            && node.count_clients(&service).unwrap() >= 1
            && node.count_services(&service).unwrap() >= 1;
        present
    });
    assert!(node.count_publishers(&topic).unwrap() >= 1);
    assert!(node.count_subscribers(&topic).unwrap() >= 1);
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    {
        assert!(node.count_clients(&service).unwrap() >= 1);
        assert!(node.count_services(&service).unwrap() >= 1);
    }

    assert!(matches!(
        node.count_publishers("bad\0topic"),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
}
