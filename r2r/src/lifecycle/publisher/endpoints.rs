//! Endpoint operations that remain available in every lifecycle state.

use std::time::Duration;

use futures::Future;

use super::LifecyclePublisher;
#[cfg(not(r2r__ros__distro__humble))]
use crate::{IncompatibleTypeStatus, MatchedStatus};
use crate::{
    LivelinessLostStatus, NetworkFlowEndpoint, Node, OfferedDeadlineMissedStatus,
    OfferedIncompatibleQosStatus, PublisherGid, QosProfile, Result, WrappedTypesupport,
};

impl<T: WrappedTypesupport> LifecyclePublisher<T> {
    /// Immediately destroy this publisher's endpoint in its owning node.
    ///
    /// Other clones become stale. Passing another node consumes this clone
    /// without destroying an endpoint there, following [`Node::destroy_publisher`].
    /// Dropping the last clone instead schedules cleanup on the next spin.
    pub fn destroy(self, node: &mut Node) {
        node.destroy_publisher(self.publisher);
    }

    /// Takes the latest offered-deadline-missed status without blocking.
    pub fn take_offered_deadline_missed(&self) -> Result<Option<OfferedDeadlineMissedStatus>> {
        self.publisher.take_offered_deadline_missed()
    }

    /// Takes the latest liveliness-lost status without blocking.
    pub fn take_liveliness_lost(&self) -> Result<Option<LivelinessLostStatus>> {
        self.publisher.take_liveliness_lost()
    }

    /// Takes offered-incompatible-QoS status, initializing the event on first use.
    pub fn take_offered_incompatible_qos(&self) -> Result<Option<OfferedIncompatibleQosStatus>> {
        self.publisher.take_offered_incompatible_qos()
    }

    /// Takes publisher matching changes without blocking.
    #[cfg(not(r2r__ros__distro__humble))]
    pub fn take_matched(&self) -> Result<Option<MatchedStatus>> {
        self.publisher.take_matched()
    }

    /// Takes incompatible-type discovery events without blocking.
    #[cfg(not(r2r__ros__distro__humble))]
    pub fn take_incompatible_type(&self) -> Result<Option<IncompatibleTypeStatus>> {
        self.publisher.take_incompatible_type()
    }

    /// Returns the QoS profile resolved by the middleware.
    pub fn get_actual_qos(&self) -> Result<QosProfile> {
        self.publisher.get_actual_qos()
    }

    /// Returns the fully resolved native topic name.
    pub fn topic_name(&self) -> Result<String> {
        self.publisher.topic_name()
    }

    /// Returns whether the middleware can loan storage for this message type.
    pub fn can_loan_messages(&self) -> Result<bool> {
        self.publisher.can_loan_messages()
    }

    /// Returns the middleware network flows used to send messages, when supported.
    pub fn get_network_flow_endpoints(&self) -> Result<Vec<NetworkFlowEndpoint>> {
        self.publisher.get_network_flow_endpoints()
    }

    /// Returns this publisher's globally unique middleware identity.
    pub fn get_gid(&self) -> Result<PublisherGid> {
        self.publisher.get_gid()
    }

    /// Manually asserts liveliness, including while publication is deactivated.
    pub fn assert_liveliness(&self) -> Result<()> {
        self.publisher.assert_liveliness()
    }

    /// Blocks until reliable data is acknowledged or the timeout expires.
    pub fn wait_for_all_acked(&self, timeout: Duration) -> Result<()> {
        self.publisher.wait_for_all_acked(timeout)
    }

    /// Waits asynchronously for reliable acknowledgements while the owner is spun.
    pub fn wait_for_all_acked_async(&self) -> Result<impl Future<Output = Result<()>>> {
        self.publisher.wait_for_all_acked_async()
    }

    /// Waits asynchronously for acknowledgements up to the timeout.
    pub fn wait_for_all_acked_async_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        self.publisher
            .wait_for_all_acked_async_with_timeout(node, timeout)
    }

    /// Returns the number of subscriptions matched to this publisher.
    pub fn get_subscription_count(&self) -> Result<usize> {
        self.publisher.get_subscription_count()
    }

    /// Waits for a matched subscription while the owner is spun.
    pub fn wait_for_subscribers(&self) -> Result<impl Future<Output = Result<()>>> {
        self.publisher.wait_for_subscribers()
    }

    /// Waits for a matched subscription up to the timeout.
    pub fn wait_for_subscribers_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        self.publisher
            .wait_for_subscribers_with_timeout(node, timeout)
    }

    #[deprecated(note = "use get_subscription_count; the RCL count includes local matches")]
    pub fn get_inter_process_subscription_count(&self) -> Result<usize> {
        self.get_subscription_count()
    }

    #[deprecated(note = "use wait_for_subscribers; local matches are included")]
    pub fn wait_for_inter_process_subscribers(&self) -> Result<impl Future<Output = Result<()>>> {
        self.wait_for_subscribers()
    }

    #[deprecated(note = "use wait_for_subscribers_with_timeout; local matches are included")]
    pub fn wait_for_inter_process_subscribers_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        self.wait_for_subscribers_with_timeout(node, timeout)
    }
}
