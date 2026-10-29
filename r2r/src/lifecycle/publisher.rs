//! Lifecycle-gated publisher.
//!
//! A [`LifecyclePublisher`] wraps an ordinary [`Publisher`] and adds the
//! `rclcpp_lifecycle::LifecyclePublisher` contract: messages are forwarded to
//! DDS only while the publisher is *activated*. Publishers created through
//! [`crate::LifecycleNode::create_publisher`] are registered as managed
//! entities: a successful transition into the `active` primary state activates
//! them automatically, and settling outside `active` deactivates them.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use crate::error::Result;
use crate::publishers::Publisher;
use crate::{NativeMessageRef, NativeMsg, Node, WrappedTypesupport};

mod endpoints;
mod loaned;
pub use loaned::LifecycleLoanedMessage;

#[cfg(test)]
mod tests;

/// Shared activation flag held by the publisher and weakly referenced from
/// the lifecycle runtime. Dropping every clone of the publisher removes the
/// entity automatically on the next transition.
pub(crate) type Activation = Arc<AtomicBool>;
pub(crate) type WeakActivation = Weak<AtomicBool>;
pub(crate) use super::managed::ManagedEntities;

/// A ROS publisher whose publication follows a shared lifecycle activation flag.
///
/// Rust, native and loaned messages are forwarded only while activated. A
/// deactivated publish returns `Ok(())` and drops the publication, matching
/// `rclcpp_lifecycle` semantics. Native message storage remains caller-owned;
/// dropped loans are returned to the middleware.
///
/// Endpoint queries and asynchronous matching/ACK waits remain available in
/// every state. This wrapper does not expose the underlying [`Publisher`], so
/// cloning or borrowing it cannot bypass the activation check. A publish that
/// observes activation may finish concurrently with deactivation; deactivation
/// does not retract messages already handed to DDS.
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

    /// Publish from reusable native message storage while activated.
    ///
    /// Deactivated publication returns `Ok(())` without touching the native
    /// message or passing it to DDS. No Rust-to-native payload copy is needed.
    pub fn publish_native(&self, msg: &mut NativeMsg<T>) -> Result<()> {
        if !self.is_activated() {
            return Ok(());
        }
        self.publisher.publish_native(msg)
    }

    /// Publish a borrowed native ROS message while activated.
    ///
    /// This preserves the lifecycle gate while forwarding directly from a scoped native
    /// subscription callback without copying into an owned Rust message.
    pub fn publish_native_ref(&self, msg: &NativeMessageRef<'_, T>) -> Result<()> {
        if !self.is_activated() {
            return Ok(());
        }
        self.publisher.publish_native_ref(msg)
    }

    /// Borrow native middleware storage for a lifecycle-gated publication.
    ///
    /// Borrowing is permitted in any lifecycle state. The returned loan checks
    /// activation when its `publish` method is called, so a loan filled before
    /// deactivation cannot subsequently publish while deactivated. Dropping
    /// an unpublished loan returns it to the middleware. Unsupported loans and
    /// invalid node ownership retain the ordinary publisher's errors.
    pub fn borrow_loaned_message<'node>(
        &self, node: &'node mut Node,
    ) -> Result<LifecycleLoanedMessage<'node, T>> {
        let message = self.publisher.borrow_loaned_message(node)?;
        Ok(LifecycleLoanedMessage::new(message, self.enabled.clone()))
    }
}
