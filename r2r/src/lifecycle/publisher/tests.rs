use std::time::{Duration, Instant};

use futures::{FutureExt, StreamExt};

use super::*;
use crate::lifecycle::node::*;
use crate::lifecycle::*;
use crate::std_msgs;
use crate::{Context, QosProfile};

fn unique(name: &str) -> String {
    format!("{name}_{}", uuid::Uuid::new_v4().simple())
}

/// Drain both nodes for `window`, returning every message received.
fn drain(
    managed: &mut LifecycleNode, subscriber_node: &mut crate::Node,
    stream: &mut crate::Subscription<std_msgs::msg::String>, window: Duration,
) -> Vec<String> {
    let deadline = Instant::now() + window;
    let mut received = Vec::new();
    while Instant::now() < deadline {
        managed.try_spin_once(Duration::from_millis(2)).unwrap();
        subscriber_node
            .try_spin_once(Duration::from_millis(2))
            .unwrap();
        while let Some(msg) = stream.next().now_or_never().flatten() {
            received.push(msg.data.to_string());
        }
    }
    received
}

#[test]
fn managed_publisher_drops_messages_until_active_and_after_deactivate() {
    let context = Context::create().unwrap();
    let name = unique("lc_pub_gating");
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    let mut subscriber_node = crate::Node::create(context, "lc_pub_gating_sub", "").unwrap();
    let mut messages = subscriber_node
        .subscribe::<std_msgs::msg::String>(&format!("/{name}/gated"), QosProfile::default())
        .unwrap();
    let publisher = managed
        .create_publisher::<std_msgs::msg::String>(&format!("/{name}/gated"), QosProfile::default())
        .unwrap();

    // Wait for discovery so delivery decisions below are meaningful.
    let ready = publisher.wait_for_subscribers().unwrap();
    futures::pin_mut!(ready);
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        managed.try_spin_once(Duration::from_millis(2)).unwrap();
        subscriber_node
            .try_spin_once(Duration::from_millis(2))
            .unwrap();
        if let Some(result) = ready.as_mut().now_or_never() {
            result.unwrap();
            break;
        }
        assert!(Instant::now() < deadline, "managed publisher did not match");
    }

    let publish_both = |text: &str| {
        publisher
            .publish(&std_msgs::msg::String { data: text.into() })
            .unwrap();
        let native = crate::NativeMsg::try_from(&std_msgs::msg::String {
            data: format!("{text}-native").into(),
        })
        .unwrap();
        publisher
            .publish_native_ref(&crate::NativeMessageRef::from_owned(&native))
            .unwrap();
    };

    // Unconfigured: message dropped.
    assert!(!publisher.is_activated());
    publish_both("unconfigured");
    assert!(
        drain(&mut managed, &mut subscriber_node, &mut messages, Duration::from_millis(200))
            .is_empty()
    );

    // Configured/inactive: still dropped.
    assert_eq!(managed.configure().unwrap(), CallbackReturn::Success);
    assert!(!publisher.is_activated());
    publish_both("inactive");
    assert!(
        drain(&mut managed, &mut subscriber_node, &mut messages, Duration::from_millis(200))
            .is_empty()
    );

    // Active: delivered.
    assert_eq!(managed.activate().unwrap(), CallbackReturn::Success);
    assert!(publisher.is_activated());
    publish_both("active");
    let delivered =
        drain(&mut managed, &mut subscriber_node, &mut messages, Duration::from_millis(500));
    assert_eq!(delivered, ["active", "active-native"]);

    // Deactivated: dropped again.
    assert_eq!(managed.deactivate().unwrap(), CallbackReturn::Success);
    assert!(!publisher.is_activated());
    publish_both("inactive-again");
    assert!(
        drain(&mut managed, &mut subscriber_node, &mut messages, Duration::from_millis(200))
            .is_empty()
    );
}

#[test]
fn native_publication_handles_failure_recovery_and_active_creation() {
    let context = Context::create().unwrap();
    let mut managed = LifecycleNode::create(context, &unique("lc_native_outcomes"), "").unwrap();
    managed.configure().unwrap();
    managed.on_activate(Box::new(|_| CallbackReturn::failure("activation refused")));
    let publisher = managed
        .create_publisher::<std_msgs::msg::Int32>("outcomes", QosProfile::default())
        .unwrap();
    assert!(matches!(managed.activate().unwrap(), CallbackReturn::Failure(_)));
    assert!(!publisher.is_activated());
    // Inactive native publication leaves caller-owned storage available.
    let mut native = crate::NativeMsg::<std_msgs::msg::Int32>::new();
    publisher.publish_native(&mut native).unwrap();
    let view = crate::NativeMessageRef::from_owned(&native);
    publisher.publish_native_ref(&view).unwrap();
    managed.on_activate(Box::new(|_| CallbackReturn::Success));
    managed.activate().unwrap();
    let active_publisher = managed
        .create_publisher::<std_msgs::msg::Int32>("created_active", QosProfile::default())
        .unwrap();
    assert!(active_publisher.is_activated());
    active_publisher.publish_native_ref(&view).unwrap();
    active_publisher.publish_native(&mut native).unwrap();
    managed.on_deactivate(Box::new(|_| CallbackReturn::error("recover to unconfigured")));
    assert!(matches!(managed.deactivate().unwrap(), CallbackReturn::Error(_)));
    assert_eq!(managed.lifecycle_state(), LifecycleState::Unconfigured);
    assert!(!publisher.is_activated());
    assert!(!active_publisher.is_activated());
    managed.shutdown().unwrap();
    assert!(!publisher.is_activated());
    // An active stale handle still reports the native publisher error; gating
    // does not swallow failures from actual publications.
    drop(managed);
    publisher.on_activate();
    assert!(matches!(
        publisher.publish_native(&mut native),
        Err(crate::Error::RCL_RET_PUBLISHER_INVALID)
    ));
    let view = crate::NativeMessageRef::from_owned(&native);
    assert!(matches!(
        publisher.publish_native_ref(&view),
        Err(crate::Error::RCL_RET_PUBLISHER_INVALID)
    ));
    publisher.on_deactivate();
    publisher.publish_native_ref(&view).unwrap();
    publisher.publish_native(&mut native).unwrap();
}

#[test]
fn loaned_publication_checks_current_activation_and_returns_dropped_loans() {
    let context = Context::create().unwrap();
    let name = unique("lc_loan_gating");
    let topic = format!("/{name}/loan");
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    let mut observer = crate::Node::create(context, &unique("lc_loan_observer"), "").unwrap();
    let publisher = managed
        .create_publisher::<std_msgs::msg::Int32>(&topic, QosProfile::default())
        .unwrap();
    let mut messages = observer
        .subscribe::<std_msgs::msg::Int32>(&topic, QosProfile::default())
        .unwrap();
    assert!(matches!(
        publisher.borrow_loaned_message(&mut observer),
        Err(crate::Error::RCL_RET_INVALID_ARGUMENT)
    ));
    let supported = publisher.can_loan_messages().unwrap();
    if !supported {
        assert!(matches!(
            publisher.borrow_loaned_message(&mut managed),
            Err(crate::Error::RCL_RET_UNSUPPORTED)
        ));
        println!("middleware reports loaning unsupported; error/ownership boundary verified");
        return;
    }

    let ready = publisher.wait_for_subscribers().unwrap();
    futures::pin_mut!(ready);
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        if let Some(result) = ready.as_mut().now_or_never() {
            result.unwrap();
            break;
        }
        assert!(Instant::now() < deadline);
        managed.try_spin_once(Duration::from_millis(2)).unwrap();
        observer.try_spin_once(Duration::from_millis(2)).unwrap();
    }
    let drain = |managed: &mut LifecycleNode,
                 observer: &mut crate::Node,
                 messages: &mut crate::Subscription<std_msgs::msg::Int32>| {
        let deadline = Instant::now() + Duration::from_millis(100);
        let mut values = Vec::new();
        while Instant::now() < deadline {
            managed.try_spin_once(Duration::from_millis(2)).unwrap();
            observer.try_spin_once(Duration::from_millis(2)).unwrap();
            while let Some(msg) = messages.next().now_or_never().flatten() {
                values.push(msg.data);
            }
        }
        values
    };

    let mut loan = publisher.borrow_loaned_message(&mut managed).unwrap();
    loan.data = 1;
    loan.publish().unwrap();
    assert!(drain(&mut managed, &mut observer, &mut messages).is_empty());
    managed.configure().unwrap();
    managed.activate().unwrap();
    let clone = publisher.clone();
    let mut loan = publisher.borrow_loaned_message(&mut managed).unwrap();
    loan.data = 2;
    clone.on_deactivate();
    loan.publish().unwrap();
    assert!(drain(&mut managed, &mut observer, &mut messages).is_empty());

    // A loan acquired while inactive may be filled and published after manual
    // activation. Publication uses the shared flag, not a borrow-time snapshot.
    let mut loan = publisher.borrow_loaned_message(&mut managed).unwrap();
    loan.data = 3;
    clone.on_activate();
    loan.publish().unwrap();
    assert_eq!(drain(&mut managed, &mut observer, &mut messages), [3]);
    let mut unpublished = publisher.borrow_loaned_message(&mut managed).unwrap();
    unpublished.data = 4;
    drop(unpublished);
    let mut reused = publisher.borrow_loaned_message(&mut managed).unwrap();
    reused.data = 5;
    reused.publish().unwrap();
    assert_eq!(drain(&mut managed, &mut observer, &mut messages), [5]);
    managed.deactivate().unwrap();
    let mut loan = publisher.borrow_loaned_message(&mut managed).unwrap();
    loan.data = 6;
    loan.publish().unwrap();
    assert!(drain(&mut managed, &mut observer, &mut messages).is_empty());
    println!("middleware loan path verified: gated return, late activation and reuse");
}

#[test]
fn publisher_options_factory_returns_a_managed_endpoint() {
    let context = Context::create().unwrap();
    let mut managed = LifecycleNode::create(context, &unique("lc_pub_options"), "").unwrap();
    let publisher: LifecyclePublisher<std_msgs::msg::Int32> = managed
        .create_publisher_with_options(
            "configured_publisher",
            QosProfile::default(),
            crate::PublisherOptions::default()
                .unique_network_flow(crate::UniqueNetworkFlowRequirement::NotRequired),
        )
        .unwrap();
    assert!(!publisher.is_activated());
    assert!(publisher
        .topic_name()
        .unwrap()
        .ends_with("/configured_publisher"));
    managed.configure().unwrap();
    managed.activate().unwrap();
    assert!(publisher.is_activated());
    managed.deactivate().unwrap();
    assert!(!publisher.is_activated());
}

#[test]
fn managed_publisher_destruction_is_immediate_and_preserves_foreign_endpoints() {
    let context = Context::create().unwrap();
    let mut managed =
        LifecycleNode::create(context.clone(), &unique("lc_pub_destroy"), "").unwrap();
    let mut other = crate::Node::create(context, &unique("lc_pub_other"), "").unwrap();
    let publisher = managed
        .create_publisher::<std_msgs::msg::Int32>("explicit_destroy", QosProfile::default())
        .unwrap();
    let other_publisher = other
        .create_publisher::<std_msgs::msg::Int32>("other_endpoint", QosProfile::default())
        .unwrap();
    managed.configure().unwrap();
    managed.activate().unwrap();
    let mut native = crate::NativeMsg::<std_msgs::msg::Int32>::new();
    publisher.clone().destroy(&mut other);
    publisher.publish_native(&mut native).unwrap();
    other_publisher.publish_native(&mut native).unwrap();
    publisher.clone().destroy(&mut managed);
    assert!(matches!(
        publisher.publish_native(&mut native),
        Err(crate::Error::RCL_RET_PUBLISHER_INVALID)
    ));
    assert!(matches!(publisher.topic_name(), Err(crate::Error::RCL_RET_PUBLISHER_INVALID)));
    other_publisher.publish_native(&mut native).unwrap();
    managed.deactivate().unwrap();
    publisher.publish_native(&mut native).unwrap();
}

#[test]
fn managed_publisher_can_be_toggled_manually() {
    let entities = ManagedEntities::default();
    let publisher_arc: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    entities.register(Arc::downgrade(&publisher_arc));
    assert!(!publisher_arc.load(Ordering::SeqCst));
    entities.set_enabled(true);
    assert!(publisher_arc.load(Ordering::SeqCst));
    entities.set_enabled(false);
    assert!(!publisher_arc.load(Ordering::SeqCst));
}

#[test]
fn managed_native_publication_obeys_activation_and_preserves_storage() {
    let context = Context::create().unwrap();
    let name = unique("lc_native_gating");
    let topic = format!("/{name}/native");
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    let mut subscriber_node = crate::Node::create(context, &unique("lc_native_sub"), "").unwrap();
    let mut messages = subscriber_node
        .subscribe::<std_msgs::msg::String>(&topic, QosProfile::default())
        .unwrap();
    let publisher = managed
        .create_publisher::<std_msgs::msg::String>(&topic, QosProfile::default())
        .unwrap();
    let ready = publisher.wait_for_subscribers().unwrap();
    futures::pin_mut!(ready);
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        if let Some(result) = ready.as_mut().now_or_never() {
            result.unwrap();
            break;
        }
        assert!(Instant::now() < deadline, "native publisher did not match");
        managed.try_spin_once(Duration::from_millis(2)).unwrap();
        subscriber_node
            .try_spin_once(Duration::from_millis(2))
            .unwrap();
    }

    let mut native = crate::NativeMsg::<std_msgs::msg::String>::try_from(&std_msgs::msg::String {
        data: "native payload".into(),
    })
    .unwrap();
    let storage = (native.data.data, native.data.capacity);
    publisher.publish_native(&mut native).unwrap();
    assert!(
        drain(&mut managed, &mut subscriber_node, &mut messages, Duration::from_millis(100))
            .is_empty(),
        "unconfigured native publish escaped"
    );
    managed.configure().unwrap();
    publisher.publish_native(&mut native).unwrap();
    assert!(
        drain(&mut managed, &mut subscriber_node, &mut messages, Duration::from_millis(100))
            .is_empty(),
        "inactive native publish escaped"
    );
    managed.activate().unwrap();
    publisher.clone().publish_native(&mut native).unwrap();
    assert_eq!(
        drain(&mut managed, &mut subscriber_node, &mut messages, Duration::from_millis(200)),
        ["native payload"]
    );
    managed.deactivate().unwrap();
    publisher.publish_native(&mut native).unwrap();
    assert!(
        drain(&mut managed, &mut subscriber_node, &mut messages, Duration::from_millis(100))
            .is_empty(),
        "deactivated native publish escaped"
    );
    assert_eq!((native.data.data, native.data.capacity), storage);
    assert_eq!(
        <std_msgs::msg::String as crate::WrappedTypesupport>::from_native(&native).data,
        "native payload"
    );
}

#[test]
fn dropped_managed_publishers_are_pruned_on_transitions() {
    let context = Context::create().unwrap();
    let mut managed = LifecycleNode::create(context, &unique("lc_pub_prune"), "").unwrap();
    {
        let publisher = managed
            .create_publisher::<std_msgs::msg::String>("pruned", QosProfile::default())
            .unwrap();
        publisher.on_activate();
        assert!(publisher.is_activated());
        // publisher dropped here
    }
    // Repeated reconciliations must not see stale entities and transitions
    // must keep succeeding.
    assert_eq!(managed.configure().unwrap(), CallbackReturn::Success);
    assert_eq!(managed.activate().unwrap(), CallbackReturn::Success);
    assert_eq!(managed.deactivate().unwrap(), CallbackReturn::Success);
    assert_eq!(managed.cleanup().unwrap(), CallbackReturn::Success);
    assert_eq!(managed.shutdown().unwrap(), CallbackReturn::Success);
    assert_eq!(managed.lifecycle_state(), LifecycleState::Finalized);
}

#[test]
fn service_driven_activation_gates_publisher() {
    use crate::lifecycle_msgs::msg::Transition;
    use crate::lifecycle_msgs::srv::ChangeState;

    let context = Context::create().unwrap();
    let name = unique("lc_pub_service");
    let mut managed = LifecycleNode::create(context.clone(), &name, "").unwrap();
    let publisher = managed
        .create_publisher::<std_msgs::msg::String>(
            &format!("/{name}/gated2"),
            QosProfile::default(),
        )
        .unwrap();
    let mut client_node = crate::Node::create(context, "lc_pub_service_client", "").unwrap();
    let client = client_node
        .create_client::<ChangeState::Service>(
            &format!("/{name}/change_state"),
            QosProfile::default(),
        )
        .unwrap();

    let request = |id: u8| ChangeState::Request {
        transition: Transition {
            id,
            label: Default::default(),
        },
    };
    for step in [
        (Transition::TRANSITION_CONFIGURE as u8, false),
        (Transition::TRANSITION_ACTIVATE as u8, true),
        (Transition::TRANSITION_DEACTIVATE as u8, false),
    ] {
        let (id, expected) = step;
        let future = client.request(&request(id)).unwrap();
        futures::pin_mut!(future);
        let deadline = Instant::now() + Duration::from_secs(5);
        let response = loop {
            if let Some(result) = future.as_mut().now_or_never() {
                break result.unwrap();
            }
            assert!(Instant::now() < deadline);
            managed.try_spin_once(Duration::ZERO).unwrap();
            client_node.try_spin_once(Duration::ZERO).unwrap();
            std::thread::yield_now();
        };
        assert!(response.success, "transition {id} should succeed");
        assert_eq!(publisher.is_activated(), expected);
    }
}
