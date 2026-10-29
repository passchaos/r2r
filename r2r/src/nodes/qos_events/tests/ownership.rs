use super::*;

#[test]
fn qos_events_duplicate_observers_manual_takes_and_foreign_nodes_are_rejected() {
    let mut node = node("qos_observer_ownership");
    let mut other = Node::create(node.context.clone(), "qos_foreign", "").unwrap();
    let (publisher, subscription) = incompatible_pair(&mut node, &topic("qos_ownership"));
    assert!(matches!(
        publisher.qos_events(&mut other, PublisherEventKind::IncompatibleQos),
        Err(Error::RCL_RET_PUBLISHER_INVALID)
    ));
    assert!(matches!(
        subscription.qos_events(&mut other, SubscriptionEventKind::IncompatibleQos),
        Err(Error::RCL_RET_SUBSCRIPTION_INVALID)
    ));
    let offered = publisher
        .qos_events(&mut node, PublisherEventKind::IncompatibleQos)
        .unwrap();
    let requested = subscription
        .qos_events(&mut node, SubscriptionEventKind::IncompatibleQos)
        .unwrap();
    assert!(matches!(
        publisher.qos_events(&mut node, PublisherEventKind::IncompatibleQos),
        Err(Error::QosEventAlreadyObserved)
    ));
    assert!(matches!(
        publisher.take_offered_incompatible_qos(),
        Err(Error::QosEventAlreadyObserved)
    ));
    assert!(matches!(
        subscription.qos_events(&mut node, SubscriptionEventKind::IncompatibleQos),
        Err(Error::QosEventAlreadyObserved)
    ));
    assert!(matches!(
        subscription.take_requested_incompatible_qos(&mut node),
        Err(Error::QosEventAlreadyObserved)
    ));
    drop(offered);
    drop(requested);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(node.qos_events.as_ref().unwrap().registrations.len(), 0);
    assert!(publisher.take_offered_incompatible_qos().is_ok());
    assert!(subscription
        .take_requested_incompatible_qos(&mut node)
        .is_ok());
    let _offered = publisher
        .qos_events(&mut node, PublisherEventKind::IncompatibleQos)
        .unwrap();
    let _requested = subscription
        .qos_events(&mut node, SubscriptionEventKind::IncompatibleQos)
        .unwrap();
}

#[test]
fn qos_events_endpoint_destruction_closes_streams_and_preserves_other_registrations() {
    let mut node = node("qos_endpoint_cleanup");
    let (publisher, subscription) = incompatible_pair(&mut node, &topic("qos_cleanup"));
    let mut offered = publisher
        .qos_events(&mut node, PublisherEventKind::IncompatibleQos)
        .unwrap();
    let mut requested = subscription
        .qos_events(&mut node, SubscriptionEventKind::IncompatibleQos)
        .unwrap();
    let clone = publisher.clone();
    node.destroy_publisher(publisher);
    assert!(matches!(
        offered.next().now_or_never(),
        Some(Some(Err(Error::RCL_RET_PUBLISHER_INVALID)))
    ));
    assert!(matches!(offered.next().now_or_never(), Some(None)));
    assert!(matches!(
        clone.qos_events(&mut node, PublisherEventKind::IncompatibleQos),
        Err(Error::RCL_RET_PUBLISHER_INVALID)
    ));
    assert_eq!(node.qos_events.as_ref().unwrap().registrations.len(), 1);
    node.destroy_subscription(subscription);
    assert!(matches!(
        requested.next().now_or_never(),
        Some(Some(Err(Error::RCL_RET_SUBSCRIPTION_INVALID)))
    ));
    assert!(matches!(requested.next().now_or_never(), Some(None)));
}

#[test]
fn qos_events_no_observers_leave_native_event_and_registry_storage_unallocated() {
    let mut node = node("qos_no_observers");
    let _subscription = node
        .subscribe::<UInt64>(&topic("qos_none"), QosProfile::default())
        .unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(node.qos_events.is_none());
    assert_eq!(node.wait_set.capacities.unwrap().events, 0);
    assert!(node.future_wake_guard.is_none());
}

#[test]
fn qos_events_subscription_only_node_drop_reports_terminal_error() {
    let mut node = node("qos_node_drop");
    let subscription = node
        .subscribe::<UInt64>(&topic("qos_drop"), QosProfile::default())
        .unwrap();
    let mut events = subscription
        .qos_events(&mut node, SubscriptionEventKind::IncompatibleQos)
        .unwrap();
    assert!(events.next().now_or_never().is_none());
    drop(node);
    assert!(matches!(
        events.next().now_or_never(),
        Some(Some(Err(Error::RCL_RET_SUBSCRIPTION_INVALID)))
    ));
    assert!(matches!(events.next().now_or_never(), Some(None)));
}

#[test]
fn qos_events_node_drop_wakes_outside_the_native_context_lock() {
    struct ContextReentry {
        context: Context,
    }
    impl std::task::Wake for ContextReentry {
        fn wake(self: Arc<Self>) {
            assert!(
                self.context.context_handle.try_lock().is_some(),
                "QoS terminal wake held native context lock"
            );
        }
    }
    let mut node = node("qos_context_reentry");
    let subscription = node
        .subscribe::<UInt64>(&topic("qos_context_reentry"), QosProfile::default())
        .unwrap();
    let mut events = subscription
        .qos_events(&mut node, SubscriptionEventKind::IncompatibleQos)
        .unwrap();
    let waker = std::task::Waker::from(Arc::new(ContextReentry {
        context: node.context.clone(),
    }));
    let mut cx = std::task::Context::from_waker(&waker);
    assert!(Pin::new(&mut events).poll_next(&mut cx).is_pending());
    drop(node);
    assert!(matches!(
        events.next().now_or_never(),
        Some(Some(Err(Error::RCL_RET_SUBSCRIPTION_INVALID)))
    ));
}

#[test]
fn qos_events_each_supported_kind_registers_and_unsupported_kinds_roll_back() {
    let mut node = node("qos_kind_matrix");
    let publisher = node
        .create_publisher::<UInt64>(&topic("qos_kind_pub"), QosProfile::default())
        .unwrap();
    let subscription = node
        .subscribe::<UInt64>(&topic("qos_kind_sub"), QosProfile::default())
        .unwrap();
    let publisher_kinds = [
        PublisherEventKind::IncompatibleQos,
        PublisherEventKind::DeadlineMissed,
        PublisherEventKind::LivelinessLost,
        #[cfg(not(r2r__ros__distro__humble))]
        PublisherEventKind::Matched,
        #[cfg(not(r2r__ros__distro__humble))]
        PublisherEventKind::IncompatibleType,
    ];
    let subscription_kinds = [
        SubscriptionEventKind::IncompatibleQos,
        SubscriptionEventKind::DeadlineMissed,
        SubscriptionEventKind::LivelinessChanged,
        SubscriptionEventKind::MessageLost,
        #[cfg(not(r2r__ros__distro__humble))]
        SubscriptionEventKind::Matched,
        #[cfg(not(r2r__ros__distro__humble))]
        SubscriptionEventKind::IncompatibleType,
    ];
    for kind in publisher_kinds {
        match publisher.qos_events(&mut node, kind) {
            Ok(stream) => node.destroy_qos_event_stream(stream),
            Err(Error::RCL_RET_UNSUPPORTED) => {}
            Err(error) => panic!("publisher event {kind:?} failed: {error:?}"),
        }
        assert!(node
            .qos_events
            .as_ref()
            .is_none_or(|events| events.registrations.is_empty()));
    }
    for kind in subscription_kinds {
        match subscription.qos_events(&mut node, kind) {
            Ok(stream) => node.destroy_qos_event_stream(stream),
            Err(Error::RCL_RET_UNSUPPORTED) => {}
            Err(error) => panic!("subscription event {kind:?} failed: {error:?}"),
        }
        assert!(node
            .qos_events
            .as_ref()
            .is_none_or(|events| events.registrations.is_empty()));
    }
}

#[test]
fn qos_events_lifecycle_publisher_status_remains_observable_while_inactive() {
    let context = Context::create().unwrap();
    let mut managed = crate::LifecycleNode::create(context, "qos_managed_status", "").unwrap();
    let name = topic("qos_managed_status");
    let publisher = managed
        .create_publisher::<UInt64>(&name, QosProfile::default().best_effort())
        .unwrap();
    assert!(!publisher.is_activated());
    let mut events = publisher
        .qos_events(&mut managed, PublisherEventKind::IncompatibleQos)
        .unwrap();
    let _subscription = managed
        .subscribe::<UInt64>(&name, QosProfile::default().reliable())
        .unwrap();
    assert!(
        matches!(next_event(&mut managed, &mut events), QosEvent::OfferedIncompatibleQos(value) if value.total_count >= 1)
    );
    assert!(!publisher.is_activated());
}

#[test]
fn qos_events_last_public_endpoint_drop_closes_stream_without_retaining_native_owner() {
    let mut node = node("qos_automatic_cleanup");
    let name = topic("qos_automatic_cleanup");
    let (publisher, subscription) = incompatible_pair(&mut node, &name);
    let native = publisher.handle.clone();
    let mut offered = publisher
        .qos_events(&mut node, PublisherEventKind::IncompatibleQos)
        .unwrap();
    let mut requested = subscription
        .qos_events(&mut node, SubscriptionEventKind::IncompatibleQos)
        .unwrap();
    assert!(offered.next().now_or_never().is_none());
    assert!(requested.next().now_or_never().is_none());
    drop(publisher);
    drop(subscription);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(native.upgrade().is_none(), "event ownership retained a dropped publisher");
    assert_eq!(node.qos_events.as_ref().unwrap().registrations.len(), 0);
    assert!(matches!(
        offered.next().now_or_never(),
        Some(Some(Err(Error::RCL_RET_PUBLISHER_INVALID)))
    ));
    assert!(matches!(
        requested.next().now_or_never(),
        Some(Some(Err(Error::RCL_RET_SUBSCRIPTION_INVALID)))
    ));
}
