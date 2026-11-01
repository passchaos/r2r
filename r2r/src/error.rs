#![allow(non_camel_case_types)]
#[cfg(any(
    r2r__ros__distro__kilted,
    r2r__ros__distro__lyrical,
    r2r__ros__distro__rolling
))]
use r2r_rcl::RCL_RET_ACTION_NOT_TERMINATED_YET as RCL_ACTION_RET_NOT_TERMINATED_YET;
use r2r_rcl::*;
use thiserror::Error;

/// r2r Result type.
pub type Result<T> = std::result::Result<T, Error>;

/// r2r Error type.
///
/// These values are mostly copied straight from the RCL headers, but
/// some are specific to r2r, such as `GoalCancelRejected` which does
/// not have an analogue in the rcl.
#[derive(Error, Clone, Debug)]
pub enum Error {
    #[error("ROS sleep canceled")]
    RosSleepCancelled,
    #[error("ROS time source changed while sleeping")]
    RosTimeSourceChanged,
    #[error("invalid RIHS type hash: {value}")]
    InvalidTypeHash { value: String },
    #[error("type description query failed: {reason}")]
    TypeDescriptionUnavailable { reason: String },
    #[error("type description identity mismatch: expected {expected}, received {actual}")]
    TypeDescriptionIdentityMismatch { expected: String, actual: String },
    #[error("this endpoint event already has an observer; drop its stream and spin cleanup before taking or observing it again")]
    QosEventAlreadyObserved,
    #[error("this timer already has a reset observer; drop it before creating another")]
    TimerResetAlreadyObserved,
    #[error("RCL reported success where an error was expected (RCL_RET_OK)")]
    RCL_RET_OK,
    #[error("unspecified RCL error (RCL_RET_ERROR)")]
    RCL_RET_ERROR,
    #[error("operation timed out (RCL_RET_TIMEOUT)")]
    RCL_RET_TIMEOUT,
    #[error("native allocation failed (RCL_RET_BAD_ALLOC)")]
    RCL_RET_BAD_ALLOC,
    #[error("invalid argument (RCL_RET_INVALID_ARGUMENT)")]
    RCL_RET_INVALID_ARGUMENT,
    #[error("operation unsupported by this ROS distribution or RMW (RCL_RET_UNSUPPORTED)")]
    RCL_RET_UNSUPPORTED,
    #[error("context is already initialized (RCL_RET_ALREADY_INIT)")]
    RCL_RET_ALREADY_INIT,
    #[error("context is not initialized (RCL_RET_NOT_INIT)")]
    RCL_RET_NOT_INIT,
    #[error("RMW implementations do not match (RCL_RET_MISMATCHED_RMW_ID)")]
    RCL_RET_MISMATCHED_RMW_ID,
    #[error("incorrect RMW implementation (RMW_RET_INCORRECT_RMW_IMPLEMENTATION)")]
    RMW_RET_INCORRECT_RMW_IMPLEMENTATION,
    #[error("invalid topic name (RCL_RET_TOPIC_NAME_INVALID)")]
    RCL_RET_TOPIC_NAME_INVALID,
    #[error("invalid service name (RCL_RET_SERVICE_NAME_INVALID)")]
    RCL_RET_SERVICE_NAME_INVALID,
    #[error("unknown name substitution (RCL_RET_UNKNOWN_SUBSTITUTION)")]
    RCL_RET_UNKNOWN_SUBSTITUTION,
    #[error("context is already shut down (RCL_RET_ALREADY_SHUTDOWN)")]
    RCL_RET_ALREADY_SHUTDOWN,
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    #[error("entity not found (RCL_RET_NOT_FOUND)")]
    RCL_RET_NOT_FOUND,
    #[error("node handle is invalid (RCL_RET_NODE_INVALID)")]
    RCL_RET_NODE_INVALID,
    #[error("node name is invalid (RCL_RET_NODE_INVALID_NAME)")]
    RCL_RET_NODE_INVALID_NAME,
    #[error("node namespace is invalid (RCL_RET_NODE_INVALID_NAMESPACE)")]
    RCL_RET_NODE_INVALID_NAMESPACE,
    #[error("node name does not exist (RCL_RET_NODE_NAME_NON_EXISTENT)")]
    RCL_RET_NODE_NAME_NON_EXISTENT,
    #[error("publisher handle is invalid (RCL_RET_PUBLISHER_INVALID)")]
    RCL_RET_PUBLISHER_INVALID,
    #[error("subscription handle is invalid (RCL_RET_SUBSCRIPTION_INVALID)")]
    RCL_RET_SUBSCRIPTION_INVALID,
    #[error("subscription take failed (RCL_RET_SUBSCRIPTION_TAKE_FAILED)")]
    RCL_RET_SUBSCRIPTION_TAKE_FAILED,
    #[error("client handle is invalid (RCL_RET_CLIENT_INVALID)")]
    RCL_RET_CLIENT_INVALID,
    #[error("client take failed (RCL_RET_CLIENT_TAKE_FAILED)")]
    RCL_RET_CLIENT_TAKE_FAILED,
    #[error("service handle is invalid (RCL_RET_SERVICE_INVALID)")]
    RCL_RET_SERVICE_INVALID,
    #[error("service take failed (RCL_RET_SERVICE_TAKE_FAILED)")]
    RCL_RET_SERVICE_TAKE_FAILED,
    #[error("timer handle is invalid (RCL_RET_TIMER_INVALID)")]
    RCL_RET_TIMER_INVALID,
    #[error("timer is canceled (RCL_RET_TIMER_CANCELED)")]
    RCL_RET_TIMER_CANCELED,
    #[error("wait set is invalid (RCL_RET_WAIT_SET_INVALID)")]
    RCL_RET_WAIT_SET_INVALID,
    #[error("wait set is empty (RCL_RET_WAIT_SET_EMPTY)")]
    RCL_RET_WAIT_SET_EMPTY,
    #[error("wait set is full (RCL_RET_WAIT_SET_FULL)")]
    RCL_RET_WAIT_SET_FULL,
    #[error("invalid ROS remap rule (RCL_RET_INVALID_REMAP_RULE)")]
    RCL_RET_INVALID_REMAP_RULE,
    #[error("invalid remap lexeme (RCL_RET_WRONG_LEXEME)")]
    RCL_RET_WRONG_LEXEME,
    #[error("invalid parameter rule (RCL_RET_INVALID_PARAM_RULE)")]
    RCL_RET_INVALID_PARAM_RULE,
    #[error("invalid log-level rule (RCL_RET_INVALID_LOG_LEVEL_RULE)")]
    RCL_RET_INVALID_LOG_LEVEL_RULE,
    #[error("invalid ROS arguments (RCL_RET_INVALID_ROS_ARGS)")]
    RCL_RET_INVALID_ROS_ARGS,
    #[error("event handle is invalid (RCL_RET_EVENT_INVALID)")]
    RCL_RET_EVENT_INVALID,
    #[error("event take failed (RCL_RET_EVENT_TAKE_FAILED)")]
    RCL_RET_EVENT_TAKE_FAILED,
    #[error("lifecycle state is already registered (RCL_RET_LIFECYCLE_STATE_REGISTERED)")]
    RCL_RET_LIFECYCLE_STATE_REGISTERED,
    #[error("lifecycle state is not registered (RCL_RET_LIFECYCLE_STATE_NOT_REGISTERED)")]
    RCL_RET_LIFECYCLE_STATE_NOT_REGISTERED,
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    #[error("action goal is not terminated yet (RCL_ACTION_RET_NOT_TERMINATED_YET)")]
    RCL_ACTION_RET_NOT_TERMINATED_YET,

    // Our own errors
    #[error("Executor operation cancelled; reset cancellation before reusing it")]
    ExecutorCancelled,
    #[error("Clock type is not RosTime")]
    ClockTypeNotRosTime,
    #[error("No typesupport built for the message type: {}", msgtype)]
    InvalidMessageType { msgtype: String },
    #[error("Native type support for {interface} could not be loaded: {err}")]
    NativeTypeSupport { interface: String, err: String },
    #[error("Could not install the process signal handler: {err}")]
    SignalHandler { err: String },
    #[error("Serde error: {}", err)]
    SerdeError { err: String },
    #[error("Invalid UTF-8 in {context}: {err}")]
    InvalidUtf8 { context: &'static str, err: String },
    #[error("RCUTILS logging error code: {0}")]
    RcutilsLoggingError(i32),

    // action errors.
    #[error("action name is invalid (RCL_RET_ACTION_NAME_INVALID)")]
    RCL_RET_ACTION_NAME_INVALID,
    #[error("goal was already accepted (RCL_RET_ACTION_GOAL_ACCEPTED)")]
    RCL_RET_ACTION_GOAL_ACCEPTED,
    #[error("goal was rejected (RCL_RET_ACTION_GOAL_REJECTED)")]
    RCL_RET_ACTION_GOAL_REJECTED,
    #[error("action client handle is invalid (RCL_RET_ACTION_CLIENT_INVALID)")]
    RCL_RET_ACTION_CLIENT_INVALID,
    #[error("action client take failed (RCL_RET_ACTION_CLIENT_TAKE_FAILED)")]
    RCL_RET_ACTION_CLIENT_TAKE_FAILED,
    #[error("action server handle is invalid (RCL_RET_ACTION_SERVER_INVALID)")]
    RCL_RET_ACTION_SERVER_INVALID,
    #[error("action server take failed (RCL_RET_ACTION_SERVER_TAKE_FAILED)")]
    RCL_RET_ACTION_SERVER_TAKE_FAILED,
    #[error("action goal handle is invalid (RCL_RET_ACTION_GOAL_HANDLE_INVALID)")]
    RCL_RET_ACTION_GOAL_HANDLE_INVALID,
    #[error("action goal event is invalid (RCL_RET_ACTION_GOAL_EVENT_INVALID)")]
    RCL_RET_ACTION_GOAL_EVENT_INVALID,

    #[error("Goal cancel request rejected by server.")]
    GoalCancelRejected,

    #[error("Unknown goal id.")]
    GoalCancelUnknownGoalID,

    #[error("Goal already in a terminal state.")]
    GoalCancelAlreadyTerminated,

    #[error("Unknown action cancel response code: {0}")]
    UnknownActionCancelResponse(i8),

    #[error("Invalid action UUID length: expected 16 bytes, got {actual}")]
    InvalidActionUuidLength { actual: usize },

    #[error("Invalid parameter name: {name}")]
    InvalidParameterName { name: String },

    #[error("Parameter {name} has already been declared")]
    ParameterAlreadyDeclared { name: String },

    #[error("Parameter {name} has not been declared")]
    ParameterNotDeclared { name: String },

    #[error("Invalid type for parameter {name} (should be {ty})")]
    InvalidParameterType { name: String, ty: &'static str },

    #[error("Unknown ROS parameter type: {type_}")]
    UnknownParameterType { type_: u8 },

    #[error(
        "Parameter {operation} response length mismatch: expected {expected} entries, got {actual}"
    )]
    ParameterResponseLengthMismatch {
        operation: &'static str,
        expected: usize,
        actual: usize,
    },

    #[error("Parameter {name} conversion failed: {msg}")]
    ParameterValueConv { name: String, msg: String },

    #[error("Invalid descriptor for parameter {name}: {reason}")]
    InvalidParameterDescriptor { name: String, reason: String },

    #[error("Parameter {name} violates its descriptor: {reason}")]
    ParameterConstraintViolation { name: String, reason: String },

    #[error("Parameter update was rejected: {reason}")]
    ParameterUpdateRejected { reason: String },

    #[error("Parameter modification from within a parameter callback is not allowed")]
    RecursiveParameterMutation,

    #[error("Service request has already been responded to")]
    ServiceRequestAlreadyResponded,

    #[error("A parameter handler has already been created for this node")]
    ParameterHandlerAlreadyCreated,

    #[error(
        "Parameter {name} was expected to be of type {expected_type} but was of type {actual_type}"
    )]
    ParameterWrongType {
        name: String,
        expected_type: &'static str,
        actual_type: &'static str,
    },

    #[error("Unknown RCL error code: {0}")]
    UnknownRclError(i32),
}

impl Error {
    pub(crate) fn from_rcl_action_error(e: i32) -> Self {
        let code = e as u32;
        if code == r2r_actions::RCL_RET_ACTION_NAME_INVALID {
            Error::RCL_RET_ACTION_NAME_INVALID
        } else {
            Self::from_rcl_error(e)
        }
    }

    pub fn from_rcl_error(e: i32) -> Self {
        let e = e as u32;
        match e {
            _ if e == RCL_RET_OK => Error::RCL_RET_OK,
            _ if e == RCL_RET_ERROR => Error::RCL_RET_ERROR,
            _ if e == RCL_RET_TIMEOUT => Error::RCL_RET_TIMEOUT,
            _ if e == RCL_RET_BAD_ALLOC => Error::RCL_RET_BAD_ALLOC,
            _ if e == RCL_RET_INVALID_ARGUMENT => Error::RCL_RET_INVALID_ARGUMENT,
            _ if e == RCL_RET_UNSUPPORTED => Error::RCL_RET_UNSUPPORTED,
            _ if e == RCL_RET_ALREADY_INIT => Error::RCL_RET_ALREADY_INIT,
            _ if e == RCL_RET_NOT_INIT => Error::RCL_RET_NOT_INIT,
            _ if e == RCL_RET_MISMATCHED_RMW_ID => Error::RCL_RET_MISMATCHED_RMW_ID,
            _ if e == RMW_RET_INCORRECT_RMW_IMPLEMENTATION => {
                Error::RMW_RET_INCORRECT_RMW_IMPLEMENTATION
            }
            _ if e == RCL_RET_TOPIC_NAME_INVALID => Error::RCL_RET_TOPIC_NAME_INVALID,
            _ if e == RCL_RET_SERVICE_NAME_INVALID => Error::RCL_RET_SERVICE_NAME_INVALID,
            _ if e == RCL_RET_UNKNOWN_SUBSTITUTION => Error::RCL_RET_UNKNOWN_SUBSTITUTION,
            _ if e == RCL_RET_ALREADY_SHUTDOWN => Error::RCL_RET_ALREADY_SHUTDOWN,
            #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
            _ if e == RCL_RET_NOT_FOUND => Error::RCL_RET_NOT_FOUND,
            _ if e == RCL_RET_NODE_INVALID => Error::RCL_RET_NODE_INVALID,
            _ if e == RCL_RET_NODE_INVALID_NAME => Error::RCL_RET_NODE_INVALID_NAME,
            _ if e == RCL_RET_NODE_INVALID_NAMESPACE => Error::RCL_RET_NODE_INVALID_NAMESPACE,
            _ if e == RCL_RET_NODE_NAME_NON_EXISTENT => Error::RCL_RET_NODE_NAME_NON_EXISTENT,
            _ if e == RCL_RET_PUBLISHER_INVALID => Error::RCL_RET_PUBLISHER_INVALID,
            _ if e == RCL_RET_SUBSCRIPTION_INVALID => Error::RCL_RET_SUBSCRIPTION_INVALID,
            _ if e == RCL_RET_SUBSCRIPTION_TAKE_FAILED => Error::RCL_RET_SUBSCRIPTION_TAKE_FAILED,
            _ if e == RCL_RET_CLIENT_INVALID => Error::RCL_RET_CLIENT_INVALID,
            _ if e == RCL_RET_CLIENT_TAKE_FAILED => Error::RCL_RET_CLIENT_TAKE_FAILED,
            _ if e == RCL_RET_SERVICE_INVALID => Error::RCL_RET_SERVICE_INVALID,
            _ if e == RCL_RET_SERVICE_TAKE_FAILED => Error::RCL_RET_SERVICE_TAKE_FAILED,
            _ if e == RCL_RET_TIMER_INVALID => Error::RCL_RET_TIMER_INVALID,
            _ if e == RCL_RET_TIMER_CANCELED => Error::RCL_RET_TIMER_CANCELED,
            _ if e == RCL_RET_WAIT_SET_INVALID => Error::RCL_RET_WAIT_SET_INVALID,
            _ if e == RCL_RET_WAIT_SET_EMPTY => Error::RCL_RET_WAIT_SET_EMPTY,
            _ if e == RCL_RET_WAIT_SET_FULL => Error::RCL_RET_WAIT_SET_FULL,
            _ if e == RCL_RET_INVALID_REMAP_RULE => Error::RCL_RET_INVALID_REMAP_RULE,
            _ if e == RCL_RET_WRONG_LEXEME => Error::RCL_RET_WRONG_LEXEME,
            _ if e == RCL_RET_INVALID_PARAM_RULE => Error::RCL_RET_INVALID_PARAM_RULE,
            _ if e == RCL_RET_INVALID_LOG_LEVEL_RULE => Error::RCL_RET_INVALID_LOG_LEVEL_RULE,
            _ if e == RCL_RET_INVALID_ROS_ARGS => Error::RCL_RET_INVALID_ROS_ARGS,
            _ if e == RCL_RET_EVENT_INVALID => Error::RCL_RET_EVENT_INVALID,
            _ if e == RCL_RET_EVENT_TAKE_FAILED => Error::RCL_RET_EVENT_TAKE_FAILED,
            _ if e == RCL_RET_LIFECYCLE_STATE_REGISTERED => {
                Error::RCL_RET_LIFECYCLE_STATE_REGISTERED
            }
            _ if e == RCL_RET_LIFECYCLE_STATE_NOT_REGISTERED => {
                Error::RCL_RET_LIFECYCLE_STATE_NOT_REGISTERED
            }
            #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
            _ if e == RCL_ACTION_RET_NOT_TERMINATED_YET => Error::RCL_ACTION_RET_NOT_TERMINATED_YET,
            _ if e == r2r_actions::RCL_RET_ACTION_NAME_INVALID => {
                Error::RCL_RET_ACTION_NAME_INVALID
            }
            _ if e == r2r_actions::RCL_RET_ACTION_GOAL_ACCEPTED => {
                Error::RCL_RET_ACTION_GOAL_ACCEPTED
            }
            _ if e == r2r_actions::RCL_RET_ACTION_GOAL_REJECTED => {
                Error::RCL_RET_ACTION_GOAL_REJECTED
            }
            _ if e == r2r_actions::RCL_RET_ACTION_CLIENT_INVALID => {
                Error::RCL_RET_ACTION_CLIENT_INVALID
            }
            _ if e == r2r_actions::RCL_RET_ACTION_CLIENT_TAKE_FAILED => {
                Error::RCL_RET_ACTION_CLIENT_TAKE_FAILED
            }
            _ if e == r2r_actions::RCL_RET_ACTION_SERVER_INVALID => {
                Error::RCL_RET_ACTION_SERVER_INVALID
            }
            _ if e == r2r_actions::RCL_RET_ACTION_SERVER_TAKE_FAILED => {
                Error::RCL_RET_ACTION_SERVER_TAKE_FAILED
            }
            _ if e == r2r_actions::RCL_RET_ACTION_GOAL_HANDLE_INVALID => {
                Error::RCL_RET_ACTION_GOAL_HANDLE_INVALID
            }
            _ if e == r2r_actions::RCL_RET_ACTION_GOAL_EVENT_INVALID => {
                Error::RCL_RET_ACTION_GOAL_EVENT_INVALID
            }
            _ => Error::UnknownRclError(e as i32),
        }
    }

    /// Internal function used by code derived for the RosParams trait.
    pub fn update_param_name(self, param_name: &str) -> Error {
        match self {
            Error::InvalidParameterName { name: _ } => Error::InvalidParameterName {
                name: param_name.to_string(),
            },
            Error::InvalidParameterType { name: _, ty } => Error::InvalidParameterType {
                name: param_name.to_string(),
                ty,
            },
            Error::ParameterValueConv { name: _, msg } => Error::ParameterValueConv {
                name: param_name.to_string(),
                msg,
            },
            _ => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_unknown_rcl_error_codes() {
        assert!(matches!(Error::from_rcl_error(-12345), Error::UnknownRclError(-12345)));
    }

    #[test]
    fn error_display_explains_common_rcl_failures() {
        let message = Error::RCL_RET_TOPIC_NAME_INVALID.to_string();
        assert!(message.contains("invalid topic name"));
        assert!(message.contains("RCL_RET_TOPIC_NAME_INVALID"));
        let message = Error::RCL_RET_BAD_ALLOC.to_string();
        assert!(message.contains("native allocation failed"));
        let message = Error::RCL_RET_TIMEOUT.to_string();
        assert!(message.contains("timed out"));
    }

    #[test]
    fn maps_missing_graph_node_error() {
        assert!(matches!(
            Error::from_rcl_error(RCL_RET_NODE_NAME_NON_EXISTENT as i32),
            Error::RCL_RET_NODE_NAME_NON_EXISTENT
        ));
    }

    #[test]
    fn maps_incorrect_rmw_implementation_error() {
        assert!(matches!(
            Error::from_rcl_error(RMW_RET_INCORRECT_RMW_IMPLEMENTATION as i32),
            Error::RMW_RET_INCORRECT_RMW_IMPLEMENTATION
        ));
    }

    #[test]
    fn maps_lifecycle_status_codes() {
        assert!(matches!(
            Error::from_rcl_error(RCL_RET_LIFECYCLE_STATE_REGISTERED as i32),
            Error::RCL_RET_LIFECYCLE_STATE_REGISTERED
        ));
        assert!(matches!(
            Error::from_rcl_error(RCL_RET_LIFECYCLE_STATE_NOT_REGISTERED as i32),
            Error::RCL_RET_LIFECYCLE_STATE_NOT_REGISTERED
        ));
    }

    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    #[test]
    fn maps_newer_rcl_status_codes() {
        assert!(matches!(
            Error::from_rcl_error(RCL_RET_NOT_FOUND as i32),
            Error::RCL_RET_NOT_FOUND
        ));
        assert!(matches!(
            Error::from_rcl_error(RCL_ACTION_RET_NOT_TERMINATED_YET as i32),
            Error::RCL_ACTION_RET_NOT_TERMINATED_YET
        ));
    }

    #[test]
    fn maps_action_and_event_codes_across_distributions() {
        let code = r2r_actions::RCL_RET_ACTION_NAME_INVALID as i32;
        #[cfg(not(any(
            r2r__ros__distro__kilted,
            r2r__ros__distro__lyrical,
            r2r__ros__distro__rolling
        )))]
        {
            assert_eq!(code, RCL_RET_EVENT_INVALID as i32);
            assert!(matches!(Error::from_rcl_error(code), Error::RCL_RET_EVENT_INVALID));
        }
        #[cfg(any(
            r2r__ros__distro__kilted,
            r2r__ros__distro__lyrical,
            r2r__ros__distro__rolling
        ))]
        {
            assert_ne!(code, RCL_RET_EVENT_INVALID as i32);
            assert!(matches!(Error::from_rcl_error(code), Error::RCL_RET_ACTION_NAME_INVALID));
        }
        assert!(matches!(
            Error::from_rcl_error(RCL_RET_EVENT_INVALID as i32),
            Error::RCL_RET_EVENT_INVALID
        ));
        assert!(matches!(Error::from_rcl_action_error(code), Error::RCL_RET_ACTION_NAME_INVALID));
    }
}
