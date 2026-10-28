//! Easy to use, runtime-agnostic async rust bindings for ROS2.
//! ---
//!
//! Minimal bindings for ROS2 that do *not* require hooking in to the
//! ROS2 build infrastructure -- `cargo build` is all you
//! need. Convenience Rust types are created by calling into the c
//! introspection libraries. This circumvents the ROS2 .msg/.idl
//! pipeline by relying on already generated C code. By default, the
//! behavior is to build bindings to the RCL and all message types
//! that can be found in the currently sourced ros environment.
//!
//! What works?
//!---
//!- ROS 2 Humble, Iron, Jazzy, Kilted, Lyrical and Rolling
//!- Building Rust types
//!- Publish/subscribe
//!- Services
//!- Actions
//!- Parameter handling
//!
//! ---
//!
//! After having sourced ROS2 (see README for more details), you can
//! try the following example:
//!
//! ``` rust
//! use futures::{executor::LocalPool, future, stream::StreamExt, task::LocalSpawnExt};
//! use r2r::QosProfile;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let ctx = r2r::Context::create()?;
//!     let mut node = r2r::Node::create(ctx, "node", "namespace")?;
//!     let subscriber =
//!         node.subscribe::<r2r::std_msgs::msg::String>("/topic", QosProfile::default())?;
//!     let publisher =
//!         node.create_publisher::<r2r::std_msgs::msg::String>("/topic", QosProfile::default())?;
//!     let mut timer = node.create_wall_timer(std::time::Duration::from_millis(1000))?;
//!
//!     // Set up a simple task executor.
//!     let mut pool = LocalPool::new();
//!     let spawner = pool.spawner();
//!
//!     // Run the subscriber in one task, printing the messages
//!     spawner.spawn_local(async move {
//!         subscriber
//!             .for_each(|msg| {
//!                 println!("got new msg: {}", msg.data);
//!                 future::ready(())
//!             })
//!             .await
//!     })?;
//!
//!     // Run the publisher in another task
//!     spawner.spawn_local(async move {
//!         let mut counter = 0;
//!         loop {
//!             let _elapsed = timer.tick().await.unwrap();
//!             let msg = r2r::std_msgs::msg::String {
//!                 data: format!("Hello, world! ({})", counter),
//!             };
//!             publisher.publish(&msg).unwrap();
//!             counter += 1;
//!         }
//!     })?;
//!
//!     // Main loop spins ros.
//!     loop {
//!         node.spin_once(std::time::Duration::from_millis(100));
//!         pool.run_until_stalled();
//!     }
//! }
//! ```

// otherwise crates using r2r needs to specify the same version of indexmap and uuid as
// this crate depend on, which seem like bad user experience.
pub extern crate indexmap;
pub extern crate parking_lot;
pub extern crate uuid;

mod error;
pub use error::{Error, Result};

mod msg_types;
mod serialization;
pub use msg_types::{
    generated_msgs::*, WrappedActionTypeSupport, WrappedNativeMsg as NativeMsg,
    WrappedNativeMsgUntyped, WrappedServiceTypeSupport, WrappedTypesupport,
};
pub use serialization::MessageSerializer;

mod utils;
pub use utils::*;

mod subscribers;
pub use subscribers::ContentFilterOptions;
pub use subscribers::{
    LivelinessChangedStatus, LoanedSubscriptionMessage, MessageInfo, MessageLostStatus,
    ReceivedMessage, RequestedDeadlineMissedStatus, RequestedIncompatibleQosStatus, Subscription,
    SubscriptionOptions,
};

mod publishers;
pub use publishers::{
    IncompatibleTypeStatus, LivelinessLostStatus, LoanedMessage, MatchedStatus,
    NetworkFlowEndpoint, OfferedDeadlineMissedStatus, OfferedIncompatibleQosStatus, Publisher,
    PublisherGid, PublisherOptions, PublisherUntyped, UniqueNetworkFlowRequirement,
};

#[cfg(any(
    r2r__ros__distro__iron,
    r2r__ros__distro__jazzy,
    r2r__ros__distro__kilted,
    r2r__ros__distro__lyrical,
    r2r__ros__distro__rolling
))]
mod service_introspection;
mod services;
#[cfg(any(
    r2r__ros__distro__iron,
    r2r__ros__distro__jazzy,
    r2r__ros__distro__kilted,
    r2r__ros__distro__lyrical,
    r2r__ros__distro__rolling
))]
pub use service_introspection::ServiceIntrospectionState;
pub use services::{
    Service, ServiceOptions, ServiceRequest, ServiceRequestInfo, ServiceRequestUntyped,
    ServiceUntyped,
};

mod availability;
mod clients;
pub use clients::ClientGid;
pub use clients::{Client, ClientUntyped, ServiceResponse, ServiceResponseInfo};

mod parameter_clients;
pub use parameter_clients::ParameterClient;

mod action_common;
pub use action_common::{ActionClientOptions, ActionGoalOptions, ActionServerOptions, GoalStatus};

mod action_clients;
pub use action_clients::{
    ActionClient, ActionClientGoal, ActionClientGoalResponse, ActionFeedbackStream,
    ActionResultFuture,
};

mod action_clients_untyped;
pub use action_clients_untyped::{
    ActionClientGoalResponseUntyped, ActionClientGoalUntyped, ActionClientUntyped,
    ActionFeedbackStreamUntyped, ActionResultFutureUntyped,
};

mod action_servers;
pub use action_servers::{
    ActionServer, ActionServerCancelRequest, ActionServerCancelStreamUntyped, ActionServerGoal,
    ActionServerGoalRequest, ActionServerGoalRequestUntyped, ActionServerGoalUntyped,
    ActionServerUntyped,
};

mod context;
pub use context::{Context, ContextOptions};

mod parameters;
pub use parameters::{
    FloatingPointRange, IntegerRange, Parameter, ParameterDeclarationOptions, ParameterDescriptor,
    ParameterEventChange, ParameterEventKind, ParameterType, ParameterValue, RosParams,
    WrongParameterType,
};

pub use r2r_macros::RosParams;

mod clocks;
pub use clocks::{
    Clock, ClockChange, ClockType, TimeJump, TimeJumpCallbackId, TimeJumpPhase, TimeJumpThreshold,
};

mod nodes;
#[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
pub use nodes::TimerCallInfo;
pub use nodes::{
    Executor, ExecutorHandle, ExecutorNodeId, FutureExecutionOptions, Node, NodeOptions,
    ParameterCallbackHandle, ParameterHandlerOptions, ParameterPostSetCallbackHandle,
    ParameterPreSetCallbackHandle, Timer, TimerOptions, TopicEndpointInfo, TopicEndpointType,
    TopicTypeHash,
};

// The checked-in bindings used by the doc-only (docs.rs) build predate the
// lifecycle FFI symbols; lifecycle is documented from native builds.
#[cfg(not(feature = "doc-only"))]
mod lifecycle;
#[cfg(not(feature = "doc-only"))]
pub use lifecycle::{
    CallbackReturn, LifecycleCallback, LifecycleNode, LifecyclePublisher, LifecycleState,
    LifecycleStateMachine, LifecycleTrigger, TransitionEdge,
};

pub mod qos;
mod strings;
pub use strings::{RosString, RosWString};

#[cfg(r2r__rosgraph_msgs__msg__Clock)]
mod time_source;
#[cfg(r2r__rosgraph_msgs__msg__Clock)]
pub use time_source::TimeSource;

pub use qos::{
    qos_check_compatible, QosCompatibility, QosCompatibilityResult, QosPolicyKind, QosProfile,
};

/// The ros version currently built against.
#[cfg(r2r__ros__distro__humble)]
pub const ROS_DISTRO: &str = "humble";
#[cfg(r2r__ros__distro__iron)]
pub const ROS_DISTRO: &str = "iron";
#[cfg(r2r__ros__distro__jazzy)]
pub const ROS_DISTRO: &str = "jazzy";
#[cfg(r2r__ros__distro__kilted)]
pub const ROS_DISTRO: &str = "kilted";
#[cfg(r2r__ros__distro__lyrical)]
pub const ROS_DISTRO: &str = "lyrical";

#[cfg(r2r__ros__distro__rolling)]
pub const ROS_DISTRO: &str = "rolling";

#[cfg(not(any(
    r2r__ros__distro__humble,
    r2r__ros__distro__iron,
    r2r__ros__distro__jazzy,
    r2r__ros__distro__kilted,
    r2r__ros__distro__lyrical,
    r2r__ros__distro__rolling
)))]
pub const ROS_DISTRO: &str = "unknown";
