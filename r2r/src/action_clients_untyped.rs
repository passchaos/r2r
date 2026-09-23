use futures::{
    channel::{mpsc, oneshot},
    future::TryFutureExt,
    stream::Stream,
};
use parking_lot::Mutex;
use std::{
    any::Any,
    collections::{HashMap, VecDeque},
    future::Future,
    mem::MaybeUninit,
    pin::Pin,
    sync::{Arc, Weak},
    task::{Context, Poll},
};

use crate::{
    action_clients::*,
    action_common::*,
    availability::AvailabilityWaiters,
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
pub struct ActionClientUntyped {
    client: Weak<Mutex<WrappedActionClientUntyped>>,
    lifecycle: Arc<ActionClientHandleLifecycle>,
}

impl Clone for ActionClientUntyped {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            lifecycle: Arc::clone(&self.lifecycle),
        }
    }
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
    registration: ResultFutureRegistration,
}

impl Future for ActionResultFutureUntyped {
    type Output = Result<(GoalStatus, Result<serde_json::Value>)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.receiver).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                self.registration.disarm();
                Poll::Ready(result.map_err(|_| Error::RCL_RET_ACTION_CLIENT_INVALID))
            }
        }
    }
}

impl Drop for ActionResultFutureUntyped {
    fn drop(&mut self) {
        self.receiver.close();
    }
}

/// Feedback stream for an accepted untyped action goal.
pub struct ActionFeedbackStreamUntyped {
    receiver: mpsc::Receiver<Result<serde_json::Value>>,
    registration: FeedbackStreamRegistration,
}

impl Stream for ActionFeedbackStreamUntyped {
    type Item = Result<serde_json::Value>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let result = Pin::new(&mut self.receiver).poll_next(cx);
        if matches!(result, Poll::Ready(None)) {
            self.registration.disarm();
        }
        result
    }
}

impl Drop for ActionFeedbackStreamUntyped {
    fn drop(&mut self) {
        self.receiver.close();
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
                let completions = { client.lock().forget_goal(&self.uuid) };
                run_detached_action_client_completions(completions);
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
            .get(&self.uuid)
            .is_some_and(|channel| !channel.lock().is_closed()))
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
            .get(&self.uuid)
            .is_some_and(|sender| !sender.is_canceled()))
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
                        .insert(uuid, Arc::new(Mutex::new(FeedbackChannel::new(sender))));
                    ActionFeedbackStreamUntyped {
                        receiver,
                        registration: FeedbackStreamRegistration::new(
                            Arc::clone(&client.canceled_feedback_goals),
                            uuid,
                        ),
                    }
                });
                let result = options.track_result.then(|| {
                    let (sender, receiver) =
                        oneshot::channel::<(GoalStatus, Result<serde_json::Value>)>();
                    client.result_senders.insert(uuid, sender);
                    ActionResultFutureUntyped {
                        receiver,
                        registration: ResultFutureRegistration::new(
                            Arc::clone(&client.canceled_result_goals),
                            uuid,
                        ),
                    }
                });
                client.register_goal_response(seq_no, uuid, goal_req_sender);
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
                                    let (result, completions) =
                                        { c.lock().send_result_request_native(uuid, native_msg) };
                                    run_detached_action_client_completions(completions);
                                    result?;
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

    /// Sends an untyped goal with tracking options and waits up to `timeout` for acceptance.
    ///
    /// The timeout only covers the goal response. Wrap a returned result future with
    /// [`Node::with_timeout`] when a separate execution deadline is required.
    pub fn send_goal_request_with_options_and_timeout(
        &self, node: &mut Node, goal: serde_json::Value, options: ActionGoalOptions,
        timeout: std::time::Duration,
    ) -> Result<impl Future<Output = Result<ActionClientGoalResponseUntyped>>> {
        let response = self.send_goal_request_with_options(goal, options)?;
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

    /// Waits asynchronously until the action server is available.
    ///
    /// The owning node must be spun while awaiting.
    pub fn wait_for_action_server(&self) -> Result<impl Future<Output = Result<()>>> {
        Node::is_available(self)
    }

    /// Waits asynchronously until the action server is available or `timeout` expires.
    ///
    /// This future is runtime-agnostic and advances while `node` is spun.
    pub fn wait_for_action_server_with_timeout(
        &self, node: &mut Node, timeout: std::time::Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        let available = self.wait_for_action_server()?;
        node.with_timeout(available, timeout)
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

    pub(crate) fn identity(&self) -> usize {
        self.lifecycle.identity
    }

    pub(crate) fn mark_explicitly_destroyed(&self) {
        self.lifecycle.mark_explicitly_destroyed();
    }
}

type GoalResponseSender = (uuid::Uuid, oneshot::Sender<(bool, builtin_interfaces::msg::Time)>);

pub(crate) fn make_action_client_untyped(
    client: Weak<Mutex<WrappedActionClientUntyped>>, identity: usize,
    dropped_action_clients: Arc<Mutex<VecDeque<usize>>>,
    active_availability_clients: Arc<Mutex<VecDeque<usize>>>,
) -> ActionClientUntyped {
    ActionClientUntyped {
        client,
        lifecycle: Arc::new(ActionClientHandleLifecycle {
            identity,
            dropped_action_clients,
            active_availability_clients,
            explicitly_destroyed: std::sync::atomic::AtomicBool::new(false),
        }),
    }
}

pub type ResultSender = oneshot::Sender<(GoalStatus, Result<serde_json::Value>)>;
pub struct WrappedActionClientUntyped {
    pub action_type_support: UntypedActionSupport,
    pub rcl_handle: rcl_action_client_t,
    pub goal_response_channels: HashMap<i64, GoalResponseSender>,
    pub goal_response_sequences: HashMap<uuid::Uuid, i64>,
    pub cancel_response_channels:
        HashMap<i64, oneshot::Sender<action_msgs::srv::CancelGoal::Response>>,
    pub canceled_cancel_requests: CanceledCancelQueue,
    pub feedback_senders: HashMap<uuid::Uuid, FeedbackSender<Result<serde_json::Value>>>,
    pub canceled_feedback_goals: CanceledFeedbackQueue,
    pub pending_feedback_goals: Arc<Mutex<PendingFeedbackQueue>>,
    pub result_requests: HashMap<i64, uuid::Uuid>,
    pub result_request_sequences: HashMap<uuid::Uuid, i64>,
    pub result_senders: HashMap<uuid::Uuid, ResultSender>,
    pub canceled_result_goals: CanceledResultQueue,
    pub goal_status: HashMap<uuid::Uuid, GoalStatus>,
    pub feedback_channel_capacity: usize,

    pub poll_available_channels: AvailabilityWaiters,
}

impl WrappedActionClientUntyped {
    fn register_goal_response(
        &mut self, sequence: i64, uuid: uuid::Uuid,
        sender: oneshot::Sender<(bool, builtin_interfaces::msg::Time)>,
    ) {
        debug_assert!(!self.goal_response_channels.contains_key(&sequence));
        debug_assert!(!self.goal_response_sequences.contains_key(&uuid));
        self.goal_response_channels.insert(sequence, (uuid, sender));
        self.goal_response_sequences.insert(uuid, sequence);
    }

    fn take_goal_response_by_sequence(&mut self, sequence: i64) -> Option<GoalResponseSender> {
        let (uuid, sender) = self.goal_response_channels.remove(&sequence)?;
        debug_assert_eq!(self.goal_response_sequences.remove(&uuid), Some(sequence));
        Some((uuid, sender))
    }

    fn take_goal_response_by_uuid(
        &mut self, uuid: &uuid::Uuid,
    ) -> Option<oneshot::Sender<(bool, builtin_interfaces::msg::Time)>> {
        let sequence = self.goal_response_sequences.remove(uuid)?;
        let (stored_uuid, sender) = self.goal_response_channels.remove(&sequence)?;
        debug_assert_eq!(&stored_uuid, uuid);
        Some(sender)
    }

    fn register_result_request(&mut self, sequence: i64, uuid: uuid::Uuid) {
        debug_assert!(!self.result_requests.contains_key(&sequence));
        debug_assert!(!self.result_request_sequences.contains_key(&uuid));
        self.result_requests.insert(sequence, uuid);
        self.result_request_sequences.insert(uuid, sequence);
    }

    fn take_result_request_by_sequence(&mut self, sequence: i64) -> Option<uuid::Uuid> {
        let uuid = self.result_requests.remove(&sequence)?;
        debug_assert_eq!(self.result_request_sequences.remove(&uuid), Some(sequence));
        Some(uuid)
    }

    fn remove_result_request_by_uuid(&mut self, uuid: &uuid::Uuid) {
        if let Some(sequence) = self.result_request_sequences.remove(uuid) {
            debug_assert_eq!(self.result_requests.remove(&sequence), Some(*uuid));
        }
    }

    pub fn get_goal_status(&self, uuid: &uuid::Uuid) -> GoalStatus {
        *self.goal_status.get(uuid).unwrap_or(&GoalStatus::Unknown)
    }

    fn take_feedback_sender(
        &mut self, uuid: &uuid::Uuid,
    ) -> Option<FeedbackSender<Result<serde_json::Value>>> {
        self.feedback_senders.remove(uuid)
    }

    fn forget_goal(&mut self, uuid: &uuid::Uuid) -> Vec<ActionClientCompletion> {
        let mut completions = Vec::new();
        if let Some(sender) = self.take_goal_response_by_uuid(uuid) {
            completions.push(drop_action_client_resources_completion(vec![sender]));
        }
        if let Some(channel) = self.take_feedback_sender(uuid) {
            completions.push(close_feedback_completion(channel));
        }
        if let Some(sender) = self.result_senders.remove(uuid) {
            completions.push(drop_action_client_resources_completion(vec![sender]));
        }
        self.remove_result_request_by_uuid(uuid);
        self.goal_status.remove(uuid);
        completions
    }

    fn forget_result(&mut self, uuid: &uuid::Uuid) -> Option<ActionClientCompletion> {
        self.result_senders.remove(uuid);
        self.remove_result_request_by_uuid(uuid);
        if self.get_goal_status(uuid).is_terminal() {
            return self
                .take_feedback_sender(uuid)
                .map(close_feedback_completion);
        }
        None
    }

    fn finish_goal(
        &mut self, uuid: &uuid::Uuid, status: GoalStatus,
    ) -> Vec<ActionClientCompletion> {
        let feedback = self
            .take_feedback_sender(uuid)
            .map(close_feedback_completion)
            .into_iter()
            .collect();
        self.result_senders.remove(uuid);
        self.remove_result_request_by_uuid(uuid);
        if let Some(tracked_status) = self.goal_status.get_mut(uuid) {
            *tracked_status = status;
        }
        feedback
    }

    fn apply_status_updates(
        &mut self, updates: &[(uuid::Uuid, GoalStatus)],
    ) -> Vec<ActionClientCompletion> {
        let mut completions = Vec::new();
        for (uuid, status) in updates {
            if let Some(tracked_status) = self.goal_status.get_mut(uuid) {
                *tracked_status = *status;
            }
            let result_is_tracked = self
                .result_senders
                .get(uuid)
                .is_some_and(|sender| !sender.is_canceled());
            if status.is_terminal() && !result_is_tracked {
                completions.extend(
                    self.take_feedback_sender(uuid)
                        .map(close_feedback_completion),
                );
            }
        }
        completions
    }

    fn route_feedback(
        &mut self, uuid: &uuid::Uuid, feedback: Box<dyn Any>,
    ) -> Result<Option<ActionClientCompletion>> {
        let feedback = feedback
            .downcast::<Result<serde_json::Value>>()
            .map_err(|_| Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        Ok(self.feedback_senders.get(uuid).and_then(|channel| {
            feedback_completion_with_queue(
                Arc::clone(channel),
                *feedback,
                Some((*uuid, Arc::clone(&self.pending_feedback_goals))),
            )
        }))
    }

    fn prune_canceled_requests(&mut self) -> Vec<ActionClientCompletion> {
        let mut completions = Vec::new();
        self.prune_canceled_cancel_requests();
        self.prune_canceled_feedback();
        let canceled_results =
            take_canceled_result_goals(&self.canceled_result_goals, &self.result_senders);
        for uuid in canceled_results {
            completions.extend(self.forget_result(&uuid));
        }
        completions
    }

    fn prune_canceled_cancel_requests(&mut self) {
        for sequence in take_canceled_cancel_sequences(
            &self.canceled_cancel_requests,
            &self.cancel_response_channels,
        ) {
            self.cancel_response_channels.remove(&sequence);
        }
    }

    fn prune_canceled_feedback(&mut self) {
        for uuid in
            take_canceled_feedback_goals(&self.canceled_feedback_goals, &self.feedback_senders)
        {
            self.feedback_senders.remove(&uuid);
        }
    }

    fn pending_feedback_completion(&mut self) -> PendingFeedbackAction {
        take_ready_pending_feedback(&self.pending_feedback_goals, &self.feedback_senders)
    }

    fn finish_result_request_send(
        &mut self, uuid: uuid::Uuid, result: Result<i64>,
    ) -> (Result<()>, Vec<ActionClientCompletion>) {
        match result {
            Ok(seq_no) => {
                self.register_result_request(seq_no, uuid);
                (Ok(()), Vec::new())
            }
            Err(err) => (Err(err), self.forget_goal(&uuid)),
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
                .insert(seq_no, cancel_req_sender);
            Ok(registered_cancel_response_future(
                cancel_req_receiver,
                Arc::clone(&self.canceled_cancel_requests),
                seq_no,
            ))
        } else {
            log::error!("could not send goal request {}", result);
            Err(Error::from_rcl_error(result))
        }
    }

    fn send_result_request_native(
        &mut self, uuid: uuid::Uuid, native_msg: WrappedNativeMsgUntyped,
    ) -> (Result<()>, Vec<ActionClientCompletion>) {
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

    fn forget_goal(&mut self, uuid: &uuid::Uuid) -> Vec<ActionClientCompletion> {
        WrappedActionClientUntyped::forget_goal(self, uuid)
    }

    fn finish_goal(
        &mut self, uuid: &uuid::Uuid, status: GoalStatus,
    ) -> Vec<ActionClientCompletion> {
        WrappedActionClientUntyped::finish_goal(self, uuid, status)
    }

    fn apply_status_updates(
        &mut self, updates: &[(uuid::Uuid, GoalStatus)],
    ) -> Vec<ActionClientCompletion> {
        WrappedActionClientUntyped::apply_status_updates(self, updates)
    }

    fn route_feedback(
        &mut self, uuid: &uuid::Uuid, feedback: Box<dyn Any>,
    ) -> Result<Option<ActionClientCompletion>> {
        WrappedActionClientUntyped::route_feedback(self, uuid, feedback)
    }

    fn handle_goal_response(
        &mut self, client: Arc<Mutex<dyn ActionClient_>>,
        completions: &mut Vec<ActionClientCompletion>,
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
            if let Some((uuid, sender)) =
                self.take_goal_response_by_sequence(request_id.sequence_number)
            {
                let destructure = self.action_type_support.destructure_goal_response_msg;
                completions.push(action_stateful_response_completion(
                    sender,
                    response_msg,
                    move |response_msg| {
                        let (accept, stamp) = destructure(response_msg);
                        let cleanup = if accept {
                            Vec::new()
                        } else {
                            client.lock().forget_goal(&uuid)
                        };
                        ((accept, stamp), cleanup)
                    },
                    "untyped action goal response",
                    Some(uuid),
                ));
            } else {
                log::error!(
                    "no such req id: {}, we have {:?}, ignoring",
                    request_id.sequence_number,
                    self.goal_response_channels.keys()
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
            if let Some(sender) = self
                .cancel_response_channels
                .remove(&request_id.sequence_number)
            {
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
                log::error!(
                    "no such req id: {}, we have {:?}, ignoring",
                    request_id.sequence_number,
                    self.cancel_response_channels.keys()
                );
            }
            Ok(ActionClientTakeResult::Taken)
        } else {
            Ok(ActionClientTakeResult::Idle)
        }
    }

    fn handle_feedback_msg(
        &mut self, client: Arc<Mutex<dyn ActionClient_>>,
        completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult> {
        match self.pending_feedback_completion() {
            PendingFeedbackAction::Deliver(completion) => {
                completions.push(completion);
                return Ok(ActionClientTakeResult::Taken);
            }
            PendingFeedbackAction::Backpressured => {
                return Ok(ActionClientTakeResult::Backpressured);
            }
            PendingFeedbackAction::None => {}
        }
        let mut feedback_msg = (self.action_type_support.make_feedback_msg)()?;
        let ret =
            unsafe { rcl_action_take_feedback(&self.rcl_handle, feedback_msg.void_ptr_mut()) };
        if action_client_take_succeeded(ret)? {
            let destructure = self.action_type_support.destructure_feedback_msg;
            completions.push(action_feedback_decode_completion(
                client,
                feedback_msg,
                move |feedback_msg| {
                    let (uuid, feedback) = destructure(feedback_msg);
                    Ok((uuid_msg_to_uuid(&uuid)?, Box::new(feedback)))
                },
            ));
            Ok(ActionClientTakeResult::Taken)
        } else {
            Ok(ActionClientTakeResult::Idle)
        }
    }

    fn handle_status_msg(
        &mut self, client: Arc<Mutex<dyn ActionClient_>>,
    ) -> Result<Vec<ActionClientCompletion>> {
        let mut status_array = WrappedNativeMsg::<action_msgs::msg::GoalStatusArray>::try_new()?;
        let ret = unsafe { rcl_action_take_status(&self.rcl_handle, status_array.void_ptr_mut()) };
        if action_client_take_succeeded(ret)? {
            Ok(vec![action_status_completion(client, status_array)])
        } else {
            Ok(Vec::new())
        }
    }

    fn handle_result_response(
        &mut self, client: Arc<Mutex<dyn ActionClient_>>,
        completions: &mut Vec<ActionClientCompletion>,
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
            if let Some(uuid) = self.take_result_request_by_sequence(request_id.sequence_number) {
                if let Some(sender) = self.result_senders.remove(&uuid) {
                    let destructure = self.action_type_support.destructure_result_response_msg;
                    completions.push(action_stateful_response_completion(
                        sender,
                        response_msg,
                        move |response_msg| {
                            let (status, result) = destructure(response_msg);
                            let status = GoalStatus::from_rcl(status);
                            let cleanup = client.lock().finish_goal(&uuid, status);
                            ((status, result), cleanup)
                        },
                        "untyped action result",
                        None,
                    ));
                    return Ok(ActionClientTakeResult::Taken);
                }
            } else {
                log::error!(
                    "no such req id: {}, we have {:?}, ignoring",
                    request_id.sequence_number,
                    self.result_requests.keys()
                );
            }
            Ok(ActionClientTakeResult::Taken)
        } else {
            Ok(ActionClientTakeResult::Idle)
        }
    }

    fn register_poll_available(&mut self, s: oneshot::Sender<()>) -> bool {
        self.poll_available_channels.register(s)
    }

    fn poll_maintenance(&mut self) -> Vec<ActionClientCompletion> {
        let mut completions = self.prune_canceled_requests();
        if let PendingFeedbackAction::Deliver(completion) = self.pending_feedback_completion() {
            completions.push(completion);
        }
        completions
    }

    fn has_pending_maintenance(&self) -> bool {
        !self.canceled_cancel_requests.lock().is_empty()
            || !self.canceled_feedback_goals.lock().is_empty()
            || !self.canceled_result_goals.lock().is_empty()
            || !self.pending_feedback_goals.lock().is_empty()
    }

    fn disable_maintenance_notifications(&self) {
        self.canceled_cancel_requests.lock().disable_notifier();
    }

    fn poll_available(&mut self, node: &mut rcl_node_t) -> Vec<ActionClientCompletion> {
        self.poll_available_channels.prune_canceled();
        if self.poll_available_channels.is_empty() {
            return Vec::new();
        }
        record_action_availability_poll();
        let available = action_server_available_helper(node, self.handle());
        let mut completions = Vec::new();
        if !matches!(available, Ok(false)) {
            let senders = self.poll_available_channels.take_all();
            completions.extend(action_availability_completion(senders, available));
        }
        completions
    }

    fn has_pending_availability(&mut self) -> bool {
        self.poll_available_channels.prune_canceled();
        !self.poll_available_channels.is_empty()
    }

    fn has_pending_work(&mut self) -> bool {
        self.prune_canceled_cancel_requests();
        self.prune_canceled_feedback();
        self.poll_available_channels.prune_canceled();
        !self.goal_response_channels.is_empty()
            || !self.cancel_response_channels.is_empty()
            || !self.feedback_senders.is_empty()
            || !self.result_senders.is_empty()
            || !self.goal_status.is_empty()
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
        if client.register_poll_available(sender) {
            self.lifecycle
                .active_availability_clients
                .lock()
                .push_back(self.lifecycle.identity);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{FutureExt, StreamExt};

    #[test]
    fn pending_work_prunes_dropped_cancel_future() {
        let canceled_cancels = Arc::new(Mutex::new(ActionClientMaintenanceQueue::default()));
        let (sender, receiver) = oneshot::channel();
        let mut client = WrappedActionClientUntyped {
            action_type_support: UntypedActionSupport::new_from(
                "example_interfaces/action/Fibonacci",
            )
            .unwrap(),
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: HashMap::new(),
            goal_response_sequences: HashMap::new(),
            cancel_response_channels: HashMap::from([(10, sender)]),
            canceled_cancel_requests: Arc::clone(&canceled_cancels),
            canceled_feedback_goals: Arc::new(Mutex::new(Default::default())),
            feedback_senders: HashMap::new(),
            pending_feedback_goals: Arc::new(Mutex::new(PendingFeedbackQueue::default())),
            result_requests: HashMap::new(),
            result_request_sequences: HashMap::new(),
            result_senders: HashMap::new(),
            canceled_result_goals: Arc::new(Mutex::new(Default::default())),
            goal_status: HashMap::new(),
            feedback_channel_capacity: 0,
            poll_available_channels: AvailabilityWaiters::default(),
        };
        let future = registered_cancel_response_future(receiver, canceled_cancels, 10);

        drop(future);

        assert!(!client.has_pending_work());
        assert!(client.cancel_response_channels.is_empty());
    }

    #[test]
    fn pending_work_prunes_dropped_feedback_stream() {
        let uuid = uuid::Uuid::from_u128(1);
        let canceled_feedback = Arc::new(Mutex::new(ActionClientMaintenanceQueue::default()));
        let (sender, receiver) = mpsc::channel(1);
        let mut client = WrappedActionClientUntyped {
            action_type_support: UntypedActionSupport::new_from(
                "example_interfaces/action/Fibonacci",
            )
            .unwrap(),
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: HashMap::new(),
            goal_response_sequences: HashMap::new(),
            cancel_response_channels: HashMap::new(),
            canceled_cancel_requests: Arc::new(Mutex::new(Default::default())),
            feedback_senders: HashMap::from([(
                uuid,
                Arc::new(Mutex::new(FeedbackChannel::new(sender))),
            )]),
            canceled_feedback_goals: Arc::clone(&canceled_feedback),
            pending_feedback_goals: Arc::new(Mutex::new(PendingFeedbackQueue::default())),
            result_requests: HashMap::new(),
            result_request_sequences: HashMap::new(),
            result_senders: HashMap::new(),
            canceled_result_goals: Arc::new(Mutex::new(Default::default())),
            goal_status: HashMap::new(),
            feedback_channel_capacity: 1,
            poll_available_channels: AvailabilityWaiters::default(),
        };
        let stream = ActionFeedbackStreamUntyped {
            receiver,
            registration: FeedbackStreamRegistration::new(canceled_feedback, uuid),
        };

        drop(stream);

        assert!(!client.has_pending_work());
        assert!(client.feedback_senders.is_empty());
    }

    #[test]
    fn pending_work_prunes_dropped_availability_waiter() {
        let mut waiters = AvailabilityWaiters::default();
        let (sender, receiver) = oneshot::channel();
        waiters.register(sender);
        let mut client = WrappedActionClientUntyped {
            action_type_support: UntypedActionSupport::new_from(
                "example_interfaces/action/Fibonacci",
            )
            .unwrap(),
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: HashMap::new(),
            goal_response_sequences: HashMap::new(),
            cancel_response_channels: HashMap::new(),
            canceled_cancel_requests: Arc::new(Mutex::new(Default::default())),
            feedback_senders: HashMap::new(),
            canceled_feedback_goals: Arc::new(Mutex::new(Default::default())),
            pending_feedback_goals: Arc::new(Mutex::new(PendingFeedbackQueue::default())),
            result_requests: HashMap::new(),
            result_request_sequences: HashMap::new(),
            result_senders: HashMap::new(),
            canceled_result_goals: Arc::new(Mutex::new(Default::default())),
            goal_status: HashMap::new(),
            feedback_channel_capacity: 1,
            poll_available_channels: waiters,
        };

        drop(receiver);

        assert!(!client.has_pending_work());
        assert!(client.poll_available_channels.is_empty());
    }

    #[test]
    fn feedback_is_routed_only_to_the_indexed_goal() {
        let target = uuid::Uuid::new_v4();
        let other = uuid::Uuid::new_v4();
        let (target_sender, mut target_receiver) = mpsc::channel(1);
        let (other_sender, mut other_receiver) = mpsc::channel(1);
        let mut client = WrappedActionClientUntyped {
            action_type_support: UntypedActionSupport::new_from(
                "example_interfaces/action/Fibonacci",
            )
            .unwrap(),
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: HashMap::new(),
            goal_response_sequences: HashMap::new(),
            cancel_response_channels: HashMap::new(),
            canceled_cancel_requests: Arc::new(Mutex::new(Default::default())),
            canceled_feedback_goals: Arc::new(Mutex::new(Default::default())),
            feedback_senders: HashMap::from([
                (target, Arc::new(Mutex::new(FeedbackChannel::new(target_sender)))),
                (other, Arc::new(Mutex::new(FeedbackChannel::new(other_sender)))),
            ]),
            pending_feedback_goals: Arc::new(Mutex::new(PendingFeedbackQueue::default())),
            result_requests: HashMap::new(),
            result_request_sequences: HashMap::new(),
            result_senders: HashMap::new(),
            canceled_result_goals: Arc::new(Mutex::new(Default::default())),
            goal_status: HashMap::new(),
            feedback_channel_capacity: 1,
            poll_available_channels: AvailabilityWaiters::default(),
        };
        let expected = serde_json::json!({ "sequence": [1, 2, 3] });

        let completion = client
            .route_feedback(&target, Box::new(Result::<serde_json::Value>::Ok(expected.clone())))
            .unwrap()
            .unwrap();
        run_detached_action_client_completions(vec![completion]);
        let delivered = target_receiver
            .next()
            .now_or_never()
            .flatten()
            .unwrap()
            .unwrap();
        assert_eq!(delivered, expected);
        assert!(other_receiver.next().now_or_never().is_none());
    }

    #[test]
    fn forgetting_pending_goal_uses_and_preserves_bidirectional_indexes() {
        let target = uuid::Uuid::new_v4();
        let other = uuid::Uuid::new_v4();
        let (target_sender, mut target_receiver) =
            oneshot::channel::<(bool, builtin_interfaces::msg::Time)>();
        let (other_sender, mut other_receiver) =
            oneshot::channel::<(bool, builtin_interfaces::msg::Time)>();
        let mut client = WrappedActionClientUntyped {
            action_type_support: UntypedActionSupport::new_from(
                "example_interfaces/action/Fibonacci",
            )
            .unwrap(),
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: HashMap::from([
                (10, (target, target_sender)),
                (11, (other, other_sender)),
            ]),
            goal_response_sequences: HashMap::from([(target, 10), (other, 11)]),
            cancel_response_channels: HashMap::new(),
            canceled_cancel_requests: Arc::new(Mutex::new(Default::default())),
            canceled_feedback_goals: Arc::new(Mutex::new(Default::default())),
            feedback_senders: HashMap::new(),
            pending_feedback_goals: Arc::new(Mutex::new(PendingFeedbackQueue::default())),
            result_requests: HashMap::new(),
            result_request_sequences: HashMap::new(),
            result_senders: HashMap::new(),
            canceled_result_goals: Arc::new(Mutex::new(Default::default())),
            goal_status: HashMap::from([
                (target, GoalStatus::Unknown),
                (other, GoalStatus::Unknown),
            ]),
            feedback_channel_capacity: 0,
            poll_available_channels: AvailabilityWaiters::default(),
        };

        let completions = client.forget_goal(&target);

        assert!(!client.goal_response_channels.contains_key(&10));
        assert!(client.goal_response_channels.contains_key(&11));
        assert!(!client.goal_response_sequences.contains_key(&target));
        assert_eq!(client.goal_response_sequences.get(&other), Some(&11));
        assert_eq!(target_receiver.try_recv().unwrap(), None);
        assert_eq!(other_receiver.try_recv().unwrap(), None);

        run_detached_action_client_completions(completions);
        assert!(matches!(target_receiver.try_recv(), Err(oneshot::Canceled)));
        assert_eq!(other_receiver.try_recv().unwrap(), None);
    }

    #[test]
    fn dropping_pending_goal_registration_removes_closed_response() {
        let uuid = uuid::Uuid::new_v4();
        let (sender, receiver) = oneshot::channel::<(bool, builtin_interfaces::msg::Time)>();
        let client = Arc::new(Mutex::new(WrappedActionClientUntyped {
            action_type_support: UntypedActionSupport::new_from(
                "example_interfaces/action/Fibonacci",
            )
            .unwrap(),
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: HashMap::from([(10, (uuid, sender))]),
            goal_response_sequences: HashMap::from([(uuid, 10)]),
            cancel_response_channels: HashMap::new(),
            canceled_cancel_requests: Arc::new(Mutex::new(Default::default())),
            canceled_feedback_goals: Arc::new(Mutex::new(Default::default())),
            feedback_senders: HashMap::new(),
            pending_feedback_goals: Arc::new(Mutex::new(PendingFeedbackQueue::default())),
            result_requests: HashMap::new(),
            result_request_sequences: HashMap::new(),
            result_senders: HashMap::new(),
            canceled_result_goals: Arc::new(Mutex::new(Default::default())),
            goal_status: HashMap::from([(uuid, GoalStatus::Unknown)]),
            feedback_channel_capacity: 0,
            poll_available_channels: AvailabilityWaiters::default(),
        }));
        let registration = PendingGoalRegistration {
            client: Arc::downgrade(&client),
            uuid,
            armed: true,
        };

        drop(receiver);
        drop(registration);

        let mut client = client.lock();
        assert!(client.goal_response_channels.is_empty());
        assert!(client.goal_response_sequences.is_empty());
        assert!(!client.goal_status.contains_key(&uuid));
        assert!(!client.has_pending_work());
    }

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
            goal_response_channels: HashMap::new(),
            goal_response_sequences: HashMap::new(),
            cancel_response_channels: HashMap::new(),
            canceled_cancel_requests: Arc::new(Mutex::new(Default::default())),
            canceled_feedback_goals: Arc::new(Mutex::new(Default::default())),
            feedback_senders: HashMap::from([
                (
                    target,
                    Arc::new(Mutex::new(FeedbackChannel {
                        sender: Some(target_feedback),
                        pending: Some(Ok(serde_json::json!({ "sequence": [1] }))),
                        closing: false,
                    })),
                ),
                (other, Arc::new(Mutex::new(FeedbackChannel::new(other_feedback)))),
            ]),
            pending_feedback_goals: Arc::new(Mutex::new(PendingFeedbackQueue::default())),
            result_requests: HashMap::from([(10, target), (11, other)]),
            result_request_sequences: HashMap::from([(target, 10), (other, 11)]),
            result_senders: HashMap::from([(target, target_result), (other, other_result)]),
            canceled_result_goals: Arc::new(Mutex::new(Default::default())),
            goal_status: HashMap::from([
                (target, GoalStatus::Accepted),
                (other, GoalStatus::Executing),
            ]),
            feedback_channel_capacity: 1,
            poll_available_channels: AvailabilityWaiters::default(),
        };

        let (result, completions) =
            client.finish_result_request_send(target, Err(Error::RCL_RET_BAD_ALLOC));
        assert!(matches!(result, Err(Error::RCL_RET_BAD_ALLOC)));
        run_detached_action_client_completions(completions);
        assert!(!client.feedback_senders.contains_key(&target));
        assert!(client.feedback_senders.contains_key(&other));
        assert!(!client
            .result_requests
            .iter()
            .any(|(_, uuid)| uuid == &target));
        assert!(client
            .result_requests
            .iter()
            .any(|(_, uuid)| uuid == &other));
        assert!(!client.result_request_sequences.contains_key(&target));
        assert_eq!(client.result_request_sequences.get(&other), Some(&11));
        assert!(!client.result_senders.contains_key(&target));
        assert!(client.result_senders.contains_key(&other));
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
            goal_response_channels: HashMap::from([(12, (other, other_goal))]),
            goal_response_sequences: HashMap::from([(other, 12)]),
            cancel_response_channels: HashMap::new(),
            canceled_cancel_requests: Arc::new(Mutex::new(Default::default())),
            canceled_feedback_goals: Arc::new(Mutex::new(Default::default())),
            feedback_senders: HashMap::from([
                (
                    target,
                    Arc::new(Mutex::new(FeedbackChannel {
                        sender: Some(target_feedback),
                        pending: Some(Ok(serde_json::json!({ "sequence": [1] }))),
                        closing: false,
                    })),
                ),
                (other, Arc::new(Mutex::new(FeedbackChannel::new(other_feedback)))),
            ]),
            pending_feedback_goals: Arc::new(Mutex::new(PendingFeedbackQueue::default())),
            result_requests: HashMap::new(),
            result_request_sequences: HashMap::new(),
            result_senders: HashMap::from([(target, target_result), (other, other_result)]),
            canceled_result_goals: Arc::new(Mutex::new(Default::default())),
            goal_status: HashMap::from([
                (target, GoalStatus::Accepted),
                (other, GoalStatus::Unknown),
            ]),
            feedback_channel_capacity: 1,
            poll_available_channels: AvailabilityWaiters::default(),
        }));
        let pending = PendingGoalRegistration {
            client: Arc::downgrade(&client),
            uuid: target,
            armed: true,
        };

        drop(pending);

        let client = client.lock();
        assert!(!client.feedback_senders.contains_key(&target));
        assert!(client.feedback_senders.contains_key(&other));
        assert!(!client.result_senders.contains_key(&target));
        assert!(client.result_senders.contains_key(&other));
        assert!(!client.goal_status.contains_key(&target));
        assert!(client.goal_status.contains_key(&other));
        assert_eq!(client.goal_response_channels.len(), 1);
        assert_eq!(client.goal_response_sequences, HashMap::from([(other, 12)]));
    }
}
