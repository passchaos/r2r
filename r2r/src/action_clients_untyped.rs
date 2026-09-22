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
    pin::Pin,
    sync::{Arc, Weak},
    task::{Context, Poll},
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

/// Result future for an accepted untyped action goal.
pub struct ActionResultFutureUntyped {
    receiver: oneshot::Receiver<(GoalStatus, Result<serde_json::Value>)>,
}

impl Future for ActionResultFutureUntyped {
    type Output = Result<(GoalStatus, Result<serde_json::Value>)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.receiver)
            .poll(cx)
            .map(|result| result.map_err(|_| Error::RCL_RET_ACTION_CLIENT_INVALID))
    }
}

/// Feedback stream for an accepted untyped action goal.
pub struct ActionFeedbackStreamUntyped {
    receiver: mpsc::Receiver<Result<serde_json::Value>>,
}

impl Stream for ActionFeedbackStreamUntyped {
    type Item = Result<serde_json::Value>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}

/// Accepted untyped action goal and the client-side channels requested for it.
pub struct ActionClientGoalResponseUntyped {
    /// Handle for querying or canceling the accepted goal.
    pub goal: ActionClientGoalUntyped,
    /// Eventual result when result tracking was requested.
    pub result: Option<ActionResultFutureUntyped>,
    /// Feedback messages when feedback tracking was requested.
    pub feedback: Option<ActionFeedbackStreamUntyped>,
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
            .any(|(uuid, channel)| uuid == &self.uuid && !channel.lock().is_closed()))
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
        let response = with_prepared_action_client(
            &client,
            || prepare_cancel_request(goal_cancel_info(&self.uuid)),
            WrappedActionClientUntyped::send_cancel_request_native,
        )?;
        Ok(response.map_ok(|_| ()))
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
    /// Requests cancellation of every cancelable goal known to the action server.
    pub fn cancel_all(&self) -> Result<impl Future<Output = Result<Vec<uuid::Uuid>>>> {
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        with_prepared_action_client(
            &client,
            || prepare_cancel_request(batch_cancel_goal_info(Default::default())),
            WrappedActionClientUntyped::send_cancel_request_native,
        )
    }

    /// Requests cancellation of every goal accepted at or before the supplied stamp.
    pub fn cancel_before(
        &self, stamp: builtin_interfaces::msg::Time,
    ) -> Result<impl Future<Output = Result<Vec<uuid::Uuid>>>> {
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        with_prepared_action_client(
            &client,
            || prepare_cancel_request(batch_cancel_goal_info(stamp)),
            WrappedActionClientUntyped::send_cancel_request_native,
        )
    }

    pub fn cancel_all_with_timeout(
        &self, node: &mut Node, timeout: std::time::Duration,
    ) -> Result<impl Future<Output = Result<Vec<uuid::Uuid>>>> {
        let response = self.cancel_all()?;
        node.with_timeout(response, timeout)
    }

    pub fn cancel_before_with_timeout(
        &self, node: &mut Node, stamp: builtin_interfaces::msg::Time, timeout: std::time::Duration,
    ) -> Result<impl Future<Output = Result<Vec<uuid::Uuid>>>> {
        let response = self.cancel_before(stamp)?;
        node.with_timeout(response, timeout)
    }

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
                ActionResultFutureUntyped,
                ActionFeedbackStreamUntyped,
            )>,
        >,
    > {
        let response = self.send_goal_request_with_options(goal, ActionGoalOptions::default())?;
        Ok(async move {
            let response = response.await?;
            let result = response
                .result
                .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
            let feedback = response
                .feedback
                .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
            Ok((response.goal, result, feedback))
        })
    }

    /// Sends an untyped goal with explicit control over result and feedback tracking.
    ///
    /// Disabling result tracking also suppresses the result request normally sent immediately
    /// after acceptance. Disabled result and feedback channels are returned as [`None`].
    pub fn send_goal_request_with_options(
        &self, goal: serde_json::Value, options: ActionGoalOptions,
    ) -> Result<impl Future<Output = Result<ActionClientGoalResponseUntyped>>> {
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;

        let uuid = uuid::Uuid::new_v4();
        let uuid_msg = unique_identifier_msgs::msg::UUID {
            uuid: uuid.as_bytes().to_vec(),
        };
        let make_goal_request_msg = client.lock().action_type_support.make_goal_request_msg;
        let fut_client = Weak::clone(&self.client);
        with_prepared_action_client(
            &client,
            || make_goal_request_msg(uuid_msg, goal),
            move |client, native_msg| {
                let mut seq_no = 0i64;
                let result = unsafe {
                    rcl_action_send_goal_request(
                        &client.rcl_handle,
                        native_msg.void_ptr(),
                        &mut seq_no,
                    )
                };
                if result != RCL_RET_OK as i32 {
                    log::error!("could not send goal request {}", result);
                    return Err(Error::from_rcl_error(result));
                }

                let (goal_req_sender, goal_req_receiver) =
                    oneshot::channel::<(bool, builtin_interfaces::msg::Time)>();
                let feedback = options.track_feedback.then(|| {
                    let (sender, receiver) = mpsc::channel::<Result<serde_json::Value>>(
                        client.feedback_channel_capacity,
                    );
                    client
                        .feedback_senders
                        .push((uuid, Arc::new(Mutex::new(FeedbackChannel::new(sender)))));
                    ActionFeedbackStreamUntyped { receiver }
                });
                let result = options.track_result.then(|| {
                    let (sender, receiver) =
                        oneshot::channel::<(GoalStatus, Result<serde_json::Value>)>();
                    client.result_senders.push((uuid, sender));
                    ActionResultFutureUntyped { receiver }
                });
                client
                    .goal_response_channels
                    .push((seq_no, uuid, goal_req_sender));
                client.goal_status.insert(uuid, GoalStatus::Unknown);
                let mut pending_registration = PendingGoalRegistration {
                    client: fut_client.clone(),
                    uuid,
                    armed: true,
                };
                Ok(async move {
                    match goal_req_receiver.await {
                        Ok((accepted, stamp)) => {
                            if accepted {
                                if options.track_result {
                                    let c = fut_client
                                        .upgrade()
                                        .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
                                    let uuid_msg = unique_identifier_msgs::msg::UUID {
                                        uuid: uuid.as_bytes().to_vec(),
                                    };
                                    let make_result_request_msg =
                                        c.lock().action_type_support.make_result_request_msg;
                                    let native_msg = make_result_request_msg(uuid_msg)?;
                                    let mut c = c.lock();
                                    c.send_result_request_native(uuid, native_msg)?;
                                }
                                let status_registration = Arc::new(GoalStatusRegistration {
                                    client: fut_client.clone(),
                                    uuid,
                                });
                                pending_registration.disarm();

                                Ok(ActionClientGoalResponseUntyped {
                                    goal: ActionClientGoalUntyped {
                                        client: fut_client,
                                        uuid,
                                        accepted_at: stamp,
                                        status_registration,
                                    },
                                    result,
                                    feedback,
                                })
                            } else {
                                Err(Error::RCL_RET_ACTION_GOAL_REJECTED)
                            }
                        }
                        Err(_) => Err(Error::RCL_RET_ACTION_CLIENT_INVALID),
                    }
                })
            },
        )
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
    pub feedback_senders: Vec<FeedbackSender<Result<serde_json::Value>>>,
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

    fn take_feedback_sender(
        &mut self, uuid: &uuid::Uuid,
    ) -> Option<FeedbackSender<Result<serde_json::Value>>> {
        let index = self
            .feedback_senders
            .iter()
            .position(|(id, _)| id == uuid)?;
        Some(self.feedback_senders.swap_remove(index))
    }

    fn forget_goal(&mut self, uuid: &uuid::Uuid) {
        self.goal_response_channels.retain(|(_, id, _)| id != uuid);
        self.feedback_senders.retain(|(id, _)| id != uuid);
        self.result_senders.retain(|(id, _)| id != uuid);
        self.result_requests.retain(|(_, id)| id != uuid);
        self.goal_status.remove(uuid);
    }

    fn forget_result(&mut self, uuid: &uuid::Uuid) -> Option<ActionClientCompletion> {
        self.result_senders.retain(|(id, _)| id != uuid);
        self.result_requests.retain(|(_, id)| id != uuid);
        if self.get_goal_status(uuid).is_terminal() {
            return self
                .take_feedback_sender(uuid)
                .map(|(_, channel)| close_feedback_completion(channel));
        }
        None
    }

    fn finish_goal(
        &mut self, uuid: &uuid::Uuid, status: GoalStatus,
    ) -> Option<ActionClientCompletion> {
        let feedback = self
            .take_feedback_sender(uuid)
            .map(|(_, channel)| close_feedback_completion(channel));
        self.result_senders.retain(|(id, _)| id != uuid);
        self.result_requests.retain(|(_, id)| id != uuid);
        if let Some(tracked_status) = self.goal_status.get_mut(uuid) {
            *tracked_status = status;
        }
        feedback
    }

    fn prune_canceled_requests(&mut self) -> Vec<ActionClientCompletion> {
        let mut completions = Vec::new();
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
            .retain(|(_, channel)| !channel.lock().is_closed());
        let canceled_results: Vec<_> = self
            .result_senders
            .iter()
            .filter_map(|(uuid, sender)| sender.is_canceled().then_some(*uuid))
            .collect();
        for uuid in canceled_results {
            completions.extend(self.forget_result(&uuid));
        }
        completions
    }

    fn pending_feedback_completion(&mut self) -> Option<ActionClientCompletion> {
        self.feedback_senders.iter().find_map(|(_, channel)| {
            let mut channel_guard = channel.lock();
            let feedback = channel_guard.pending.take()?;
            let Some(sender) = channel_guard.sender.take() else {
                channel_guard.pending = Some(feedback);
                return None;
            };
            drop(channel_guard);
            Some(detached_feedback_completion(Arc::clone(channel), sender, feedback))
        })
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

    fn send_cancel_request_native(
        &mut self, native_msg: WrappedNativeMsg<action_msgs::srv::CancelGoal::Request>,
    ) -> Result<impl Future<Output = Result<Vec<uuid::Uuid>>>> {
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

    fn send_result_request_native(
        &mut self, uuid: uuid::Uuid, native_msg: WrappedNativeMsgUntyped,
    ) -> Result<()> {
        let result = {
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
        };
        self.finish_result_request_send(uuid, result)
    }
}

impl ActionClient_ for WrappedActionClientUntyped {
    fn handle(&self) -> &rcl_action_client_t {
        &self.rcl_handle
    }

    fn forget_goal(&mut self, uuid: &uuid::Uuid) {
        WrappedActionClientUntyped::forget_goal(self, uuid);
    }

    fn handle_goal_response(
        &mut self, completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult> {
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
                completions.push(action_response_completion(
                    sender,
                    (accept, stamp),
                    "untyped action goal response",
                    Some(uuid),
                ));
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
            Ok(ActionClientTakeResult::Taken)
        } else {
            Ok(ActionClientTakeResult::Idle)
        }
    }

    fn handle_cancel_response(
        &mut self, completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult> {
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
                completions.push(action_response_conversion_completion(
                    sender,
                    response_msg,
                    |response_msg| {
                        action_msgs::srv::CancelGoal::Response::from_native(&response_msg)
                    },
                    "untyped action cancel response",
                    None,
                ));
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
            Ok(ActionClientTakeResult::Taken)
        } else {
            Ok(ActionClientTakeResult::Idle)
        }
    }

    fn handle_feedback_msg(
        &mut self, completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult> {
        if let Some(completion) = self.pending_feedback_completion() {
            completions.push(completion);
            return Ok(ActionClientTakeResult::Taken);
        }
        let mut feedback_msg = (self.action_type_support.make_feedback_msg)()?;
        let ret =
            unsafe { rcl_action_take_feedback(&self.rcl_handle, feedback_msg.void_ptr_mut()) };
        if action_client_take_succeeded(ret)? {
            let (uuid, feedback) =
                (self.action_type_support.destructure_feedback_msg)(feedback_msg);
            let msg_uuid = uuid_msg_to_uuid(&uuid)?;
            if let Some((_, channel)) = self
                .feedback_senders
                .iter()
                .find(|(uuid, _)| uuid == &msg_uuid)
            {
                completions.extend(feedback_completion(Arc::clone(channel), feedback));
            }
            Ok(ActionClientTakeResult::Taken)
        } else {
            Ok(ActionClientTakeResult::Idle)
        }
    }

    fn handle_status_msg(&mut self) -> Result<Vec<ActionClientCompletion>> {
        let mut completions = Vec::new();
        let mut status_array = WrappedNativeMsg::<action_msgs::msg::GoalStatusArray>::try_new()?;
        let ret = unsafe { rcl_action_take_status(&self.rcl_handle, status_array.void_ptr_mut()) };
        if action_client_take_succeeded(ret)? {
            let arr = action_msgs::msg::GoalStatusArray::from_native(&status_array);
            for a in &arr.status_list {
                let uuid = uuid_msg_to_uuid(&a.goal_info.goal_id)?;
                let status = GoalStatus::from_rcl(a.status);
                if let Some(tracked_status) = self.goal_status.get_mut(&uuid) {
                    *tracked_status = status;
                }
                let result_is_tracked = self
                    .result_senders
                    .iter()
                    .any(|(id, sender)| id == &uuid && !sender.is_canceled());
                if status.is_terminal() && !result_is_tracked {
                    completions.extend(
                        self.take_feedback_sender(&uuid)
                            .map(|(_, channel)| close_feedback_completion(channel)),
                    );
                }
            }
        }
        Ok(completions)
    }

    fn handle_result_response(
        &mut self, completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult> {
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
                    completions.extend(self.finish_goal(&uuid, status));
                    completions.push(action_response_completion(
                        sender,
                        (status, result),
                        "untyped action result",
                        None,
                    ));
                    return Ok(ActionClientTakeResult::Taken);
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
            Ok(ActionClientTakeResult::Taken)
        } else {
            Ok(ActionClientTakeResult::Idle)
        }
    }

    fn register_poll_available(&mut self, s: oneshot::Sender<()>) {
        self.poll_available_channels.push(s);
    }

    fn poll_available(&mut self, node: &mut rcl_node_t) -> Vec<ActionClientCompletion> {
        let mut completions = self.prune_canceled_requests();
        completions.extend(self.pending_feedback_completion());
        self.poll_available_channels
            .retain(|sender| !sender.is_canceled());
        if self.poll_available_channels.is_empty() {
            return completions;
        }
        let available = action_server_available_helper(node, self.handle());
        if !matches!(available, Ok(false)) {
            let senders = std::mem::take(&mut self.poll_available_channels);
            completions.extend(action_availability_completion(senders, available));
        }
        completions
    }

    fn has_pending_work(&mut self) -> bool {
        self.goal_response_channels
            .iter()
            .any(|(_, _, sender)| !sender.is_canceled())
            || self
                .cancel_response_channels
                .iter()
                .any(|(_, sender)| !sender.is_canceled())
            || self
                .feedback_senders
                .iter()
                .any(|(_, channel)| !channel.lock().is_closed())
            || self
                .result_senders
                .iter()
                .any(|(_, sender)| !sender.is_canceled())
            || self
                .poll_available_channels
                .iter()
                .any(|sender| !sender.is_canceled())
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
            feedback_senders: vec![
                (
                    target,
                    Arc::new(Mutex::new(FeedbackChannel {
                        sender: Some(target_feedback),
                        pending: Some(Ok(serde_json::json!({ "sequence": [1] }))),
                        closing: false,
                    })),
                ),
                (other, Arc::new(Mutex::new(FeedbackChannel::new(other_feedback)))),
            ],
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
            feedback_senders: vec![
                (
                    target,
                    Arc::new(Mutex::new(FeedbackChannel {
                        sender: Some(target_feedback),
                        pending: Some(Ok(serde_json::json!({ "sequence": [1] }))),
                        closing: false,
                    })),
                ),
                (other, Arc::new(Mutex::new(FeedbackChannel::new(other_feedback)))),
            ],
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
