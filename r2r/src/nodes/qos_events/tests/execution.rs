use super::*;

#[test]
fn qos_events_shared_executor_offsets_and_node_removal_keep_native_ownership() {
    let context = Context::create().unwrap();
    let mut a = Node::create(context.clone(), "qos_shared_a", "").unwrap();
    let mut b = Node::create(context.clone(), "qos_shared_b", "").unwrap();
    let (pa, _sa) = incompatible_pair(&mut a, &topic("qos_shared_a"));
    let (pb, _sb) = incompatible_pair(&mut b, &topic("qos_shared_b"));
    let mut ea = pa
        .qos_events(&mut a, PublisherEventKind::IncompatibleQos)
        .unwrap();
    let mut eb = pb
        .qos_events(&mut b, PublisherEventKind::IncompatibleQos)
        .unwrap();
    let mut executor = Executor::new(context);
    let id = executor.add_node(a).unwrap();
    executor.add_node(b).unwrap();
    executor
        .spin_until_future_complete_with_options(
            async {
                let (a, b) = future::join(ea.next(), eb.next()).await;
                assert!(matches!(a, Some(Ok(QosEvent::OfferedIncompatibleQos(_)))));
                assert!(matches!(b, Some(Ok(QosEvent::OfferedIncompatibleQos(_)))));
                Ok(())
            },
            Duration::from_secs(3),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    let mut returned = executor.remove_node(id).unwrap();
    drop(ea);
    returned.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(returned.qos_events.as_ref().unwrap().registrations.len(), 0);
    let _new = pa
        .qos_events(&mut returned, PublisherEventKind::IncompatibleQos)
        .unwrap();
}

#[test]
fn qos_events_same_count_membership_changes_preserve_ordinary_and_action_offsets() {
    use crate::example_interfaces::action::Fibonacci;
    let mut node = node("qos_layout_actions");
    let name = topic("qos_layout_actions");
    let publisher = node
        .create_publisher::<UInt64>(&name, QosProfile::default().best_effort())
        .unwrap();
    let mut events = publisher
        .qos_events(&mut node, PublisherEventKind::IncompatibleQos)
        .unwrap();
    let stale = node
        .subscribe::<UInt64>(&name, QosProfile::default().best_effort())
        .unwrap();
    let action = format!("{name}/action");
    let client = node
        .create_action_client::<Fibonacci::Action>(&action)
        .unwrap();
    let mut requests = node
        .create_action_server::<Fibonacci::Action>(&action)
        .unwrap();
    // Populate and use the first layout, then swap one ordinary subscription at the same count.
    node.try_spin_once(Duration::ZERO).unwrap();
    node.destroy_subscription(stale);
    let _replacement = node
        .subscribe::<UInt64>(&name, QosProfile::default().reliable())
        .unwrap();
    let ready = client.wait_for_action_server().unwrap();
    node.spin_until_future_complete_with_options(
        async {
            let server = async {
                let request = requests
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
                assert_eq!(request.goal.order, 3);
                let (mut goal, _cancel) = request.accept()?;
                goal.succeed(Fibonacci::Result {
                    sequence: vec![0, 1, 1],
                })?;
                Ok::<_, Error>(())
            };
            let application = async {
                ready.await?;
                let (_, result, _feedback) = client
                    .send_goal_request(Fibonacci::Goal { order: 3 })?
                    .await?;
                let (status, result) = result.await?;
                assert_eq!(status, crate::GoalStatus::Succeeded);
                assert_eq!(result.sequence, [0, 1, 1]);
                assert!(matches!(
                    events.next().await,
                    Some(Ok(QosEvent::OfferedIncompatibleQos(_)))
                ));
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
async fn qos_events_consumption_and_drop_wake_the_current_background_executor() {
    use std::sync::atomic::Ordering;
    let mut node = node("qos_background_rearm");
    let topic = topic("qos_background_rearm");
    let publisher = node
        .create_publisher::<UInt64>(&topic, QosProfile::default().best_effort())
        .unwrap();
    let mut events = publisher
        .qos_events(&mut node, PublisherEventKind::IncompatibleQos)
        .unwrap();
    let channel = node.qos_events.as_ref().unwrap().registrations[0]
        .channel
        .clone();
    let _subscription = node
        .subscribe::<UInt64>(&topic, QosProfile::default().reliable())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while channel.can_take() {
        assert!(Instant::now() < deadline);
        node.try_spin_once(Duration::from_millis(20)).unwrap();
    }
    let old_guard = node.future_wake_guard.clone().unwrap();
    let _sibling = node
        .subscribe::<UInt64>(&topic, QosProfile::default().reliable())
        .unwrap();
    let mut executor = Executor::new(node.context.clone());
    let id = executor.add_node(node).unwrap();
    executor.spin_once(Duration::ZERO).unwrap();
    let guard = future_executor::FutureDriver::cached_future_guard(&executor).unwrap();
    assert!(!Arc::ptr_eq(&guard, &old_guard));
    let old_triggers = old_guard.triggers.load(Ordering::Acquire);
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !guard.waiting.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(events.next().await, Some(Ok(QosEvent::OfferedIncompatibleQos(_)))));
    let next = tokio::time::timeout(Duration::from_secs(3), events.next())
        .await
        .unwrap();
    assert!(
        matches!(next, Some(Ok(QosEvent::OfferedIncompatibleQos(value))) if value.total_count >= 2)
    );
    assert_eq!(old_guard.triggers.load(Ordering::Acquire), old_triggers);
    assert!(guard.triggers.load(Ordering::Acquire) >= 1);
    drop(events);
    let mut recovered = task.stop().await.unwrap();
    recovered.reset_cancel();
    recovered.spin_once(Duration::ZERO).unwrap();
    let node = recovered.node_mut(id).unwrap();
    assert_eq!(node.qos_events.as_ref().unwrap().registrations.len(), 0);
    let _new = publisher
        .qos_events(node, PublisherEventKind::IncompatibleQos)
        .unwrap();
}
