use super::{install_guard, node, wait_until_native_wait};
use crate::{Error, FutureExecutionOptions, Node, QosProfile, Result};
use std::{thread, time::Duration};

#[test]
fn publisher_matching_observes_graph_change_in_wake_driven_execution() {
    let mut node = node("wake_publisher_match_test");
    let topic = format!("/wake_pub_match_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
        .unwrap();
    let guard = install_guard(&mut node);
    let context = node.context.clone();
    let (stop, stopped) = std::sync::mpsc::channel();
    let peer = thread::spawn(move || {
        wait_until_native_wait(&guard);
        let mut peer = Node::create(context, "wake_publisher_match_peer", "").unwrap();
        let _subscription = peer
            .subscribe::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
            .unwrap();
        let _ = stopped.recv_timeout(Duration::from_secs(5));
    });
    let result = node.spin_until_future_complete_with_options(
        publisher.wait_for_subscribers().unwrap(),
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    );
    let _ = stop.send(());
    peer.join().unwrap();
    result.unwrap();
    assert!(node.publishers_with_active_waits.is_empty());
}

#[test]
fn subscription_matching_observes_graph_change_in_wake_driven_execution() {
    let mut node = node("wake_subscription_match_test");
    let topic = format!("/wake_sub_match_{}", uuid::Uuid::new_v4().simple());
    let subscription = node
        .subscribe::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
        .unwrap();
    let guard = install_guard(&mut node);
    let context = node.context.clone();
    let (stop, stopped) = std::sync::mpsc::channel();
    let peer = thread::spawn(move || {
        wait_until_native_wait(&guard);
        let mut peer = Node::create(context, "wake_subscription_match_peer", "").unwrap();
        let _publisher = peer
            .create_publisher::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
            .unwrap();
        let _ = stopped.recv_timeout(Duration::from_secs(5));
    });
    let available = subscription.wait_for_publishers(&mut node).unwrap();
    let result = node.spin_until_future_complete_with_options(
        available,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    );
    let _ = stop.send(());
    peer.join().unwrap();
    result.unwrap();
    assert!(node.subscriptions_with_active_waits.is_empty());
}

#[test]
fn untyped_publisher_matching_is_notification_driven() {
    let mut node = node("wake_untyped_match_test");
    let topic = format!("/wake_untyped_match_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher_untyped(&topic, "std_msgs/msg/String", QosProfile::default())
        .unwrap();
    let guard = install_guard(&mut node);
    let context = node.context.clone();
    let (stop, stopped) = std::sync::mpsc::channel();
    let peer = thread::spawn(move || {
        wait_until_native_wait(&guard);
        let mut peer = Node::create(context, "wake_untyped_match_peer", "").unwrap();
        let _subscription = peer
            .subscribe::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
            .unwrap();
        let _ = stopped.recv_timeout(Duration::from_secs(5));
    });
    let result = node.spin_until_future_complete_with_options(
        publisher.wait_for_subscribers().unwrap(),
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    );
    let _ = stop.send(());
    peer.join().unwrap();
    result.unwrap();
}

#[test]
fn dropped_matching_observers_are_removed_before_the_next_wait() {
    let mut node = node("wake_dropped_match_test");
    let topic = format!("/wake_dropped_match_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
        .unwrap();
    let subscription = node
        .subscribe::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
        .unwrap();
    let pub_wait = publisher.wait_for_subscribers().unwrap();
    let sub_wait = subscription.wait_for_publishers(&mut node).unwrap();
    drop(pub_wait);
    drop(sub_wait);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(node.publishers_with_active_waits.is_empty());
    assert!(node.subscriptions_with_active_waits.is_empty());
    node.destroy_publisher(publisher);
    node.destroy_subscription(subscription);
}

#[test]
fn reliable_acknowledgements_complete_in_wake_driven_execution() {
    use futures::StreamExt;
    let mut node = node("wake_ack_test");
    let topic = format!("/wake_ack_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<crate::std_msgs::msg::UInt64MultiArray>(&topic, QosProfile::default())
        .unwrap();
    let mut received = node
        .subscribe::<crate::std_msgs::msg::UInt64MultiArray>(&topic, QosProfile::default())
        .unwrap();
    node.spin_until_future_complete_with_options(
        publisher.wait_for_subscribers().unwrap(),
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let data: Vec<u64> = (0..8192).collect();
    publisher
        .publish(&crate::std_msgs::msg::UInt64MultiArray {
            data: data.clone(),
            ..Default::default()
        })
        .unwrap();
    let acked = publisher.wait_for_all_acked_async().unwrap();
    let result = node.spin_until_future_complete_with_options(
        async {
            let observed = received
                .next()
                .await
                .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
            assert_eq!(observed.data, data);
            acked.await
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    );
    assert!(matches!(result, Ok(()) | Err(Error::RCL_RET_UNSUPPORTED)), "{result:?}");
}

#[test]
fn matching_timeout_preserves_error_and_drops_registration() {
    let mut node = node("wake_match_timeout_test");
    let topic = format!("/wake_match_timeout_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
        .unwrap();
    let result: Result<()> = node.spin_until_future_complete_with_options(
        publisher.wait_for_subscribers().unwrap(),
        Duration::from_millis(20),
        FutureExecutionOptions::wake_driven(),
    );
    assert!(matches!(result, Err(Error::RCL_RET_TIMEOUT)));
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(node.publishers_with_active_waits.is_empty());
}

#[test]
fn non_notifying_ack_query_uses_bounded_wait_and_rechecks_completion() {
    use std::sync::atomic::Ordering;
    let mut node = node("wake_ack_query_test");
    let topic = format!("/wake_ack_query_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
        .unwrap();
    let state = publisher.handle.upgrade().unwrap();
    state
        .ack_query_override
        .store(r2r_rcl::RCL_RET_TIMEOUT as i32, Ordering::Release);
    let guard = install_guard(&mut node);
    let observer = guard.clone();
    let query = state.clone();
    let producer = thread::spawn(move || {
        wait_until_native_wait(&observer);
        let timeout = observer.wait_timeout_ns.load(Ordering::Acquire);
        assert!((0..=10_000_000).contains(&timeout), "ACK wait was not bounded: {timeout}");
        // Simulate middleware ACK state changing without a Rust/native event notification.
        query
            .ack_query_override
            .store(r2r_rcl::RCL_RET_OK as i32, Ordering::Release);
    });
    let acked = publisher.wait_for_all_acked_async().unwrap();
    let result = node.spin_until_future_complete_with_options(
        acked,
        Duration::from_millis(500),
        FutureExecutionOptions::wake_driven(),
    );
    producer.join().unwrap();
    result.unwrap();
    assert!(node.publishers_with_active_waits.is_empty());
}

#[test]
fn cancelled_ack_observer_removes_the_periodic_query_requirement() {
    use std::sync::atomic::Ordering;
    let mut node = node("wake_ack_cancel_test");
    let topic = format!("/wake_ack_cancel_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
        .unwrap();
    let state = publisher.handle.upgrade().unwrap();
    state
        .ack_query_override
        .store(r2r_rcl::RCL_RET_TIMEOUT as i32, Ordering::Release);
    drop(publisher.wait_for_all_acked_async().unwrap());
    let guard = install_guard(&mut node);
    let result = node.spin_until_future_complete_with_options(
        futures::future::pending::<Result<()>>(),
        Duration::from_millis(40),
        FutureExecutionOptions::wake_driven(),
    );
    assert!(matches!(result, Err(Error::RCL_RET_TIMEOUT)));
    assert!(guard.wait_timeout_ns.load(Ordering::Acquire) > 10_000_000);
    assert!(node.publishers_with_active_waits.is_empty());
}
