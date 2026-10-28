//! Native ROS 2 lifecycle (managed node) support.
//!
//! This module exposes the rcl lifecycle state machine directly, without the
//! hidden service/publisher machinery that `rclcpp` wires up internally.
//!
//! - [`LifecycleStateMachine`] is a safe, owned wrapper over
//!   `rcl_lifecycle_state_machine_t`. It owns the standard transition map
//!   (`unconfigured` / `inactive` / `active` / `finalized` plus the
//!   intermediate `configuring`, `cleaningup`, `activating`, `deactivating`,
//!   `shuttingdown`, `errorprocessing` states) and can be attached to any
//!   [`Node`]. rcl still creates its transition-event publisher, but driving
//!   the machine through the wrapper never publishes.
//! - State ids and transition ids match the `lifecycle_msgs/msg/State` and
//!   `lifecycle_msgs/msg/Transition` constants.
//!
//! The five built-in rcl lifecycle services are deliberately not created
//! (rcl runs in publisher-only mode): they would be plain rcl services
//! invisible to the r2r executor. The higher-level managed node support
//! instead drives this machine from native r2r services so that all readiness
//! is handled by the unified executor.

pub mod state_machine;

pub use state_machine::{LifecycleState, LifecycleStateMachine, LifecycleTrigger, TransitionEdge};
