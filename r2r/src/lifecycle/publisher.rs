//! Lifecycle-gated publisher.
//!
//! A [`LifecyclePublisher`] wraps an ordinary [`Publisher`] and adds the
//! `rclcpp_lifecycle::LifecyclePublisher` contract: messages are forwarded to
//! DDS only while the publisher is *activated*. Publishers created through
//! [`crate::LifecycleNode::create_publisher`] are registered as managed
//! entities: a successful transition into the `active` primary state activates
//! them automatically, and leaving `active` deactivates them.

use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use parking_lot::Mutex;

use crate::error::Result;
use crate::publishers::Publisher;
use crate::WrappedTypesupport;

/// Shared activation flag held by the publisher and weakly referenced from
/// the lifecycle runtime. Dropping every clone of the publisher removes the
/// entity automatically on the next transition.
pub(crate) type Activation = Arc<AtomicBool>;
pub(crate) type WeakActivation = Weak<AtomicBool>;

/// Registry of managed entities inside the lifecycle runtime.
#[derive(Default)]
pub(crate) struct ManagedEntities {
    entities: Mutex<Vec<WeakActivation>>,
}

impl ManagedEntities {
    pub fn register(&self, entity: WeakActivation) {
        self.entities.lock().push(entity);
    }

    /// Activate or deactivate every still-alive managed entity. Dead weak
    /// references are pruned while iterating.
    pub fn set_enabled(&self, enabled: bool) {
        let mut entities = self.entities.lock();
        entities.retain(|weak| match weak.upgrade() {
            Some(activation) => {
                activation.store(enabled, Ordering::SeqCst);
                true
            }
            None => false,
        });
    }
}

/// A ROS publisher that only publishes while its managed node is `active`.
///
/// All ordinary [`Publisher`] methods are available through `Deref`. When the
/// publisher is deactivated, [`LifecyclePublisher::publish`] drops the message
/// silently (with a debug log), matching `rclcpp_lifecycle` semantics.
pub struct LifecyclePublisher<T>
where
    T: WrappedTypesupport,
{
    publisher: Publisher<T>,
    enabled: Activation,
}

impl<T: WrappedTypesupport> Clone for LifecyclePublisher<T> {
    fn clone(&self) -> Self {
        LifecyclePublisher {
            publisher: self.publisher.clone(),
            enabled: self.enabled.clone(),
        }
    }
}

impl<T: WrappedTypesupport> LifecyclePublisher<T> {
    pub(crate) fn new(publisher: Publisher<T>) -> (Self, WeakActivation) {
        let enabled = Arc::new(AtomicBool::new(false));
        let weak = Arc::downgrade(&enabled);
        (LifecyclePublisher { publisher, enabled }, weak)
    }

    /// Whether the publisher currently forwards messages to DDS.
    pub fn is_activated(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Enable publishing. Managed publishers are activated automatically when
    /// the node enters the `active` state; this allows manual control too.
    pub fn on_activate(&self) {
        self.enabled.store(true, Ordering::SeqCst);
    }

    /// Disable publishing; subsequent [`Self::publish`] calls drop messages.
    pub fn on_deactivate(&self) {
        self.enabled.store(false, Ordering::SeqCst);
    }

    /// Publish a ROS message, dropping it when deactivated.
    ///
    /// Returns `Ok(())` in both cases; use [`Self::is_activated`] beforehand
    /// to distinguish a gated drop from an actual DDS publish.
    pub fn publish(&self, msg: &T) -> Result<()> {
        if !self.is_activated() {
            log::debug!(
                "dropping message on deactivated lifecycle publisher '{}'",
                self.publisher.topic_name().unwrap_or_default()
            );
            return Ok(());
        }
        self.publisher.publish(msg)
    }
}

impl<T: WrappedTypesupport> Deref for LifecyclePublisher<T> {
    type Target = Publisher<T>;

    fn deref(&self) -> &Publisher<T> {
        &self.publisher
    }
}

#[cfg(test)]
mod tests {
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
            .create_publisher::<std_msgs::msg::String>(
                &format!("/{name}/gated"),
                QosProfile::default(),
            )
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
            if ready.as_mut().now_or_never().is_some() || Instant::now() > deadline {
                break;
            }
        }

        let msg = |text: &str| std_msgs::msg::String { data: text.into() };

        // Unconfigured: message dropped.
        assert!(!publisher.is_activated());
        publisher.publish(&msg("unconfigured")).unwrap();
        assert!(drain(
            &mut managed,
            &mut subscriber_node,
            &mut messages,
            Duration::from_millis(200)
        )
        .is_empty());

        // Configured/inactive: still dropped.
        assert_eq!(managed.configure().unwrap(), CallbackReturn::Success);
        assert!(!publisher.is_activated());
        publisher.publish(&msg("inactive")).unwrap();
        assert!(drain(
            &mut managed,
            &mut subscriber_node,
            &mut messages,
            Duration::from_millis(200)
        )
        .is_empty());

        // Active: delivered.
        assert_eq!(managed.activate().unwrap(), CallbackReturn::Success);
        assert!(publisher.is_activated());
        publisher.publish(&msg("active")).unwrap();
        let delivered =
            drain(&mut managed, &mut subscriber_node, &mut messages, Duration::from_millis(500));
        assert_eq!(delivered, vec!["active".to_string()]);

        // Deactivated: dropped again.
        assert_eq!(managed.deactivate().unwrap(), CallbackReturn::Success);
        assert!(!publisher.is_activated());
        publisher.publish(&msg("inactive-again")).unwrap();
        assert!(drain(
            &mut managed,
            &mut subscriber_node,
            &mut messages,
            Duration::from_millis(200)
        )
        .is_empty());
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
}
