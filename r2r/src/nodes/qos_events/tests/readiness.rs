use super::*;

#[test]
fn qos_events_native_incompatible_statuses_wake_without_periodic_polling() {
    let mut node = node("qos_native_incompatible");
    let topic = topic("qos_native_incompatible");
    // Register publisher event before creating the incompatible endpoint so native readiness
    // owns the entire notification, independent of late observer behavior in an RMW.
    let publisher = node
        .create_publisher::<UInt64>(&topic, QosProfile::default().best_effort())
        .unwrap();
    let mut offered = publisher
        .qos_events(&mut node, PublisherEventKind::IncompatibleQos)
        .unwrap();
    let subscription = node
        .subscribe::<UInt64>(&topic, QosProfile::default().reliable())
        .unwrap();
    let mut requested = subscription
        .qos_events(&mut node, SubscriptionEventKind::IncompatibleQos)
        .unwrap();
    let offered = next_event(&mut node, &mut offered);
    let requested = next_event(&mut node, &mut requested);
    assert!(
        matches!(offered, QosEvent::OfferedIncompatibleQos(status) if status.total_count >= 1 && status.total_count_change >= 1 && status.last_policy_kind == crate::QosPolicyKind::Reliability)
    );
    assert!(
        matches!(requested, QosEvent::RequestedIncompatibleQos(status) if status.total_count >= 1 && status.total_count_change >= 1 && status.last_policy_kind == crate::QosPolicyKind::Reliability)
    );
    assert_eq!(publisher.get_subscription_count().unwrap(), 0);
    assert!(node.wait_set.capacities.unwrap().events >= 1);
    assert!(node.future_wake_guard.is_some());
    assert!(
        node.future_wake_guard
            .as_ref()
            .unwrap()
            .wait_timeout_ns
            .load(std::sync::atomic::Ordering::Acquire)
            > 1_000_000_000
    );
}

#[test]
fn qos_events_idle_full_slot_excludes_native_event_and_does_not_spin() {
    let mut node = node("qos_event_backpressure");
    let topic = topic("qos_backpressure");
    let publisher = node
        .create_publisher::<UInt64>(&topic, QosProfile::default().best_effort())
        .unwrap();
    let mut events = publisher
        .qos_events(&mut node, PublisherEventKind::IncompatibleQos)
        .unwrap();
    let subscription = node
        .subscribe::<UInt64>(&topic, QosProfile::default().reliable())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while node.qos_events.as_ref().unwrap().registrations[0]
        .channel
        .can_take()
    {
        assert!(Instant::now() < deadline);
        node.try_spin_once(Duration::from_millis(20)).unwrap();
    }
    // More incompatible endpoints update native counters while the single Rust slot is full.
    let sibling = node
        .subscribe::<UInt64>(&topic, QosProfile::default().reliable())
        .unwrap();
    let started = Instant::now();
    // RMW discovery can leave one transient readiness notification unrelated to the full
    // event. Repeated waits must settle instead of observing the same status indefinitely.
    for _ in 0..4 {
        node.try_spin_once(Duration::from_millis(80)).unwrap();
    }
    assert_eq!(node.wait_set.capacities.unwrap().events, 0);
    assert!(
        started.elapsed() >= Duration::from_millis(60),
        "full event created an idle spin loop"
    );
    assert!(matches!(
        events.next().now_or_never(),
        Some(Some(Ok(QosEvent::OfferedIncompatibleQos(_))))
    ));
    let status = next_event(&mut node, &mut events);
    assert!(
        matches!(status, QosEvent::OfferedIncompatibleQos(value) if value.total_count >= 2 && value.total_count_change >= 1)
    );
    drop(subscription);
    drop(sibling);
}

#[test]
fn qos_events_deadline_status_waits_on_native_timer_and_shared_message_delivery() {
    let mut node = node("qos_native_deadline");
    let topic = topic("qos_deadline");
    let qos = QosProfile::default().deadline(Duration::from_millis(40));
    let publisher = node
        .create_publisher::<UInt64>(&topic, qos.clone())
        .unwrap();
    let mut offered = publisher
        .qos_events(&mut node, PublisherEventKind::DeadlineMissed)
        .unwrap();
    let mut subscription = node.subscribe::<UInt64>(&topic, qos).unwrap();
    let mut requested = subscription
        .qos_events(&mut node, SubscriptionEventKind::DeadlineMissed)
        .unwrap();
    let ready = publisher.wait_for_subscribers().unwrap();
    node.spin_until_future_complete_with_options(
        ready,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    publisher.publish(&UInt64 { data: 73 }).unwrap();
    node.spin_until_future_complete_with_options(async {
        assert_eq!(subscription.next().await.unwrap().data, 73);
        let (a, b) = future::join(offered.next(), requested.next()).await;
        assert!(matches!(a, Some(Ok(QosEvent::OfferedDeadlineMissed(value))) if value.total_count >= 1 && value.total_count_change >= 1));
        assert!(matches!(b, Some(Ok(QosEvent::RequestedDeadlineMissed(value))) if value.total_count >= 1 && value.total_count_change >= 1));
        Ok(())
    }, Duration::from_secs(3), FutureExecutionOptions::wake_driven()).unwrap();
}

#[test]
fn qos_events_untyped_and_native_callback_endpoints_use_the_same_wait_path() {
    let mut node = node("qos_runtime_endpoints");
    let topic = topic("qos_runtime");
    let publisher = node
        .create_publisher_untyped(
            &topic,
            "std_msgs/msg/UInt64",
            QosProfile::default().best_effort(),
        )
        .unwrap();
    let mut offered = publisher
        .qos_events(&mut node, PublisherEventKind::IncompatibleQos)
        .unwrap();
    let callback = node
        .subscribe_native_callback::<UInt64, _>(&topic, QosProfile::default().reliable(), |_, _| {
            panic!("incompatible endpoints delivered data")
        })
        .unwrap();
    let mut requested = callback
        .qos_events(&mut node, SubscriptionEventKind::IncompatibleQos)
        .unwrap();
    assert!(matches!(
        next_event(&mut node, &mut offered),
        QosEvent::OfferedIncompatibleQos(_)
    ));
    assert!(matches!(
        next_event(&mut node, &mut requested),
        QosEvent::RequestedIncompatibleQos(_)
    ));
    node.destroy_native_callback_subscription(callback);
    assert!(matches!(
        requested.next().now_or_never(),
        Some(Some(Err(Error::RCL_RET_SUBSCRIPTION_INVALID)))
    ));
    node.destroy_publisher_untyped(publisher);
    assert!(matches!(
        offered.next().now_or_never(),
        Some(Some(Err(Error::RCL_RET_PUBLISHER_INVALID)))
    ));
}

#[test]
fn qos_events_stable_layout_reuses_native_wait_storage_and_rebuilds_at_changes() {
    let mut node = node("qos_stable_layout");
    let publisher = node
        .create_publisher::<UInt64>(&topic("qos_stable"), QosProfile::default())
        .unwrap();
    let events = publisher
        .qos_events(&mut node, PublisherEventKind::IncompatibleQos)
        .unwrap();
    for _ in 0..3 {
        node.try_spin_once(Duration::ZERO).unwrap();
    }
    assert_eq!(node.wait_set.event_layout_rebuilds, 0);
    let _timer = node.create_wall_timer(Duration::from_secs(60)).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(node.wait_set.event_layout_rebuilds, 1);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(node.wait_set.event_layout_rebuilds, 1);
    drop(events);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(node.wait_set.event_layout_rebuilds, 2);
}

#[cfg(not(r2r__ros__distro__humble))]
#[test]
fn qos_events_matching_reports_native_connect_and_disconnect_counts() {
    let mut node = node("qos_native_matching");
    let name = topic("qos_native_matching");
    let publisher = node
        .create_publisher::<UInt64>(&name, QosProfile::default())
        .unwrap();
    let mut events = publisher
        .qos_events(&mut node, PublisherEventKind::Matched)
        .unwrap();
    let subscription = node
        .subscribe::<UInt64>(&name, QosProfile::default())
        .unwrap();
    assert!(
        matches!(next_event(&mut node, &mut events), QosEvent::Matched(value) if value.total_count == 1 && value.current_count == 1 && value.current_count_change == 1)
    );
    node.destroy_subscription(subscription);
    assert!(
        matches!(next_event(&mut node, &mut events), QosEvent::Matched(value) if value.total_count == 1 && value.current_count == 0 && value.current_count_change == -1)
    );
}

#[test]
fn qos_events_liveliness_reports_native_lease_expiry() {
    let mut node = node("qos_native_liveliness");
    let name = topic("qos_native_liveliness");
    let qos = QosProfile::default()
        .liveliness(crate::qos::LivelinessPolicy::ManualByTopic)
        .liveliness_lease_duration(Duration::from_millis(80));
    let publisher = node.create_publisher::<UInt64>(&name, qos.clone()).unwrap();
    let mut lost = publisher
        .qos_events(&mut node, PublisherEventKind::LivelinessLost)
        .unwrap();
    let subscription = node.subscribe::<UInt64>(&name, qos).unwrap();
    let mut changed = subscription
        .qos_events(&mut node, SubscriptionEventKind::LivelinessChanged)
        .unwrap();
    let ready = publisher.wait_for_subscribers().unwrap();
    node.spin_until_future_complete_with_options(
        ready,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    publisher.assert_liveliness().unwrap();
    assert!(
        matches!(next_event(&mut node, &mut lost), QosEvent::LivelinessLost(value) if value.total_count >= 1 && value.total_count_change >= 1)
    );
    node.spin_until_future_complete_with_options(
        async {
            loop {
                let event = changed
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)??;
                if let QosEvent::LivelinessChanged(value) = event {
                    if value.not_alive_count >= 1 && value.not_alive_count_change >= 1 {
                        break;
                    }
                }
            }
            Ok(())
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
}
