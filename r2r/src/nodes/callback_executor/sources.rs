//! Node-owned native subscription frontends for callback group input rings.

use super::*;
use std::task::Waker;
mod registry;
mod service;
mod subscription;
mod timer;
pub(in crate::nodes) use registry::Sources;
pub use service::GroupedService;
pub use timer::GroupedTimer;
pub(crate) use timer::GroupedTimerActivation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Native input buffering and per-source dispatch bounds for grouped callbacks.
/// DDS history/QoS is independent from this preallocated owned-input ring.
pub struct CallbackQueueOptions {
    /// Exact number of waiting inputs, excluding the handler currently executing.
    pub capacity: usize,
    /// Maximum native inputs admitted for this source after one wait snapshot.
    pub max_inputs_per_spin: usize,
}
impl Default for CallbackQueueOptions {
    fn default() -> Self {
        Self {
            capacity: 16,
            max_inputs_per_spin: 16,
        }
    }
}
impl CallbackQueueOptions {
    /// Sets a nonzero ring capacity; invalid sizes fail before endpoint creation.
    pub const fn capacity(mut self, capacity: usize) -> Self {
        self.capacity = capacity;
        self
    }
    /// Sets a nonzero native batch limit, trading throughput against dispatch fairness.
    pub const fn max_inputs_per_spin(mut self, maximum: usize) -> Self {
        self.max_inputs_per_spin = maximum;
        self
    }
    fn validate(self) -> Result<Self> {
        if self.capacity == 0 || self.capacity >= usize::MAX >> 2 || self.max_inputs_per_spin == 0 {
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        } else {
            Ok(self)
        }
    }
}

/// Group buffering and middleware subscription settings, configured independently.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroupedSubscriptionOptions {
    pub queue: CallbackQueueOptions,
    pub endpoint: NativeCallbackSubscriptionOptions,
}
impl GroupedSubscriptionOptions {
    /// Sets owned-input capacity and native dispatch bounds.
    pub const fn queue(mut self, queue: CallbackQueueOptions) -> Self {
        self.queue = queue;
        self
    }
    /// Sets local-publication/network-flow options and the endpoint batch bound.
    pub const fn endpoint(mut self, endpoint: NativeCallbackSubscriptionOptions) -> Self {
        self.endpoint = endpoint;
        self
    }
}
impl From<CallbackQueueOptions> for GroupedSubscriptionOptions {
    fn from(queue: CallbackQueueOptions) -> Self {
        Self {
            queue,
            ..Default::default()
        }
    }
}

/// Owns a native subscription whose scoped handler executes in a callback group.
///
/// Drop closes pending input and wakes native cleanup. A handler already
/// selected retains independent native message ownership until it returns.
/// Callback errors close the handler and propagate through CallbackExecutorTask.
pub struct GroupedSubscription {
    id: uuid::Uuid,
    control: Arc<dyn input::Control>,
}
impl GroupedSubscription {
    #[cfg(test)]
    pub(in crate::nodes) fn source_id(&self) -> uuid::Uuid {
        self.id
    }
    /// Fully resolved native topic name. Node ownership must match this token.
    pub fn topic_name(&self, node: &Node) -> Result<String> {
        node.get_subscription_topic_name(self.id)
    }
    /// Native QoS resolved by the selected middleware.
    pub fn actual_qos(&self, node: &Node) -> Result<QosProfile> {
        node.get_subscription_actual_qos(self.id)
    }
    /// Current matched publisher count.
    pub fn get_publisher_count(&self, node: &Node) -> Result<usize> {
        node.get_subscription_publisher_count(self.id)
    }
    /// Graph-driven publisher matching; spin the owning Node while awaiting.
    pub fn wait_for_publishers(&self, node: &mut Node) -> Result<impl Future<Output = Result<()>>> {
        Ok(node
            .register_subscription_publisher_wait(self.id)?
            .map_err(|_| Error::RCL_RET_SUBSCRIPTION_INVALID))
    }
    /// Matches publishers up to a Node-driven timeout.
    pub fn wait_for_publishers_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        Node::validate_timeout(timeout)?;
        let matched = self.wait_for_publishers(node)?;
        node.with_timeout(matched, timeout)
    }
    /// Creates a bounded native QoS event stream for the owning subscription.
    pub fn qos_events(
        &self, node: &mut Node, kind: crate::SubscriptionEventKind,
    ) -> Result<crate::QosEventStream> {
        node.subscription_qos_events(self.id, kind)
    }
    /// Whether its input/handler registration remains open.
    pub fn is_valid(&self) -> bool {
        !self.control.is_closed()
    }
}
impl Drop for GroupedSubscription {
    fn drop(&mut self) {
        self.control.close();
    }
}

/// Native frontends are polled only by the uniquely owned Node job. Concrete
/// sources describe their own wait-set kind; user handlers live in group jobs.
trait Source {
    fn prepare(&mut self) -> Result<()> {
        Ok(())
    }
    fn id(&self) -> uuid::Uuid;
    fn group_id(&self) -> CallbackGroupId;
    fn subscriber(&self) -> Option<&dyn Subscriber_> {
        None
    }
    fn subscriber_mut(&mut self) -> Option<&mut dyn Subscriber_> {
        None
    }
    fn is_closed(&self) -> bool;
    fn is_waitable(&self) -> bool;
    fn set_wake(&mut self, wake: Option<Waker>);
    fn capacities(&self) -> WaitSetCapacities;
    fn register(&self, ws: &mut rcl_wait_set_t, index: &mut usize) -> Result<()>;
    fn ready(&self, ws: &rcl_wait_set_t, index: usize) -> Result<bool>;
    fn maximum(&self) -> usize;
    fn take(&mut self) -> Result<bool>;
    fn destroy(&mut self, node: &mut rcl_node_t);
}

fn ready<T>(
    pointers: *mut *const T, length: usize, index: usize, expected: *const T,
) -> Result<bool> {
    if pointers.is_null() || index >= length {
        return Err(Error::RCL_RET_WAIT_SET_INVALID);
    }
    let pointer = unsafe { *pointers.add(index) };
    if pointer.is_null() {
        Ok(false)
    } else if pointer == expected {
        Ok(true)
    } else {
        Err(Error::RCL_RET_WAIT_SET_INVALID)
    }
}
