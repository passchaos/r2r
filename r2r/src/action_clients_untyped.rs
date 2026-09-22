use futures::{
    channel::{mpsc, oneshot},
    future::TryFutureExt,
    stream::Stream,
};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    future::Future,
    mem::MaybeUninit,
    sync::{Arc, Weak},
};

use crate::{
    action_clients::*,
    action_common::*,
    error::*,
    msg_types::{
        generated_msgs::{action_msgs, builtin_interfaces, unique_identifier_msgs},
        *,
    },
    Node,
};
use r2r_actions::*;
use r2r_rcl::*;
//
// TODO: refactor this to separate out shared code between typed action client and this.
//

unsafe impl Send for ActionClientUntyped {}

/// Action client (untyped)
///
/// Use this to make goal requests to an action server, without having
/// the concrete types at compile-time.
#[derive(Clone)]
pub struct ActionClientUntyped {
    client: Weak<Mutex<WrappedActionClientUntyped>>,
}

unsafe impl Send for ActionClientGoalUntyped {}

/// Action client goal handle (untyped)
///
/// This can be used to cancel goals and query the status of goals.
pub struct ActionClientGoalUntyped {
    client: Weak<Mutex<WrappedActionClientUntyped>>,
    pub uuid: uuid::Uuid,
    accepted_at: builtin_interfaces::msg::Time,
    status_registration: Arc<GoalStatusRegistration>,
}

struct GoalStatusRegistration {
    client: Weak<Mutex<WrappedActionClientUntyped>>,
    uuid: uuid::Uuid,
}

struct PendingGoalRegistration {
    client: Weak<Mutex<WrappedActionClientUntyped>>,
    uuid: uuid::Uuid,
    armed: bool,
}

impl PendingGoalRegistration {
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for PendingGoalRegistration {
    fn drop(&mut self) {
        if self.armed {
            if let Some(client) = self.client.upgrade() {
                client.lock().forget_goal(&self.uuid);
            }
        }
    }
}

impl Drop for GoalStatusRegistration {
    fn drop(&mut self) {
        if let Some(client) = self.client.upgrade() {
            client.lock().goal_status.remove(&self.uuid);
        }
    }
}

impl Clone for ActionClientGoalUntyped {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            uuid: self.uuid,
            accepted_at: self.accepted_at.clone(),
            status_registration: self.status_registration.clone(),
        }
    }
}

impl ActionClientGoalUntyped {
    /// Get the current status of this goal.
    pub fn get_status(&self) -> Result<GoalStatus> {
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let client = client.lock();

        Ok(client.get_goal_status(&self.uuid))
    }

    /// Returns the server timestamp at which this goal was accepted.
    pub fn accepted_at(&self) -> &builtin_interfaces::msg::Time {
        &self.accepted_at
    }

    /// Returns whether this goal's feedback stream is still being observed.
    pub fn is_feedback_aware(&self) -> Result<bool> {
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let client = client.lock();
        Ok(client
            .feedback_senders
            .iter()
            .any(|(uuid, sender)| uuid == &self.uuid && !sender.is_closed()))
    }

    /// Returns whether this goal's result future is still being observed.
    pub fn is_result_aware(&self) -> Result<bool> {
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let client = client.lock();
        Ok(client
            .result_senders
            .iter()
            .any(|(uuid, sender)| uuid == &self.uuid && !sender.is_canceled()))
    }

    /// Send a cancel request for this goal to the server.
    ///
    /// If the server accepts and completes the request, the future completes without error.
    /// Otherwise, one of these errors can be returned:
    /// - `GoalCancelRejected`
    /// - `GoalCancelUnknownGoalID`
    /// - `GoalCancelAlreadyTerminated`
    pub fn cancel(&self) -> Result<impl Future<Output = Result<()>>> {
        // upgrade to actual ref. if still alive
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let mut client = client.lock();

        client.send_cancel_request(&self.uuid)
    }

    /// Send a cancel request and wait up to `timeout` for the server response.
    pub fn cancel_with_timeout(
        &self, node: &mut Node, timeout: std::time::Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        let response = self.cancel()?;
        node.with_timeout(response, timeout)
    }
}

impl ActionClientUntyped {
    /// Make a new goal request.
    ///
    /// If the server accepts the new goal, the future resolves to a triple of:
    /// - A goal handle.
    /// - A new future for the eventual result. (as `serde_json::Value`)
    /// - A stream of feedback messages. (as `serde_json::Value`)
    #[allow(clippy::type_complexity)]
    pub fn send_goal_request(
        &self,
        goal: serde_json::Value, // T::Goal
    ) -> Result<
        impl Future<
            Output = Result<(
                ActionClientGoalUntyped,
                impl Future<Output = Result<(GoalStatus, Result<serde_json::Value>)>>, // T::Result
                impl Stream<Item = Result<serde_json::Value>> + Unpin, // T::Feedback
            )>,
        >,
    > {
        // upgrade to actual ref. if still alive
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let mut client = client.lock();

        let uuid = uuid::Uuid::new_v4();
        let uuid_msg = unique_identifier_msgs::msg::UUID {
            uuid: uuid.as_bytes().to_vec(),
        };

        let native_msg = (client.action_type_support.make_goal_request_msg)(uuid_msg, goal)?;

        let mut seq_no = 0i64;
        let result = unsafe {
            rcl_action_send_goal_request(&client.rcl_handle, native_msg.void_ptr(), &mut seq_no)
        };

        if result == RCL_RET_OK as i32 {
            let (goal_req_sender, goal_req_receiver) =
                oneshot::channel::<(bool, builtin_interfaces::msg::Time)>();
            let (feedback_sender, feedback_receiver) =
                mpsc::channel::<Result<serde_json::Value>>(client.feedback_channel_capacity);
            client.feedback_senders.push((uuid, feedback_sender));
            let (result_sender, result_receiver) =
                oneshot::channel::<(GoalStatus, Result<serde_json::Value>)>();
            client.result_senders.push((uuid, result_sender));
            client
                .goal_response_channels
                .push((seq_no, uuid, goal_req_sender));
            client.goal_status.insert(uuid, GoalStatus::Unknown);
            let fut_client = Weak::clone(&self.client);
            let mut pending_registration = PendingGoalRegistration {
                client: fut_client.clone(),
                uuid,
                armed: true,
            };
            let future = async move {
                match goal_req_receiver.await {
                    Ok((accepted, stamp)) => {
                        if accepted {
                            // on goal accept we immediately send the result request
                            {
                                let c = fut_client
                                    .upgrade()
                                    .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
                                let mut c = c.lock();
                                c.send_result_request(uuid)?;
                            }
                            let status_registration = Arc::new(GoalStatusRegistration {
                                client: fut_client.clone(),
                                uuid,
                            });
                            pending_registration.disarm();

                            Ok((
                                ActionClientGoalUntyped {
                                    client: fut_client,
                                    uuid,
                                    accepted_at: stamp,
                                    status_registration,
                                },
                                result_receiver.map_err(|_| Error::RCL_RET_ACTION_CLIENT_INVALID),
                                feedback_receiver,
                            ))
                        } else {
                            Err(Error::RCL_RET_ACTION_GOAL_REJECTED)
                        }
                    }
                    Err(_) => Err(Error::RCL_RET_ACTION_CLIENT_INVALID),
                }
            };
            Ok(future)
        } else {
            log::error!("could not send goal request {}", result);
            Err(Error::from_rcl_error(result))
        }
    }

    /// Make an untyped goal request and wait up to `timeout` for acceptance.
    ///
    /// The timeout only covers the goal response. Wrap the returned result future with
    /// [`Node::with_timeout`] when a separate execution deadline is required.
    #[allow(clippy::type_complexity)]
    pub fn send_goal_request_with_timeout(
        &self, node: &mut Node, goal: serde_json::Value, timeout: std::time::Duration,
    ) -> Result<
        impl Future<
            Output = Result<(
                ActionClientGoalUntyped,
                impl Future<Output = Result<(GoalStatus, Result<serde_json::Value>)>>,
                impl Stream<Item = Result<serde_json::Value>> + Unpin,
            )>,
        >,
    > {
        let response = self.send_goal_request(goal)?;
        node.with_timeout(response, timeout)
    }

    /// Checks whether the action server is currently available without registering a waiter.
    pub fn is_available(&self, node: &Node) -> Result<bool> {
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        if !node.owns_action_client(&client) {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let client = client.lock();
        action_server_available_helper(node.node_handle.as_ref(), client.handle())
    }

    /// Returns the fully resolved action name used by the native client.
    pub fn action_name(&self) -> Result<String> {
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let client = client.lock();
        action_client_name(client.handle())
    }
}

type GoalResponseSender = oneshot::Sender<(bool, builtin_interfaces::msg::Time)>;

pub fn make_action_client_untyped(
    client: Weak<Mutex<WrappedActionClientUntyped>>,
) -> ActionClientUntyped {
    ActionClientUntyped { client }
}

pub type ResultSender = (uuid::Uuid, oneshot::Sender<(GoalStatus, Result<serde_json::Value>)>);
pub struct WrappedActionClientUntyped {
    pub action_type_support: UntypedActionSupport,
    pub rcl_handle: rcl_action_client_t,
    pub goal_response_channels: Vec<(i64, uuid::Uuid, GoalResponseSender)>,
    pub cancel_response_channels:
        Vec<(i64, oneshot::Sender<action_msgs::srv::CancelGoal::Response>)>,
    pub feedback_senders: Vec<(uuid::Uuid, mpsc::Sender<Result<serde_json::Value>>)>,
    pub pending_feedback: Option<(uuid::Uuid, Result<serde_json::Value>)>,
    pub result_requests: Vec<(i64, uuid::Uuid)>,
    pub result_senders: Vec<ResultSender>,
    pub goal_status: HashMap<uuid::Uuid, GoalStatus>,
    pub feedback_channel_capacity: usize,

    pub poll_available_channels: Vec<oneshot::Sender<()>>,
}

impl WrappedActionClientUntyped {
    pub fn get_goal_status(&self, uuid: &uuid::Uuid) -> GoalStatus {
        *self.goal_status.get(uuid).unwrap_or(&GoalStatus::Unknown)
    }

    fn forget_goal(&mut self, uuid: &uuid::Uuid) {
        self.goal_response_channels.retain(|(_, id, _)| id != uuid);
        self.feedback_senders.retain(|(id, _)| id != uuid);
        self.result_senders.retain(|(id, _)| id != uuid);
        self.result_requests.retain(|(_, id)| id != uuid);
        if self
            .pending_feedback
            .as_ref()
            .is_some_and(|(id, _)| id == uuid)
        {
            self.pending_feedback = None;
        }
        self.goal_status.remove(uuid);
    }

    fn forget_result(&mut self, uuid: &uuid::Uuid) {
        self.result_senders.retain(|(id, _)| id != uuid);
        self.result_requests.retain(|(_, id)| id != uuid);
    }

    fn finish_goal(&mut self, uuid: &uuid::Uuid, status: GoalStatus) {
        self.feedback_senders.retain(|(id, _)| id != uuid);
        self.result_senders.retain(|(id, _)| id != uuid);
        self.result_requests.retain(|(_, id)| id != uuid);
        if self
            .pending_feedback
            .as_ref()
            .is_some_and(|(id, _)| id == uuid)
        {
            self.pending_feedback = None;
        }
        if let Some(tracked_status) = self.goal_status.get_mut(uuid) {
            *tracked_status = status;
        }
    }

    fn prune_canceled_requests(&mut self) {
        let canceled_goals: Vec<_> = self
            .goal_response_channels
            .iter()
            .filter_map(|(_, uuid, sender)| sender.is_canceled().then_some(*uuid))
            .collect();
        self.goal_response_channels
            .retain(|(_, _, sender)| !sender.is_canceled());
        for uuid in canceled_goals {
            self.forget_goal(&uuid);
        }
        self.cancel_response_channels
            .retain(|(_, sender)| !sender.is_canceled());
        self.feedback_senders
            .retain(|(_, sender)| !sender.is_closed());
        let canceled_results: Vec<_> = self
            .result_senders
            .iter()
            .filter_map(|(uuid, sender)| sender.is_canceled().then_some(*uuid))
            .collect();
        for uuid in canceled_results {
            self.forget_result(&uuid);
        }
        if self
            .pending_feedback
            .as_ref()
            .is_some_and(|(uuid, _)| !self.feedback_senders.iter().any(|(id, _)| id == uuid))
        {
            self.pending_feedback = None;
        }
    }

    fn flush_pending_feedback(&mut self) -> bool {
        let Some((uuid, feedback)) = self.pending_feedback.take() else {
            return true;
        };
        let Some((_, sender)) = self.feedback_senders.iter_mut().find(|(id, _)| id == &uuid) else {
            return true;
        };
        match sender.try_send(feedback) {
            Ok(()) => true,
            Err(err) if err.is_full() => {
                self.pending_feedback = Some((uuid, err.into_inner()));
                false
            }
            Err(_) => true,
        }
    }

    fn finish_result_request_send(&mut self, uuid: uuid::Uuid, result: Result<i64>) -> Result<()> {
        match result {
            Ok(seq_no) => {
                self.result_requests.push((seq_no, uuid));
                Ok(())
            }
            Err(err) => {
                self.forget_goal(&uuid);
                Err(err)
            }
        }
    }

    pub fn send_cancel_request(
        &mut self, goal: &uuid::Uuid,
    ) -> Result<impl Future<Output = Result<()>>> {
        let msg = action_msgs::srv::CancelGoal::Request {
            goal_info: action_msgs::msg::GoalInfo {
                goal_id: unique_identifier_msgs::msg::UUID {
                    uuid: goal.as_bytes().to_vec(),
                },
                ..action_msgs::msg::GoalInfo::default()
            },
        };
        let native_msg = WrappedNativeMsg::<action_msgs::srv::CancelGoal::Request>::try_from(&msg)?;
        let mut seq_no = 0i64;
        let result = unsafe {
            rcl_action_send_cancel_request(&self.rcl_handle, native_msg.void_ptr(), &mut seq_no)
        };

        if result == RCL_RET_OK as i32 {
            let (cancel_req_sender, cancel_req_receiver) =
                oneshot::channel::<action_msgs::srv::CancelGoal::Response>();

            self.cancel_response_channels
                .push((seq_no, cancel_req_sender));
            Ok(cancel_response_future(cancel_req_receiver))
        } else {
            log::error!("could not send goal request {}", result);
            Err(Error::from_rcl_error(result))
        }
    }
}

impl ActionClient_ for WrappedActionClientUntyped {
    fn handle(&self) -> &rcl_action_client_t {
        &self.rcl_handle
    }

    fn handle_goal_response(&mut self) -> Result<()> {
        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut response_msg = (self.action_type_support.make_goal_response_msg)()?;

        let ret = unsafe {
            rcl_action_take_goal_response(
                &self.rcl_handle,
                request_id.as_mut_ptr(),
                response_msg.void_ptr_mut(),
            )
        };
        if action_client_take_succeeded(ret)? {
            let request_id = unsafe { request_id.assume_init() };
            if let Some(idx) = self
                .goal_response_channels
                .iter()
                .position(|(id, _, _)| id == &request_id.sequence_number)
            {
                let (_, uuid, sender) = self.goal_response_channels.swap_remove(idx);
                let (accept, stamp) =
                    (self.action_type_support.destructure_goal_response_msg)(response_msg);
                if !accept {
                    self.forget_goal(&uuid);
                }
                match sender.send((accept, stamp)) {
                    Ok(()) => {}
                    Err(e) => {
                        self.forget_goal(&uuid);
                        log::debug!("error sending to action client: {:?}", e);
                    }
                }
            } else {
                let we_have: String = self
                    .goal_response_channels
                    .iter()
                    .map(|(id, _, _)| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                log::error!(
                    "no such req id: {}, we have [{}], ignoring",
                    request_id.sequence_number,
                    we_have
                );
            }
        }
        Ok(())
    }

    fn handle_cancel_response(&mut self) -> Result<()> {
        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut response_msg =
            WrappedNativeMsg::<action_msgs::srv::CancelGoal::Response>::try_new()?;

        let ret = unsafe {
            rcl_action_take_cancel_response(
                &self.rcl_handle,
                request_id.as_mut_ptr(),
                response_msg.void_ptr_mut(),
            )
        };
        if action_client_take_succeeded(ret)? {
            let request_id = unsafe { request_id.assume_init() };
            if let Some(idx) = self
                .cancel_response_channels
                .iter()
                .position(|(id, _)| id == &request_id.sequence_number)
            {
                let (_, sender) = self.cancel_response_channels.swap_remove(idx);
                let response = action_msgs::srv::CancelGoal::Response::from_native(&response_msg);
                if let Err(e) = sender.send(response) {
                    log::error!("warning: could not send cancel response msg ({:?})", e)
                }
            } else {
                let we_have: String = self
                    .cancel_response_channels
                    .iter()
                    .map(|(id, _)| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                log::error!(
                    "no such req id: {}, we have [{}], ignoring",
                    request_id.sequence_number,
                    we_have
                );
            }
        }
        Ok(())
    }

    fn handle_feedback_msg(&mut self) -> Result<()> {
        if !self.flush_pending_feedback() {
            return Ok(());
        }
        let mut feedback_msg = (self.action_type_support.make_feedback_msg)()?;
        let ret =
            unsafe { rcl_action_take_feedback(&self.rcl_handle, feedback_msg.void_ptr_mut()) };
        if action_client_take_succeeded(ret)? {
            let (uuid, feedback) =
                (self.action_type_support.destructure_feedback_msg)(feedback_msg);
            let msg_uuid = uuid_msg_to_uuid(&uuid)?;
            if let Some((_, sender)) = self
                .feedback_senders
                .iter_mut()
                .find(|(uuid, _)| uuid == &msg_uuid)
            {
                if let Err(err) = sender.try_send(feedback) {
                    if err.is_full() {
                        self.pending_feedback = Some((msg_uuid, err.into_inner()));
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_status_msg(&mut self) -> Result<()> {
        let mut status_array = WrappedNativeMsg::<action_msgs::msg::GoalStatusArray>::try_new()?;
        let ret = unsafe { rcl_action_take_status(&self.rcl_handle, status_array.void_ptr_mut()) };
        if action_client_take_succeeded(ret)? {
            let arr = action_msgs::msg::GoalStatusArray::from_native(&status_array);
            for a in &arr.status_list {
                let uuid = uuid_msg_to_uuid(&a.goal_info.goal_id)?;
                if let Some(status) = self.goal_status.get_mut(&uuid) {
                    *status = GoalStatus::from_rcl(a.status);
                }
            }
        }
        Ok(())
    }

    fn handle_result_response(&mut self) -> Result<()> {
        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut response_msg = (self.action_type_support.make_result_response_msg)()?;
        let ret = unsafe {
            rcl_action_take_result_response(
                &self.rcl_handle,
                request_id.as_mut_ptr(),
                response_msg.void_ptr_mut(),
            )
        };

        if action_client_take_succeeded(ret)? {
            let request_id = unsafe { request_id.assume_init() };
            if let Some(idx) = self
                .result_requests
                .iter()
                .position(|(id, _)| id == &request_id.sequence_number)
            {
                let (_, uuid) = self.result_requests.swap_remove(idx);
                if let Some(idx) = self
                    .result_senders
                    .iter()
                    .position(|(suuid, _)| suuid == &uuid)
                {
                    let (_, sender) = self.result_senders.swap_remove(idx);
                    let (status, result) =
                        (self.action_type_support.destructure_result_response_msg)(response_msg);
                    let status = GoalStatus::from_rcl(status);
                    self.finish_goal(&uuid, status);
                    match sender.send((status, result)) {
                        Ok(()) => {}
                        Err(e) => {
                            log::debug!("error sending result to action client: {:?}", e);
                        }
                    }
                }
            } else {
                let we_have: String = self
                    .result_requests
                    .iter()
                    .map(|(id, _)| id.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                log::error!(
                    "no such req id: {}, we have [{}], ignoring",
                    request_id.sequence_number,
                    we_have
                );
            }
        }
        Ok(())
    }

    fn send_result_request(&mut self, uuid: uuid::Uuid) -> Result<()> {
        let result = (|| {
            let uuid_msg = unique_identifier_msgs::msg::UUID {
                uuid: uuid.as_bytes().to_vec(),
            };
            let native_msg = (self.action_type_support.make_result_request_msg)(uuid_msg)?;
            let mut seq_no = 0i64;
            let ret = unsafe {
                rcl_action_send_result_request(&self.rcl_handle, native_msg.void_ptr(), &mut seq_no)
            };
            if ret == RCL_RET_OK as i32 {
                Ok(seq_no)
            } else {
                log::error!("could not send request {}", ret);
                Err(Error::from_rcl_error(ret))
            }
        })();
        self.finish_result_request_send(uuid, result)
    }

    fn register_poll_available(&mut self, s: oneshot::Sender<()>) {
        self.poll_available_channels.push(s);
    }

    fn poll_available(&mut self, node: &mut rcl_node_t) {
        self.prune_canceled_requests();
        self.flush_pending_feedback();
        self.poll_available_channels
            .retain(|sender| !sender.is_canceled());
        if self.poll_available_channels.is_empty() {
            return;
        }
        let available = action_server_available_helper(node, self.handle());
        match available {
            Ok(true) => {
                // send ok and close channels
                while let Some(sender) = self.poll_available_channels.pop() {
                    let _res = sender.send(()); // we ignore if receiver dropped.
                }
            }
            Ok(false) => {
                // not available...
            }
            Err(_) => {
                // error, close all channels
                self.poll_available_channels.clear();
            }
        }
    }

    fn has_pending_work(&mut self) -> bool {
        self.prune_canceled_requests();
        self.poll_available_channels
            .retain(|sender| !sender.is_canceled());
        !self.goal_response_channels.is_empty()
            || !self.cancel_response_channels.is_empty()
            || !self.feedback_senders.is_empty()
            || self.pending_feedback.is_some()
            || !self.result_requests.is_empty()
            || !self.result_senders.is_empty()
            || !self.poll_available_channels.is_empty()
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_action_client_fini(&mut self.rcl_handle, node);
        }
    }
}

use crate::nodes::IsAvailablePollable;

impl IsAvailablePollable for ActionClientUntyped {
    fn register_poll_available(&self, sender: oneshot::Sender<()>) -> Result<()> {
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let mut client = client.lock();
        client.register_poll_available(sender);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_result_request_clears_only_the_affected_goal() {
        let target = uuid::Uuid::new_v4();
        let other = uuid::Uuid::new_v4();
        let (target_feedback, _) = mpsc::channel(1);
        let (other_feedback, _) = mpsc::channel(1);
        let (target_result, _) = oneshot::channel();
        let (other_result, _) = oneshot::channel();
        let mut client = WrappedActionClientUntyped {
            action_type_support: UntypedActionSupport::new_from(
                "example_interfaces/action/Fibonacci",
            )
            .unwrap(),
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: Vec::new(),
            cancel_response_channels: Vec::new(),
            feedback_senders: vec![(target, target_feedback), (other, other_feedback)],
            pending_feedback: Some((target, Ok(serde_json::json!({ "sequence": [1] })))),
            result_requests: vec![(10, target), (11, other)],
            result_senders: vec![(target, target_result), (other, other_result)],
            goal_status: HashMap::from([
                (target, GoalStatus::Accepted),
                (other, GoalStatus::Executing),
            ]),
            feedback_channel_capacity: 1,
            poll_available_channels: Vec::new(),
        };

        assert!(matches!(
            client.finish_result_request_send(target, Err(Error::RCL_RET_BAD_ALLOC)),
            Err(Error::RCL_RET_BAD_ALLOC)
        ));
        assert!(!client
            .feedback_senders
            .iter()
            .any(|(uuid, _)| uuid == &target));
        assert!(client
            .feedback_senders
            .iter()
            .any(|(uuid, _)| uuid == &other));
        assert!(client.pending_feedback.is_none());
        assert!(!client
            .result_requests
            .iter()
            .any(|(_, uuid)| uuid == &target));
        assert!(client
            .result_requests
            .iter()
            .any(|(_, uuid)| uuid == &other));
        assert!(!client
            .result_senders
            .iter()
            .any(|(uuid, _)| uuid == &target));
        assert!(client.result_senders.iter().any(|(uuid, _)| uuid == &other));
        assert!(!client.goal_status.contains_key(&target));
        assert_eq!(client.goal_status.get(&other), Some(&GoalStatus::Executing));
    }

    #[test]
    fn dropping_unpolled_acceptance_future_cleans_delivered_goal() {
        let target = uuid::Uuid::new_v4();
        let other = uuid::Uuid::new_v4();
        let (other_goal, _) = oneshot::channel();
        let (target_feedback, _) = mpsc::channel(1);
        let (other_feedback, _) = mpsc::channel(1);
        let (target_result, _) = oneshot::channel();
        let (other_result, _) = oneshot::channel();
        let client = Arc::new(Mutex::new(WrappedActionClientUntyped {
            action_type_support: UntypedActionSupport::new_from(
                "example_interfaces/action/Fibonacci",
            )
            .unwrap(),
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            // The target response has already been delivered, so only the other request remains.
            goal_response_channels: vec![(12, other, other_goal)],
            cancel_response_channels: Vec::new(),
            feedback_senders: vec![(target, target_feedback), (other, other_feedback)],
            pending_feedback: Some((target, Ok(serde_json::json!({ "sequence": [1] })))),
            result_requests: Vec::new(),
            result_senders: vec![(target, target_result), (other, other_result)],
            goal_status: HashMap::from([
                (target, GoalStatus::Accepted),
                (other, GoalStatus::Unknown),
            ]),
            feedback_channel_capacity: 1,
            poll_available_channels: Vec::new(),
        }));
        let pending = PendingGoalRegistration {
            client: Arc::downgrade(&client),
            uuid: target,
            armed: true,
        };

        drop(pending);

        let client = client.lock();
        assert!(!client
            .feedback_senders
            .iter()
            .any(|(uuid, _)| uuid == &target));
        assert!(client
            .feedback_senders
            .iter()
            .any(|(uuid, _)| uuid == &other));
        assert!(client.pending_feedback.is_none());
        assert!(!client
            .result_senders
            .iter()
            .any(|(uuid, _)| uuid == &target));
        assert!(client.result_senders.iter().any(|(uuid, _)| uuid == &other));
        assert!(!client.goal_status.contains_key(&target));
        assert!(client.goal_status.contains_key(&other));
        assert_eq!(client.goal_response_channels.len(), 1);
    }
}
