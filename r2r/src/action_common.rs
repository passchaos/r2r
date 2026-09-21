use std::time::Duration;

use crate::{Error, QosProfile, Result};
use r2r_actions::{rcl_action_client_options_t, rcl_action_server_options_t};

/// Optional QoS overrides for an action client. Unset fields retain the ROS distribution's
/// native defaults.
#[derive(Clone, Debug)]
pub struct ActionClientOptions {
    pub goal_service_qos: Option<QosProfile>,
    pub result_service_qos: Option<QosProfile>,
    pub cancel_service_qos: Option<QosProfile>,
    pub feedback_topic_qos: Option<QosProfile>,
    pub status_topic_qos: Option<QosProfile>,
    /// Rust-side buffer for each accepted goal's feedback stream.
    pub feedback_channel_capacity: usize,
}

impl Default for ActionClientOptions {
    fn default() -> Self {
        Self {
            goal_service_qos: None,
            result_service_qos: None,
            cancel_service_qos: None,
            feedback_topic_qos: None,
            status_topic_qos: None,
            feedback_channel_capacity: 10,
        }
    }
}

impl ActionClientOptions {
    pub fn goal_service_qos(mut self, qos: QosProfile) -> Self {
        self.goal_service_qos = Some(qos);
        self
    }

    pub fn result_service_qos(mut self, qos: QosProfile) -> Self {
        self.result_service_qos = Some(qos);
        self
    }

    pub fn cancel_service_qos(mut self, qos: QosProfile) -> Self {
        self.cancel_service_qos = Some(qos);
        self
    }

    pub fn feedback_topic_qos(mut self, qos: QosProfile) -> Self {
        self.feedback_topic_qos = Some(qos);
        self
    }

    pub fn status_topic_qos(mut self, qos: QosProfile) -> Self {
        self.status_topic_qos = Some(qos);
        self
    }

    pub fn feedback_channel_capacity(mut self, capacity: usize) -> Self {
        self.feedback_channel_capacity = capacity;
        self
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate_channel_capacity(self.feedback_channel_capacity)
    }

    pub(crate) fn apply(&self, options: &mut rcl_action_client_options_t) -> Result<()> {
        self.validate()?;
        if let Some(qos) = &self.goal_service_qos {
            options.goal_service_qos = qos.clone().into();
        }
        if let Some(qos) = &self.result_service_qos {
            options.result_service_qos = qos.clone().into();
        }
        if let Some(qos) = &self.cancel_service_qos {
            options.cancel_service_qos = qos.clone().into();
        }
        if let Some(qos) = &self.feedback_topic_qos {
            options.feedback_topic_qos = qos.clone().into();
        }
        if let Some(qos) = &self.status_topic_qos {
            options.status_topic_qos = qos.clone().into();
        }
        Ok(())
    }
}

/// Optional QoS and result-retention overrides for an action server. Unset fields retain the ROS
/// distribution's native defaults.
#[derive(Clone, Debug)]
pub struct ActionServerOptions {
    pub goal_service_qos: Option<QosProfile>,
    pub cancel_service_qos: Option<QosProfile>,
    pub result_service_qos: Option<QosProfile>,
    pub feedback_topic_qos: Option<QosProfile>,
    pub status_topic_qos: Option<QosProfile>,
    pub result_timeout: Option<Duration>,
    /// Rust-side buffer for the goal request stream returned at server creation.
    pub goal_request_channel_capacity: usize,
    /// Rust-side buffer for each accepted goal's cancel request stream.
    pub cancel_request_channel_capacity: usize,
}

impl Default for ActionServerOptions {
    fn default() -> Self {
        Self {
            goal_service_qos: None,
            cancel_service_qos: None,
            result_service_qos: None,
            feedback_topic_qos: None,
            status_topic_qos: None,
            result_timeout: None,
            goal_request_channel_capacity: 10,
            cancel_request_channel_capacity: 10,
        }
    }
}

impl ActionServerOptions {
    pub fn goal_service_qos(mut self, qos: QosProfile) -> Self {
        self.goal_service_qos = Some(qos);
        self
    }

    pub fn cancel_service_qos(mut self, qos: QosProfile) -> Self {
        self.cancel_service_qos = Some(qos);
        self
    }

    pub fn result_service_qos(mut self, qos: QosProfile) -> Self {
        self.result_service_qos = Some(qos);
        self
    }

    pub fn feedback_topic_qos(mut self, qos: QosProfile) -> Self {
        self.feedback_topic_qos = Some(qos);
        self
    }

    pub fn status_topic_qos(mut self, qos: QosProfile) -> Self {
        self.status_topic_qos = Some(qos);
        self
    }

    pub fn result_timeout(mut self, timeout: Duration) -> Self {
        self.result_timeout = Some(timeout);
        self
    }

    pub fn goal_request_channel_capacity(mut self, capacity: usize) -> Self {
        self.goal_request_channel_capacity = capacity;
        self
    }

    pub fn cancel_request_channel_capacity(mut self, capacity: usize) -> Self {
        self.cancel_request_channel_capacity = capacity;
        self
    }

    pub(crate) fn validate(&self) -> Result<()> {
        validate_channel_capacity(self.goal_request_channel_capacity)?;
        validate_channel_capacity(self.cancel_request_channel_capacity)
    }

    pub(crate) fn apply(&self, options: &mut rcl_action_server_options_t) -> Result<()> {
        self.validate()?;
        if let Some(qos) = &self.goal_service_qos {
            options.goal_service_qos = qos.clone().into();
        }
        if let Some(qos) = &self.cancel_service_qos {
            options.cancel_service_qos = qos.clone().into();
        }
        if let Some(qos) = &self.result_service_qos {
            options.result_service_qos = qos.clone().into();
        }
        if let Some(qos) = &self.feedback_topic_qos {
            options.feedback_topic_qos = qos.clone().into();
        }
        if let Some(qos) = &self.status_topic_qos {
            options.status_topic_qos = qos.clone().into();
        }
        if let Some(timeout) = self.result_timeout {
            options.result_timeout.nanoseconds =
                i64::try_from(timeout.as_nanos()).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        }
        Ok(())
    }
}

fn validate_channel_capacity(capacity: usize) -> Result<()> {
    if capacity >= usize::MAX >> 2 {
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    } else {
        Ok(())
    }
}

/// The status of a goal.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum GoalStatus {
    Unknown,
    Accepted,
    Executing,
    Canceling,
    Succeeded,
    Canceled,
    Aborted,
}

impl GoalStatus {
    #[allow(dead_code)]
    pub fn to_rcl(&self) -> i8 {
        match self {
            GoalStatus::Unknown => crate::action_msgs::msg::GoalStatus::STATUS_UNKNOWN as i8,
            GoalStatus::Accepted => crate::action_msgs::msg::GoalStatus::STATUS_ACCEPTED as i8,
            GoalStatus::Executing => crate::action_msgs::msg::GoalStatus::STATUS_EXECUTING as i8,
            GoalStatus::Canceling => crate::action_msgs::msg::GoalStatus::STATUS_CANCELING as i8,
            GoalStatus::Succeeded => crate::action_msgs::msg::GoalStatus::STATUS_SUCCEEDED as i8,
            GoalStatus::Canceled => crate::action_msgs::msg::GoalStatus::STATUS_CANCELED as i8,
            GoalStatus::Aborted => crate::action_msgs::msg::GoalStatus::STATUS_ABORTED as i8,
        }
    }

    pub fn from_rcl(s: i8) -> Self {
        match s {
            s if s == crate::action_msgs::msg::GoalStatus::STATUS_UNKNOWN as i8 => {
                GoalStatus::Unknown
            }
            s if s == crate::action_msgs::msg::GoalStatus::STATUS_ACCEPTED as i8 => {
                GoalStatus::Accepted
            }
            s if s == crate::action_msgs::msg::GoalStatus::STATUS_EXECUTING as i8 => {
                GoalStatus::Executing
            }
            s if s == crate::action_msgs::msg::GoalStatus::STATUS_CANCELING as i8 => {
                GoalStatus::Canceling
            }
            s if s == crate::action_msgs::msg::GoalStatus::STATUS_SUCCEEDED as i8 => {
                GoalStatus::Succeeded
            }
            s if s == crate::action_msgs::msg::GoalStatus::STATUS_CANCELED as i8 => {
                GoalStatus::Canceled
            }
            s if s == crate::action_msgs::msg::GoalStatus::STATUS_ABORTED as i8 => {
                GoalStatus::Aborted
            }
            _ => GoalStatus::Unknown,
        }
    }
}

impl std::fmt::Display for GoalStatus {
    fn fmt(&self, fmtr: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            GoalStatus::Unknown => "unknown",
            GoalStatus::Accepted => "accepted",
            GoalStatus::Executing => "executing",
            GoalStatus::Canceling => "canceling",
            GoalStatus::Succeeded => "succeeded",
            GoalStatus::Canceled => "canceled",
            GoalStatus::Aborted => "aborted",
        };

        write!(fmtr, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use r2r_actions::{
        rcl_action_client_get_default_options, rcl_action_server_get_default_options,
    };

    #[test]
    fn unknown_wire_status_maps_to_unknown() {
        assert_eq!(GoalStatus::from_rcl(i8::MAX), GoalStatus::Unknown);
    }

    #[test]
    fn action_options_override_only_requested_fields() {
        let mut client = unsafe { rcl_action_client_get_default_options() };
        let default_goal_depth = client.goal_service_qos.depth;
        ActionClientOptions::default()
            .feedback_topic_qos(QosProfile::sensor_data())
            .apply(&mut client)
            .unwrap();
        assert_eq!(client.goal_service_qos.depth, default_goal_depth);
        assert_eq!(client.feedback_topic_qos.depth, QosProfile::sensor_data().depth);

        let mut server = unsafe { rcl_action_server_get_default_options() };
        ActionServerOptions::default()
            .result_timeout(Duration::from_millis(250))
            .status_topic_qos(QosProfile::services_default())
            .apply(&mut server)
            .unwrap();
        assert_eq!(server.result_timeout.nanoseconds, 250_000_000);
        assert_eq!(server.status_topic_qos.depth, QosProfile::services_default().depth);
        assert_eq!(ActionClientOptions::default().feedback_channel_capacity, 10);
        assert_eq!(ActionServerOptions::default().goal_request_channel_capacity, 10);
        assert_eq!(ActionServerOptions::default().cancel_request_channel_capacity, 10);
    }

    #[test]
    fn action_server_options_reject_oversized_timeout() {
        let mut server = unsafe { rcl_action_server_get_default_options() };
        assert!(ActionServerOptions::default()
            .result_timeout(Duration::from_nanos(i64::MAX as u64 + 1))
            .apply(&mut server)
            .is_err());
    }

    #[test]
    fn action_options_reject_oversized_channel_capacities() {
        assert!(ActionClientOptions::default()
            .feedback_channel_capacity(usize::MAX)
            .validate()
            .is_err());
        assert!(ActionServerOptions::default()
            .goal_request_channel_capacity(usize::MAX)
            .validate()
            .is_err());
        assert!(ActionServerOptions::default()
            .cancel_request_channel_capacity(usize::MAX)
            .validate()
            .is_err());
    }
}
