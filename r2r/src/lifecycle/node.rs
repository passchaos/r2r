//! Public managed (lifecycle) node: [`LifecycleNode`].
//!
//! The managed node owns a regular [`Node`] plus a map-only
//! [`LifecycleStateMachine`]. The five standard lifecycle services and the
//! transition-event publisher are native r2r entities, so all readiness is
//! handled by the same wait set/executor as every other r2r entity.
//!
//! Transition semantics mirror `rclcpp_lifecycle` exactly: an external command
//! (`configure`, ...) moves the machine into the corresponding intermediate
//! state, the user callback for that state runs outside the state-machine
//! lock, and its [`CallbackReturn`] drives the generic
//! `transition_success` / `transition_failure` / `transition_error` edge. An
//! `Error` return additionally invokes the `on_error` callback from the
//! error-processing state.

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use parking_lot::Mutex;

use r2r_rcl::rosidl_service_type_support_t;

use crate::clocks::Clock;
use crate::context::Context;
use crate::error::Result;
use crate::lifecycle::publisher::{LifecyclePublisher, ManagedEntities};
use crate::lifecycle::services::LifecycleService;
use crate::lifecycle::state_machine::{
    LifecycleState, LifecycleStateMachine, LifecycleTrigger, TransitionEdge,
};
use crate::lifecycle_msgs;
use crate::lifecycle_msgs::msg::{
    State as LcState, Transition, TransitionDescription, TransitionEvent,
};
use crate::nodes::Node;
use crate::publishers::Publisher;
use crate::{QosProfile, WrappedServiceTypeSupport};

/// Outcome of a lifecycle transition callback.
///
/// Matches the three `lifecycle_msgs/msg/Transition.TRANSITION_CALLBACK_*`
/// results: `Success` drives the `*_success` edge, `Failure` the `*_failure`
/// edge and `Error` enters the error-processing state and invokes the
/// configured error callback.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallbackReturn {
    /// `TRANSITION_CALLBACK_SUCCESS` (97).
    Success,
    /// `TRANSITION_CALLBACK_FAILURE` (98); the optional reason is logged.
    Failure(String),
    /// `TRANSITION_CALLBACK_ERROR` (99); the optional reason is logged before
    /// the error callback runs.
    Error(String),
}

impl CallbackReturn {
    /// Shorthand for [`CallbackReturn::Failure`] with a reason.
    pub fn failure(reason: impl Into<String>) -> Self {
        CallbackReturn::Failure(reason.into())
    }

    /// Shorthand for [`CallbackReturn::Error`] with a reason.
    pub fn error(reason: impl Into<String>) -> Self {
        CallbackReturn::Error(reason.into())
    }

    /// rcl label of the generic outcome edge this result triggers.
    fn outcome_label(&self) -> &'static str {
        match self {
            CallbackReturn::Success => "transition_success",
            CallbackReturn::Failure(_) => "transition_failure",
            CallbackReturn::Error(_) => "transition_error",
        }
    }

    fn log_reason(&self, stage: &str) {
        match self {
            CallbackReturn::Success => {}
            CallbackReturn::Failure(reason) => {
                log::warn!("lifecycle {stage} callback failed: {reason}")
            }
            CallbackReturn::Error(reason) => {
                log::error!("lifecycle {stage} callback errored: {reason}")
            }
        }
    }
}

/// User callback invoked when the machine enters an intermediate state.
///
/// The argument is the primary state the transition started from (the same
/// `previous_state` rclcpp passes). Callbacks run on the thread driving the
/// node (the spin handling a `change_state` request, or the caller thread for
/// direct [`LifecycleNode`] commands), and must not trigger another lifecycle
/// transition re-entrantly.
pub type LifecycleCallback = dyn FnMut(LifecycleState) -> CallbackReturn + Send + 'static;

#[derive(Default)]
struct LifecycleCallbacks {
    configure: Option<Box<LifecycleCallback>>,
    cleanup: Option<Box<LifecycleCallback>>,
    shutdown: Option<Box<LifecycleCallback>>,
    activate: Option<Box<LifecycleCallback>>,
    deactivate: Option<Box<LifecycleCallback>>,
    error: Option<Box<LifecycleCallback>>,
}

/// Identifies one of the five built-in lifecycle services.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleServiceKind {
    ChangeState,
    GetState,
    GetAvailableStates,
    GetAvailableTransitions,
    GetTransitionGraph,
}

impl LifecycleServiceKind {
    pub fn all() -> [LifecycleServiceKind; 5] {
        [
            LifecycleServiceKind::ChangeState,
            LifecycleServiceKind::GetState,
            LifecycleServiceKind::GetAvailableStates,
            LifecycleServiceKind::GetAvailableTransitions,
            LifecycleServiceKind::GetTransitionGraph,
        ]
    }

    fn topic_suffix(self) -> &'static str {
        match self {
            LifecycleServiceKind::ChangeState => "change_state",
            LifecycleServiceKind::GetState => "get_state",
            LifecycleServiceKind::GetAvailableStates => "get_available_states",
            LifecycleServiceKind::GetAvailableTransitions => "get_available_transitions",
            LifecycleServiceKind::GetTransitionGraph => "get_transition_graph",
        }
    }

    pub(crate) fn type_support(self) -> *const rosidl_service_type_support_t {
        use lifecycle_msgs::srv::*;
        match self {
            LifecycleServiceKind::ChangeState => ChangeState::Service::get_ts(),
            LifecycleServiceKind::GetState => GetState::Service::get_ts(),
            LifecycleServiceKind::GetAvailableStates => GetAvailableStates::Service::get_ts(),
            // rclcpp registers the GetAvailableTransitions type support for
            // get_transition_graph as well (identical srv type).
            LifecycleServiceKind::GetAvailableTransitions
            | LifecycleServiceKind::GetTransitionGraph => {
                GetAvailableTransitions::Service::get_ts()
            }
        }
    }
}

/// Shared mutable lifecycle state, referenced by the inline services.
pub(crate) struct LifecycleRuntime {
    machine: Mutex<LifecycleStateMachine>,
    callbacks: Mutex<LifecycleCallbacks>,
    entities: ManagedEntities,
    events: Publisher<TransitionEvent>,
    clock: Arc<Mutex<Clock>>,
}

impl LifecycleRuntime {
    pub fn current_state(&self) -> LifecycleState {
        self.machine.lock().current_state()
    }

    pub fn current_state_msg(&self) -> LcState {
        let state = self.machine.lock().current_state();
        state_msg(state)
    }

    pub fn all_states_msgs(&self) -> Vec<LcState> {
        self.machine
            .lock()
            .states()
            .into_iter()
            .map(state_msg)
            .collect()
    }

    pub fn available_transition_descriptions(&self) -> Vec<TransitionDescription> {
        self.machine
            .lock()
            .transitions_from_current()
            .into_iter()
            .map(edge_description)
            .collect()
    }

    pub fn transition_graph_descriptions(&self) -> Vec<TransitionDescription> {
        self.machine
            .lock()
            .transition_graph()
            .into_iter()
            .map(edge_description)
            .collect()
    }

    /// Execute the full rclcpp change_state protocol for an external command
    /// id (or a label, which takes precedence like in rclcpp). Returns the
    /// primary callback outcome. An unknown edge from the current state yields
    /// `Ok(Failure)` without running a callback (and the service reports
    /// `success=false`), matching the rclcpp service response.
    pub fn change_state(&self, requested_id: u8, label: Option<&str>) -> Result<CallbackReturn> {
        let (previous, external_edge) = {
            let machine = self.machine.lock();
            let previous = machine.current_state();
            let edge = match label {
                Some(label) => machine.current_transition_by_label(label),
                None => machine.current_transition_by_id(requested_id),
            };
            let Some(edge) = edge else {
                return Ok(CallbackReturn::Failure(
                    "transition not available from current state".into(),
                ));
            };
            (previous, edge)
        };

        // External command: primary state -> intermediate state.
        {
            let mut machine = self.machine.lock();
            machine.trigger_id(external_edge.id as u8)?;
        }
        self.publish_event(&external_edge);

        let intermediate = self.machine.lock().current_state();
        let primary = self.run_callback(intermediate, previous);
        primary.log_reason(stage_name(intermediate));

        // Primary callback outcome: intermediate -> primary (or errorprocessing).
        let outcome_label = primary.outcome_label();
        let outcome_edge = {
            let mut machine = self.machine.lock();
            let edge = machine
                .current_transition_by_label(outcome_label)
                .ok_or(crate::Error::RCL_RET_ERROR)?;
            machine.trigger_label(outcome_label)?;
            edge
        };
        self.publish_event(&outcome_edge);

        // Only an Error return enters the error-processing callback; the
        // reported primary outcome is unchanged by recovery (rclcpp reports
        // success from the primary callback return).
        if let CallbackReturn::Error(_) = primary {
            let error_state = self.machine.lock().current_state();
            let recovery = self.run_callback(error_state, previous);
            recovery.log_reason("error");
            let recovery_label = recovery.outcome_label();
            let recovery_edge = {
                let mut machine = self.machine.lock();
                let edge = machine
                    .current_transition_by_label(recovery_label)
                    .ok_or(crate::Error::RCL_RET_ERROR)?;
                machine.trigger_label(recovery_label)?;
                edge
            };
            self.publish_event(&recovery_edge);
        }

        // Managed publishers forward only while the node is active. The flag
        // is reconciled with the settled primary state after every command
        // (including shutdown and error recovery), so an activate failure
        // leaves them deactivated.
        self.entities
            .set_enabled(self.machine.lock().current_state() == LifecycleState::Active);

        Ok(primary)
    }

    /// Register a managed publisher, activated/deactivated with the node.
    pub fn register_managed_entity(&self, entity: crate::lifecycle::publisher::WeakActivation) {
        self.entities.register(entity);
    }

    fn run_callback(
        &self, intermediate: LifecycleState, previous: LifecycleState,
    ) -> CallbackReturn {
        let mut callbacks = self.callbacks.lock();
        let slot: &mut Option<Box<LifecycleCallback>> = match intermediate {
            LifecycleState::Configuring => &mut callbacks.configure,
            LifecycleState::CleaningUp => &mut callbacks.cleanup,
            LifecycleState::ShuttingDown => &mut callbacks.shutdown,
            LifecycleState::Activating => &mut callbacks.activate,
            LifecycleState::Deactivating => &mut callbacks.deactivate,
            LifecycleState::ErrorProcessing => &mut callbacks.error,
            _ => return CallbackReturn::Success,
        };
        match slot {
            Some(callback) => callback(previous),
            // Unset callbacks succeed immediately, matching rclcpp.
            None => CallbackReturn::Success,
        }
    }

    fn publish_event(&self, edge: &TransitionEdge) {
        let timestamp = self
            .clock
            .lock()
            .get_now_ns()
            .ok()
            .and_then(|ns| u64::try_from(ns).ok())
            .unwrap_or(0);
        let event = TransitionEvent {
            timestamp,
            transition: Transition {
                id: edge.id as u8,
                label: edge.label.clone().into(),
            },
            start_state: state_msg(edge.start),
            goal_state: state_msg(edge.goal),
        };
        if let Err(err) = self.events.publish(&event) {
            log::warn!("could not publish lifecycle transition event: {err}");
        }
    }
}

fn stage_name(state: LifecycleState) -> &'static str {
    match state {
        LifecycleState::Configuring => "configure",
        LifecycleState::CleaningUp => "cleanup",
        LifecycleState::ShuttingDown => "shutdown",
        LifecycleState::Activating => "activate",
        LifecycleState::Deactivating => "deactivate",
        LifecycleState::ErrorProcessing => "error",
        _ => "unknown",
    }
}

fn state_msg(state: LifecycleState) -> LcState {
    LcState {
        id: state.id(),
        label: state.label().to_owned().into(),
    }
}

fn edge_description(edge: TransitionEdge) -> TransitionDescription {
    TransitionDescription {
        transition: Transition {
            id: edge.id as u8,
            label: edge.label.into(),
        },
        start_state: state_msg(edge.start),
        goal_state: state_msg(edge.goal),
    }
}

/// A ROS 2 managed (lifecycle) node.
///
/// This is a thin owner around a regular [`Node`] (all ordinary node methods
/// are available through `Deref`/`DerefMut`) that additionally:
///
/// - starts in the `unconfigured` primary state with the standard lifecycle
///   state machine;
/// - serves `change_state`, `get_state`, `get_available_states`,
///   `get_available_transitions` and `get_transition_graph` on the standard
///   node-relative topics;
/// - publishes `lifecycle_msgs/msg/TransitionEvent` on `~/transition_event`;
/// - runs the registered configure/cleanup/activate/deactivate/shutdown/error
///   callbacks with `rclcpp_lifecycle` semantics.
///
/// All lifecycle entities are native r2r entities, so simply spinning the node
/// (or owning it through the unified [`crate::Executor`]) also services
/// lifecycle traffic.
pub struct LifecycleNode {
    // Declared before `node` so it drops after the node and its internal
    // services, which hold Arc clones into the runtime.
    runtime: Arc<LifecycleRuntime>,
    node: Option<Node>,
}

impl LifecycleNode {
    /// Create a managed node with the standard lifecycle services enabled.
    pub fn create(context: Context, name: &str, namespace: &str) -> Result<LifecycleNode> {
        let mut node = Node::create(context, name, namespace)?;
        let fqn = node.fully_qualified_name()?;

        let machine = LifecycleStateMachine::new()?;
        let events = node.create_publisher::<TransitionEvent>(
            &format!("{fqn}/transition_event"),
            QosProfile::default(),
        )?;
        let runtime = Arc::new(LifecycleRuntime {
            machine: Mutex::new(machine),
            callbacks: Mutex::new(LifecycleCallbacks::default()),
            entities: ManagedEntities::default(),
            events,
            clock: node.ros_clock.clone(),
        });

        for kind in LifecycleServiceKind::all() {
            let topic = format!("{fqn}/{}", kind.topic_suffix());
            let service =
                LifecycleService::create(node.node_handle.as_mut(), &topic, kind, runtime.clone())?;
            node.add_service(uuid::Uuid::new_v4(), service);
        }

        Ok(LifecycleNode {
            runtime,
            node: Some(node),
        })
    }

    /// Extract the underlying [`Node`], transferring ownership to the caller
    /// (for example to hand it to an [`crate::Executor`]).
    ///
    /// The lifecycle services keep the runtime alive (their service objects
    /// hold a shared clone), so `change_state` and the other lifecycle
    /// services continue to work while the node is spun. Direct Rust
    /// transition methods are no longer available after extraction.
    pub fn into_node(self) -> Node {
        self.node.expect("lifecycle node already extracted")
    }

    /// The node's current lifecycle state.
    pub fn lifecycle_state(&self) -> LifecycleState {
        self.runtime.current_state()
    }

    /// All states registered in the state machine (including the
    /// unreachable `unknown` state and the intermediate states), matching the
    /// `get_available_states` service.
    pub fn available_states(&self) -> Vec<LifecycleState> {
        self.runtime.machine.lock().states()
    }

    /// Transitions directly available from the current state.
    pub fn available_transitions(&self) -> Vec<TransitionEdge> {
        self.runtime.machine.lock().transitions_from_current()
    }

    /// The full standard transition graph (all 25 edges).
    pub fn transition_graph(&self) -> Vec<TransitionEdge> {
        self.runtime.machine.lock().transition_graph()
    }

    /// Create a lifecycle-gated publisher.
    ///
    /// The publisher starts deactivated: messages are dropped until the node
    /// reaches the `active` primary state (via [`Self::activate`] or a
    /// successful `change_state` service call). It is deactivated again when
    /// the node leaves `active` through `deactivate`, `shutdown` or a
    /// transition failure/error.
    pub fn create_publisher<T>(
        &mut self, topic: &str, qos_profile: QosProfile,
    ) -> Result<LifecyclePublisher<T>>
    where
        T: crate::WrappedTypesupport,
    {
        let publisher = self
            .node
            .as_mut()
            .expect("lifecycle node already extracted")
            .create_publisher::<T>(topic, qos_profile)?;
        let (managed, weak) = LifecyclePublisher::new(publisher);
        self.runtime.register_managed_entity(weak);
        // A publisher created on an already active node starts active too.
        if self.lifecycle_state() == LifecycleState::Active {
            managed.on_activate();
        }
        Ok(managed)
    }

    /// Run the `configure` transition (`unconfigured -> configuring -> inactive`
    /// on success).
    pub fn configure(&self) -> Result<CallbackReturn> {
        self.trigger_external(LifecycleTrigger::Configure)
    }

    /// Run the `cleanup` transition (`inactive -> cleaningup -> unconfigured`).
    pub fn cleanup(&self) -> Result<CallbackReturn> {
        self.trigger_external(LifecycleTrigger::Cleanup)
    }

    /// Run the `activate` transition (`inactive -> activating -> active`).
    pub fn activate(&self) -> Result<CallbackReturn> {
        self.trigger_external(LifecycleTrigger::Activate)
    }

    /// Run the `deactivate` transition (`active -> deactivating -> inactive`).
    pub fn deactivate(&self) -> Result<CallbackReturn> {
        self.trigger_external(LifecycleTrigger::Deactivate)
    }

    /// Run the shutdown transition valid for the current primary state.
    pub fn shutdown(&self) -> Result<CallbackReturn> {
        let trigger = match self.lifecycle_state() {
            LifecycleState::Unconfigured => LifecycleTrigger::UnconfiguredShutdown,
            LifecycleState::Inactive => LifecycleTrigger::InactiveShutdown,
            LifecycleState::Active => LifecycleTrigger::ActiveShutdown,
            _ => return Err(crate::Error::RCL_RET_INVALID_ARGUMENT),
        };
        self.trigger_external(trigger)
    }

    fn trigger_external(&self, trigger: LifecycleTrigger) -> Result<CallbackReturn> {
        self.runtime.change_state(trigger.id(), None)
    }

    /// Register the callback run while entering the `configuring` state.
    pub fn on_configure(&self, callback: Box<LifecycleCallback>) -> &Self {
        self.runtime.callbacks.lock().configure = Some(callback);
        self
    }

    /// Register the callback run while entering the `cleaningup` state.
    pub fn on_cleanup(&self, callback: Box<LifecycleCallback>) -> &Self {
        self.runtime.callbacks.lock().cleanup = Some(callback);
        self
    }

    /// Register the callback run while entering the `shuttingdown` state.
    pub fn on_shutdown(&self, callback: Box<LifecycleCallback>) -> &Self {
        self.runtime.callbacks.lock().shutdown = Some(callback);
        self
    }

    /// Register the callback run while entering the `activating` state.
    pub fn on_activate(&self, callback: Box<LifecycleCallback>) -> &Self {
        self.runtime.callbacks.lock().activate = Some(callback);
        self
    }

    /// Register the callback run while entering the `deactivating` state.
    pub fn on_deactivate(&self, callback: Box<LifecycleCallback>) -> &Self {
        self.runtime.callbacks.lock().deactivate = Some(callback);
        self
    }

    /// Register the callback run from the `errorprocessing` state after a
    /// primary callback returned [`CallbackReturn::Error`].
    pub fn on_error(&self, callback: Box<LifecycleCallback>) -> &Self {
        self.runtime.callbacks.lock().error = Some(callback);
        self
    }
}

impl Deref for LifecycleNode {
    type Target = Node;

    fn deref(&self) -> &Node {
        self.node
            .as_ref()
            .expect("lifecycle node already extracted")
    }
}

impl DerefMut for LifecycleNode {
    fn deref_mut(&mut self) -> &mut Node {
        self.node
            .as_mut()
            .expect("lifecycle node already extracted")
    }
}

#[cfg(test)]
mod node_tests;
