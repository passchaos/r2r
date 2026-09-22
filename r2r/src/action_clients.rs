use futures::{
    channel::{mpsc, oneshot},
    future::TryFutureExt,
    stream::Stream,
};
use parking_lot::Mutex;
use std::{
    collections::HashMap,
    ffi::{CStr, CString},
    future::Future,
    mem::MaybeUninit,
    pin::Pin,
    sync::{Arc, Weak},
    task::{Context, Poll},
};

use crate::{
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

pub(crate) struct ActionClientCompletion {
    notify: Box<dyn FnOnce() -> bool + Send>,
    cleanup_goal_on_failure: Option<uuid::Uuid>,
}

pub(crate) fn complete_action_client_operation<T: ?Sized>(
    client: &Mutex<T>, operation: impl FnOnce(&mut T) -> Result<Vec<ActionClientCompletion>>,
    mut cleanup_goal: impl FnMut(&mut T, &uuid::Uuid),
) -> Result<()> {
    let completions = { operation(&mut *client.lock())? };
    for completion in completions {
        let ActionClientCompletion {
            notify,
            cleanup_goal_on_failure,
        } = completion;
        if !notify() {
            if let Some(uuid) = cleanup_goal_on_failure {
                cleanup_goal(&mut *client.lock(), &uuid);
            }
        }
    }
    Ok(())
}

pub(crate) fn action_response_completion<T: Send + 'static>(
    sender: oneshot::Sender<T>, response: T, description: &'static str,
    cleanup_goal_on_failure: Option<uuid::Uuid>,
) -> ActionClientCompletion {
    ActionClientCompletion {
        notify: Box::new(move || {
            if sender.send(response).is_err() {
                log::debug!("could not deliver {description}");
                false
            } else {
                true
            }
        }),
        cleanup_goal_on_failure,
    }
}

pub(crate) fn action_availability_completion(
    senders: Vec<oneshot::Sender<()>>, available: Result<bool>,
) -> Option<ActionClientCompletion> {
    match available {
        Ok(false) => None,
        Ok(true) => Some(ActionClientCompletion {
            notify: Box::new(move || {
                for sender in senders {
                    let _ = sender.send(());
                }
                true
            }),
            cleanup_goal_on_failure: None,
        }),
        Err(_) => Some(ActionClientCompletion {
            notify: Box::new(move || {
                drop(senders);
                true
            }),
            cleanup_goal_on_failure: None,
        }),
    }
}

unsafe impl<T> Send for ActionClient<T> where T: WrappedActionTypeSupport {}

/// Action client
///
/// Use this to make goal requests to an action server.
#[derive(Clone)]
pub struct ActionClient<T>
where
    T: WrappedActionTypeSupport,
{
    client: Weak<Mutex<WrappedActionClient<T>>>,
}

unsafe impl<T> Send for ActionClientGoal<T> where T: WrappedActionTypeSupport {}

/// Action client goal handle
///
/// This can be used to cancel goals and query the status of goals.
pub struct ActionClientGoal<T>
where
    T: WrappedActionTypeSupport,
{
    client: Weak<Mutex<WrappedActionClient<T>>>,
    pub uuid: uuid::Uuid,
    accepted_at: builtin_interfaces::msg::Time,
    status_registration: Arc<GoalStatusRegistration<T>>,
}

/// Result future for an accepted typed action goal.
pub struct ActionResultFuture<T>
where
    T: WrappedActionTypeSupport,
{
    receiver: oneshot::Receiver<(GoalStatus, T::Result)>,
}

impl<T> Future for ActionResultFuture<T>
where
    T: WrappedActionTypeSupport,
{
    type Output = Result<(GoalStatus, T::Result)>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.receiver)
            .poll(cx)
            .map(|result| result.map_err(|_| Error::RCL_RET_ACTION_CLIENT_INVALID))
    }
}

/// Feedback stream for an accepted typed action goal.
pub struct ActionFeedbackStream<T>
where
    T: WrappedActionTypeSupport,
{
    receiver: mpsc::Receiver<T::Feedback>,
}

impl<T> Stream for ActionFeedbackStream<T>
where
    T: WrappedActionTypeSupport,
{
    type Item = T::Feedback;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}

/// Accepted typed action goal and the client-side channels requested for it.
pub struct ActionClientGoalResponse<T>
where
    T: WrappedActionTypeSupport,
{
    /// Handle for querying or canceling the accepted goal.
    pub goal: ActionClientGoal<T>,
    /// Eventual result when result tracking was requested.
    pub result: Option<ActionResultFuture<T>>,
    /// Feedback messages when feedback tracking was requested.
    pub feedback: Option<ActionFeedbackStream<T>>,
}

struct GoalStatusRegistration<T>
where
    T: WrappedActionTypeSupport,
{
    client: Weak<Mutex<WrappedActionClient<T>>>,
    uuid: uuid::Uuid,
}

struct PendingGoalRegistration<T>
where
    T: WrappedActionTypeSupport,
{
    client: Weak<Mutex<WrappedActionClient<T>>>,
    uuid: uuid::Uuid,
    armed: bool,
}

impl<T> PendingGoalRegistration<T>
where
    T: WrappedActionTypeSupport,
{
    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl<T> Drop for PendingGoalRegistration<T>
where
    T: WrappedActionTypeSupport,
{
    fn drop(&mut self) {
        if self.armed {
            if let Some(client) = self.client.upgrade() {
                client.lock().forget_goal(&self.uuid);
            }
        }
    }
}

impl<T> Drop for GoalStatusRegistration<T>
where
    T: WrappedActionTypeSupport,
{
    fn drop(&mut self) {
        if let Some(client) = self.client.upgrade() {
            client.lock().goal_status.remove(&self.uuid);
        }
    }
}

impl<T> Clone for ActionClientGoal<T>
where
    T: WrappedActionTypeSupport,
{
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            uuid: self.uuid,
            accepted_at: self.accepted_at.clone(),
            status_registration: self.status_registration.clone(),
        }
    }
}

impl<T: 'static> ActionClientGoal<T>
where
    T: WrappedActionTypeSupport,
{
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

        let response = client.send_cancel_request(&self.uuid)?;
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

impl<T: 'static> ActionClient<T>
where
    T: WrappedActionTypeSupport,
{
    /// Requests cancellation of every cancelable goal known to the action server.
    pub fn cancel_all(&self) -> Result<impl Future<Output = Result<Vec<uuid::Uuid>>>> {
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let mut client = client.lock();
        client.send_cancel_request_for(batch_cancel_goal_info(Default::default()))
    }

    /// Requests cancellation of every goal accepted at or before the supplied stamp.
    pub fn cancel_before(
        &self, stamp: builtin_interfaces::msg::Time,
    ) -> Result<impl Future<Output = Result<Vec<uuid::Uuid>>>> {
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let mut client = client.lock();
        client.send_cancel_request_for(batch_cancel_goal_info(stamp))
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
    /// - A new future for the eventual result.
    /// - A stream of feedback messages.
    #[allow(clippy::type_complexity)]
    pub fn send_goal_request(
        &self, goal: T::Goal,
    ) -> Result<
        impl Future<
            Output = Result<(ActionClientGoal<T>, ActionResultFuture<T>, ActionFeedbackStream<T>)>,
        >,
    >
    where
        T: WrappedActionTypeSupport,
    {
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

    /// Sends a goal with explicit control over result and feedback tracking.
    ///
    /// Disabling result tracking also suppresses the result request normally sent immediately
    /// after acceptance. Disabled result and feedback channels are returned as [`None`].
    pub fn send_goal_request_with_options(
        &self, goal: T::Goal, options: ActionGoalOptions,
    ) -> Result<impl Future<Output = Result<ActionClientGoalResponse<T>>>>
    where
        T: WrappedActionTypeSupport,
    {
        let client = self
            .client
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let mut client = client.lock();

        let uuid = uuid::Uuid::new_v4();
        let uuid_msg = unique_identifier_msgs::msg::UUID {
            uuid: uuid.as_bytes().to_vec(),
        };
        let request_msg = T::make_goal_request_msg(uuid_msg, goal);
        let native_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Request,
        >::try_from(&request_msg)?;
        let mut seq_no = 0i64;
        let result = unsafe {
            rcl_action_send_goal_request(&client.rcl_handle, native_msg.void_ptr(), &mut seq_no)
        };

        if result == RCL_RET_OK as i32 {
            let (goal_req_sender, goal_req_receiver) =
                oneshot::channel::<(bool, builtin_interfaces::msg::Time)>();
            let feedback = options.track_feedback.then(|| {
                let (sender, receiver) =
                    mpsc::channel::<T::Feedback>(client.feedback_channel_capacity);
                client.feedback_senders.push((uuid, sender));
                ActionFeedbackStream { receiver }
            });
            let result = options.track_result.then(|| {
                let (sender, receiver) = oneshot::channel::<(GoalStatus, T::Result)>();
                client.result_senders.push((uuid, sender));
                ActionResultFuture { receiver }
            });
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
                            if options.track_result {
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

                            Ok(ActionClientGoalResponse {
                                goal: ActionClientGoal {
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
            };
            Ok(future)
        } else {
            log::error!("could not send goal request {}", result);
            Err(Error::from_rcl_error(result))
        }
    }

    /// Make a new goal request and wait up to `timeout` for acceptance.
    ///
    /// The timeout only covers the goal response. Wrap the returned result future with
    /// [`Node::with_timeout`] when a separate execution deadline is required.
    #[allow(clippy::type_complexity)]
    pub fn send_goal_request_with_timeout(
        &self, node: &mut Node, goal: T::Goal, timeout: std::time::Duration,
    ) -> Result<
        impl Future<
            Output = Result<(
                ActionClientGoal<T>,
                impl Future<Output = Result<(GoalStatus, T::Result)>>,
                impl Stream<Item = T::Feedback> + Unpin,
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

pub fn make_action_client<T>(client: Weak<Mutex<WrappedActionClient<T>>>) -> ActionClient<T>
where
    T: WrappedActionTypeSupport,
{
    ActionClient { client }
}

pub type ResultSender<R> = (uuid::Uuid, oneshot::Sender<(GoalStatus, R)>);
pub struct WrappedActionClient<T>
where
    T: WrappedActionTypeSupport,
{
    pub rcl_handle: rcl_action_client_t,
    pub goal_response_channels: Vec<(i64, uuid::Uuid, GoalResponseSender)>,
    pub cancel_response_channels:
        Vec<(i64, oneshot::Sender<action_msgs::srv::CancelGoal::Response>)>,
    pub feedback_senders: Vec<(uuid::Uuid, mpsc::Sender<T::Feedback>)>,
    pub pending_feedback: Option<(uuid::Uuid, T::Feedback)>,
    pub result_requests: Vec<(i64, uuid::Uuid)>,
    pub result_senders: Vec<ResultSender<T::Result>>,
    pub goal_status: HashMap<uuid::Uuid, GoalStatus>,
    pub feedback_channel_capacity: usize,

    pub poll_available_channels: Vec<oneshot::Sender<()>>,
}

pub trait ActionClient_ {
    fn handle(&self) -> &rcl_action_client_t;
    fn destroy(&mut self, node: &mut rcl_node_t);
    fn has_pending_work(&mut self) -> bool;
    fn forget_goal(&mut self, uuid: &uuid::Uuid);

    fn handle_goal_response(&mut self) -> Result<Option<ActionClientCompletion>>;
    fn handle_cancel_response(&mut self) -> Result<Option<ActionClientCompletion>>;
    fn handle_feedback_msg(&mut self) -> Result<()>;
    fn handle_status_msg(&mut self) -> Result<()>;
    fn handle_result_response(&mut self) -> Result<Option<ActionClientCompletion>>;

    fn send_result_request(&mut self, uuid: uuid::Uuid) -> Result<()>;

    fn register_poll_available(&mut self, s: oneshot::Sender<()>);
    fn poll_available(&mut self, node: &mut rcl_node_t) -> Option<ActionClientCompletion>;
}

impl<T> WrappedActionClient<T>
where
    T: WrappedActionTypeSupport,
{
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
        if self.get_goal_status(uuid).is_terminal() {
            self.feedback_senders.retain(|(id, _)| id != uuid);
            if self
                .pending_feedback
                .as_ref()
                .is_some_and(|(id, _)| id == uuid)
            {
                self.pending_feedback = None;
            }
        }
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
    ) -> Result<impl Future<Output = Result<Vec<uuid::Uuid>>>>
    where
        T: WrappedActionTypeSupport,
    {
        self.send_cancel_request_for(action_msgs::msg::GoalInfo {
            goal_id: unique_identifier_msgs::msg::UUID {
                uuid: goal.as_bytes().to_vec(),
            },
            ..Default::default()
        })
    }

    fn send_cancel_request_for(
        &mut self, goal_info: action_msgs::msg::GoalInfo,
    ) -> Result<impl Future<Output = Result<Vec<uuid::Uuid>>>> {
        let msg = action_msgs::srv::CancelGoal::Request { goal_info };
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

impl<T: 'static> ActionClient_ for WrappedActionClient<T>
where
    T: WrappedActionTypeSupport,
{
    fn handle(&self) -> &rcl_action_client_t {
        &self.rcl_handle
    }

    fn forget_goal(&mut self, uuid: &uuid::Uuid) {
        WrappedActionClient::forget_goal(self, uuid);
    }

    fn handle_goal_response(&mut self) -> Result<Option<ActionClientCompletion>> {
        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut response_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response,
        >::try_new()?;

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
                let response = <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response::from_native(&response_msg);
                let (accept, stamp) = T::destructure_goal_response_msg(response);
                if !accept {
                    self.forget_goal(&uuid);
                }
                return Ok(Some(action_response_completion(
                    sender,
                    (accept, stamp),
                    "action goal response",
                    Some(uuid),
                )));
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
        Ok(None)
    }

    fn handle_cancel_response(&mut self) -> Result<Option<ActionClientCompletion>> {
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
                return Ok(Some(action_response_completion(
                    sender,
                    response,
                    "action cancel response",
                    None,
                )));
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
        Ok(None)
    }

    fn handle_feedback_msg(&mut self) -> Result<()> {
        if !self.flush_pending_feedback() {
            return Ok(());
        }
        let mut feedback_msg = WrappedNativeMsg::<T::FeedbackMessage>::try_new()?;
        let ret =
            unsafe { rcl_action_take_feedback(&self.rcl_handle, feedback_msg.void_ptr_mut()) };
        if action_client_take_succeeded(ret)? {
            let msg = T::FeedbackMessage::from_native(&feedback_msg);
            let (uuid, feedback) = T::destructure_feedback_msg(msg);
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
                let status = GoalStatus::from_rcl(a.status);
                if let Some(tracked_status) = self.goal_status.get_mut(&uuid) {
                    *tracked_status = status;
                }
                let result_is_tracked = self
                    .result_senders
                    .iter()
                    .any(|(id, sender)| id == &uuid && !sender.is_canceled());
                if status.is_terminal() && !result_is_tracked {
                    self.feedback_senders.retain(|(id, _)| id != &uuid);
                    if self
                        .pending_feedback
                        .as_ref()
                        .is_some_and(|(id, _)| id == &uuid)
                    {
                        self.pending_feedback = None;
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_result_response(&mut self) -> Result<Option<ActionClientCompletion>> {
        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut response_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response,
        >::try_new()?;

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
                    let response = <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response::from_native(&response_msg);
                    let (status, result) = T::destructure_result_response_msg(response);
                    let status = GoalStatus::from_rcl(status);
                    self.finish_goal(&uuid, status);
                    return Ok(Some(action_response_completion(
                        sender,
                        (status, result),
                        "action result",
                        None,
                    )));
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
        Ok(None)
    }

    fn send_result_request(&mut self, uuid: uuid::Uuid) -> Result<()> {
        let result = (|| {
            let uuid_msg = unique_identifier_msgs::msg::UUID {
                uuid: uuid.as_bytes().to_vec(),
            };
            let request_msg = T::make_result_request_msg(uuid_msg);
            let native_msg = WrappedNativeMsg::<
                <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Request,
            >::try_from(&request_msg)?;
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

    fn poll_available(&mut self, node: &mut rcl_node_t) -> Option<ActionClientCompletion> {
        self.prune_canceled_requests();
        self.flush_pending_feedback();
        self.poll_available_channels
            .retain(|sender| !sender.is_canceled());
        if self.poll_available_channels.is_empty() {
            return None;
        }
        let available = action_server_available_helper(node, self.handle());
        if matches!(available, Ok(false)) {
            None
        } else {
            let senders = std::mem::take(&mut self.poll_available_channels);
            action_availability_completion(senders, available)
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

pub(crate) fn batch_cancel_goal_info(
    stamp: builtin_interfaces::msg::Time,
) -> action_msgs::msg::GoalInfo {
    action_msgs::msg::GoalInfo {
        goal_id: unique_identifier_msgs::msg::UUID { uuid: vec![0; 16] },
        stamp,
    }
}

pub(crate) fn action_client_take_succeeded(ret: rcl_ret_t) -> Result<bool> {
    if ret == RCL_RET_OK as i32 {
        Ok(true)
    } else if ret == RCL_RET_ACTION_CLIENT_TAKE_FAILED as i32 {
        Ok(false)
    } else {
        Err(Error::from_rcl_error(ret))
    }
}

pub(crate) async fn cancel_response_future(
    receiver: oneshot::Receiver<action_msgs::srv::CancelGoal::Response>,
) -> Result<Vec<uuid::Uuid>> {
    let response = receiver
        .await
        .map_err(|_| Error::RCL_RET_ACTION_CLIENT_INVALID)?;
    match response.return_code {
        code if code == action_msgs::srv::CancelGoal::Response::ERROR_NONE as i8 => response
            .goals_canceling
            .into_iter()
            .map(|goal| uuid_msg_to_uuid(&goal.goal_id))
            .collect(),
        code if code == action_msgs::srv::CancelGoal::Response::ERROR_REJECTED as i8 => {
            Err(Error::GoalCancelRejected)
        }
        code if code == action_msgs::srv::CancelGoal::Response::ERROR_UNKNOWN_GOAL_ID as i8 => {
            Err(Error::GoalCancelUnknownGoalID)
        }
        code if code == action_msgs::srv::CancelGoal::Response::ERROR_GOAL_TERMINATED as i8 => {
            Err(Error::GoalCancelAlreadyTerminated)
        }
        code => Err(Error::UnknownActionCancelResponse(code)),
    }
}

pub fn create_action_client_helper(
    node: &mut rcl_node_t, action_name: &str, action_ts: *const rosidl_action_type_support_t,
    overrides: &ActionClientOptions,
) -> Result<rcl_action_client_t> {
    let mut client_handle = unsafe { rcl_action_get_zero_initialized_client() };
    let action_name_c_string =
        CString::new(action_name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;

    let result = unsafe {
        let mut client_options = rcl_action_client_get_default_options();
        overrides.apply(&mut client_options)?;
        rcl_action_client_init(
            &mut client_handle,
            node,
            action_ts,
            action_name_c_string.as_ptr(),
            &client_options,
        )
    };
    if result == RCL_RET_OK as i32 {
        Ok(client_handle)
    } else {
        Err(Error::from_rcl_error(result))
    }
}

pub fn action_client_get_num_waits(
    rcl_handle: &rcl_action_client_t, num_subs: &mut usize, num_gc: &mut usize,
    num_timers: &mut usize, num_clients: &mut usize, num_services: &mut usize,
) -> Result<()> {
    unsafe {
        let result = rcl_action_client_wait_set_get_num_entities(
            rcl_handle,
            num_subs,
            num_gc,
            num_timers,
            num_clients,
            num_services,
        );
        if result == RCL_RET_OK as i32 {
            Ok(())
        } else {
            Err(Error::from_rcl_error(result))
        }
    }
}

use crate::nodes::IsAvailablePollable;

impl<T: 'static> IsAvailablePollable for ActionClient<T>
where
    T: WrappedActionTypeSupport,
{
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

pub fn action_server_available_helper(
    node: &rcl_node_t, client: &rcl_action_client_t,
) -> Result<bool> {
    let mut avail = false;
    let result = unsafe { rcl_action_server_is_available(node, client, &mut avail) };

    if result == RCL_RET_OK as i32 {
        Ok(avail)
    } else {
        log::error!("could not check if action server is available {}", result);
        Err(Error::from_rcl_error(result))
    }
}

pub(crate) fn action_client_name(client: &rcl_action_client_t) -> Result<String> {
    let name = unsafe { rcl_action_client_get_action_name(client) };
    if name.is_null() {
        return Err(Error::RCL_RET_ACTION_CLIENT_INVALID);
    }
    unsafe { CStr::from_ptr(name) }
        .to_str()
        .map(str::to_owned)
        .map_err(|err| Error::InvalidUtf8 {
            context: "action client name",
            err: err.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::example_interfaces::action::Fibonacci;
    use futures::{
        task::{waker_ref, ArcWake},
        Future,
    };
    use std::{
        pin::Pin,
        sync::atomic::{AtomicBool, Ordering},
        task::{Context, Poll},
    };

    struct LockingWake {
        client: Arc<Mutex<()>>,
        woke: AtomicBool,
    }

    impl ArcWake for LockingWake {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            let _client = arc_self.client.lock();
            arc_self.woke.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn action_completions_wake_after_releasing_the_client_lock() {
        let client = Arc::new(Mutex::new(()));
        let (sender, mut receiver) = oneshot::channel();
        let wake = Arc::new(LockingWake {
            client: Arc::clone(&client),
            woke: AtomicBool::new(false),
        });
        let waker = waker_ref(&wake);
        let mut context = Context::from_waker(&waker);
        assert!(matches!(Pin::new(&mut receiver).poll(&mut context), Poll::Pending));

        complete_action_client_operation(
            &client,
            |_| {
                Ok(vec![action_response_completion(
                    sender,
                    (),
                    "test response",
                    None,
                )])
            },
            |_, _| {},
        )
        .unwrap();

        assert!(wake.woke.load(Ordering::SeqCst));
        assert!(matches!(Pin::new(&mut receiver).poll(&mut context), Poll::Ready(Ok(()))));
    }

    #[test]
    fn failed_goal_completion_cleans_registration_after_relocking() {
        let target = uuid::Uuid::new_v4();
        let (feedback_sender, _feedback_receiver) = mpsc::channel(1);
        let (result_sender, _result_receiver) = oneshot::channel();
        let client = Mutex::new(WrappedActionClient::<Fibonacci::Action> {
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: Vec::new(),
            cancel_response_channels: Vec::new(),
            feedback_senders: vec![(target, feedback_sender)],
            pending_feedback: Some((target, Fibonacci::Feedback::default())),
            result_requests: vec![(1, target)],
            result_senders: vec![(target, result_sender)],
            goal_status: HashMap::from([(target, GoalStatus::Accepted)]),
            feedback_channel_capacity: 1,
            poll_available_channels: Vec::new(),
        });
        let (sender, receiver) = oneshot::channel();
        drop(receiver);

        complete_action_client_operation(
            &client,
            |_| {
                Ok(vec![action_response_completion(
                    sender,
                    (),
                    "test goal response",
                    Some(target),
                )])
            },
            |client, uuid| client.forget_goal(uuid),
        )
        .unwrap();

        let client = client.lock();
        assert!(client.feedback_senders.is_empty());
        assert!(client.pending_feedback.is_none());
        assert!(client.result_requests.is_empty());
        assert!(client.result_senders.is_empty());
        assert!(!client.goal_status.contains_key(&target));
    }

    #[test]
    fn classifies_action_client_take_results() {
        assert!(action_client_take_succeeded(RCL_RET_OK as i32).unwrap());
        assert!(!action_client_take_succeeded(RCL_RET_ACTION_CLIENT_TAKE_FAILED as i32).unwrap());
        assert!(matches!(
            action_client_take_succeeded(RCL_RET_ACTION_CLIENT_INVALID as i32),
            Err(Error::RCL_RET_ACTION_CLIENT_INVALID)
        ));
    }

    #[test]
    fn closed_cancel_channel_reports_action_client_invalid() {
        let (sender, receiver) = oneshot::channel();
        drop(sender);

        assert!(matches!(
            futures::executor::block_on(cancel_response_future(receiver)),
            Err(Error::RCL_RET_ACTION_CLIENT_INVALID)
        ));
    }

    #[test]
    fn batch_cancel_response_decodes_all_goal_ids_strictly() {
        let first = uuid::Uuid::from_u128(1);
        let second = uuid::Uuid::from_u128(2);
        let (sender, receiver) = oneshot::channel();
        sender
            .send(action_msgs::srv::CancelGoal::Response {
                return_code: action_msgs::srv::CancelGoal::Response::ERROR_NONE as i8,
                goals_canceling: [first, second]
                    .into_iter()
                    .map(|uuid| action_msgs::msg::GoalInfo {
                        goal_id: unique_identifier_msgs::msg::UUID {
                            uuid: uuid.as_bytes().to_vec(),
                        },
                        ..Default::default()
                    })
                    .collect(),
            })
            .unwrap();
        assert_eq!(
            futures::executor::block_on(cancel_response_future(receiver)).unwrap(),
            vec![first, second]
        );

        let (sender, receiver) = oneshot::channel();
        sender
            .send(action_msgs::srv::CancelGoal::Response {
                return_code: action_msgs::srv::CancelGoal::Response::ERROR_NONE as i8,
                goals_canceling: vec![action_msgs::msg::GoalInfo {
                    goal_id: unique_identifier_msgs::msg::UUID { uuid: vec![0; 15] },
                    ..Default::default()
                }],
            })
            .unwrap();
        assert!(matches!(
            futures::executor::block_on(cancel_response_future(receiver)),
            Err(Error::InvalidActionUuidLength { actual: 15 })
        ));

        let stamp = builtin_interfaces::msg::Time {
            sec: 42,
            nanosec: 7,
        };
        let request = batch_cancel_goal_info(stamp.clone());
        assert_eq!(request.goal_id.uuid, vec![0; 16]);
        assert_eq!(request.stamp, stamp);
    }

    #[test]
    fn failed_result_request_clears_only_the_affected_goal() {
        let target = uuid::Uuid::new_v4();
        let other = uuid::Uuid::new_v4();
        let (target_feedback, _) = mpsc::channel(1);
        let (other_feedback, _) = mpsc::channel(1);
        let (target_result, _) = oneshot::channel();
        let (other_result, _) = oneshot::channel();
        let mut client = WrappedActionClient::<Fibonacci::Action> {
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: Vec::new(),
            cancel_response_channels: Vec::new(),
            feedback_senders: vec![(target, target_feedback), (other, other_feedback)],
            pending_feedback: Some((target, Fibonacci::Feedback::default())),
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
        let client = Arc::new(Mutex::new(WrappedActionClient::<Fibonacci::Action> {
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            // The target response has already been delivered, so only the other request remains.
            goal_response_channels: vec![(12, other, other_goal)],
            cancel_response_channels: Vec::new(),
            feedback_senders: vec![(target, target_feedback), (other, other_feedback)],
            pending_feedback: Some((target, Fibonacci::Feedback::default())),
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
