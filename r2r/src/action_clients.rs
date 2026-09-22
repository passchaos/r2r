use futures::{
    channel::{mpsc, oneshot},
    future::TryFutureExt,
    stream::Stream,
};
use parking_lot::Mutex;
use std::{
    any::Any,
    collections::{HashMap, VecDeque},
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
    notify: Box<dyn FnOnce() -> Result<bool>>,
    cleanup_goal_on_failure: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionClientTakeResult {
    Idle,
    Taken,
    Backpressured,
}

pub(crate) enum PendingFeedbackAction {
    None,
    Backpressured,
    Deliver(ActionClientCompletion),
}

const MAX_ACTION_RESPONSES_PER_SPIN: usize = 16;
const MAX_ACTION_FEEDBACK_PER_SPIN: usize = 16;

pub(crate) fn collect_action_client_responses<C: ?Sized>(
    client: &mut C, completions: &mut Vec<ActionClientCompletion>,
    mut take: impl FnMut(&mut C, &mut Vec<ActionClientCompletion>) -> Result<ActionClientTakeResult>,
) -> Result<()> {
    for _ in 0..MAX_ACTION_RESPONSES_PER_SPIN {
        match take(client, completions)? {
            ActionClientTakeResult::Taken => {}
            ActionClientTakeResult::Idle | ActionClientTakeResult::Backpressured => break,
        }
    }
    Ok(())
}

pub(crate) fn complete_action_client_operation<T: ?Sized>(
    client: &Mutex<T>,
    operation: impl FnOnce(&mut T, &mut Vec<ActionClientCompletion>) -> Result<()>,
    cleanup_goal: impl FnMut(&mut T, &uuid::Uuid) -> Vec<ActionClientCompletion>,
) -> Result<()> {
    let mut completions = Vec::new();
    let operation_result = { operation(&mut *client.lock(), &mut completions) };
    let completion_result = run_action_client_completions(client, completions, cleanup_goal);
    operation_result.and(completion_result)
}

fn run_action_client_completions<T: ?Sized>(
    client: &Mutex<T>, completions: Vec<ActionClientCompletion>,
    mut cleanup_goal: impl FnMut(&mut T, &uuid::Uuid) -> Vec<ActionClientCompletion>,
) -> Result<()> {
    let mut completions = VecDeque::from(completions);
    let mut first_error = None;
    while let Some(completion) = completions.pop_front() {
        let ActionClientCompletion {
            notify,
            cleanup_goal_on_failure,
        } = completion;
        match notify() {
            Ok(false) => {
                if let Some(uuid) = cleanup_goal_on_failure {
                    let cleanup = { cleanup_goal(&mut *client.lock(), &uuid) };
                    completions.extend(cleanup);
                }
            }
            Ok(true) => {}
            Err(err) => {
                first_error.get_or_insert(err);
            }
        }
    }
    first_error.map_or(Ok(()), Err)
}

pub(crate) fn run_detached_action_client_completions(completions: Vec<ActionClientCompletion>) {
    for completion in completions {
        debug_assert!(completion.cleanup_goal_on_failure.is_none());
        if let Err(err) = (completion.notify)() {
            log::error!("could not complete detached action client cleanup: {err}");
        }
    }
}

pub(crate) fn complete_action_feedback_batch<T: ?Sized>(
    client: &Mutex<T>,
    mut operation: impl FnMut(
        &mut T,
        &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult>,
    mut cleanup_goal: impl FnMut(&mut T, &uuid::Uuid) -> Vec<ActionClientCompletion>,
) -> Result<()> {
    for _ in 0..MAX_ACTION_FEEDBACK_PER_SPIN {
        let mut completions = Vec::new();
        let take_result = { operation(&mut *client.lock(), &mut completions) };
        let completion_result =
            run_action_client_completions(client, completions, &mut cleanup_goal);
        let take_result = take_result?;
        completion_result?;
        match take_result {
            ActionClientTakeResult::Taken => {}
            ActionClientTakeResult::Idle | ActionClientTakeResult::Backpressured => break,
        }
    }
    Ok(())
}

pub(crate) fn action_stateful_response_completion<N, T>(
    sender: oneshot::Sender<T>, native: N,
    convert: impl FnOnce(N) -> (T, Vec<ActionClientCompletion>) + 'static,
    description: &'static str, cleanup_goal_on_failure: Option<uuid::Uuid>,
) -> ActionClientCompletion
where
    N: 'static,
    T: 'static,
{
    ActionClientCompletion {
        notify: Box::new(move || {
            let (response, cleanup) = convert(native);
            run_detached_action_client_completions(cleanup);
            if sender.send(response).is_err() {
                log::debug!("could not deliver {description}");
                Ok(false)
            } else {
                Ok(true)
            }
        }),
        cleanup_goal_on_failure,
    }
}

pub(crate) fn action_response_conversion_completion<N, T>(
    sender: oneshot::Sender<T>, native: N, convert: impl FnOnce(N) -> T + Send + 'static,
    description: &'static str, cleanup_goal_on_failure: Option<uuid::Uuid>,
) -> ActionClientCompletion
where
    N: Send + 'static,
    T: Send + 'static,
{
    ActionClientCompletion {
        notify: Box::new(move || {
            let response = convert(native);
            if sender.send(response).is_err() {
                log::debug!("could not deliver {description}");
                Ok(false)
            } else {
                Ok(true)
            }
        }),
        cleanup_goal_on_failure,
    }
}

pub(crate) fn action_status_completion(
    client: Arc<Mutex<dyn ActionClient_>>,
    status_array: WrappedNativeMsg<action_msgs::msg::GoalStatusArray>,
) -> ActionClientCompletion {
    ActionClientCompletion {
        notify: Box::new(move || {
            let status_array = action_msgs::msg::GoalStatusArray::from_native(&status_array);
            let updates = decode_status_updates(&status_array)?;
            let cleanup = { client.lock().apply_status_updates(&updates) };
            run_detached_action_client_completions(cleanup);
            Ok(true)
        }),
        cleanup_goal_on_failure: None,
    }
}

pub(crate) fn action_feedback_decode_completion<N>(
    client: Arc<Mutex<dyn ActionClient_>>, native: N,
    decode: impl FnOnce(N) -> Result<(uuid::Uuid, Box<dyn Any>)> + 'static,
) -> ActionClientCompletion
where
    N: 'static,
{
    ActionClientCompletion {
        notify: Box::new(move || {
            let (uuid, feedback) = decode(native)?;
            let completion = { client.lock().route_feedback(&uuid, feedback)? };
            if let Some(completion) = completion {
                (completion.notify)()
            } else {
                Ok(true)
            }
        }),
        cleanup_goal_on_failure: None,
    }
}

fn decode_status_updates(
    status_array: &action_msgs::msg::GoalStatusArray,
) -> Result<Vec<(uuid::Uuid, GoalStatus)>> {
    status_array
        .status_list
        .iter()
        .map(|status| {
            Ok((
                uuid_msg_to_uuid(&status.goal_info.goal_id)?,
                GoalStatus::from_rcl(status.status),
            ))
        })
        .collect()
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
                Ok(true)
            }),
            cleanup_goal_on_failure: None,
        }),
        Err(_) => Some(ActionClientCompletion {
            notify: Box::new(move || {
                drop(senders);
                Ok(true)
            }),
            cleanup_goal_on_failure: None,
        }),
    }
}

pub(crate) struct FeedbackChannel<T> {
    pub(crate) sender: Option<mpsc::Sender<T>>,
    pub(crate) pending: Option<T>,
    pub(crate) closing: bool,
}

pub(crate) type FeedbackSender<T> = Arc<Mutex<FeedbackChannel<T>>>;

impl<T> FeedbackChannel<T> {
    pub(crate) fn new(sender: mpsc::Sender<T>) -> Self {
        Self {
            sender: Some(sender),
            pending: None,
            closing: false,
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.closing || self.sender.as_ref().is_some_and(mpsc::Sender::is_closed)
    }
}

pub(crate) fn feedback_completion<T: Send + 'static>(
    channel: Arc<Mutex<FeedbackChannel<T>>>, feedback: T,
) -> Option<ActionClientCompletion> {
    let mut channel_guard = channel.lock();
    let Some(sender) = channel_guard.sender.take() else {
        debug_assert!(channel_guard.pending.is_none());
        channel_guard.pending.get_or_insert(feedback);
        return None;
    };
    drop(channel_guard);
    Some(detached_feedback_completion(channel, sender, feedback))
}

pub(crate) fn take_ready_pending_feedback<'a, T: Send + 'static>(
    channels: impl IntoIterator<Item = &'a FeedbackSender<T>>,
) -> PendingFeedbackAction {
    let mut saw_pending = false;
    for channel in channels {
        let mut channel_guard = channel.lock();
        if channel_guard.pending.is_none() {
            continue;
        }
        saw_pending = true;
        let Some(sender) = channel_guard.sender.as_mut() else {
            continue;
        };
        let mut context = Context::from_waker(futures::task::noop_waker_ref());
        if matches!(sender.poll_ready(&mut context), Poll::Pending) {
            continue;
        }
        let feedback = channel_guard.pending.take().unwrap();
        let sender = channel_guard.sender.take().unwrap();
        drop(channel_guard);
        return PendingFeedbackAction::Deliver(detached_feedback_completion(
            Arc::clone(channel),
            sender,
            feedback,
        ));
    }
    if saw_pending {
        PendingFeedbackAction::Backpressured
    } else {
        PendingFeedbackAction::None
    }
}

pub(crate) fn detached_feedback_completion<T: Send + 'static>(
    channel: Arc<Mutex<FeedbackChannel<T>>>, mut sender: mpsc::Sender<T>, feedback: T,
) -> ActionClientCompletion {
    ActionClientCompletion {
        notify: Box::new(move || {
            let pending = match sender.try_send(feedback) {
                Ok(()) => None,
                Err(err) if err.is_full() => Some(err.into_inner()),
                Err(_) => None,
            };
            let mut channel = channel.lock();
            if channel.closing {
                drop(channel);
                drop(sender);
                drop(pending);
            } else {
                channel.sender = Some(sender);
                channel.pending = pending;
            }
            Ok(true)
        }),
        cleanup_goal_on_failure: None,
    }
}

pub(crate) fn close_feedback_completion<T: Send + 'static>(
    channel: Arc<Mutex<FeedbackChannel<T>>>,
) -> ActionClientCompletion {
    ActionClientCompletion {
        notify: Box::new(move || {
            let (sender, pending) = {
                let mut channel = channel.lock();
                channel.closing = true;
                (channel.sender.take(), channel.pending.take())
            };
            drop(sender);
            drop(pending);
            Ok(true)
        }),
        cleanup_goal_on_failure: None,
    }
}

pub(crate) fn drop_action_client_resources_completion<T: Send + 'static>(
    resources: T,
) -> ActionClientCompletion {
    ActionClientCompletion {
        notify: Box::new(move || {
            drop(resources);
            Ok(true)
        }),
        cleanup_goal_on_failure: None,
    }
}

pub(crate) fn with_prepared_action_client<C: ?Sized, P, T>(
    client: &Mutex<C>, prepare: impl FnOnce() -> Result<P>,
    operation: impl FnOnce(&mut C, P) -> Result<T>,
) -> Result<T> {
    let prepared = prepare()?;
    operation(&mut *client.lock(), prepared)
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
                let completions = { client.lock().forget_goal(&self.uuid) };
                run_detached_action_client_completions(completions);
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
            WrappedActionClient::send_cancel_request_native,
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
        with_prepared_action_client(
            &client,
            || prepare_cancel_request(batch_cancel_goal_info(Default::default())),
            WrappedActionClient::send_cancel_request_native,
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
            WrappedActionClient::send_cancel_request_native,
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

        let uuid = uuid::Uuid::new_v4();
        let fut_client = Weak::clone(&self.client);
        with_prepared_action_client(
            &client,
            || {
                let uuid_msg = unique_identifier_msgs::msg::UUID {
                    uuid: uuid.as_bytes().to_vec(),
                };
                let request_msg = T::make_goal_request_msg(uuid_msg, goal);
                WrappedNativeMsg::try_from(&request_msg)
            },
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
                    let (sender, receiver) =
                        mpsc::channel::<T::Feedback>(client.feedback_channel_capacity);
                    client
                        .feedback_senders
                        .insert(uuid, Arc::new(Mutex::new(FeedbackChannel::new(sender))));
                    ActionFeedbackStream { receiver }
                });
                let result = options.track_result.then(|| {
                    let (sender, receiver) = oneshot::channel::<(GoalStatus, T::Result)>();
                    client.result_senders.insert(uuid, sender);
                    ActionResultFuture { receiver }
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
                                    let native_msg = prepare_result_request::<T>(uuid)?;
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
                })
            },
        )
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

type GoalResponseSender = (uuid::Uuid, oneshot::Sender<(bool, builtin_interfaces::msg::Time)>);

pub fn make_action_client<T>(client: Weak<Mutex<WrappedActionClient<T>>>) -> ActionClient<T>
where
    T: WrappedActionTypeSupport,
{
    ActionClient { client }
}

pub type ResultSender<R> = oneshot::Sender<(GoalStatus, R)>;
pub struct WrappedActionClient<T>
where
    T: WrappedActionTypeSupport,
{
    pub rcl_handle: rcl_action_client_t,
    pub goal_response_channels: HashMap<i64, GoalResponseSender>,
    pub goal_response_sequences: HashMap<uuid::Uuid, i64>,
    pub cancel_response_channels:
        HashMap<i64, oneshot::Sender<action_msgs::srv::CancelGoal::Response>>,
    pub feedback_senders: HashMap<uuid::Uuid, FeedbackSender<T::Feedback>>,
    pub result_requests: HashMap<i64, uuid::Uuid>,
    pub result_request_sequences: HashMap<uuid::Uuid, i64>,
    pub result_senders: HashMap<uuid::Uuid, ResultSender<T::Result>>,
    pub goal_status: HashMap<uuid::Uuid, GoalStatus>,
    pub feedback_channel_capacity: usize,

    pub poll_available_channels: Vec<oneshot::Sender<()>>,
}

pub trait ActionClient_ {
    fn handle(&self) -> &rcl_action_client_t;
    fn destroy(&mut self, node: &mut rcl_node_t);
    fn has_pending_work(&mut self) -> bool;
    fn forget_goal(&mut self, uuid: &uuid::Uuid) -> Vec<ActionClientCompletion>;
    fn finish_goal(&mut self, uuid: &uuid::Uuid, status: GoalStatus)
        -> Vec<ActionClientCompletion>;
    fn apply_status_updates(
        &mut self, updates: &[(uuid::Uuid, GoalStatus)],
    ) -> Vec<ActionClientCompletion>;
    fn route_feedback(
        &mut self, uuid: &uuid::Uuid, feedback: Box<dyn Any>,
    ) -> Result<Option<ActionClientCompletion>>;

    fn handle_goal_response(
        &mut self, client: Arc<Mutex<dyn ActionClient_>>,
        completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult>;
    fn handle_cancel_response(
        &mut self, completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult>;
    fn handle_feedback_msg(
        &mut self, client: Arc<Mutex<dyn ActionClient_>>,
        completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult>;
    fn handle_status_msg(
        &mut self, client: Arc<Mutex<dyn ActionClient_>>,
    ) -> Result<Vec<ActionClientCompletion>>;
    fn handle_result_response(
        &mut self, client: Arc<Mutex<dyn ActionClient_>>,
        completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult>;

    fn register_poll_available(&mut self, s: oneshot::Sender<()>);
    fn poll_available(&mut self, node: &mut rcl_node_t) -> Vec<ActionClientCompletion>;
}

impl<T> WrappedActionClient<T>
where
    T: WrappedActionTypeSupport,
{
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

    fn take_feedback_sender(&mut self, uuid: &uuid::Uuid) -> Option<FeedbackSender<T::Feedback>> {
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
            .downcast::<T::Feedback>()
            .map_err(|_| Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        Ok(self
            .feedback_senders
            .get(uuid)
            .and_then(|channel| feedback_completion(Arc::clone(channel), *feedback)))
    }

    fn prune_canceled_requests(&mut self) -> Vec<ActionClientCompletion> {
        let mut completions = Vec::new();
        let canceled_goals: Vec<_> = self
            .goal_response_channels
            .iter()
            .filter_map(|(_, (uuid, sender))| sender.is_canceled().then_some(*uuid))
            .collect();
        for uuid in canceled_goals {
            completions.extend(self.forget_goal(&uuid));
        }
        self.cancel_response_channels
            .retain(|_, sender| !sender.is_canceled());
        self.feedback_senders
            .retain(|_, channel| !channel.lock().is_closed());
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

    fn pending_feedback_completion(&mut self) -> PendingFeedbackAction {
        take_ready_pending_feedback(self.feedback_senders.values())
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
            Ok(cancel_response_future(cancel_req_receiver))
        } else {
            log::error!("could not send goal request {}", result);
            Err(Error::from_rcl_error(result))
        }
    }

    fn send_result_request_native(
        &mut self, uuid: uuid::Uuid,
        native_msg: WrappedNativeMsg<
            <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Request,
        >,
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

impl<T: 'static> ActionClient_ for WrappedActionClient<T>
where
    T: WrappedActionTypeSupport,
{
    fn handle(&self) -> &rcl_action_client_t {
        &self.rcl_handle
    }

    fn forget_goal(&mut self, uuid: &uuid::Uuid) -> Vec<ActionClientCompletion> {
        WrappedActionClient::forget_goal(self, uuid)
    }

    fn finish_goal(
        &mut self, uuid: &uuid::Uuid, status: GoalStatus,
    ) -> Vec<ActionClientCompletion> {
        WrappedActionClient::finish_goal(self, uuid, status)
    }

    fn apply_status_updates(
        &mut self, updates: &[(uuid::Uuid, GoalStatus)],
    ) -> Vec<ActionClientCompletion> {
        WrappedActionClient::apply_status_updates(self, updates)
    }

    fn route_feedback(
        &mut self, uuid: &uuid::Uuid, feedback: Box<dyn Any>,
    ) -> Result<Option<ActionClientCompletion>> {
        WrappedActionClient::route_feedback(self, uuid, feedback)
    }

    fn handle_goal_response(
        &mut self, client: Arc<Mutex<dyn ActionClient_>>,
        completions: &mut Vec<ActionClientCompletion>,
    ) -> Result<ActionClientTakeResult> {
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
            if let Some((uuid, sender)) =
                self.take_goal_response_by_sequence(request_id.sequence_number)
            {
                completions.push(action_stateful_response_completion(
                    sender,
                    response_msg,
                    move |response_msg| {
                        let response = <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response::from_native(&response_msg);
                        let (accept, stamp) = T::destructure_goal_response_msg(response);
                        let cleanup = if accept {
                            Vec::new()
                        } else {
                            client.lock().forget_goal(&uuid)
                        };
                        ((accept, stamp), cleanup)
                    },
                    "action goal response",
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
                    "action cancel response",
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
        let mut feedback_msg = WrappedNativeMsg::<T::FeedbackMessage>::try_new()?;
        let ret =
            unsafe { rcl_action_take_feedback(&self.rcl_handle, feedback_msg.void_ptr_mut()) };
        if action_client_take_succeeded(ret)? {
            completions.push(action_feedback_decode_completion(
                client,
                feedback_msg,
                |feedback_msg| {
                    let message = T::FeedbackMessage::from_native(&feedback_msg);
                    let (uuid, feedback) = T::destructure_feedback_msg(message);
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
            if let Some(uuid) = self.take_result_request_by_sequence(request_id.sequence_number) {
                if let Some(sender) = self.result_senders.remove(&uuid) {
                    completions.push(action_stateful_response_completion(
                        sender,
                        response_msg,
                        move |response_msg| {
                            let response = <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response::from_native(&response_msg);
                            let (status, result) = T::destructure_result_response_msg(response);
                            let status = GoalStatus::from_rcl(status);
                            let cleanup = client.lock().finish_goal(&uuid, status);
                            ((status, result), cleanup)
                        },
                        "action result",
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

    fn register_poll_available(&mut self, s: oneshot::Sender<()>) {
        self.poll_available_channels.push(s);
    }

    fn poll_available(&mut self, node: &mut rcl_node_t) -> Vec<ActionClientCompletion> {
        let mut completions = self.prune_canceled_requests();
        if let PendingFeedbackAction::Deliver(completion) = self.pending_feedback_completion() {
            completions.push(completion);
        }
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
            .any(|(_, (_, sender))| !sender.is_canceled())
            || self
                .cancel_response_channels
                .iter()
                .any(|(_, sender)| !sender.is_canceled())
            || self
                .feedback_senders
                .values()
                .any(|channel| !channel.lock().is_closed())
            || self
                .result_senders
                .values()
                .any(|sender| !sender.is_canceled())
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

pub(crate) fn batch_cancel_goal_info(
    stamp: builtin_interfaces::msg::Time,
) -> action_msgs::msg::GoalInfo {
    action_msgs::msg::GoalInfo {
        goal_id: unique_identifier_msgs::msg::UUID { uuid: vec![0; 16] },
        stamp,
    }
}

pub(crate) fn goal_cancel_info(uuid: &uuid::Uuid) -> action_msgs::msg::GoalInfo {
    action_msgs::msg::GoalInfo {
        goal_id: unique_identifier_msgs::msg::UUID {
            uuid: uuid.as_bytes().to_vec(),
        },
        ..Default::default()
    }
}

pub(crate) fn prepare_cancel_request(
    goal_info: action_msgs::msg::GoalInfo,
) -> Result<WrappedNativeMsg<action_msgs::srv::CancelGoal::Request>> {
    WrappedNativeMsg::try_from(&action_msgs::srv::CancelGoal::Request { goal_info })
}

fn prepare_result_request<T>(
    uuid: uuid::Uuid,
) -> Result<
    WrappedNativeMsg<
        <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Request,
    >,
>
where
    T: WrappedActionTypeSupport,
{
    let uuid_msg = unique_identifier_msgs::msg::UUID {
        uuid: uuid.as_bytes().to_vec(),
    };
    WrappedNativeMsg::try_from(&T::make_result_request_msg(uuid_msg))
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
        Future, FutureExt, StreamExt,
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

    struct GoalCleanupWake {
        client: Weak<Mutex<WrappedActionClient<Fibonacci::Action>>>,
        woke: AtomicBool,
    }

    struct StatefulResponseWake {
        client: Arc<Mutex<bool>>,
        woke: AtomicBool,
    }

    impl ArcWake for LockingWake {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            let _client = arc_self.client.lock();
            arc_self.woke.store(true, Ordering::SeqCst);
        }
    }

    impl ArcWake for GoalCleanupWake {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            let client = arc_self.client.upgrade().unwrap();
            assert!(client.try_lock().is_some());
            arc_self.woke.store(true, Ordering::SeqCst);
        }
    }

    impl ArcWake for StatefulResponseWake {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            assert!(*arc_self.client.lock());
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
            |_, completions| {
                completions.push(action_stateful_response_completion(
                    sender,
                    (),
                    |response| (response, Vec::new()),
                    "test response",
                    None,
                ));
                Ok(())
            },
            |_, _| Vec::new(),
        )
        .unwrap();

        assert!(wake.woke.load(Ordering::SeqCst));
        assert!(matches!(Pin::new(&mut receiver).poll(&mut context), Poll::Ready(Ok(()))));
    }

    #[test]
    fn action_responses_are_converted_after_releasing_the_client_lock() {
        let client = Arc::new(Mutex::new(()));
        let (sender, mut receiver) = oneshot::channel();
        let client_for_conversion = Arc::clone(&client);

        complete_action_client_operation(
            &*client,
            |_, completions| {
                completions.push(action_response_conversion_completion(
                    sender,
                    41,
                    move |value| {
                        assert!(client_for_conversion.try_lock().is_some());
                        value + 1
                    },
                    "test response",
                    None,
                ));
                Ok(())
            },
            |_, _| Vec::new(),
        )
        .unwrap();

        assert_eq!(receiver.try_recv().unwrap(), Some(42));
    }

    #[test]
    fn stateful_action_responses_commit_before_waking() {
        let client = Arc::new(Mutex::new(false));
        let (sender, mut receiver) = oneshot::channel();
        let wake = Arc::new(StatefulResponseWake {
            client: Arc::clone(&client),
            woke: AtomicBool::new(false),
        });
        let waker = waker_ref(&wake);
        let mut context = Context::from_waker(&waker);
        assert!(matches!(Pin::new(&mut receiver).poll(&mut context), Poll::Pending));
        let client_for_conversion = Arc::clone(&client);

        complete_action_client_operation(
            &*client,
            |_, completions| {
                completions.push(action_stateful_response_completion(
                    sender,
                    41,
                    move |value| {
                        assert!(client_for_conversion.try_lock().is_some());
                        *client_for_conversion.lock() = true;
                        (value + 1, Vec::new())
                    },
                    "test stateful response",
                    None,
                ));
                Ok(())
            },
            |_, _| Vec::new(),
        )
        .unwrap();

        assert!(wake.woke.load(Ordering::SeqCst));
        assert!(matches!(Pin::new(&mut receiver).poll(&mut context), Poll::Ready(Ok(42))));
    }

    #[test]
    fn feedback_decode_completions_run_outside_the_client_lock() {
        struct FeedbackRouteTestClient {
            routed: Arc<Mutex<Option<i32>>>,
        }

        impl ActionClient_ for FeedbackRouteTestClient {
            fn handle(&self) -> &rcl_action_client_t {
                unreachable!()
            }
            fn destroy(&mut self, _node: &mut rcl_node_t) {}
            fn has_pending_work(&mut self) -> bool {
                false
            }
            fn forget_goal(&mut self, _uuid: &uuid::Uuid) -> Vec<ActionClientCompletion> {
                Vec::new()
            }
            fn finish_goal(
                &mut self, _uuid: &uuid::Uuid, _status: GoalStatus,
            ) -> Vec<ActionClientCompletion> {
                Vec::new()
            }
            fn apply_status_updates(
                &mut self, _updates: &[(uuid::Uuid, GoalStatus)],
            ) -> Vec<ActionClientCompletion> {
                Vec::new()
            }
            fn route_feedback(
                &mut self, _uuid: &uuid::Uuid, feedback: Box<dyn Any>,
            ) -> Result<Option<ActionClientCompletion>> {
                *self.routed.lock() = Some(*feedback.downcast::<i32>().unwrap());
                Ok(None)
            }
            fn handle_goal_response(
                &mut self, _client: Arc<Mutex<dyn ActionClient_>>,
                _completions: &mut Vec<ActionClientCompletion>,
            ) -> Result<ActionClientTakeResult> {
                unreachable!()
            }
            fn handle_cancel_response(
                &mut self, _completions: &mut Vec<ActionClientCompletion>,
            ) -> Result<ActionClientTakeResult> {
                unreachable!()
            }
            fn handle_feedback_msg(
                &mut self, _client: Arc<Mutex<dyn ActionClient_>>,
                _completions: &mut Vec<ActionClientCompletion>,
            ) -> Result<ActionClientTakeResult> {
                unreachable!()
            }
            fn handle_status_msg(
                &mut self, _client: Arc<Mutex<dyn ActionClient_>>,
            ) -> Result<Vec<ActionClientCompletion>> {
                unreachable!()
            }
            fn handle_result_response(
                &mut self, _client: Arc<Mutex<dyn ActionClient_>>,
                _completions: &mut Vec<ActionClientCompletion>,
            ) -> Result<ActionClientTakeResult> {
                unreachable!()
            }
            fn register_poll_available(&mut self, _sender: oneshot::Sender<()>) {}
            fn poll_available(&mut self, _node: &mut rcl_node_t) -> Vec<ActionClientCompletion> {
                Vec::new()
            }
        }

        let routed = Arc::new(Mutex::new(None));
        let client: Arc<Mutex<dyn ActionClient_>> = Arc::new(Mutex::new(FeedbackRouteTestClient {
            routed: Arc::clone(&routed),
        }));
        let client_for_decode = Arc::clone(&client);
        let completion = action_feedback_decode_completion(Arc::clone(&client), 41, move |value| {
            assert!(client_for_decode.try_lock().is_some());
            Ok((uuid::Uuid::nil(), Box::new(value + 1)))
        });

        assert!(matches!((completion.notify)(), Ok(true)));
        assert_eq!(*routed.lock(), Some(42));
    }

    #[test]
    fn status_decoding_rejects_a_batch_before_applying_any_updates() {
        let valid = uuid::Uuid::new_v4();
        let status_array = action_msgs::msg::GoalStatusArray {
            status_list: vec![
                action_msgs::msg::GoalStatus {
                    goal_info: action_msgs::msg::GoalInfo {
                        goal_id: unique_identifier_msgs::msg::UUID {
                            uuid: valid.as_bytes().to_vec(),
                        },
                        ..Default::default()
                    },
                    status: action_msgs::msg::GoalStatus::STATUS_EXECUTING as i8,
                },
                action_msgs::msg::GoalStatus {
                    goal_info: action_msgs::msg::GoalInfo {
                        goal_id: unique_identifier_msgs::msg::UUID { uuid: vec![0; 15] },
                        ..Default::default()
                    },
                    status: action_msgs::msg::GoalStatus::STATUS_SUCCEEDED as i8,
                },
            ],
        };

        assert!(matches!(
            decode_status_updates(&status_array),
            Err(Error::InvalidActionUuidLength { actual: 15 })
        ));
    }

    #[test]
    fn action_completions_run_after_a_failed_locked_operation() {
        let client = Mutex::new(());
        let (sender, mut receiver) = oneshot::channel();

        let result = complete_action_client_operation(
            &client,
            |_, completions| {
                completions.push(action_stateful_response_completion(
                    sender,
                    42,
                    |response| (response, Vec::new()),
                    "test response",
                    None,
                ));
                Err(Error::RCL_RET_ERROR)
            },
            |_, _| Vec::new(),
        );

        assert!(matches!(result, Err(Error::RCL_RET_ERROR)));
        assert_eq!(receiver.try_recv().unwrap(), Some(42));
    }

    #[test]
    fn action_completion_errors_do_not_skip_later_completions() {
        let client = Mutex::new(());
        let completed = Arc::new(Mutex::new(Vec::new()));
        let first = Arc::clone(&completed);
        let second = Arc::clone(&completed);

        let result = complete_action_client_operation(
            &client,
            |_, completions| {
                completions.push(ActionClientCompletion {
                    notify: Box::new(move || {
                        first.lock().push("first");
                        Err(Error::RCL_RET_ERROR)
                    }),
                    cleanup_goal_on_failure: None,
                });
                completions.push(ActionClientCompletion {
                    notify: Box::new(move || {
                        second.lock().push("second");
                        Ok(true)
                    }),
                    cleanup_goal_on_failure: None,
                });
                Ok(())
            },
            |_, _| Vec::new(),
        );

        assert!(matches!(result, Err(Error::RCL_RET_ERROR)));
        assert_eq!(*completed.lock(), ["first", "second"]);
    }

    #[test]
    fn action_response_batches_stop_at_idle_and_fairness_limit() {
        let calls = std::cell::Cell::new(0);
        let mut client = ();
        let mut completions = Vec::new();
        collect_action_client_responses(&mut client, &mut completions, |_, _| {
            let call = calls.get();
            calls.set(call + 1);
            if call == 2 {
                Ok(ActionClientTakeResult::Idle)
            } else {
                Ok(ActionClientTakeResult::Taken)
            }
        })
        .unwrap();
        assert_eq!(calls.get(), 3);

        calls.set(0);
        collect_action_client_responses(&mut client, &mut completions, |_, _| {
            calls.set(calls.get() + 1);
            Ok(ActionClientTakeResult::Taken)
        })
        .unwrap();
        assert_eq!(calls.get(), MAX_ACTION_RESPONSES_PER_SPIN);
    }

    #[test]
    fn action_response_indexes_support_out_of_order_lookup() {
        let entries = (0..128_i64)
            .map(|sequence| {
                let uuid = uuid::Uuid::from_u128(sequence as u128 + 1);
                let (sender, _receiver) =
                    oneshot::channel::<(bool, builtin_interfaces::msg::Time)>();
                (sequence, (uuid, sender))
            })
            .collect::<HashMap<_, _>>();
        let mut entries = entries;

        for sequence in (0..128_i64).rev() {
            let (uuid, _sender) = entries.remove(&sequence).unwrap();
            assert_eq!(uuid, uuid::Uuid::from_u128(sequence as u128 + 1));
        }
        assert!(entries.is_empty());
    }

    #[test]
    fn action_feedback_batches_stop_at_idle_and_fairness_limit() {
        let client = Mutex::new(0usize);
        let calls = std::cell::Cell::new(0);
        complete_action_feedback_batch(
            &client,
            |_, _| {
                let call = calls.get();
                calls.set(call + 1);
                if call == 2 {
                    Ok(ActionClientTakeResult::Idle)
                } else {
                    Ok(ActionClientTakeResult::Taken)
                }
            },
            |_, _| Vec::new(),
        )
        .unwrap();
        assert_eq!(calls.get(), 3);

        calls.set(0);
        complete_action_feedback_batch(
            &client,
            |_, _| {
                calls.set(calls.get() + 1);
                Ok(ActionClientTakeResult::Taken)
            },
            |_, _| Vec::new(),
        )
        .unwrap();
        assert_eq!(calls.get(), MAX_ACTION_FEEDBACK_PER_SPIN);

        calls.set(0);
        complete_action_feedback_batch(
            &client,
            |_, _| {
                calls.set(calls.get() + 1);
                Ok(ActionClientTakeResult::Backpressured)
            },
            |_, _| Vec::new(),
        )
        .unwrap();
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn feedback_completions_wake_after_releasing_the_client_lock() {
        let client = Arc::new(Mutex::new(()));
        let (sender, mut receiver) = mpsc::channel(0);
        let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
        let wake = Arc::new(LockingWake {
            client: Arc::clone(&client),
            woke: AtomicBool::new(false),
        });
        let waker = waker_ref(&wake);
        let mut context = Context::from_waker(&waker);
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Pending));

        complete_action_client_operation(
            &*client,
            |_, completions| {
                completions.extend(feedback_completion(Arc::clone(&channel), 42));
                Ok(())
            },
            |_, _| Vec::new(),
        )
        .unwrap();

        assert!(wake.woke.load(Ordering::SeqCst));
        assert_eq!(receiver.next().now_or_never().flatten(), Some(42));
        assert!(channel.lock().sender.is_some());
    }

    #[test]
    fn feedback_is_routed_only_to_the_indexed_goal() {
        let target = uuid::Uuid::new_v4();
        let other = uuid::Uuid::new_v4();
        let (target_sender, mut target_receiver) = mpsc::channel(1);
        let (other_sender, mut other_receiver) = mpsc::channel(1);
        let mut client = WrappedActionClient::<Fibonacci::Action> {
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: HashMap::new(),
            goal_response_sequences: HashMap::new(),
            cancel_response_channels: HashMap::new(),
            feedback_senders: HashMap::from([
                (target, Arc::new(Mutex::new(FeedbackChannel::new(target_sender)))),
                (other, Arc::new(Mutex::new(FeedbackChannel::new(other_sender)))),
            ]),
            result_requests: HashMap::new(),
            result_request_sequences: HashMap::new(),
            result_senders: HashMap::new(),
            goal_status: HashMap::new(),
            feedback_channel_capacity: 1,
            poll_available_channels: Vec::new(),
        };

        let completion = client
            .route_feedback(&target, Box::new(Fibonacci::Feedback::default()))
            .unwrap()
            .unwrap();
        assert!(matches!((completion.notify)(), Ok(true)));
        assert!(target_receiver.next().now_or_never().flatten().is_some());
        assert!(other_receiver.next().now_or_never().is_none());
    }

    #[test]
    fn full_feedback_channel_preserves_the_pending_message() {
        let (sender, mut receiver) = mpsc::channel(0);
        let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));

        let waker = futures::task::noop_waker_ref();
        let mut context = Context::from_waker(waker);
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Pending));
        (feedback_completion(Arc::clone(&channel), 41)
            .unwrap()
            .notify)()
        .unwrap();
        (feedback_completion(Arc::clone(&channel), 42)
            .unwrap()
            .notify)()
        .unwrap();
        assert_eq!(channel.lock().pending, Some(42));

        assert_eq!(receiver.next().now_or_never().flatten(), Some(41));
        let pending = channel.lock().pending.take().unwrap();
        (feedback_completion(Arc::clone(&channel), pending)
            .unwrap()
            .notify)()
        .unwrap();
        assert_eq!(receiver.next().now_or_never().flatten(), Some(42));
        assert!(channel.lock().pending.is_none());
    }

    #[test]
    fn pending_feedback_skips_a_blocked_goal_for_a_ready_goal() {
        let (blocked_sender, mut blocked_receiver) = mpsc::channel(0);
        let blocked = Arc::new(Mutex::new(FeedbackChannel::new(blocked_sender)));
        let waker = futures::task::noop_waker_ref();
        let mut context = Context::from_waker(waker);
        assert!(matches!(Pin::new(&mut blocked_receiver).poll_next(&mut context), Poll::Pending));
        (feedback_completion(Arc::clone(&blocked), 41)
            .unwrap()
            .notify)()
        .unwrap();
        blocked.lock().pending = Some(42);

        let (ready_sender, mut ready_receiver) = mpsc::channel(0);
        let ready = Arc::new(Mutex::new(FeedbackChannel {
            sender: Some(ready_sender),
            pending: Some(52),
            closing: false,
        }));
        assert!(matches!(Pin::new(&mut ready_receiver).poll_next(&mut context), Poll::Pending));

        let action = take_ready_pending_feedback([&blocked, &ready]);
        let PendingFeedbackAction::Deliver(completion) = action else {
            panic!("a ready goal should be selected after a blocked goal");
        };
        assert!(matches!((completion.notify)(), Ok(true)));

        assert_eq!(ready_receiver.next().now_or_never().flatten(), Some(52));
        assert!(ready.lock().pending.is_none());
        assert_eq!(blocked.lock().pending, Some(42));
        assert_eq!(blocked_receiver.next().now_or_never().flatten(), Some(41));
    }

    #[test]
    fn pending_feedback_reports_backpressure_when_every_goal_is_blocked() {
        fn blocked_feedback_channel(
            queued: i32, pending: i32,
        ) -> (FeedbackSender<i32>, mpsc::Receiver<i32>) {
            let (sender, mut receiver) = mpsc::channel(0);
            let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
            let waker = futures::task::noop_waker_ref();
            let mut context = Context::from_waker(waker);
            assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Pending));
            (feedback_completion(Arc::clone(&channel), queued)
                .unwrap()
                .notify)()
            .unwrap();
            channel.lock().pending = Some(pending);
            (channel, receiver)
        }

        let (first, mut first_receiver) = blocked_feedback_channel(41, 42);
        let (second, mut second_receiver) = blocked_feedback_channel(51, 52);

        assert!(matches!(
            take_ready_pending_feedback([&first, &second]),
            PendingFeedbackAction::Backpressured
        ));
        assert_eq!(first.lock().pending, Some(42));
        assert_eq!(second.lock().pending, Some(52));
        assert_eq!(first_receiver.next().now_or_never().flatten(), Some(41));
        assert_eq!(second_receiver.next().now_or_never().flatten(), Some(51));
    }

    #[test]
    fn feedback_streams_close_after_releasing_the_client_lock() {
        let client = Arc::new(Mutex::new(()));
        let (sender, mut receiver) = mpsc::channel::<Fibonacci::Feedback>(0);
        let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
        let wake = Arc::new(LockingWake {
            client: Arc::clone(&client),
            woke: AtomicBool::new(false),
        });
        let waker = waker_ref(&wake);
        let mut context = Context::from_waker(&waker);
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Pending));

        complete_action_client_operation(
            &*client,
            |_, completions| {
                completions.push(close_feedback_completion(Arc::clone(&channel)));
                Ok(())
            },
            |_, _| Vec::new(),
        )
        .unwrap();

        assert!(wake.woke.load(Ordering::SeqCst));
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Ready(None)));
    }

    #[test]
    fn forgetting_goals_drops_observers_after_releasing_the_client_lock() {
        let target = uuid::Uuid::new_v4();
        let (feedback_sender, mut feedback_receiver) = mpsc::channel(0);
        let (result_sender, mut result_receiver) = oneshot::channel();
        let client = Arc::new(Mutex::new(WrappedActionClient::<Fibonacci::Action> {
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: HashMap::new(),
            goal_response_sequences: HashMap::new(),
            cancel_response_channels: HashMap::new(),
            feedback_senders: HashMap::from([(
                target,
                Arc::new(Mutex::new(FeedbackChannel::new(feedback_sender))),
            )]),
            result_requests: HashMap::from([(1, target)]),
            result_request_sequences: HashMap::from([(target, 1)]),
            result_senders: HashMap::from([(target, result_sender)]),
            goal_status: HashMap::from([(target, GoalStatus::Accepted)]),
            feedback_channel_capacity: 0,
            poll_available_channels: Vec::new(),
        }));
        let wake = Arc::new(GoalCleanupWake {
            client: Arc::downgrade(&client),
            woke: AtomicBool::new(false),
        });
        let waker = waker_ref(&wake);
        let mut context = Context::from_waker(&waker);
        assert!(matches!(
            Pin::new(&mut feedback_receiver).poll_next(&mut context),
            Poll::Pending
        ));
        assert!(matches!(Pin::new(&mut result_receiver).poll(&mut context), Poll::Pending));

        complete_action_client_operation(
            &*client,
            |client, completions| {
                completions.extend(client.forget_goal(&target));
                Ok(())
            },
            |client, uuid| client.forget_goal(uuid),
        )
        .unwrap();

        assert!(wake.woke.load(Ordering::SeqCst));
        assert!(matches!(
            Pin::new(&mut feedback_receiver).poll_next(&mut context),
            Poll::Ready(None)
        ));
        assert!(matches!(Pin::new(&mut result_receiver).poll(&mut context), Poll::Ready(Err(_))));
    }

    #[test]
    fn forgetting_pending_goal_uses_and_preserves_bidirectional_indexes() {
        let target = uuid::Uuid::new_v4();
        let other = uuid::Uuid::new_v4();
        let (target_sender, mut target_receiver) =
            oneshot::channel::<(bool, builtin_interfaces::msg::Time)>();
        let (other_sender, mut other_receiver) =
            oneshot::channel::<(bool, builtin_interfaces::msg::Time)>();
        let mut client = WrappedActionClient::<Fibonacci::Action> {
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: HashMap::from([
                (10, (target, target_sender)),
                (11, (other, other_sender)),
            ]),
            goal_response_sequences: HashMap::from([(target, 10), (other, 11)]),
            cancel_response_channels: HashMap::new(),
            feedback_senders: HashMap::new(),
            result_requests: HashMap::new(),
            result_request_sequences: HashMap::new(),
            result_senders: HashMap::new(),
            goal_status: HashMap::from([
                (target, GoalStatus::Unknown),
                (other, GoalStatus::Unknown),
            ]),
            feedback_channel_capacity: 0,
            poll_available_channels: Vec::new(),
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
    fn failed_goal_completion_cleans_registration_after_relocking() {
        let target = uuid::Uuid::new_v4();
        let (feedback_sender, _feedback_receiver) = mpsc::channel(1);
        let (result_sender, _result_receiver) = oneshot::channel();
        let client = Mutex::new(WrappedActionClient::<Fibonacci::Action> {
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            goal_response_channels: HashMap::new(),
            goal_response_sequences: HashMap::new(),
            cancel_response_channels: HashMap::new(),
            feedback_senders: HashMap::from([(
                target,
                Arc::new(Mutex::new(FeedbackChannel {
                    sender: Some(feedback_sender),
                    pending: Some(Fibonacci::Feedback::default()),
                    closing: false,
                })),
            )]),
            result_requests: HashMap::from([(1, target)]),
            result_request_sequences: HashMap::from([(target, 1)]),
            result_senders: HashMap::from([(target, result_sender)]),
            goal_status: HashMap::from([(target, GoalStatus::Accepted)]),
            feedback_channel_capacity: 1,
            poll_available_channels: Vec::new(),
        });
        let (sender, receiver) = oneshot::channel();
        drop(receiver);

        complete_action_client_operation(
            &client,
            |_, completions| {
                completions.push(action_stateful_response_completion(
                    sender,
                    (),
                    |response| (response, Vec::new()),
                    "test goal response",
                    Some(target),
                ));
                Ok(())
            },
            |client, uuid| client.forget_goal(uuid),
        )
        .unwrap();

        let client = client.lock();
        assert!(client.feedback_senders.is_empty());
        assert!(client.result_requests.is_empty());
        assert!(client.result_senders.is_empty());
        assert!(!client.goal_status.contains_key(&target));
    }

    #[test]
    fn action_payloads_are_prepared_before_locking_the_native_handle() {
        let client = Mutex::new(());

        let prepared = with_prepared_action_client(
            &client,
            || {
                assert!(client.try_lock().is_some());
                Ok(41)
            },
            |_client, value| Ok(value + 1),
        )
        .unwrap();

        assert_eq!(prepared, 42);
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
            goal_response_channels: HashMap::new(),
            goal_response_sequences: HashMap::new(),
            cancel_response_channels: HashMap::new(),
            feedback_senders: HashMap::from([
                (
                    target,
                    Arc::new(Mutex::new(FeedbackChannel {
                        sender: Some(target_feedback),
                        pending: Some(Fibonacci::Feedback::default()),
                        closing: false,
                    })),
                ),
                (other, Arc::new(Mutex::new(FeedbackChannel::new(other_feedback)))),
            ]),
            result_requests: HashMap::from([(10, target), (11, other)]),
            result_request_sequences: HashMap::from([(target, 10), (other, 11)]),
            result_senders: HashMap::from([(target, target_result), (other, other_result)]),
            goal_status: HashMap::from([
                (target, GoalStatus::Accepted),
                (other, GoalStatus::Executing),
            ]),
            feedback_channel_capacity: 1,
            poll_available_channels: Vec::new(),
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
        let client = Arc::new(Mutex::new(WrappedActionClient::<Fibonacci::Action> {
            rcl_handle: unsafe { rcl_action_get_zero_initialized_client() },
            // The target response has already been delivered, so only the other request remains.
            goal_response_channels: HashMap::from([(12, (other, other_goal))]),
            goal_response_sequences: HashMap::from([(other, 12)]),
            cancel_response_channels: HashMap::new(),
            feedback_senders: HashMap::from([
                (
                    target,
                    Arc::new(Mutex::new(FeedbackChannel {
                        sender: Some(target_feedback),
                        pending: Some(Fibonacci::Feedback::default()),
                        closing: false,
                    })),
                ),
                (other, Arc::new(Mutex::new(FeedbackChannel::new(other_feedback)))),
            ]),
            result_requests: HashMap::new(),
            result_request_sequences: HashMap::new(),
            result_senders: HashMap::from([(target, target_result), (other, other_result)]),
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
