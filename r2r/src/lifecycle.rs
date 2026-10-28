//! Native ROS 2 lifecycle (managed node) support.
//!
//! - [`LifecycleStateMachine`] is a safe owned wrapper over
//!   `rcl_lifecycle_state_machine_t` holding the standard transition map
//!   (`unconfigured` / `inactive` / `active` / `finalized` plus the
//!   intermediate `configuring`, `cleaningup`, `activating`, `deactivating`,
//!   `shuttingdown`, `errorprocessing` states). It is map-only: no node,
//!   publisher or services are created.
//! - [`LifecycleNode`] owns a regular [`Node`] plus one such machine and
//!   serves the five standard lifecycle services and the transition-event
//!   publisher as native r2r entities, so all readiness is handled by the
//!   unified executor.
//!
//! The five built-in rcl lifecycle services are never created (rcl runs in
//! publisher-less map-only mode): rcl entities created here would be
//! invisible to the r2r executor. Instead the managed node drives its machine
//! from native r2r services with `rclcpp_lifecycle` transition semantics.

pub mod node;
pub mod services;
pub mod state_machine;

pub use node::{CallbackReturn, LifecycleCallback, LifecycleNode};
pub use state_machine::{LifecycleState, LifecycleStateMachine, LifecycleTrigger, TransitionEdge};
