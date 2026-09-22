use futures::{
    channel::{mpsc, oneshot},
    future::{join_all, FutureExt, JoinAll},
    stream::{Stream, StreamExt},
    task::noop_waker_ref,
};
use parking_lot::Mutex;
use std::{
    collections::{HashMap, HashSet},
    ffi::{CStr, CString},
    marker::PhantomData,
    mem::MaybeUninit,
    pin::Pin,
    sync::{Arc, Weak},
    task::{Context, Poll},
};

use crate::{
    action_common::{ActionServerOptions, GoalStatus},
    clocks::Clock,
    error::*,
    msg_types::{
        generated_msgs::{action_msgs, builtin_interfaces, unique_identifier_msgs},
        *,
    },
};
use r2r_actions::*;
use r2r_rcl::*;

pub(crate) type ActionServerCompletion = Box<dyn FnOnce() -> Result<()>>;

const MAX_ACTION_SERVER_REQUESTS_PER_SPIN: usize = 16;

fn finish_action_server_operation(
    operation_result: Result<()>, completions: Vec<ActionServerCompletion>,
) -> Result<()> {
    let mut first_error = operation_result.err();
    for completion in completions {
        if let Err(err) = completion() {
            first_error.get_or_insert(err);
        }
    }
    first_error.map_or(Ok(()), Err)
}

pub(crate) fn complete_action_server_operation<S: ?Sized>(
    server: &Mutex<S>,
    operation: impl FnOnce(&mut S, &mut Vec<ActionServerCompletion>) -> Result<()>,
) -> Result<()> {
    let mut completions = Vec::new();
    let operation_result = {
        let mut server = server.lock();
        operation(&mut *server, &mut completions)
    };
    finish_action_server_operation(operation_result, completions)
}

pub(crate) fn complete_action_server_request_batch<S: ?Sized>(
    server: &Mutex<S>, mut operation: impl FnMut(&mut S) -> Result<Option<ActionServerCompletion>>,
) -> Result<()> {
    for _ in 0..MAX_ACTION_SERVER_REQUESTS_PER_SPIN {
        let completion = { operation(&mut *server.lock())? };
        let Some(completion) = completion else {
            break;
        };
        completion()?;
    }
    Ok(())
}

fn request_delivery<T: Send + 'static>(
    sender_slot: Arc<Mutex<Option<mpsc::Sender<T>>>>, mut sender: mpsc::Sender<T>, request: T,
    on_failed: impl FnOnce(T) -> Result<()> + 'static,
) -> ActionServerCompletion {
    Box::new(move || {
        let result = match sender.try_send(request) {
            Ok(()) => Ok(()),
            Err(err) => on_failed(err.into_inner()),
        };
        *sender_slot.lock() = Some(sender);
        result
    })
}

fn sender_drop_completion<T: Send + 'static>(
    sender_slot: Arc<Mutex<Option<mpsc::Sender<T>>>>,
) -> ActionServerCompletion {
    let sender = sender_slot.lock().take();
    Box::new(move || {
        drop(sender);
        drop(sender_slot);
        Ok(())
    })
}

fn with_prepared_action_server<S: ?Sized, P, T>(
    server: &Mutex<S>, prepare: impl FnOnce() -> Result<P>,
    operation: impl FnOnce(&mut S, P) -> Result<T>,
) -> Result<T> {
    let prepared = prepare()?;
    operation(&mut *server.lock(), prepared)
}

pub trait ActionServer_ {
    fn handle(&self) -> &rcl_action_server_t;
    fn handle_mut(&mut self) -> &mut rcl_action_server_t;
    fn reject_goal_request(
        &mut self, uuid: &uuid::Uuid, request_id: &mut rmw_request_id_t,
        completions: &mut Vec<ActionServerCompletion>,
    ) -> Result<()>;
    fn handle_goal_request(
        &mut self, server: Arc<Mutex<dyn ActionServer_>>,
    ) -> Result<Option<ActionServerCompletion>>;
    fn take_pending_cancel_delivery(
        &mut self, server: Arc<Mutex<dyn ActionServer_>>,
    ) -> Option<ActionServerCompletion>;
    fn finish_cancel_delivery(&mut self, pending: PendingCancelRequest);
    fn send_completed_cancel_requests(&mut self) -> Result<()>;
    fn handle_cancel_request(
        &mut self, server: Arc<Mutex<dyn ActionServer_>>,
    ) -> Result<Option<ActionServerCompletion>>;
    fn handle_result_request(&mut self) -> Result<()>;
    fn handle_goal_expired(&mut self, completions: &mut Vec<ActionServerCompletion>) -> Result<()>;
    fn action_name(&self) -> Result<String>;
    fn publish_status(&self) -> Result<()>;
    fn set_goal_state(
        &mut self, uuid: &uuid::Uuid, new_state: rcl_action_goal_event_t,
    ) -> Result<()>;
    fn notify_goal_done(&self) -> Result<()>;
    fn add_result(&mut self, uuid: uuid::Uuid, msg: Box<dyn VoidPtr>) -> Result<()>;
    fn cancel_goal(&mut self, uuid: &uuid::Uuid) -> Result<()>;
    fn execute_goal(&mut self, uuid: &uuid::Uuid) -> Result<()>;
    fn get_goal_status(&self, uuid: &uuid::Uuid) -> Result<GoalStatus>;
    fn is_cancelling(&self, uuid: &uuid::Uuid) -> Result<bool>;
    fn add_goal_handle(&mut self, uuid: uuid::Uuid, goal_handle: *mut rcl_action_goal_handle_t);
    fn forget_cancel_sender(
        &mut self, uuid: &uuid::Uuid, completions: &mut Vec<ActionServerCompletion>,
    );
    fn should_remove(&self) -> bool;
    fn destroy(&mut self, node: &mut rcl_node_t);
}

/// Request to cancel an active goal.
pub struct ActionServerCancelRequest {
    pub uuid: uuid::Uuid,
    response_sender: oneshot::Sender<(uuid::Uuid, bool)>,
}

impl ActionServerCancelRequest {
    /// Accepts the cancel request. The action server should now cancel the corresponding goal.
    pub fn accept(self) {
        if self.response_sender.send((self.uuid, true)).is_err() {
            log::error!("warning: could not send goal canellation accept msg")
        }
    }
    /// Rejects the cancel request.
    pub fn reject(self) {
        if self.response_sender.send((self.uuid, false)).is_err() {
            log::error!("warning: could not send goal cancellation rejection")
        }
    }
}

/// Stream of cancel requests for a runtime-typed action goal.
pub struct ActionServerCancelStreamUntyped {
    inner: Pin<Box<dyn Stream<Item = ActionServerCancelRequest> + Send>>,
}

impl Stream for ActionServerCancelStreamUntyped {
    type Item = ActionServerCancelRequest;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

/// Stream of goal requests for a runtime-typed action server.
pub struct ActionServerUntyped {
    inner: Pin<Box<dyn Stream<Item = ActionServerGoalRequestUntyped> + Send>>,
}

impl Stream for ActionServerUntyped {
    type Item = ActionServerGoalRequestUntyped;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.inner.as_mut().poll_next(cx)
    }
}

pub(crate) fn create_action_server_untyped_for<T>(
    node: &mut crate::Node, action_name: &str, options: ActionServerOptions,
) -> Result<ActionServerUntyped>
where
    T: 'static + WrappedActionTypeSupport,
{
    let requests = node.create_action_server_with_options::<T>(action_name, options)?;
    Ok(ActionServerUntyped {
        inner: Box::pin(requests.map(ActionServerGoalRequestUntyped::from_typed)),
    })
}

trait ErasedGoalRequest: Send {
    fn action_name(&self) -> Result<String>;
    fn accept(
        self: Box<Self>, execute: bool, goal: Result<serde_json::Value>,
    ) -> Result<(ActionServerGoalUntyped, ActionServerCancelStreamUntyped)>;
    fn reject(self: Box<Self>) -> Result<()>;
}

impl<T> ErasedGoalRequest for ActionServerGoalRequest<T>
where
    T: 'static + WrappedActionTypeSupport,
{
    fn action_name(&self) -> Result<String> {
        ActionServerGoalRequest::action_name(self)
    }

    fn accept(
        self: Box<Self>, execute: bool, goal: Result<serde_json::Value>,
    ) -> Result<(ActionServerGoalUntyped, ActionServerCancelStreamUntyped)> {
        let (server_goal, cancel_requests) = (*self).accept_with_execution(execute)?;
        Ok((
            ActionServerGoalUntyped {
                uuid: server_goal.uuid,
                goal,
                inner: Box::new(server_goal),
            },
            ActionServerCancelStreamUntyped {
                inner: Box::pin(cancel_requests),
            },
        ))
    }

    fn reject(self: Box<Self>) -> Result<()> {
        (*self).reject()
    }
}

/// Runtime-typed action goal request with a JSON payload.
pub struct ActionServerGoalRequestUntyped {
    pub uuid: uuid::Uuid,
    pub goal: Result<serde_json::Value>,
    inner: Box<dyn ErasedGoalRequest>,
}

impl ActionServerGoalRequestUntyped {
    fn from_typed<T>(request: ActionServerGoalRequest<T>) -> Self
    where
        T: 'static + WrappedActionTypeSupport,
    {
        let uuid = request.uuid;
        let goal = serde_json::to_value(&request.goal).map_err(|err| Error::SerdeError {
            err: err.to_string(),
        });
        Self {
            uuid,
            goal,
            inner: Box::new(request),
        }
    }

    pub fn action_name(&self) -> Result<String> {
        self.inner.action_name()
    }

    pub fn accept(self) -> Result<(ActionServerGoalUntyped, ActionServerCancelStreamUntyped)> {
        let Self { goal, inner, .. } = self;
        inner.accept(true, goal)
    }

    pub fn accept_deferred(
        self,
    ) -> Result<(ActionServerGoalUntyped, ActionServerCancelStreamUntyped)> {
        let Self { goal, inner, .. } = self;
        inner.accept(false, goal)
    }

    pub fn reject(self) -> Result<()> {
        self.inner.reject()
    }
}

trait ErasedServerGoal: Send {
    fn clone_box(&self) -> Box<dyn ErasedServerGoal>;
    fn action_name(&self) -> Result<String>;
    fn get_status(&self) -> Result<GoalStatus>;
    fn execute(&self) -> Result<()>;
    fn is_cancelling(&self) -> Result<bool>;
    fn publish_feedback(&self, msg: serde_json::Value) -> Result<()>;
    fn cancel(&mut self, msg: serde_json::Value) -> Result<()>;
    fn abort(&mut self, msg: serde_json::Value) -> Result<()>;
    fn succeed(&mut self, msg: serde_json::Value) -> Result<()>;
}

impl<T> ErasedServerGoal for ActionServerGoal<T>
where
    T: 'static + WrappedActionTypeSupport,
{
    fn clone_box(&self) -> Box<dyn ErasedServerGoal> {
        Box::new(self.clone())
    }

    fn action_name(&self) -> Result<String> {
        ActionServerGoal::action_name(self)
    }

    fn get_status(&self) -> Result<GoalStatus> {
        ActionServerGoal::get_status(self)
    }

    fn execute(&self) -> Result<()> {
        ActionServerGoal::execute(self)
    }

    fn is_cancelling(&self) -> Result<bool> {
        ActionServerGoal::is_cancelling(self)
    }

    fn publish_feedback(&self, msg: serde_json::Value) -> Result<()> {
        let msg = serde_json::from_value(msg).map_err(|err| Error::SerdeError {
            err: err.to_string(),
        })?;
        ActionServerGoal::publish_feedback(self, msg)
    }

    fn cancel(&mut self, msg: serde_json::Value) -> Result<()> {
        let msg = serde_json::from_value(msg).map_err(|err| Error::SerdeError {
            err: err.to_string(),
        })?;
        ActionServerGoal::cancel(self, msg)
    }

    fn abort(&mut self, msg: serde_json::Value) -> Result<()> {
        let msg = serde_json::from_value(msg).map_err(|err| Error::SerdeError {
            err: err.to_string(),
        })?;
        ActionServerGoal::abort(self, msg)
    }

    fn succeed(&mut self, msg: serde_json::Value) -> Result<()> {
        let msg = serde_json::from_value(msg).map_err(|err| Error::SerdeError {
            err: err.to_string(),
        })?;
        ActionServerGoal::succeed(self, msg)
    }
}

/// Handle to an accepted runtime-typed action goal.
pub struct ActionServerGoalUntyped {
    pub uuid: uuid::Uuid,
    pub goal: Result<serde_json::Value>,
    inner: Box<dyn ErasedServerGoal>,
}

impl Clone for ActionServerGoalUntyped {
    fn clone(&self) -> Self {
        Self {
            uuid: self.uuid,
            goal: self.goal.clone(),
            inner: self.inner.clone_box(),
        }
    }
}

impl ActionServerGoalUntyped {
    pub fn action_name(&self) -> Result<String> {
        self.inner.action_name()
    }

    pub fn get_status(&self) -> Result<GoalStatus> {
        self.inner.get_status()
    }

    pub fn execute(&self) -> Result<()> {
        self.inner.execute()
    }

    pub fn is_cancelling(&self) -> Result<bool> {
        self.inner.is_cancelling()
    }

    pub fn publish_feedback(&self, msg: serde_json::Value) -> Result<()> {
        self.inner.publish_feedback(msg)
    }

    pub fn cancel(&mut self, msg: serde_json::Value) -> Result<()> {
        self.inner.cancel(msg)
    }

    pub fn abort(&mut self, msg: serde_json::Value) -> Result<()> {
        self.inner.abort(msg)
    }

    pub fn succeed(&mut self, msg: serde_json::Value) -> Result<()> {
        self.inner.succeed(msg)
    }
}

/// Request to the action server to accept a new `Goal`.
pub struct ActionServerGoalRequest<T>
where
    T: WrappedActionTypeSupport,
{
    pub uuid: uuid::Uuid,
    pub goal: T::Goal,
    clock: Arc<Mutex<Clock>>,
    cancel_requests: mpsc::Receiver<ActionServerCancelRequest>,
    response: GoalRequestResponseGuard,
}

struct GoalRequestResponseGuard {
    server: Weak<Mutex<dyn ActionServer_>>,
    uuid: uuid::Uuid,
    request_id: rmw_request_id_t,
    response_sent: bool,
}

unsafe impl<T> Send for ActionServerGoalRequest<T> where T: WrappedActionTypeSupport {}

impl<T: 'static> ActionServerGoalRequest<T>
where
    T: WrappedActionTypeSupport,
{
    /// Returns the fully resolved action name used by the native server.
    pub fn action_name(&self) -> Result<String> {
        let server = self
            .response
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let server = server.lock();
        server.action_name()
    }

    /// Accept the goal request and become a ServerGoal.
    /// Returns a handle to the goal and a stream on which cancel requests can be received.
    pub fn accept(
        self,
    ) -> Result<(ActionServerGoal<T>, impl Stream<Item = ActionServerCancelRequest> + Unpin)> {
        self.accept_with_execution(true)
    }

    /// Accept the goal without starting its execution.
    ///
    /// The returned goal remains in [`GoalStatus::Accepted`] until
    /// [`ActionServerGoal::execute`] is called.
    pub fn accept_deferred(
        self,
    ) -> Result<(ActionServerGoal<T>, impl Stream<Item = ActionServerCancelRequest> + Unpin)> {
        self.accept_with_execution(false)
    }

    fn accept_with_execution(
        mut self, execute: bool,
    ) -> Result<(ActionServerGoal<T>, mpsc::Receiver<ActionServerCancelRequest>)> {
        let uuid_msg = unique_identifier_msgs::msg::UUID {
            uuid: self.uuid.as_bytes().to_vec(),
        };
        let time = {
            let mut clock = self.clock.lock();
            let mut now = 0;
            let ret = unsafe { rcl_clock_get_now(clock.clock_handle.as_mut(), &mut now) };
            if ret != RCL_RET_OK as i32 {
                return Err(Error::from_rcl_error(ret));
            }
            time_point_to_builtin(now)?
        };
        let goal_info = action_msgs::msg::GoalInfo {
            goal_id: uuid_msg,
            stamp: time.clone(),
        };
        let native_goal_info =
            WrappedNativeMsg::<action_msgs::msg::GoalInfo>::try_from(&goal_info)?;
        let response_msg = T::make_goal_response_msg(true, time);
        let mut response_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response,
        >::try_from(&response_msg)?;

        let server = self
            .response
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let mut server = server.lock();

        let goal_handle: *mut rcl_action_goal_handle_t =
            unsafe { rcl_action_accept_new_goal(server.handle_mut(), &*native_goal_info) };
        if goal_handle.is_null() {
            return Err(Error::RCL_RET_ACTION_GOAL_HANDLE_INVALID);
        }
        // RCL owns the allocation containing `goal_handle`, but not the handle's internal
        // allocation. Keep the documented value copy so it can be finalized after RCL expires
        // the goal or destroys the server-side storage.
        server.add_goal_handle(self.uuid, goal_handle);

        if execute {
            let ret = unsafe {
                rcl_action_update_goal_state(
                    goal_handle,
                    rcl_action_goal_event_t::GOAL_EVENT_EXECUTE,
                )
            };
            if ret != RCL_RET_OK as i32 {
                let mut completions = Vec::new();
                if let Err(rollback_err) = abort_goal_after_failed_accept(&mut *server, &self.uuid)
                {
                    log::error!(
                        "could not abort goal after execution setup failed: {rollback_err}"
                    );
                }
                server.forget_cancel_sender(&self.uuid, &mut completions);
                drop(server);
                return finish_action_server_operation(
                    Err(Error::from_rcl_error(ret)),
                    completions,
                )
                .map(|()| unreachable!());
            }
        }

        // send response
        self.response.response_sent = true;
        let ret = unsafe {
            rcl_action_send_goal_response(
                server.handle_mut(),
                &mut self.response.request_id,
                response_msg.void_ptr_mut(),
            )
        };
        if ret != RCL_RET_OK as i32 {
            let mut completions = Vec::new();
            if let Err(rollback_err) = abort_goal_after_failed_accept(&mut *server, &self.uuid) {
                log::error!(
                    "could not abort goal after sending its response failed: {rollback_err}"
                );
            }
            server.forget_cancel_sender(&self.uuid, &mut completions);
            drop(server);
            return finish_action_server_operation(Err(Error::from_rcl_error(ret)), completions)
                .map(|()| unreachable!());
        }

        let g = ActionServerGoal {
            uuid: self.uuid,
            goal: self.goal,
            lifecycle: Arc::new(ServerGoalLifecycle {
                server: self.response.server.clone(),
                uuid: self.uuid,
                action_type: PhantomData,
            }),
        };

        if let Err(err) = server.publish_status() {
            // The accepted response is already on the wire, so returning an error here would
            // drop the only application handle for a live goal. Keep the goal manageable and
            // report the notification failure instead.
            log::error!("could not publish initial action goal status: {err}");
        }

        Ok((g, self.cancel_requests))
    }

    /// reject the goal request and be consumed in the process
    pub fn reject(mut self) -> Result<()> {
        let server = self
            .response
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        self.response.response_sent = true;
        complete_action_server_operation(&server, |server, completions| {
            server.reject_goal_request(&self.uuid, &mut self.response.request_id, completions)
        })
    }
}

impl Drop for GoalRequestResponseGuard {
    fn drop(&mut self) {
        if self.response_sent {
            return;
        }
        let Some(server) = self.server.upgrade() else {
            return;
        };
        if let Err(err) = complete_action_server_operation(&server, |server, completions| {
            server.reject_goal_request(&self.uuid, &mut self.request_id, completions)
        }) {
            log::error!("could not reject dropped action goal request: {err}");
        }
        self.response_sent = true;
    }
}
pub type ActiveCancelRequest = (
    rmw_request_id_t,
    action_msgs::srv::CancelGoal::Response,
    JoinAll<oneshot::Receiver<(uuid::Uuid, bool)>>,
);

pub(crate) struct PendingCancelDelivery {
    uuid: uuid::Uuid,
    request: ActionServerCancelRequest,
}

struct DetachedCancelDelivery {
    sender_slot: Arc<Mutex<Option<mpsc::Sender<ActionServerCancelRequest>>>>,
    sender: mpsc::Sender<ActionServerCancelRequest>,
    delivery: PendingCancelDelivery,
}

fn cancel_request_delivery(
    detached: Vec<DetachedCancelDelivery>, mut pending: PendingCancelRequest,
    finish: impl FnOnce(PendingCancelRequest) -> Result<()> + 'static,
) -> ActionServerCompletion {
    Box::new(move || {
        for DetachedCancelDelivery {
            sender_slot,
            mut sender,
            delivery,
        } in detached
        {
            let uuid = delivery.uuid;
            match sender.try_send(delivery.request) {
                Ok(()) => {}
                Err(err) if err.is_full() => pending.deliveries.push(PendingCancelDelivery {
                    uuid,
                    request: err.into_inner(),
                }),
                Err(_) => {}
            }
            *sender_slot.lock() = Some(sender);
        }
        finish(pending)
    })
}

pub(crate) struct PendingCancelRequest {
    request_id: rmw_request_id_t,
    response: action_msgs::srv::CancelGoal::Response,
    deliveries: Vec<PendingCancelDelivery>,
    decisions: Vec<oneshot::Receiver<(uuid::Uuid, bool)>>,
}

struct CancelResponse(rcl_action_cancel_response_t);

impl CancelResponse {
    fn new() -> Self {
        Self(unsafe { rcl_action_get_zero_initialized_cancel_response() })
    }
}

impl Drop for CancelResponse {
    fn drop(&mut self) {
        let ret = unsafe { rcl_action_cancel_response_fini(&mut self.0) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize action cancel response: {}", ret);
        }
    }
}

fn retain_accepted_cancellations(
    response: &mut action_msgs::srv::CancelGoal::Response,
    decisions: impl IntoIterator<Item = (uuid::Uuid, bool)>,
) -> Result<Vec<uuid::Uuid>> {
    let accepted: HashSet<_> = decisions
        .into_iter()
        .filter_map(|(uuid, accept)| accept.then_some(uuid))
        .collect();

    let mut canceled = Vec::new();
    let mut retain = Vec::with_capacity(response.goals_canceling.len());
    for goal_info in &response.goals_canceling {
        let uuid = uuid_msg_to_uuid(&goal_info.goal_id)?;
        let is_accepted = accepted.contains(&uuid);
        retain.push(is_accepted);
        if is_accepted {
            canceled.push(uuid);
        }
    }
    let mut retain = retain.into_iter();
    response.goals_canceling.retain(|_| {
        // The mask was built with exactly one entry per goal above.
        retain.next().unwrap_or(false)
    });
    Ok(canceled)
}

pub struct WrappedActionServer<T>
where
    T: WrappedActionTypeSupport,
{
    pub rcl_handle: rcl_action_server_t,
    pub clock: Arc<Mutex<Clock>>,
    pub goal_request_sender: Arc<Mutex<Option<mpsc::Sender<ActionServerGoalRequest<T>>>>>,
    pub cancel_senders:
        HashMap<uuid::Uuid, Arc<Mutex<Option<mpsc::Sender<ActionServerCancelRequest>>>>>,
    pub(crate) pending_cancel_request: Option<PendingCancelRequest>,
    pub active_cancel_requests: Vec<ActiveCancelRequest>,
    pub(crate) goals: HashMap<uuid::Uuid, OwnedGoalHandle>,
    pub result_msgs: HashMap<uuid::Uuid, Box<dyn VoidPtr>>,
    pub result_requests: HashMap<uuid::Uuid, Vec<rmw_request_id_t>>,
    pub cancel_request_channel_capacity: usize,
}

pub(crate) struct OwnedGoalHandle(Option<rcl_action_goal_handle_t>);

impl OwnedGoalHandle {
    unsafe fn copy_from(handle: *const rcl_action_goal_handle_t) -> Self {
        Self(Some(std::ptr::read(handle)))
    }

    fn as_ptr(&self) -> *const rcl_action_goal_handle_t {
        self.0.as_ref().expect("owned goal handle was finalized")
    }

    fn as_mut_ptr(&mut self) -> *mut rcl_action_goal_handle_t {
        self.0.as_mut().expect("owned goal handle was finalized")
    }

    fn fini(&mut self) -> rcl_ret_t {
        let Some(mut handle) = self.0.take() else {
            return RCL_RET_OK as i32;
        };
        unsafe { rcl_action_goal_handle_fini(&mut handle) }
    }
}

impl Drop for OwnedGoalHandle {
    fn drop(&mut self) {
        let ret = self.fini();
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize action goal handle: {ret}");
        }
    }
}

impl<T> WrappedActionServer<T>
where
    T: WrappedActionTypeSupport,
{
    fn finish_cancel_delivery(&mut self, mut pending: PendingCancelRequest) {
        pending
            .deliveries
            .retain(|delivery| self.cancel_senders.contains_key(&delivery.uuid));
        if pending.deliveries.is_empty() {
            self.active_cancel_requests.push((
                pending.request_id,
                pending.response,
                join_all(pending.decisions),
            ));
        } else {
            self.pending_cancel_request = Some(pending);
        }
    }

    fn take_pending_cancel_delivery(
        &mut self, server: Arc<Mutex<dyn ActionServer_>>,
    ) -> Option<ActionServerCompletion> {
        let mut pending = self.pending_cancel_request.take()?;
        let mut detached = Vec::new();
        let deliveries = std::mem::take(&mut pending.deliveries);
        for delivery in deliveries {
            let Some(sender_slot) = self.cancel_senders.get(&delivery.uuid).cloned() else {
                continue;
            };
            let Some(sender) = sender_slot.lock().take() else {
                pending.deliveries.push(delivery);
                continue;
            };
            detached.push(DetachedCancelDelivery {
                sender_slot,
                sender,
                delivery,
            });
        }

        if detached.is_empty() {
            self.finish_cancel_delivery(pending);
            return None;
        }

        Some(cancel_request_delivery(detached, pending, move |pending| {
            server.lock().finish_cancel_delivery(pending);
            Ok(())
        }))
    }
}

impl<T: 'static> ActionServer_ for WrappedActionServer<T>
where
    T: WrappedActionTypeSupport,
{
    fn handle(&self) -> &rcl_action_server_t {
        &self.rcl_handle
    }

    fn handle_mut(&mut self) -> &mut rcl_action_server_t {
        &mut self.rcl_handle
    }

    fn take_pending_cancel_delivery(
        &mut self, server: Arc<Mutex<dyn ActionServer_>>,
    ) -> Option<ActionServerCompletion> {
        WrappedActionServer::take_pending_cancel_delivery(self, server)
    }

    fn finish_cancel_delivery(&mut self, pending: PendingCancelRequest) {
        WrappedActionServer::finish_cancel_delivery(self, pending);
    }

    fn reject_goal_request(
        &mut self, uuid: &uuid::Uuid, request_id: &mut rmw_request_id_t,
        completions: &mut Vec<ActionServerCompletion>,
    ) -> Result<()> {
        self.forget_cancel_sender(uuid, completions);
        let response = T::make_goal_response_msg(false, builtin_interfaces::msg::Time::default());
        let mut response = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response,
        >::try_from(&response)?;
        let ret = unsafe {
            rcl_action_send_goal_response(&self.rcl_handle, request_id, response.void_ptr_mut())
        };
        if ret == RCL_RET_OK as i32 {
            Ok(())
        } else {
            Err(Error::from_rcl_error(ret))
        }
    }

    fn is_cancelling(&self, uuid: &uuid::Uuid) -> Result<bool> {
        Ok(self.get_goal_status(uuid)? == GoalStatus::Canceling)
    }

    fn get_goal_status(&self, uuid: &uuid::Uuid) -> Result<GoalStatus> {
        if let Some(handle) = self.goals.get(uuid) {
            let mut state = action_msgs::msg::GoalStatus::STATUS_UNKNOWN as u8;
            let ret = unsafe { rcl_action_goal_handle_get_status(handle.as_ptr(), &mut state) };

            if ret != RCL_RET_OK as i32 {
                log::debug!("action server: Failed to get goal handle state: {}", ret);
                return Err(Error::from_rcl_error(ret));
            }
            return Ok(GoalStatus::from_rcl(state as i8));
        }
        Err(Error::RCL_RET_ACTION_GOAL_HANDLE_INVALID)
    }

    fn execute_goal(&mut self, uuid: &uuid::Uuid) -> Result<()> {
        let handle = self
            .goals
            .get_mut(uuid)
            .ok_or(Error::RCL_RET_ACTION_GOAL_HANDLE_INVALID)?;
        let ret = unsafe {
            rcl_action_update_goal_state(
                handle.as_mut_ptr(),
                rcl_action_goal_event_t::GOAL_EVENT_EXECUTE,
            )
        };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        self.publish_status()
    }

    fn cancel_goal(&mut self, uuid: &uuid::Uuid) -> Result<()> {
        if let Some(handle) = self.goals.get_mut(uuid) {
            let ret = unsafe {
                rcl_action_update_goal_state(
                    handle.as_mut_ptr(),
                    rcl_action_goal_event_t::GOAL_EVENT_CANCEL_GOAL,
                )
            };

            if ret != RCL_RET_OK as i32 {
                return Err(Error::from_rcl_error(ret));
            }
        }
        Ok(())
    }

    fn set_goal_state(
        &mut self, uuid: &uuid::Uuid, new_state: rcl_action_goal_event_t,
    ) -> Result<()> {
        let goal_info = action_msgs::msg::GoalInfo {
            goal_id: unique_identifier_msgs::msg::UUID {
                uuid: uuid.as_bytes().to_vec(),
            },
            ..action_msgs::msg::GoalInfo::default()
        };
        let goal_info_native =
            WrappedNativeMsg::<action_msgs::msg::GoalInfo>::try_from(&goal_info)?;

        // does this goal exist?
        let goal_exists =
            unsafe { rcl_action_server_goal_exists(self.handle(), &*goal_info_native) };

        if !goal_exists {
            log::debug!("tried to publish result without a goal");
            return Err(Error::RCL_RET_ACTION_GOAL_HANDLE_INVALID);
        }

        if let Some(handle) = self.goals.get_mut(uuid) {
            let ret = unsafe { rcl_action_update_goal_state(handle.as_mut_ptr(), new_state) };
            if ret != RCL_RET_OK as i32 {
                return Err(Error::from_rcl_error(ret));
            }

            Ok(())
        } else {
            Err(Error::RCL_RET_ACTION_GOAL_HANDLE_INVALID)
        }
    }

    fn notify_goal_done(&self) -> Result<()> {
        let ret = unsafe { rcl_action_notify_goal_done(self.handle()) };
        if ret == RCL_RET_OK as i32 {
            Ok(())
        } else {
            Err(Error::from_rcl_error(ret))
        }
    }

    fn send_completed_cancel_requests(&mut self) -> Result<()> {
        let mut canceled = vec![];
        let mut responses = vec![];
        let mut first_error = None;
        self.active_cancel_requests
            .retain_mut(|(request_id, msg, fut)| {
                let boxed = fut.boxed();
                if let Some(results) = boxed.now_or_never() {
                    let mut response_msg = msg.clone();
                    let requested_cancels = response_msg.goals_canceling.len();
                    let decisions = results.into_iter().filter_map(|result| match result {
                        Ok(decision) => Some(decision),
                        Err(oneshot::Canceled) => {
                            log::error!("Warning, cancel request not handled!");
                            None
                        }
                    });
                    match retain_accepted_cancellations(&mut response_msg, decisions) {
                        Ok(accepted) => canceled.extend(accepted),
                        Err(err) => {
                            first_error.get_or_insert(err);
                            return false;
                        }
                    }

                    // check if all cancels were rejected.
                    if requested_cancels >= 1 && response_msg.goals_canceling.is_empty() {
                        response_msg.return_code =
                            action_msgs::srv::CancelGoal::Response::ERROR_REJECTED as i8;
                    }

                    responses.push((*request_id, response_msg));

                    false
                } else {
                    true
                }
            });

        for uuid in &canceled {
            if let Err(err) = self.cancel_goal(uuid) {
                first_error.get_or_insert(err);
            }
        }
        if !canceled.is_empty() {
            // at least one goal state changed, publish a new status message
            if let Err(err) = self.publish_status() {
                first_error.get_or_insert(err);
            }
        }

        // send out responses
        for (mut request_id, response_msg) in responses {
            // send out response msg.
            let mut native_msg =
                match WrappedNativeMsg::<action_msgs::srv::CancelGoal::Response>::try_from(
                    &response_msg,
                ) {
                    Ok(message) => message,
                    Err(err) => {
                        first_error.get_or_insert(err);
                        continue;
                    }
                };
            let ret = unsafe {
                rcl_action_send_cancel_response(
                    &self.rcl_handle,
                    &mut request_id,
                    native_msg.void_ptr_mut(),
                )
            };

            if ret != RCL_RET_OK as i32 {
                first_error.get_or_insert_with(|| Error::from_rcl_error(ret));
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    fn handle_goal_request(
        &mut self, server: Arc<Mutex<dyn ActionServer_>>,
    ) -> Result<Option<ActionServerCompletion>> {
        {
            let mut goal_request_sender = self.goal_request_sender.lock();
            let Some(goal_request_sender) = goal_request_sender.as_mut() else {
                return Ok(None);
            };
            if !goal_request_sender.is_closed() {
                let mut cx = Context::from_waker(noop_waker_ref());
                if goal_request_sender.poll_ready(&mut cx) == Poll::Pending {
                    return Ok(None);
                }
            }
        }

        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut request_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Request,
        >::try_new()?;
        let ret = unsafe {
            rcl_action_take_goal_request(
                &self.rcl_handle,
                request_id.as_mut_ptr(),
                request_msg.void_ptr_mut(),
            )
        };

        if !action_server_take_succeeded(ret)? {
            return Ok(None);
        }
        let mut request_id = unsafe { request_id.assume_init() };
        let msg = <<<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Request>::from_native(&request_msg);
        let (uuid_msg, goal) = T::destructure_goal_request_msg(msg);
        let uuid = match uuid_msg_to_uuid(&uuid_msg) {
            Ok(uuid) => uuid,
            Err(err) => {
                let mut completions = Vec::new();
                if let Err(reject_err) =
                    self.reject_goal_request(&uuid::Uuid::nil(), &mut request_id, &mut completions)
                {
                    log::error!("could not reject malformed action goal request: {reject_err}");
                }
                return Err(err);
            }
        };

        let (cancel_sender, cancel_receiver) =
            mpsc::channel::<ActionServerCancelRequest>(self.cancel_request_channel_capacity);
        self.cancel_senders
            .insert(uuid, Arc::new(Mutex::new(Some(cancel_sender))));

        let gr: ActionServerGoalRequest<T> = ActionServerGoalRequest {
            uuid,
            goal,
            clock: Arc::clone(&self.clock),
            cancel_requests: cancel_receiver,
            response: GoalRequestResponseGuard {
                server: Arc::downgrade(&server),
                uuid,
                request_id,
                response_sent: false,
            },
        };

        let sender = self
            .goal_request_sender
            .lock()
            .take()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let sender_slot = Arc::clone(&self.goal_request_sender);
        Ok(Some(request_delivery(sender_slot, sender, gr, move |mut request| {
            request.response.response_sent = true;
            let server = request
                .response
                .server
                .upgrade()
                .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
            complete_action_server_operation(&server, |server, completions| {
                server.reject_goal_request(
                    &request.uuid,
                    &mut request.response.request_id,
                    completions,
                )
            })
        })))
    }

    fn handle_cancel_request(
        &mut self, server: Arc<Mutex<dyn ActionServer_>>,
    ) -> Result<Option<ActionServerCompletion>> {
        if self.pending_cancel_request.is_some() {
            return Ok(self.take_pending_cancel_delivery(server));
        }
        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut request_msg = WrappedNativeMsg::<action_msgs::srv::CancelGoal::Request>::try_new()?;
        let ret = unsafe {
            rcl_action_take_cancel_request(
                &self.rcl_handle,
                request_id.as_mut_ptr(),
                request_msg.void_ptr_mut(),
            )
        };

        if !action_server_take_succeeded(ret)? {
            return Ok(None);
        }

        // `rcl_action_take_cancel_request` only initializes the request id on success.
        let request_id = unsafe { request_id.assume_init() };

        let mut cancel_response = CancelResponse::new();
        let ret = unsafe {
            rcl_action_process_cancel_request(
                &self.rcl_handle,
                &*request_msg,
                &mut cancel_response.0,
            )
        };

        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let response_msg =
            action_msgs::srv::CancelGoal::Response::from_native(&cancel_response.0.msg);

        let deliveries_and_decisions = response_msg
            .goals_canceling
            .iter()
            .map(|goal_info| {
                let uuid = uuid_msg_to_uuid(&goal_info.goal_id)?;
                let (response_sender, response_receiver) = oneshot::channel::<(uuid::Uuid, bool)>();
                Ok((
                    PendingCancelDelivery {
                        uuid,
                        request: ActionServerCancelRequest {
                            uuid,
                            response_sender,
                        },
                    },
                    response_receiver,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let (deliveries, decisions): (Vec<_>, Vec<_>) =
            deliveries_and_decisions.into_iter().unzip();

        self.pending_cancel_request = Some(PendingCancelRequest {
            request_id,
            response: response_msg,
            deliveries,
            decisions,
        });
        Ok(self.take_pending_cancel_delivery(server))
    }

    fn handle_goal_expired(&mut self, completions: &mut Vec<ActionServerCompletion>) -> Result<()> {
        let mut goal_info = WrappedNativeMsg::<action_msgs::msg::GoalInfo>::try_new()?;

        loop {
            let mut num_expired = 0;
            let ret = unsafe {
                rcl_action_expire_goals(&self.rcl_handle, &mut *goal_info, 1, &mut num_expired)
            };
            if ret != RCL_RET_OK as i32 {
                return Err(Error::from_rcl_error(ret));
            }
            if num_expired == 0 {
                return Ok(());
            }

            let gi = action_msgs::msg::GoalInfo::from_native(&goal_info);
            let uuid = uuid_msg_to_uuid(&gi.goal_id)?;
            log::debug!("goal expired: {} - {}", uuid, num_expired);
            self.goals.remove(&uuid);
            self.forget_cancel_sender(&uuid, completions);
            self.result_msgs.remove(&uuid);
            self.result_requests.remove(&uuid);
        }
    }

    fn action_name(&self) -> Result<String> {
        let name = unsafe { rcl_action_server_get_action_name(&self.rcl_handle) };
        if name.is_null() {
            return Err(Error::RCL_RET_ACTION_SERVER_INVALID);
        }
        unsafe { CStr::from_ptr(name) }
            .to_str()
            .map(str::to_owned)
            .map_err(|err| Error::InvalidUtf8 {
                context: "action server name",
                err: err.to_string(),
            })
    }

    fn publish_status(&self) -> Result<()> {
        publish_action_status(&self.rcl_handle)
    }

    fn add_goal_handle(&mut self, uuid: uuid::Uuid, goal_handle: *mut rcl_action_goal_handle_t) {
        self.goals
            .insert(uuid, unsafe { OwnedGoalHandle::copy_from(goal_handle) });
    }

    fn forget_cancel_sender(
        &mut self, uuid: &uuid::Uuid, completions: &mut Vec<ActionServerCompletion>,
    ) {
        if let Some(sender_slot) = self.cancel_senders.remove(uuid) {
            completions.push(sender_drop_completion(sender_slot));
        }
    }

    fn should_remove(&self) -> bool {
        self.goal_request_sender
            .lock()
            .as_ref()
            .is_some_and(mpsc::Sender::is_closed)
            && self.pending_cancel_request.is_none()
            && self.active_cancel_requests.is_empty()
            && self.cancel_senders.is_empty()
            && self.goals.is_empty()
            && self.result_msgs.is_empty()
            && self.result_requests.is_empty()
    }

    // bit of a hack...
    fn add_result(&mut self, uuid: uuid::Uuid, mut msg: Box<dyn VoidPtr>) -> Result<()> {
        // if there are already requests for this goal, send the result immediately.
        let mut first_error = None;
        if let Some(rr) = self.result_requests.remove(&uuid) {
            for mut req in rr {
                let ret = unsafe {
                    rcl_action_send_result_response(&self.rcl_handle, &mut req, msg.void_ptr_mut())
                };
                if ret != RCL_RET_OK as i32 {
                    first_error.get_or_insert_with(|| Error::from_rcl_error(ret));
                }
            }
        }
        self.result_msgs.insert(uuid, msg);
        first_error.map_or(Ok(()), Err)
    }

    fn handle_result_request(&mut self) -> Result<()> {
        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut request_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Request,
        >::try_new()?;
        let ret = unsafe {
            rcl_action_take_result_request(
                &self.rcl_handle,
                request_id.as_mut_ptr(),
                request_msg.void_ptr_mut(),
            )
        };

        if !action_server_take_succeeded(ret)? {
            return Ok(());
        }
        let mut request_id = unsafe { request_id.assume_init() };

        let msg = <<<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Request>::from_native(&request_msg);
        let goal_info = action_msgs::msg::GoalInfo {
            goal_id: T::destructure_result_request_msg(msg),
            ..action_msgs::msg::GoalInfo::default()
        };
        let uuid = uuid_msg_to_uuid(&goal_info.goal_id)?;
        let goal_info_native =
            WrappedNativeMsg::<action_msgs::msg::GoalInfo>::try_from(&goal_info)?;

        // does this goal exist?
        let goal_exists =
            unsafe { rcl_action_server_goal_exists(&self.rcl_handle, &*goal_info_native) };

        if !goal_exists {
            // Goal does not exists
            let msg = T::make_result_response_msg(
                action_msgs::msg::GoalStatus::STATUS_UNKNOWN as i8,
                T::Result::default(),
            );
            let mut response_msg = WrappedNativeMsg::<
                <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response,
            >::try_from(&msg)?;
            let ret = unsafe {
                rcl_action_send_result_response(
                    &self.rcl_handle,
                    &mut request_id,
                    response_msg.void_ptr_mut(),
                )
            };
            if ret != RCL_RET_OK as i32 {
                return Err(Error::from_rcl_error(ret));
            }
        } else if let Some(response_msg) = self.result_msgs.get_mut(&uuid) {
            let ret = unsafe {
                rcl_action_send_result_response(
                    &self.rcl_handle,
                    &mut request_id,
                    response_msg.void_ptr_mut(),
                )
            };
            if ret != RCL_RET_OK as i32 {
                return Err(Error::from_rcl_error(ret));
            }
        } else {
            // keep request for later when result comes in
            self.result_requests
                .entry(uuid)
                .or_default()
                .push(request_id);
        }
        Ok(())
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_action_server_fini(&mut self.rcl_handle, node);
        }
        // RCL has now released the storage containing its handle copies. Finalize the internal
        // allocations through the copies owned by r2r.
        self.goals.clear();
    }
}

fn publish_action_status(server: &rcl_action_server_t) -> Result<()> {
    unsafe {
        let mut status = rcl_action_get_zero_initialized_goal_status_array();
        let ret = rcl_action_get_goal_status_array(server, &mut status);
        if ret != RCL_RET_OK as i32 {
            rcl_action_goal_status_array_fini(&mut status);
            return Err(Error::from_rcl_error(ret));
        }
        let ret =
            rcl_action_publish_status(server, &status as *const _ as *const std::os::raw::c_void);
        let result = if ret == RCL_RET_OK as i32 {
            Ok(())
        } else {
            Err(Error::from_rcl_error(ret))
        };
        let fini_ret = rcl_action_goal_status_array_fini(&mut status);
        if fini_ret != RCL_RET_OK as i32 {
            log::error!("could not finalize action goal status array: {fini_ret}");
        }
        result
    }
}

fn complete_goal(
    server: &mut dyn ActionServer_, uuid: uuid::Uuid, event: rcl_action_goal_event_t,
    result: Box<dyn VoidPtr>,
) -> Result<()> {
    server.set_goal_state(&uuid, event)?;

    let mut first_error = server.add_result(uuid, result).err();
    if let Err(err) = server.publish_status() {
        first_error.get_or_insert(err);
    }
    if let Err(err) = server.notify_goal_done() {
        first_error.get_or_insert(err);
    }
    first_error.map_or(Ok(()), Err)
}

fn abort_goal_after_failed_accept(server: &mut dyn ActionServer_, uuid: &uuid::Uuid) -> Result<()> {
    server.set_goal_state(uuid, rcl_action_goal_event_t::GOAL_EVENT_ABORT)?;
    server.notify_goal_done()
}

fn time_point_to_builtin(time: rcl_time_point_value_t) -> Result<builtin_interfaces::msg::Time> {
    let seconds = time.div_euclid(1_000_000_000);
    let nanoseconds = time.rem_euclid(1_000_000_000);
    Ok(builtin_interfaces::msg::Time {
        sec: i32::try_from(seconds).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?,
        nanosec: nanoseconds as u32,
    })
}

fn action_server_take_succeeded(ret: rcl_ret_t) -> Result<bool> {
    if ret == RCL_RET_OK as i32 {
        Ok(true)
    } else if ret == RCL_RET_ACTION_SERVER_TAKE_FAILED as i32 {
        Ok(false)
    } else {
        Err(Error::from_rcl_error(ret))
    }
}

/// A handle to an active `Goal`
#[derive(Clone)]
pub struct ActionServerGoal<T>
where
    T: WrappedActionTypeSupport,
{
    pub uuid: uuid::Uuid,
    pub goal: T::Goal,
    lifecycle: Arc<ServerGoalLifecycle<T>>,
}

struct ServerGoalLifecycle<T>
where
    T: WrappedActionTypeSupport,
{
    server: Weak<Mutex<dyn ActionServer_>>,
    uuid: uuid::Uuid,
    action_type: PhantomData<T>,
}

impl<T> Drop for ServerGoalLifecycle<T>
where
    T: WrappedActionTypeSupport,
{
    fn drop(&mut self) {
        let Some(server) = self.server.upgrade() else {
            return;
        };
        if !matches!(
            server.lock().get_goal_status(&self.uuid),
            Ok(GoalStatus::Accepted | GoalStatus::Executing | GoalStatus::Canceling)
        ) {
            return;
        }

        // Allocate the result before changing state. If allocation fails, the goal remains in a
        // state that a future recovery path can still complete.
        let result = T::make_result_response_msg(
            action_msgs::msg::GoalStatus::STATUS_CANCELED as i8,
            T::Result::default(),
        );
        let result = match WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response,
        >::try_from(&result)
        {
            Ok(result) => result,
            Err(err) => {
                log::error!("could not allocate result for abandoned action goal: {err}");
                return;
            }
        };
        let mut server = server.lock();
        let needs_cancel = match server.get_goal_status(&self.uuid) {
            Ok(GoalStatus::Accepted | GoalStatus::Executing) => true,
            Ok(GoalStatus::Canceling) => false,
            Ok(
                GoalStatus::Succeeded
                | GoalStatus::Canceled
                | GoalStatus::Aborted
                | GoalStatus::Unknown,
            )
            | Err(_) => return,
        };
        if needs_cancel {
            if let Err(err) = server.cancel_goal(&self.uuid) {
                log::error!("could not cancel abandoned action goal: {err}");
                return;
            }
        }
        if let Err(err) = complete_goal(
            &mut *server,
            self.uuid,
            rcl_action_goal_event_t::GOAL_EVENT_CANCELED,
            Box::new(result),
        ) {
            log::error!("could not complete abandoned action goal: {err}");
        }
    }
}

unsafe impl<T> Send for ActionServerGoal<T> where T: WrappedActionTypeSupport {}

impl<T: 'static> ActionServerGoal<T>
where
    T: WrappedActionTypeSupport,
{
    /// Returns the fully resolved action name used by the native server.
    pub fn action_name(&self) -> Result<String> {
        let server = self
            .lifecycle
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let server = server.lock();
        server.action_name()
    }

    /// Return the server-side state of this goal.
    pub fn get_status(&self) -> Result<GoalStatus> {
        let action_server = self
            .lifecycle
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let action_server = action_server.lock();
        action_server.get_goal_status(&self.uuid)
    }

    /// Start executing a goal accepted with [`ActionServerGoalRequest::accept_deferred`].
    pub fn execute(&self) -> Result<()> {
        let action_server = self
            .lifecycle
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let result = action_server.lock().execute_goal(&self.uuid);
        result
    }

    pub fn is_cancelling(&self) -> Result<bool> {
        let action_server = self
            .lifecycle
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;

        let action_server = action_server.lock();
        action_server.is_cancelling(&self.uuid)
    }

    pub fn publish_feedback(&self, msg: T::Feedback) -> Result<()>
    where
        T: WrappedActionTypeSupport,
    {
        // upgrade to actual ref. if still alive
        let action_server = self
            .lifecycle
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;

        let uuid_msg = unique_identifier_msgs::msg::UUID {
            uuid: self.uuid.as_bytes().to_vec(),
        };
        let feedback_msg = T::make_feedback_msg(uuid_msg, msg);
        let mut native_msg = WrappedNativeMsg::<T::FeedbackMessage>::try_from(&feedback_msg)?;
        let ret = unsafe {
            rcl_action_publish_feedback(action_server.lock().handle(), native_msg.void_ptr_mut())
        };

        if ret != RCL_RET_OK as i32 {
            log::error!("could not publish {}", Error::from_rcl_error(ret));
            return Err(Error::from_rcl_error(ret));
        }
        Ok(())
    }

    pub fn cancel(&mut self, msg: T::Result) -> Result<()> {
        // upgrade to actual ref. if still alive
        let action_server = self
            .lifecycle
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        with_prepared_action_server(
            &action_server,
            || {
                let result_msg = T::make_result_response_msg(
                    action_msgs::msg::GoalStatus::STATUS_CANCELED as i8,
                    msg,
                );
                WrappedNativeMsg::try_from(&result_msg)
            },
            |action_server, native_msg| {
                complete_goal(
                    action_server,
                    self.uuid,
                    rcl_action_goal_event_t::GOAL_EVENT_CANCELED,
                    Box::new(native_msg),
                )
            },
        )
    }

    pub fn abort(&mut self, msg: T::Result) -> Result<()> {
        // upgrade to actual ref. if still alive
        let action_server = self
            .lifecycle
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        with_prepared_action_server(
            &action_server,
            || {
                let result_msg = T::make_result_response_msg(
                    action_msgs::msg::GoalStatus::STATUS_ABORTED as i8,
                    msg,
                );
                WrappedNativeMsg::try_from(&result_msg)
            },
            |action_server, native_msg| {
                complete_goal(
                    action_server,
                    self.uuid,
                    rcl_action_goal_event_t::GOAL_EVENT_ABORT,
                    Box::new(native_msg),
                )
            },
        )
    }

    pub fn succeed(&mut self, msg: T::Result) -> Result<()>
    where
        T: WrappedActionTypeSupport,
    {
        // upgrade to actual ref. if still alive
        let action_server = self
            .lifecycle
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        with_prepared_action_server(
            &action_server,
            || {
                let result_msg = T::make_result_response_msg(
                    action_msgs::msg::GoalStatus::STATUS_SUCCEEDED as i8,
                    msg,
                );
                WrappedNativeMsg::try_from(&result_msg)
            },
            |action_server, native_msg| {
                complete_goal(
                    action_server,
                    self.uuid,
                    rcl_action_goal_event_t::GOAL_EVENT_SUCCEED,
                    Box::new(native_msg),
                )
            },
        )
    }
}

pub fn create_action_server_helper(
    node: &mut rcl_node_t, action_name: &str, clock_handle: *mut rcl_clock_t,
    action_ts: *const rosidl_action_type_support_t, overrides: &ActionServerOptions,
) -> Result<rcl_action_server_t> {
    let mut server_handle = unsafe { rcl_action_get_zero_initialized_server() };
    let action_name_c_string =
        CString::new(action_name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;

    let result = unsafe {
        let mut server_options = rcl_action_server_get_default_options();
        overrides.apply(&mut server_options)?;

        rcl_action_server_init(
            &mut server_handle,
            node,
            clock_handle,
            action_ts,
            action_name_c_string.as_ptr(),
            &server_options,
        )
    };
    if result == RCL_RET_OK as i32 {
        Ok(server_handle)
    } else {
        Err(Error::from_rcl_error(result))
    }
}

pub fn action_server_get_num_waits(
    rcl_handle: &rcl_action_server_t, num_subs: &mut usize, num_gc: &mut usize,
    num_timers: &mut usize, num_clients: &mut usize, num_services: &mut usize,
) -> Result<()> {
    unsafe {
        let result = rcl_action_server_wait_set_get_num_entities(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{example_interfaces::action::Fibonacci, Context, Node};
    use futures::{
        executor::LocalPool,
        task::{waker_ref, ArcWake, LocalSpawnExt},
        StreamExt,
    };
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };

    struct DummyResult;

    impl VoidPtr for DummyResult {
        fn void_ptr(&self) -> *const std::os::raw::c_void {
            std::ptr::null()
        }

        fn void_ptr_mut(&mut self) -> *mut std::os::raw::c_void {
            std::ptr::null_mut()
        }
    }

    struct CompletionTestServer {
        handle: rcl_action_server_t,
        calls: Mutex<Vec<&'static str>>,
        state_error: bool,
        add_result_error: bool,
        publish_status_error: bool,
        notify_error: bool,
    }

    impl CompletionTestServer {
        fn new(
            state_error: bool, add_result_error: bool, publish_status_error: bool,
            notify_error: bool,
        ) -> Self {
            Self {
                handle: unsafe { rcl_action_get_zero_initialized_server() },
                calls: Mutex::new(Vec::new()),
                state_error,
                add_result_error,
                publish_status_error,
                notify_error,
            }
        }
    }

    impl ActionServer_ for CompletionTestServer {
        fn handle(&self) -> &rcl_action_server_t {
            &self.handle
        }

        fn handle_mut(&mut self) -> &mut rcl_action_server_t {
            &mut self.handle
        }

        fn reject_goal_request(
            &mut self, _uuid: &uuid::Uuid, _request_id: &mut rmw_request_id_t,
            _completions: &mut Vec<ActionServerCompletion>,
        ) -> Result<()> {
            unreachable!()
        }

        fn handle_goal_request(
            &mut self, _server: Arc<Mutex<dyn ActionServer_>>,
        ) -> Result<Option<ActionServerCompletion>> {
            unreachable!()
        }

        fn send_completed_cancel_requests(&mut self) -> Result<()> {
            unreachable!()
        }

        fn take_pending_cancel_delivery(
            &mut self, _server: Arc<Mutex<dyn ActionServer_>>,
        ) -> Option<ActionServerCompletion> {
            None
        }

        fn finish_cancel_delivery(&mut self, _pending: PendingCancelRequest) {}

        fn handle_cancel_request(
            &mut self, _server: Arc<Mutex<dyn ActionServer_>>,
        ) -> Result<Option<ActionServerCompletion>> {
            unreachable!()
        }

        fn handle_result_request(&mut self) -> Result<()> {
            unreachable!()
        }

        fn handle_goal_expired(
            &mut self, _completions: &mut Vec<ActionServerCompletion>,
        ) -> Result<()> {
            unreachable!()
        }

        fn action_name(&self) -> Result<String> {
            unreachable!()
        }

        fn publish_status(&self) -> Result<()> {
            self.calls.lock().push("status");
            if self.publish_status_error {
                Err(Error::RCL_RET_ERROR)
            } else {
                Ok(())
            }
        }

        fn set_goal_state(
            &mut self, _uuid: &uuid::Uuid, _new_state: rcl_action_goal_event_t,
        ) -> Result<()> {
            self.calls.lock().push("state");
            if self.state_error {
                Err(Error::RCL_RET_ACTION_GOAL_EVENT_INVALID)
            } else {
                Ok(())
            }
        }

        fn notify_goal_done(&self) -> Result<()> {
            self.calls.lock().push("notify");
            if self.notify_error {
                Err(Error::RCL_RET_TIMEOUT)
            } else {
                Ok(())
            }
        }

        fn add_result(&mut self, _uuid: uuid::Uuid, _msg: Box<dyn VoidPtr>) -> Result<()> {
            self.calls.lock().push("result");
            if self.add_result_error {
                Err(Error::RCL_RET_BAD_ALLOC)
            } else {
                Ok(())
            }
        }

        fn cancel_goal(&mut self, _uuid: &uuid::Uuid) -> Result<()> {
            unreachable!()
        }

        fn execute_goal(&mut self, _uuid: &uuid::Uuid) -> Result<()> {
            unreachable!()
        }

        fn get_goal_status(&self, _uuid: &uuid::Uuid) -> Result<GoalStatus> {
            unreachable!()
        }

        fn is_cancelling(&self, _uuid: &uuid::Uuid) -> Result<bool> {
            unreachable!()
        }

        fn add_goal_handle(
            &mut self, _uuid: uuid::Uuid, _goal_handle: *mut rcl_action_goal_handle_t,
        ) {
            unreachable!()
        }

        fn forget_cancel_sender(
            &mut self, _uuid: &uuid::Uuid, _completions: &mut Vec<ActionServerCompletion>,
        ) {
            unreachable!()
        }

        fn should_remove(&self) -> bool {
            false
        }

        fn destroy(&mut self, _node: &mut rcl_node_t) {}
    }

    fn goal_info(uuid: uuid::Uuid) -> action_msgs::msg::GoalInfo {
        action_msgs::msg::GoalInfo {
            goal_id: unique_identifier_msgs::msg::UUID {
                uuid: uuid.as_bytes().to_vec(),
            },
            ..Default::default()
        }
    }

    #[test]
    fn retains_all_accepted_cancellations() {
        let accepted_a = uuid::Uuid::from_u128(1);
        let rejected = uuid::Uuid::from_u128(2);
        let accepted_b = uuid::Uuid::from_u128(3);
        let mut response = action_msgs::srv::CancelGoal::Response {
            goals_canceling: vec![
                goal_info(accepted_a),
                goal_info(rejected),
                goal_info(accepted_b),
            ],
            ..Default::default()
        };

        let canceled = retain_accepted_cancellations(
            &mut response,
            [(accepted_a, true), (rejected, false), (accepted_b, true)],
        );

        assert_eq!(canceled.unwrap(), vec![accepted_a, accepted_b]);
        assert_eq!(response.goals_canceling.len(), 2);
        assert_eq!(uuid_msg_to_uuid(&response.goals_canceling[0].goal_id).unwrap(), accepted_a);
        assert_eq!(uuid_msg_to_uuid(&response.goals_canceling[1].goal_id).unwrap(), accepted_b);
    }

    #[test]
    fn malformed_cancel_uuid_does_not_partially_filter_response() {
        let accepted = uuid::Uuid::from_u128(1);
        let mut response = action_msgs::srv::CancelGoal::Response {
            goals_canceling: vec![
                goal_info(accepted),
                action_msgs::msg::GoalInfo {
                    goal_id: unique_identifier_msgs::msg::UUID { uuid: vec![0; 15] },
                    ..Default::default()
                },
            ],
            ..Default::default()
        };

        assert!(matches!(
            retain_accepted_cancellations(&mut response, [(accepted, true)]),
            Err(Error::InvalidActionUuidLength { actual: 15 })
        ));
        assert_eq!(response.goals_canceling.len(), 2);
    }

    #[test]
    fn classifies_action_server_take_results() {
        assert!(action_server_take_succeeded(RCL_RET_OK as i32).unwrap());
        assert!(!action_server_take_succeeded(RCL_RET_ACTION_SERVER_TAKE_FAILED as i32).unwrap());
        assert!(matches!(
            action_server_take_succeeded(RCL_RET_ACTION_SERVER_INVALID as i32),
            Err(Error::RCL_RET_ACTION_SERVER_INVALID)
        ));
    }

    #[test]
    fn reports_action_status_publication_failures() {
        let invalid_server = unsafe { rcl_action_get_zero_initialized_server() };

        assert!(matches!(
            publish_action_status(&invalid_server),
            Err(Error::RCL_RET_ACTION_SERVER_INVALID)
        ));
    }

    #[test]
    fn terminal_goal_completion_attempts_every_post_transition_step() {
        let mut server = CompletionTestServer::new(false, true, true, true);
        let result = complete_goal(
            &mut server,
            uuid::Uuid::nil(),
            rcl_action_goal_event_t::GOAL_EVENT_SUCCEED,
            Box::new(DummyResult),
        );

        assert!(matches!(result, Err(Error::RCL_RET_BAD_ALLOC)));
        assert_eq!(*server.calls.lock(), ["state", "result", "status", "notify"]);
    }

    #[test]
    fn terminal_goal_completion_stops_after_failed_transition() {
        let mut server = CompletionTestServer::new(true, false, false, false);
        let result = complete_goal(
            &mut server,
            uuid::Uuid::nil(),
            rcl_action_goal_event_t::GOAL_EVENT_SUCCEED,
            Box::new(DummyResult),
        );

        assert!(matches!(result, Err(Error::RCL_RET_ACTION_GOAL_EVENT_INVALID)));
        assert_eq!(*server.calls.lock(), ["state"]);
    }

    #[test]
    fn action_server_results_are_prepared_before_locking_the_server() {
        let server = Mutex::new(());

        let prepared = with_prepared_action_server(
            &server,
            || {
                assert!(server.try_lock().is_some());
                Ok(41)
            },
            |_server, value| Ok(value + 1),
        )
        .unwrap();

        assert_eq!(prepared, 42);
    }

    struct LockingWake {
        server: Arc<Mutex<()>>,
        woke: AtomicBool,
    }

    impl ArcWake for LockingWake {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            let _server = arc_self.server.lock();
            arc_self.woke.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn action_server_requests_wake_streams_after_releasing_the_server_lock() {
        let server = Arc::new(Mutex::new(()));
        let (sender, mut receiver) = mpsc::channel(1);
        let sender = Arc::new(Mutex::new(Some(sender)));
        let wake = Arc::new(LockingWake {
            server: Arc::clone(&server),
            woke: AtomicBool::new(false),
        });
        let waker = waker_ref(&wake);
        let mut context = std::task::Context::from_waker(&waker);
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Pending));
        assert!(matches!(
            sender.lock().as_mut().unwrap().poll_ready(&mut context),
            Poll::Ready(Ok(()))
        ));

        complete_action_server_operation(&server, |_, completions| {
            let detached = sender.lock().take().unwrap();
            completions.push(request_delivery(Arc::clone(&sender), detached, 42, |_| Ok(())));
            Ok(())
        })
        .unwrap();

        assert!(wake.woke.load(Ordering::SeqCst));
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Ready(Some(42))));
    }

    #[test]
    fn action_server_completions_run_after_a_failed_locked_operation() {
        let server = Mutex::new(());
        let (sender, mut receiver) = mpsc::channel(1);
        let sender = Arc::new(Mutex::new(Some(sender)));

        let result = complete_action_server_operation(&server, |_, completions| {
            let detached = sender.lock().take().unwrap();
            completions.push(request_delivery(Arc::clone(&sender), detached, 42, |_| Ok(())));
            Err(Error::RCL_RET_ERROR)
        });

        assert!(matches!(result, Err(Error::RCL_RET_ERROR)));
        assert!(sender.lock().is_some());
        assert_eq!(receiver.next().now_or_never().flatten(), Some(42));
    }

    #[test]
    fn action_server_request_batches_stop_at_idle_and_fairness_limit() {
        let server = Mutex::new(0usize);
        complete_action_server_request_batch(&server, |calls| {
            *calls += 1;
            if *calls == 3 {
                Ok(None)
            } else {
                Ok(Some(Box::new(|| Ok(()))))
            }
        })
        .unwrap();
        assert_eq!(*server.lock(), 3);

        *server.lock() = 0;
        complete_action_server_request_batch(&server, |calls| {
            *calls += 1;
            Ok(Some(Box::new(|| Ok(()))))
        })
        .unwrap();
        assert_eq!(*server.lock(), MAX_ACTION_SERVER_REQUESTS_PER_SPIN);
    }

    #[test]
    fn cancel_requests_wake_streams_after_releasing_the_server_lock() {
        let server = Arc::new(Mutex::new(()));
        let (sender, mut receiver) = mpsc::channel::<ActionServerCancelRequest>(0);
        let sender_slot = Arc::new(Mutex::new(Some(sender)));
        let wake = Arc::new(LockingWake {
            server: Arc::clone(&server),
            woke: AtomicBool::new(false),
        });
        let waker = waker_ref(&wake);
        let mut context = std::task::Context::from_waker(&waker);
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Pending));

        let uuid = uuid::Uuid::nil();
        let (response_sender, _response_receiver) = oneshot::channel();
        let sender = sender_slot.lock().take().unwrap();
        let pending = PendingCancelRequest {
            request_id: unsafe { std::mem::zeroed() },
            response: action_msgs::srv::CancelGoal::Response::default(),
            deliveries: Vec::new(),
            decisions: Vec::new(),
        };

        let server_for_finish = Arc::clone(&server);
        complete_action_server_operation(&*server, |_, completions| {
            completions.push(cancel_request_delivery(
                vec![DetachedCancelDelivery {
                    sender_slot: Arc::clone(&sender_slot),
                    sender,
                    delivery: PendingCancelDelivery {
                        uuid,
                        request: ActionServerCancelRequest {
                            uuid,
                            response_sender,
                        },
                    },
                }],
                pending,
                move |_| {
                    assert!(server_for_finish.try_lock().is_some());
                    Ok(())
                },
            ));
            Ok(())
        })
        .unwrap();

        assert!(wake.woke.load(Ordering::SeqCst));
        assert!(sender_slot.lock().is_some());
        assert_eq!(receiver.next().now_or_never().flatten().unwrap().uuid, uuid);
    }

    #[test]
    fn cancel_streams_close_after_releasing_the_server_lock() {
        let server = Arc::new(Mutex::new(()));
        let (sender, mut receiver) = mpsc::channel::<ActionServerCancelRequest>(0);
        let sender_slot = Arc::new(Mutex::new(Some(sender)));
        let wake = Arc::new(LockingWake {
            server: Arc::clone(&server),
            woke: AtomicBool::new(false),
        });
        let waker = waker_ref(&wake);
        let mut context = std::task::Context::from_waker(&waker);
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Pending));

        complete_action_server_operation(&*server, |_, completions| {
            completions.push(sender_drop_completion(Arc::clone(&sender_slot)));
            Ok(())
        })
        .unwrap();

        assert!(wake.woke.load(Ordering::SeqCst));
        assert!(matches!(Pin::new(&mut receiver).poll_next(&mut context), Poll::Ready(None)));
    }

    #[test]
    fn full_cancel_channel_preserves_the_pending_request() {
        let uuid = uuid::Uuid::nil();
        let (sender, mut receiver) = mpsc::channel::<ActionServerCancelRequest>(0);
        let sender_slot = Arc::new(Mutex::new(Some(sender)));
        let (first_response_sender, _first_response_receiver) = oneshot::channel();
        let first = PendingCancelRequest {
            request_id: unsafe { std::mem::zeroed() },
            response: action_msgs::srv::CancelGoal::Response::default(),
            deliveries: vec![PendingCancelDelivery {
                uuid,
                request: ActionServerCancelRequest {
                    uuid,
                    response_sender: first_response_sender,
                },
            }],
            decisions: Vec::new(),
        };
        let first_sender = sender_slot.lock().take().unwrap();
        let completed = Arc::new(Mutex::new(None));
        let completed_for_delivery = Arc::clone(&completed);
        cancel_request_delivery(
            vec![DetachedCancelDelivery {
                sender_slot: Arc::clone(&sender_slot),
                sender: first_sender,
                delivery: first.deliveries.into_iter().next().unwrap(),
            }],
            PendingCancelRequest {
                deliveries: Vec::new(),
                ..first
            },
            move |pending| {
                *completed_for_delivery.lock() = Some(pending);
                Ok(())
            },
        )()
        .unwrap();
        assert!(completed.lock().take().unwrap().deliveries.is_empty());

        let (second_response_sender, second_response_receiver) = oneshot::channel();
        let second = PendingCancelRequest {
            request_id: unsafe { std::mem::zeroed() },
            response: action_msgs::srv::CancelGoal::Response::default(),
            deliveries: Vec::new(),
            decisions: vec![second_response_receiver],
        };
        let second_delivery = PendingCancelDelivery {
            uuid,
            request: ActionServerCancelRequest {
                uuid,
                response_sender: second_response_sender,
            },
        };
        let second_sender = sender_slot.lock().take().unwrap();
        let completed = Arc::new(Mutex::new(None));
        let completed_for_delivery = Arc::clone(&completed);
        cancel_request_delivery(
            vec![DetachedCancelDelivery {
                sender_slot: Arc::clone(&sender_slot),
                sender: second_sender,
                delivery: second_delivery,
            }],
            second,
            move |pending| {
                *completed_for_delivery.lock() = Some(pending);
                Ok(())
            },
        )()
        .unwrap();
        let mut pending = completed.lock().take().unwrap();
        assert_eq!(pending.deliveries.len(), 1);
        assert_eq!(pending.decisions.len(), 1);

        assert_eq!(receiver.next().now_or_never().flatten().unwrap().uuid, uuid);
        let second_sender = sender_slot.lock().take().unwrap();
        let second_delivery = pending.deliveries.pop().unwrap();
        let completed = Arc::new(Mutex::new(None));
        let completed_for_delivery = Arc::clone(&completed);
        cancel_request_delivery(
            vec![DetachedCancelDelivery {
                sender_slot,
                sender: second_sender,
                delivery: second_delivery,
            }],
            pending,
            move |pending| {
                *completed_for_delivery.lock() = Some(pending);
                Ok(())
            },
        )()
        .unwrap();
        let pending = completed.lock().take().unwrap();
        assert!(pending.deliveries.is_empty());
        assert_eq!(pending.decisions.len(), 1);
        assert_eq!(receiver.next().now_or_never().flatten().unwrap().uuid, uuid);
    }

    #[test]
    fn owned_goal_handle_finalizes_its_internal_allocation() {
        let mut native = unsafe { rcl_action_get_zero_initialized_goal_handle() };
        let info = unsafe { rcl_action_get_zero_initialized_goal_info() };
        let ret = unsafe {
            rcl_action_goal_handle_init(&mut native, &info, rcutils_get_default_allocator())
        };
        assert_eq!(ret, RCL_RET_OK as i32);

        let mut owned = unsafe { OwnedGoalHandle::copy_from(&native) };
        assert!(unsafe { rcl_action_goal_handle_is_valid(owned.as_ptr()) });
        assert_eq!(owned.fini(), RCL_RET_OK as i32);
        assert!(owned.0.is_none());
        assert_eq!(owned.fini(), RCL_RET_OK as i32);
    }

    #[test]
    fn converts_action_timestamps_without_losing_negative_fraction() {
        assert_eq!(
            time_point_to_builtin(-1).unwrap(),
            builtin_interfaces::msg::Time {
                sec: -1,
                nanosec: 999_999_999,
            }
        );
        assert_eq!(
            time_point_to_builtin(1_500_000_001).unwrap(),
            builtin_interfaces::msg::Time {
                sec: 1,
                nanosec: 500_000_001,
            }
        );
        assert!(time_point_to_builtin((i32::MAX as i64 + 1) * 1_000_000_000).is_err());
    }

    #[test]
    fn accepted_goal_can_defer_execution() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "deferred_action_test", "").unwrap();
        let mut requests = node
            .create_action_server::<Fibonacci::Action>("/deferred_action")
            .unwrap();
        let client = node
            .create_action_client::<Fibonacci::Action>("/deferred_action")
            .unwrap();
        let mut pool = LocalPool::new();
        let (goal_sender, goal_receiver) = oneshot::channel();
        pool.spawner()
            .spawn_local(async move {
                let request = requests.next().await.unwrap();
                let (goal, _cancel_requests) = request.accept_deferred().unwrap();
                goal_sender.send(goal).ok();
            })
            .unwrap();

        let response = client
            .send_goal_request(Fibonacci::Goal { order: 1 })
            .unwrap();
        let mut response = Box::pin(response);
        let mut goal_receiver = Box::pin(goal_receiver);
        let server_goal = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            pool.run_until_stalled();
            if let Some(server_goal) = goal_receiver.as_mut().now_or_never() {
                break server_goal.unwrap();
            }
        };
        let (client_goal, result_future) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            pool.run_until_stalled();
            if let Some(client_goal) = response.as_mut().now_or_never() {
                let (goal, result, _feedback) = client_goal.unwrap();
                break (goal, result);
            }
        };

        assert_eq!(server_goal.get_status().unwrap(), GoalStatus::Accepted);
        assert_eq!(client_goal.get_status().unwrap(), GoalStatus::Accepted);
        server_goal.execute().unwrap();
        assert_eq!(server_goal.get_status().unwrap(), GoalStatus::Executing);

        let mut server_goal = server_goal;
        server_goal
            .succeed(Fibonacci::Result { sequence: vec![1] })
            .unwrap();
        let mut result_future = Box::pin(result_future);
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(20)).unwrap();
            pool.run_until_stalled();
            if let Some(result) = result_future.as_mut().now_or_never() {
                assert_eq!(result.unwrap().0, GoalStatus::Succeeded);
                return;
            }
        }
        panic!("client did not receive the deferred goal result");
    }

    #[test]
    fn dropping_last_server_goal_cancels_abandoned_goal() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "abandoned_action_test", "").unwrap();
        let mut requests = node
            .create_action_server::<Fibonacci::Action>("/abandoned_action")
            .unwrap();
        let client = node
            .create_action_client::<Fibonacci::Action>("/abandoned_action")
            .unwrap();
        let response = client
            .send_goal_request(Fibonacci::Goal { order: 1 })
            .unwrap();
        let mut response = Box::pin(response);
        let mut server_goal = None;
        let mut cancel_requests = None;
        let (client_goal, result) = loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if server_goal.is_none() {
                if let Some(request) = requests.next().now_or_never().flatten() {
                    let (goal, cancels) = request.accept().unwrap();
                    server_goal = Some(goal);
                    cancel_requests = Some(cancels);
                }
            }
            if let Some(response) = response.as_mut().now_or_never() {
                let (goal, result, _feedback) = response.unwrap();
                break (goal, result);
            }
        };

        let goal = server_goal.unwrap();
        let last_goal = goal.clone();
        drop(goal);
        assert_eq!(last_goal.get_status().unwrap(), GoalStatus::Executing);
        drop(last_goal);

        let mut result = Box::pin(result);
        for _ in 0..100 {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(result) = result.as_mut().now_or_never() {
                let (status, result) = result.unwrap();
                assert_eq!(status, GoalStatus::Canceled);
                assert_eq!(result, Fibonacci::Result::default());
                assert_eq!(client_goal.get_status().unwrap(), GoalStatus::Canceled);
                drop(cancel_requests);
                return;
            }
        }
        panic!("client did not receive the abandoned goal result");
    }
}
