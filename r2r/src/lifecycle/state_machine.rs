//! Safe owned wrapper over `rcl_lifecycle_state_machine_t`.

use std::ffi::{c_char, CStr, CString};
use std::slice;

use r2r_rcl::*;

use crate::error::{Error, Result};

/// A state in a ROS 2 lifecycle state machine.
///
/// Numeric ids match the `lifecycle_msgs/msg/State` constants. The four
/// primary states are stable node states; the transition-state variants are
/// intermediate states that only exist while a transition callback runs.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LifecycleState {
    /// `PRIMARY_STATE_UNKNOWN` (0). Registered in the default map but not
    /// reachable from any transition.
    Unknown,
    /// `PRIMARY_STATE_UNCONFIGURED` (1), the initial state.
    Unconfigured,
    /// `PRIMARY_STATE_INACTIVE` (2).
    Inactive,
    /// `PRIMARY_STATE_ACTIVE` (3).
    Active,
    /// `PRIMARY_STATE_FINALIZED` (4), the terminal state.
    Finalized,
    /// `TRANSITION_STATE_CONFIGURING` (10).
    Configuring,
    /// `TRANSITION_STATE_CLEANINGUP` (11).
    CleaningUp,
    /// `TRANSITION_STATE_SHUTTINGDOWN` (12).
    ShuttingDown,
    /// `TRANSITION_STATE_ACTIVATING` (13).
    Activating,
    /// `TRANSITION_STATE_DEACTIVATING` (14).
    Deactivating,
    /// `TRANSITION_STATE_ERRORPROCESSING` (15).
    ErrorProcessing,
    /// A state id not known to this crate (future or custom state maps).
    Other(u8),
}

impl LifecycleState {
    /// Build from a `lifecycle_msgs/msg/State` id.
    pub fn from_id(id: u8) -> Self {
        use lifecycle_msgs_ids::*;
        match id {
            PRIMARY_STATE_UNKNOWN => LifecycleState::Unknown,
            PRIMARY_STATE_UNCONFIGURED => LifecycleState::Unconfigured,
            PRIMARY_STATE_INACTIVE => LifecycleState::Inactive,
            PRIMARY_STATE_ACTIVE => LifecycleState::Active,
            PRIMARY_STATE_FINALIZED => LifecycleState::Finalized,
            TRANSITION_STATE_CONFIGURING => LifecycleState::Configuring,
            TRANSITION_STATE_CLEANINGUP => LifecycleState::CleaningUp,
            TRANSITION_STATE_SHUTTINGDOWN => LifecycleState::ShuttingDown,
            TRANSITION_STATE_ACTIVATING => LifecycleState::Activating,
            TRANSITION_STATE_DEACTIVATING => LifecycleState::Deactivating,
            TRANSITION_STATE_ERRORPROCESSING => LifecycleState::ErrorProcessing,
            other => LifecycleState::Other(other),
        }
    }

    /// The numeric `lifecycle_msgs/msg/State` id.
    pub fn id(self) -> u8 {
        use lifecycle_msgs_ids::*;
        match self {
            LifecycleState::Unknown => PRIMARY_STATE_UNKNOWN,
            LifecycleState::Unconfigured => PRIMARY_STATE_UNCONFIGURED,
            LifecycleState::Inactive => PRIMARY_STATE_INACTIVE,
            LifecycleState::Active => PRIMARY_STATE_ACTIVE,
            LifecycleState::Finalized => PRIMARY_STATE_FINALIZED,
            LifecycleState::Configuring => TRANSITION_STATE_CONFIGURING,
            LifecycleState::CleaningUp => TRANSITION_STATE_CLEANINGUP,
            LifecycleState::ShuttingDown => TRANSITION_STATE_SHUTTINGDOWN,
            LifecycleState::Activating => TRANSITION_STATE_ACTIVATING,
            LifecycleState::Deactivating => TRANSITION_STATE_DEACTIVATING,
            LifecycleState::ErrorProcessing => TRANSITION_STATE_ERRORPROCESSING,
            LifecycleState::Other(id) => id,
        }
    }

    /// The label rcl stores for the standard states (`"unconfigured"`, ...).
    /// Returns `""` for [`LifecycleState::Other`].
    pub fn label(self) -> &'static str {
        match self {
            LifecycleState::Unknown => "unknown",
            LifecycleState::Unconfigured => "unconfigured",
            LifecycleState::Inactive => "inactive",
            LifecycleState::Active => "active",
            LifecycleState::Finalized => "finalized",
            LifecycleState::Configuring => "configuring",
            LifecycleState::CleaningUp => "cleaningup",
            LifecycleState::ShuttingDown => "shuttingdown",
            LifecycleState::Activating => "activating",
            LifecycleState::Deactivating => "deactivating",
            LifecycleState::ErrorProcessing => "errorprocessing",
            LifecycleState::Other(_) => "",
        }
    }

    /// True for the four stable primary states.
    pub fn is_primary(self) -> bool {
        matches!(
            self,
            LifecycleState::Unconfigured
                | LifecycleState::Inactive
                | LifecycleState::Active
                | LifecycleState::Finalized
        )
    }

    /// True for the intermediate states entered while a transition callback
    /// runs.
    pub fn is_intermediate(self) -> bool {
        !self.is_primary() && !matches!(self, LifecycleState::Unknown | LifecycleState::Other(_))
    }
}

/// Ids from `lifecycle_msgs/msg/State`. Kept as private constants so the enum
/// mapping does not depend on generated-code alias names.
mod lifecycle_msgs_ids {
    pub const PRIMARY_STATE_UNKNOWN: u8 = 0;
    pub const PRIMARY_STATE_UNCONFIGURED: u8 = 1;
    pub const PRIMARY_STATE_INACTIVE: u8 = 2;
    pub const PRIMARY_STATE_ACTIVE: u8 = 3;
    pub const PRIMARY_STATE_FINALIZED: u8 = 4;
    pub const TRANSITION_STATE_CONFIGURING: u8 = 10;
    pub const TRANSITION_STATE_CLEANINGUP: u8 = 11;
    pub const TRANSITION_STATE_SHUTTINGDOWN: u8 = 12;
    pub const TRANSITION_STATE_ACTIVATING: u8 = 13;
    pub const TRANSITION_STATE_DEACTIVATING: u8 = 14;
    pub const TRANSITION_STATE_ERRORPROCESSING: u8 = 15;
}

/// A trigger that can be applied to a [`LifecycleStateMachine`].
///
/// Numeric ids match the `lifecycle_msgs/msg/Transition` constants. The first
/// seven are the externally visible lifecycle commands; the rest are the
/// callback outcomes that move the machine out of an intermediate state.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LifecycleTrigger {
    /// `TRANSITION_CONFIGURE` (1): `unconfigured -> configuring`.
    Configure,
    /// `TRANSITION_CLEANUP` (2): `inactive -> cleaningup`.
    Cleanup,
    /// `TRANSITION_ACTIVATE` (3): `inactive -> activating`.
    Activate,
    /// `TRANSITION_DEACTIVATE` (4): `active -> deactivating`.
    Deactivate,
    /// `TRANSITION_UNCONFIGURED_SHUTDOWN` (5).
    UnconfiguredShutdown,
    /// `TRANSITION_INACTIVE_SHUTDOWN` (6).
    InactiveShutdown,
    /// `TRANSITION_ACTIVE_SHUTDOWN` (7).
    ActiveShutdown,
    /// `TRANSITION_ON_CONFIGURE_SUCCESS` (10).
    OnConfigureSuccess,
    /// `TRANSITION_ON_CONFIGURE_FAILURE` (11).
    OnConfigureFailure,
    /// `TRANSITION_ON_CONFIGURE_ERROR` (12).
    OnConfigureError,
    /// `TRANSITION_ON_CLEANUP_SUCCESS` (20).
    OnCleanupSuccess,
    /// `TRANSITION_ON_CLEANUP_FAILURE` (21).
    OnCleanupFailure,
    /// `TRANSITION_ON_CLEANUP_ERROR` (22).
    OnCleanupError,
    /// `TRANSITION_ON_ACTIVATE_SUCCESS` (30).
    OnActivateSuccess,
    /// `TRANSITION_ON_ACTIVATE_FAILURE` (31).
    OnActivateFailure,
    /// `TRANSITION_ON_ACTIVATE_ERROR` (32).
    OnActivateError,
    /// `TRANSITION_ON_DEACTIVATE_SUCCESS` (40).
    OnDeactivateSuccess,
    /// `TRANSITION_ON_DEACTIVATE_FAILURE` (41).
    OnDeactivateFailure,
    /// `TRANSITION_ON_DEACTIVATE_ERROR` (42).
    OnDeactivateError,
    /// `TRANSITION_ON_SHUTDOWN_SUCCESS` (50).
    OnShutdownSuccess,
    /// `TRANSITION_ON_SHUTDOWN_FAILURE` (51).
    OnShutdownFailure,
    /// `TRANSITION_ON_SHUTDOWN_ERROR` (52).
    OnShutdownError,
    /// `TRANSITION_ON_ERROR_SUCCESS` (60).
    OnErrorSuccess,
    /// `TRANSITION_ON_ERROR_FAILURE` (61).
    OnErrorFailure,
    /// `TRANSITION_ON_ERROR_ERROR` (62).
    OnErrorError,
    /// Any other transition id (future or custom state maps).
    Other(u8),
}

impl LifecycleTrigger {
    /// Build from a `lifecycle_msgs/msg/Transition` id.
    pub fn from_id(id: u8) -> Self {
        match id {
            1 => LifecycleTrigger::Configure,
            2 => LifecycleTrigger::Cleanup,
            3 => LifecycleTrigger::Activate,
            4 => LifecycleTrigger::Deactivate,
            5 => LifecycleTrigger::UnconfiguredShutdown,
            6 => LifecycleTrigger::InactiveShutdown,
            7 => LifecycleTrigger::ActiveShutdown,
            10 => LifecycleTrigger::OnConfigureSuccess,
            11 => LifecycleTrigger::OnConfigureFailure,
            12 => LifecycleTrigger::OnConfigureError,
            20 => LifecycleTrigger::OnCleanupSuccess,
            21 => LifecycleTrigger::OnCleanupFailure,
            22 => LifecycleTrigger::OnCleanupError,
            30 => LifecycleTrigger::OnActivateSuccess,
            31 => LifecycleTrigger::OnActivateFailure,
            32 => LifecycleTrigger::OnActivateError,
            40 => LifecycleTrigger::OnDeactivateSuccess,
            41 => LifecycleTrigger::OnDeactivateFailure,
            42 => LifecycleTrigger::OnDeactivateError,
            50 => LifecycleTrigger::OnShutdownSuccess,
            51 => LifecycleTrigger::OnShutdownFailure,
            52 => LifecycleTrigger::OnShutdownError,
            60 => LifecycleTrigger::OnErrorSuccess,
            61 => LifecycleTrigger::OnErrorFailure,
            62 => LifecycleTrigger::OnErrorError,
            other => LifecycleTrigger::Other(other),
        }
    }

    /// The numeric `lifecycle_msgs/msg/Transition` id.
    pub fn id(self) -> u8 {
        match self {
            LifecycleTrigger::Configure => 1,
            LifecycleTrigger::Cleanup => 2,
            LifecycleTrigger::Activate => 3,
            LifecycleTrigger::Deactivate => 4,
            LifecycleTrigger::UnconfiguredShutdown => 5,
            LifecycleTrigger::InactiveShutdown => 6,
            LifecycleTrigger::ActiveShutdown => 7,
            LifecycleTrigger::OnConfigureSuccess => 10,
            LifecycleTrigger::OnConfigureFailure => 11,
            LifecycleTrigger::OnConfigureError => 12,
            LifecycleTrigger::OnCleanupSuccess => 20,
            LifecycleTrigger::OnCleanupFailure => 21,
            LifecycleTrigger::OnCleanupError => 22,
            LifecycleTrigger::OnActivateSuccess => 30,
            LifecycleTrigger::OnActivateFailure => 31,
            LifecycleTrigger::OnActivateError => 32,
            LifecycleTrigger::OnDeactivateSuccess => 40,
            LifecycleTrigger::OnDeactivateFailure => 41,
            LifecycleTrigger::OnDeactivateError => 42,
            LifecycleTrigger::OnShutdownSuccess => 50,
            LifecycleTrigger::OnShutdownFailure => 51,
            LifecycleTrigger::OnShutdownError => 52,
            LifecycleTrigger::OnErrorSuccess => 60,
            LifecycleTrigger::OnErrorFailure => 61,
            LifecycleTrigger::OnErrorError => 62,
            LifecycleTrigger::Other(id) => id,
        }
    }
}

/// One edge of the lifecycle transition graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionEdge {
    /// `lifecycle_msgs/msg/Transition` id of the edge.
    pub id: u32,
    /// rcl label of the edge: external commands (`"configure"`,
    /// `"activate"`, `"shutdown"`, ...) or the generic callback labels
    /// `"transition_success"` / `"transition_failure"` / `"transition_error"`.
    pub label: String,
    /// State the edge leaves.
    pub start: LifecycleState,
    /// State the edge reaches.
    pub goal: LifecycleState,
}

impl TransitionEdge {
    /// # Safety
    /// `raw` must point to a fully initialized transition whose `start` and
    /// `goal` pointers are valid for the machine's lifetime.
    unsafe fn from_raw(raw: *const rcl_lifecycle_transition_t) -> Self {
        let t = &*raw;
        let start = if t.start.is_null() {
            LifecycleState::Unknown
        } else {
            LifecycleState::from_id((*t.start).id)
        };
        let goal = if t.goal.is_null() {
            LifecycleState::Unknown
        } else {
            LifecycleState::from_id((*t.goal).id)
        };
        TransitionEdge {
            id: t.id,
            label: c_str_to_str(t.label).to_owned(),
            start,
            goal,
        }
    }
}

/// Owned, finalized-on-drop wrapper around `rcl_lifecycle_state_machine_t`.
///
/// This wraps only the *state map*: the standard eleven states and
/// twenty-five transitions are allocated with the rcutils default allocator,
/// while rcl's communication interface is never initialized. No node, DDS
/// publisher or lifecycle service is created, so constructing and driving the
/// machine needs neither an initialized context nor any graph traffic. The
/// managed [`crate::LifecycleNode`] owns one of these and serves its lifecycle
/// services/publisher through native r2r entities, which keeps all readiness
/// inside the unified executor.
///
/// The raw rcl functions are not thread-safe; mutation is therefore
/// serialized by the managed node (the machine is always kept behind a
/// `Mutex` there). Direct standalone use must likewise synchronize concurrent
/// access.
pub struct LifecycleStateMachine {
    handle: Box<rcl_lifecycle_state_machine_t>,
}

// SAFETY: the machine contains raw pointers but owns no thread-local state,
// and every managed-node access goes through a mutex. Triggers require
// `&mut self` in standalone use, and the type stays `!Sync` so shared
// references cannot cross threads.
unsafe impl Send for LifecycleStateMachine {}

impl LifecycleStateMachine {
    /// Create the default lifecycle state machine.
    ///
    /// The machine starts in the `unconfigured` primary state. No rcl
    /// communication entities are created.
    pub fn new() -> Result<Self> {
        let mut handle = Box::new(unsafe { rcl_lifecycle_get_zero_initialized_state_machine() });
        let mut options = unsafe { rcl_lifecycle_get_default_state_machine_options() };
        // Map-only: never initialize the node-bound communication interface.
        options.enable_com_interface = false;
        handle.options = options;
        // Map-only initialization: allocate the standard states/transitions
        // without the node-bound publisher and services.
        let ret = unsafe {
            rcl_lifecycle_init_default_state_machine(handle.as_mut(), &options.allocator)
        };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        Ok(LifecycleStateMachine { handle })
    }

    /// Whether the underlying rcl state machine is initialized.
    pub fn is_initialized(&self) -> bool {
        unsafe {
            rcl_lifecycle_state_machine_is_initialized(self.handle.as_ref()) == RCL_RET_OK as i32
        }
    }

    /// The state the machine is currently in.
    pub fn current_state(&self) -> LifecycleState {
        unsafe { LifecycleState::from_id((*self.handle.current_state).id) }
    }

    /// The rcl label of the current state (`"unconfigured"`, `"active"`, ...).
    ///
    /// The returned borrow is valid for as long as the machine is borrowed.
    pub fn current_state_label(&self) -> &str {
        unsafe { c_str_to_str((*self.handle.current_state).label) }
    }

    /// Look up an edge leaving the current state by
    /// `lifecycle_msgs/msg/Transition` id.
    pub(crate) fn current_transition_by_id(&self, id: u8) -> Option<TransitionEdge> {
        let state = self.handle.current_state;
        let raw = unsafe { rcl_lifecycle_get_transition_by_id(state, id) };
        if raw.is_null() {
            None
        } else {
            Some(unsafe { TransitionEdge::from_raw(raw) })
        }
    }

    /// Look up an edge leaving the current state by its rcl label.
    pub(crate) fn current_transition_by_label(&self, label: &str) -> Option<TransitionEdge> {
        let c_label = CString::new(label).ok()?;
        let state = self.handle.current_state;
        let raw = unsafe { rcl_lifecycle_get_transition_by_label(state, c_label.as_ptr()) };
        if raw.is_null() {
            None
        } else {
            Some(unsafe { TransitionEdge::from_raw(raw) })
        }
    }

    /// Transitions registered as directly triggerable from the current state.
    pub fn transitions_from_current(&self) -> Vec<TransitionEdge> {
        let state = self.handle.current_state;
        unsafe {
            // The finalized state (and the registered `unknown` state) have
            // no outgoing transitions: the array pointer is NULL.
            if (*state).valid_transitions.is_null() || (*state).valid_transition_size == 0 {
                return Vec::new();
            }
            // rcl stores the per-state transitions as an array of structs.
            let edges = slice::from_raw_parts(
                (*state).valid_transitions as *const rcl_lifecycle_transition_t,
                (*state).valid_transition_size as usize,
            );
            edges
                .iter()
                .map(|t| TransitionEdge::from_raw(t as *const _))
                .collect()
        }
    }

    /// Every state registered in the transition map.
    pub fn states(&self) -> Vec<LifecycleState> {
        let map = &self.handle.transition_map;
        unsafe {
            // rcl stores the registered states as an array of structs.
            let states = slice::from_raw_parts(
                map.states as *const rcl_lifecycle_state_t,
                map.states_size as usize,
            );
            states
                .iter()
                .map(|s| LifecycleState::from_id(s.id))
                .collect()
        }
    }

    /// The full transition graph (all registered edges), keyed independently
    /// of the current state.
    pub fn transition_graph(&self) -> Vec<TransitionEdge> {
        let map = &self.handle.transition_map;
        unsafe {
            let edges = slice::from_raw_parts(
                map.transitions as *const rcl_lifecycle_transition_t,
                map.transitions_size as usize,
            );
            edges
                .iter()
                .map(|t| TransitionEdge::from_raw(t as *const _))
                .collect()
        }
    }

    /// Apply a typed trigger to the machine and return the resulting state.
    ///
    /// Returns an error if the trigger is not valid from the current state
    /// (typically `RCL_RET_NOT_FOUND`).
    pub fn trigger(&mut self, trigger: LifecycleTrigger) -> Result<LifecycleState> {
        self.trigger_id(trigger.id())
    }

    /// Apply a trigger by its `lifecycle_msgs/msg/Transition` id.
    pub fn trigger_id(&mut self, id: u8) -> Result<LifecycleState> {
        let ret =
            unsafe { rcl_lifecycle_trigger_transition_by_id(self.handle.as_mut(), id, false) };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        Ok(self.current_state())
    }

    /// Apply a trigger looked up by its rcl label. External commands use
    /// `"configure"`, `"cleanup"`, `"activate"`, `"deactivate"` and
    /// `"shutdown"`; callback outcomes use the generic rcl labels
    /// `"transition_success"`, `"transition_failure"` and
    /// `"transition_error"` (resolved within the current state).
    pub fn trigger_label(&mut self, label: &str) -> Result<LifecycleState> {
        let c_label = CString::new(label).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let ret = unsafe {
            rcl_lifecycle_trigger_transition_by_label(self.handle.as_mut(), c_label.as_ptr(), false)
        };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        Ok(self.current_state())
    }
}

impl std::fmt::Debug for LifecycleStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LifecycleStateMachine")
            .field("initialized", &self.is_initialized())
            .field("current_state", &self.current_state())
            .finish()
    }
}

impl Drop for LifecycleStateMachine {
    fn drop(&mut self) {
        // Map-only machines never initialized the communication interface;
        // free just the registered states/transitions.
        let ret = unsafe {
            rcl_lifecycle_transition_map_fini(
                &mut self.handle.transition_map,
                &self.handle.options.allocator,
            )
        };
        if ret != RCL_RET_OK as i32 {
            log::error!("failed to finalize lifecycle transition map (rcl ret {})", ret);
        }
    }
}

/// View a NUL-terminated rcl string for the lifetime of the machine that owns
/// it.
///
/// # Safety
/// `ptr` must reference a valid, NUL-terminated C string for the borrowed
/// lifetime chosen by the caller.
unsafe fn c_str_to_str<'a>(ptr: *const c_char) -> &'a str {
    if ptr.is_null() {
        ""
    } else {
        CStr::from_ptr(ptr).to_str().unwrap_or("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle_msgs;

    #[test]
    fn standard_ids_match_lifecycle_msgs_constants() {
        use lifecycle_msgs::msg::{State, Transition};
        assert_eq!(LifecycleState::Unknown.id() as u32, State::PRIMARY_STATE_UNKNOWN as u32);
        assert_eq!(
            LifecycleState::Unconfigured.id() as u32,
            State::PRIMARY_STATE_UNCONFIGURED as u32
        );
        assert_eq!(LifecycleState::Inactive.id() as u32, State::PRIMARY_STATE_INACTIVE as u32);
        assert_eq!(LifecycleState::Active.id() as u32, State::PRIMARY_STATE_ACTIVE as u32);
        assert_eq!(LifecycleState::Finalized.id() as u32, State::PRIMARY_STATE_FINALIZED as u32);
        assert_eq!(
            LifecycleState::Configuring.id() as u32,
            State::TRANSITION_STATE_CONFIGURING as u32
        );
        assert_eq!(
            LifecycleState::CleaningUp.id() as u32,
            State::TRANSITION_STATE_CLEANINGUP as u32
        );
        assert_eq!(
            LifecycleState::ShuttingDown.id() as u32,
            State::TRANSITION_STATE_SHUTTINGDOWN as u32
        );
        assert_eq!(
            LifecycleState::Activating.id() as u32,
            State::TRANSITION_STATE_ACTIVATING as u32
        );
        assert_eq!(
            LifecycleState::Deactivating.id() as u32,
            State::TRANSITION_STATE_DEACTIVATING as u32
        );
        assert_eq!(
            LifecycleState::ErrorProcessing.id() as u32,
            State::TRANSITION_STATE_ERRORPROCESSING as u32
        );

        assert_eq!(
            LifecycleTrigger::Configure.id() as u32,
            Transition::TRANSITION_CONFIGURE as u32
        );
        assert_eq!(LifecycleTrigger::Cleanup.id() as u32, Transition::TRANSITION_CLEANUP as u32);
        assert_eq!(LifecycleTrigger::Activate.id() as u32, Transition::TRANSITION_ACTIVATE as u32);
        assert_eq!(
            LifecycleTrigger::Deactivate.id() as u32,
            Transition::TRANSITION_DEACTIVATE as u32
        );
        for (trigger, expected) in [
            (
                LifecycleTrigger::UnconfiguredShutdown,
                Transition::TRANSITION_UNCONFIGURED_SHUTDOWN as u32,
            ),
            (
                LifecycleTrigger::InactiveShutdown,
                Transition::TRANSITION_INACTIVE_SHUTDOWN as u32,
            ),
            (LifecycleTrigger::ActiveShutdown, Transition::TRANSITION_ACTIVE_SHUTDOWN as u32),
            (
                LifecycleTrigger::OnConfigureSuccess,
                Transition::TRANSITION_ON_CONFIGURE_SUCCESS as u32,
            ),
            (
                LifecycleTrigger::OnConfigureFailure,
                Transition::TRANSITION_ON_CONFIGURE_FAILURE as u32,
            ),
            (
                LifecycleTrigger::OnConfigureError,
                Transition::TRANSITION_ON_CONFIGURE_ERROR as u32,
            ),
            (
                LifecycleTrigger::OnCleanupSuccess,
                Transition::TRANSITION_ON_CLEANUP_SUCCESS as u32,
            ),
            (
                LifecycleTrigger::OnCleanupFailure,
                Transition::TRANSITION_ON_CLEANUP_FAILURE as u32,
            ),
            (LifecycleTrigger::OnCleanupError, Transition::TRANSITION_ON_CLEANUP_ERROR as u32),
            (
                LifecycleTrigger::OnActivateSuccess,
                Transition::TRANSITION_ON_ACTIVATE_SUCCESS as u32,
            ),
            (
                LifecycleTrigger::OnActivateFailure,
                Transition::TRANSITION_ON_ACTIVATE_FAILURE as u32,
            ),
            (
                LifecycleTrigger::OnActivateError,
                Transition::TRANSITION_ON_ACTIVATE_ERROR as u32,
            ),
            (
                LifecycleTrigger::OnDeactivateSuccess,
                Transition::TRANSITION_ON_DEACTIVATE_SUCCESS as u32,
            ),
            (
                LifecycleTrigger::OnDeactivateFailure,
                Transition::TRANSITION_ON_DEACTIVATE_FAILURE as u32,
            ),
            (
                LifecycleTrigger::OnDeactivateError,
                Transition::TRANSITION_ON_DEACTIVATE_ERROR as u32,
            ),
            (
                LifecycleTrigger::OnShutdownSuccess,
                Transition::TRANSITION_ON_SHUTDOWN_SUCCESS as u32,
            ),
            (
                LifecycleTrigger::OnShutdownFailure,
                Transition::TRANSITION_ON_SHUTDOWN_FAILURE as u32,
            ),
            (
                LifecycleTrigger::OnShutdownError,
                Transition::TRANSITION_ON_SHUTDOWN_ERROR as u32,
            ),
            (LifecycleTrigger::OnErrorSuccess, Transition::TRANSITION_ON_ERROR_SUCCESS as u32),
            (LifecycleTrigger::OnErrorFailure, Transition::TRANSITION_ON_ERROR_FAILURE as u32),
            (LifecycleTrigger::OnErrorError, Transition::TRANSITION_ON_ERROR_ERROR as u32),
        ] {
            assert_eq!(trigger.id() as u32, expected, "{:?}", trigger);
            assert_eq!(LifecycleTrigger::from_id(trigger.id()), trigger);
        }
    }

    #[test]
    fn starts_unconfigured_and_reports_default_map() {
        let sm = LifecycleStateMachine::new().unwrap();
        assert!(sm.is_initialized());
        assert_eq!(sm.current_state(), LifecycleState::Unconfigured);
        assert_eq!(sm.current_state_label(), "unconfigured");

        // rcl registers `unknown` (0) first; it is unreachable from the graph.
        let states: Vec<u8> = sm.states().iter().map(|s| s.id()).collect();
        assert_eq!(states, vec![0, 1, 2, 3, 4, 10, 11, 12, 13, 14, 15]);

        let graph = sm.transition_graph();
        assert_eq!(graph.len(), 25, "default rcl map has 25 edges");
        assert_eq!(
            graph
                .iter()
                .filter(|e| e.start == LifecycleState::Configuring)
                .count(),
            3
        );
        // success/failure finalize; shutdown error re-enters error processing.
        assert_eq!(
            graph
                .iter()
                .filter(|e| e.goal == LifecycleState::Finalized)
                .count(),
            4
        );
    }

    #[test]
    fn available_transitions_follow_primary_states() {
        let mut sm = LifecycleStateMachine::new().unwrap();

        let unconfigured: Vec<u8> = sm
            .transitions_from_current()
            .iter()
            .map(|t| t.id as u8)
            .collect();
        assert_eq!(unconfigured, vec![1, 5]);

        // configure (success callback) reaches inactive.
        assert_eq!(sm.trigger(LifecycleTrigger::Configure).unwrap(), LifecycleState::Configuring);
        assert_eq!(sm.current_state_label(), "configuring");
        let configuring: Vec<u8> = sm
            .transitions_from_current()
            .iter()
            .map(|t| t.id as u8)
            .collect();
        assert_eq!(configuring, vec![10, 11, 12]);
        assert_eq!(
            sm.trigger(LifecycleTrigger::OnConfigureSuccess).unwrap(),
            LifecycleState::Inactive
        );

        let inactive: Vec<u8> = sm
            .transitions_from_current()
            .iter()
            .map(|t| t.id as u8)
            .collect();
        assert_eq!(inactive, vec![2, 3, 6]);

        // activate (success) reaches active.
        assert_eq!(sm.trigger(LifecycleTrigger::Activate).unwrap(), LifecycleState::Activating);
        assert_eq!(
            sm.trigger(LifecycleTrigger::OnActivateSuccess).unwrap(),
            LifecycleState::Active
        );
        let active: Vec<u8> = sm
            .transitions_from_current()
            .iter()
            .map(|t| t.id as u8)
            .collect();
        assert_eq!(active, vec![4, 7]);
    }

    #[test]
    fn full_happy_path_reaches_finalized() {
        let mut sm = LifecycleStateMachine::new().unwrap();

        // rcl labels the external commands and the generic callback labels.
        assert_eq!(sm.trigger_label("configure").unwrap(), LifecycleState::Configuring);
        assert_eq!(sm.trigger_label("transition_success").unwrap(), LifecycleState::Inactive);
        assert_eq!(sm.trigger(LifecycleTrigger::Activate).unwrap(), LifecycleState::Activating);
        assert_eq!(
            sm.trigger(LifecycleTrigger::OnActivateSuccess).unwrap(),
            LifecycleState::Active
        );
        assert_eq!(sm.trigger(LifecycleTrigger::Deactivate).unwrap(), LifecycleState::Deactivating);
        assert_eq!(
            sm.trigger(LifecycleTrigger::OnDeactivateSuccess).unwrap(),
            LifecycleState::Inactive
        );
        assert_eq!(sm.trigger(LifecycleTrigger::Cleanup).unwrap(), LifecycleState::CleaningUp);
        assert_eq!(
            sm.trigger(LifecycleTrigger::OnCleanupSuccess).unwrap(),
            LifecycleState::Unconfigured
        );
        assert_eq!(
            sm.trigger(LifecycleTrigger::UnconfiguredShutdown).unwrap(),
            LifecycleState::ShuttingDown
        );
        assert_eq!(
            sm.trigger(LifecycleTrigger::OnShutdownSuccess).unwrap(),
            LifecycleState::Finalized
        );
        assert!(sm.transitions_from_current().is_empty());
        assert!(sm.trigger(LifecycleTrigger::Configure).is_err());
        assert!(sm.trigger_label("configure").is_err());
    }

    #[test]
    fn invalid_triggers_from_current_state_are_errors() {
        let mut sm = LifecycleStateMachine::new().unwrap();
        // activate is only valid from inactive.
        assert!(sm.trigger(LifecycleTrigger::Activate).is_err());
        assert!(sm.trigger_id(200).is_err());
        assert!(sm.trigger_label("not_a_real_transition").is_err());
        // interior NUL cannot be a label.
        assert!(sm.trigger_label("con\0figure").is_err());
        // machine stays in unconfigured after all rejected triggers.
        assert_eq!(sm.current_state(), LifecycleState::Unconfigured);
    }

    #[test]
    fn configure_failure_returns_to_unconfigured() {
        let mut sm = LifecycleStateMachine::new().unwrap();
        sm.trigger(LifecycleTrigger::Configure).unwrap();
        assert_eq!(
            sm.trigger(LifecycleTrigger::OnConfigureFailure).unwrap(),
            LifecycleState::Unconfigured
        );
    }

    #[test]
    fn cleanup_and_activate_failures_return_to_inactive() {
        let mut sm = LifecycleStateMachine::new().unwrap();
        sm.trigger(LifecycleTrigger::Configure).unwrap();
        sm.trigger(LifecycleTrigger::OnConfigureSuccess).unwrap();

        sm.trigger(LifecycleTrigger::Activate).unwrap();
        assert_eq!(
            sm.trigger(LifecycleTrigger::OnActivateFailure).unwrap(),
            LifecycleState::Inactive
        );

        sm.trigger(LifecycleTrigger::Cleanup).unwrap();
        assert_eq!(
            sm.trigger(LifecycleTrigger::OnCleanupFailure).unwrap(),
            LifecycleState::Inactive
        );
    }

    #[test]
    fn deactivate_failure_returns_to_active() {
        let mut sm = LifecycleStateMachine::new().unwrap();
        sm.trigger(LifecycleTrigger::Configure).unwrap();
        sm.trigger(LifecycleTrigger::OnConfigureSuccess).unwrap();
        sm.trigger(LifecycleTrigger::Activate).unwrap();
        sm.trigger(LifecycleTrigger::OnActivateSuccess).unwrap();

        sm.trigger(LifecycleTrigger::Deactivate).unwrap();
        assert_eq!(
            sm.trigger(LifecycleTrigger::OnDeactivateFailure).unwrap(),
            LifecycleState::Active
        );
    }

    #[test]
    fn error_recovery_and_fatal_paths() {
        let mut sm = LifecycleStateMachine::new().unwrap();

        // configure error -> errorprocessing -> success -> unconfigured.
        sm.trigger(LifecycleTrigger::Configure).unwrap();
        sm.trigger(LifecycleTrigger::OnConfigureError).unwrap();
        assert_eq!(sm.current_state(), LifecycleState::ErrorProcessing);
        let edges: Vec<u8> = sm
            .transitions_from_current()
            .iter()
            .map(|t| t.id as u8)
            .collect();
        assert_eq!(edges, vec![60, 61, 62]);
        assert_eq!(
            sm.trigger(LifecycleTrigger::OnErrorSuccess).unwrap(),
            LifecycleState::Unconfigured
        );

        // A second error that cannot be handled finalizes the node.
        sm.trigger(LifecycleTrigger::Configure).unwrap();
        sm.trigger(LifecycleTrigger::OnConfigureError).unwrap();
        assert_eq!(
            sm.trigger(LifecycleTrigger::OnErrorFailure).unwrap(),
            LifecycleState::Finalized
        );
    }

    #[test]
    fn shutdown_failure_still_finalizes_from_each_primary_state() {
        for trigger in [
            LifecycleTrigger::UnconfiguredShutdown,
            LifecycleTrigger::InactiveShutdown,
            LifecycleTrigger::ActiveShutdown,
        ] {
            let mut sm = LifecycleStateMachine::new().unwrap();
            if trigger != LifecycleTrigger::UnconfiguredShutdown {
                sm.trigger(LifecycleTrigger::Configure).unwrap();
                sm.trigger(LifecycleTrigger::OnConfigureSuccess).unwrap();
                if trigger == LifecycleTrigger::ActiveShutdown {
                    sm.trigger(LifecycleTrigger::Activate).unwrap();
                    sm.trigger(LifecycleTrigger::OnActivateSuccess).unwrap();
                }
            }
            sm.trigger(trigger).unwrap();
            assert_eq!(
                sm.trigger(LifecycleTrigger::OnShutdownFailure).unwrap(),
                LifecycleState::Finalized,
                "shutdown failure from {:?}",
                trigger
            );
        }
    }

    #[test]
    fn graph_edges_match_standard_map() {
        let sm = LifecycleStateMachine::new().unwrap();
        let graph = sm.transition_graph();

        let expected: &[(u32, LifecycleState, LifecycleState)] = &[
            (1, LifecycleState::Unconfigured, LifecycleState::Configuring),
            (2, LifecycleState::Inactive, LifecycleState::CleaningUp),
            (3, LifecycleState::Inactive, LifecycleState::Activating),
            (4, LifecycleState::Active, LifecycleState::Deactivating),
            (5, LifecycleState::Unconfigured, LifecycleState::ShuttingDown),
            (6, LifecycleState::Inactive, LifecycleState::ShuttingDown),
            (7, LifecycleState::Active, LifecycleState::ShuttingDown),
            (10, LifecycleState::Configuring, LifecycleState::Inactive),
            (11, LifecycleState::Configuring, LifecycleState::Unconfigured),
            (12, LifecycleState::Configuring, LifecycleState::ErrorProcessing),
            (20, LifecycleState::CleaningUp, LifecycleState::Unconfigured),
            (21, LifecycleState::CleaningUp, LifecycleState::Inactive),
            (22, LifecycleState::CleaningUp, LifecycleState::ErrorProcessing),
            (30, LifecycleState::Activating, LifecycleState::Active),
            (31, LifecycleState::Activating, LifecycleState::Inactive),
            (32, LifecycleState::Activating, LifecycleState::ErrorProcessing),
            (40, LifecycleState::Deactivating, LifecycleState::Inactive),
            (41, LifecycleState::Deactivating, LifecycleState::Active),
            (42, LifecycleState::Deactivating, LifecycleState::ErrorProcessing),
            (50, LifecycleState::ShuttingDown, LifecycleState::Finalized),
            (51, LifecycleState::ShuttingDown, LifecycleState::Finalized),
            // rcl routes a shutdown *error* back into error processing.
            (52, LifecycleState::ShuttingDown, LifecycleState::ErrorProcessing),
            (60, LifecycleState::ErrorProcessing, LifecycleState::Unconfigured),
            (61, LifecycleState::ErrorProcessing, LifecycleState::Finalized),
            (62, LifecycleState::ErrorProcessing, LifecycleState::Finalized),
        ];

        for (id, start, goal) in expected {
            let edge = graph
                .iter()
                .find(|e| e.id == *id)
                .unwrap_or_else(|| panic!("missing transition {id}"));
            assert_eq!(edge.start, *start, "transition {id} start");
            assert_eq!(edge.goal, *goal, "transition {id} goal");
            assert!(!edge.label.is_empty());
        }
    }

    #[test]
    fn machines_can_be_created_and_finalized_repeatedly() {
        for _ in 0..3 {
            let mut sm = LifecycleStateMachine::new().unwrap();
            assert_eq!(sm.current_state(), LifecycleState::Unconfigured);
            sm.trigger(LifecycleTrigger::Configure).unwrap();
            sm.trigger(LifecycleTrigger::OnConfigureSuccess).unwrap();
            assert_eq!(sm.current_state(), LifecycleState::Inactive);
            // drop finalizes the machine before the next one is created
        }
        let again = LifecycleStateMachine::new().unwrap();
        assert_eq!(again.current_state(), LifecycleState::Unconfigured);
    }
}
