use futures::{
    channel::{mpsc, oneshot},
    future::{join_all, FutureExt, JoinAll},
    stream::Stream,
    task::noop_waker_ref,
};
use parking_lot::Mutex;
use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    mem::MaybeUninit,
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

pub trait ActionServer_ {
    fn handle(&self) -> &rcl_action_server_t;
    fn handle_mut(&mut self) -> &mut rcl_action_server_t;
    fn now(&mut self) -> Result<builtin_interfaces::msg::Time>;
    fn reject_goal_request(
        &mut self, uuid: &uuid::Uuid, request_id: &mut rmw_request_id_t,
    ) -> Result<()>;
    fn handle_goal_request(&mut self, server: Arc<Mutex<dyn ActionServer_>>) -> Result<()>;
    fn send_completed_cancel_requests(&mut self) -> Result<()>;
    fn handle_cancel_request(&mut self) -> Result<()>;
    fn handle_result_request(&mut self) -> Result<()>;
    fn handle_goal_expired(&mut self) -> Result<()>;
    fn publish_status(&self) -> Result<()>;
    fn set_goal_state(
        &mut self, uuid: &uuid::Uuid, new_state: rcl_action_goal_event_t,
    ) -> Result<()>;
    fn add_result(&mut self, uuid: uuid::Uuid, msg: Box<dyn VoidPtr>) -> Result<()>;
    fn cancel_goal(&mut self, uuid: &uuid::Uuid) -> Result<()>;
    fn execute_goal(&mut self, uuid: &uuid::Uuid) -> Result<()>;
    fn get_goal_status(&self, uuid: &uuid::Uuid) -> Result<GoalStatus>;
    fn is_cancelling(&self, uuid: &uuid::Uuid) -> Result<bool>;
    fn add_goal_handle(&mut self, uuid: uuid::Uuid, goal_handle: *mut rcl_action_goal_handle_t);
    fn forget_cancel_sender(&mut self, uuid: &uuid::Uuid);
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

/// Request to the action server to accept a new `Goal`.
pub struct ActionServerGoalRequest<T>
where
    T: WrappedActionTypeSupport,
{
    pub uuid: uuid::Uuid,
    pub goal: T::Goal,
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
        let server = self
            .response
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let mut server = server.lock();
        let time = server.now()?;
        let goal_info = action_msgs::msg::GoalInfo {
            goal_id: uuid_msg,
            stamp: time.clone(),
        };
        let native_goal_info =
            WrappedNativeMsg::<action_msgs::msg::GoalInfo>::try_from(&goal_info)?;

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
                if let Err(rollback_err) =
                    server.set_goal_state(&self.uuid, rcl_action_goal_event_t::GOAL_EVENT_ABORT)
                {
                    log::error!(
                        "could not abort goal after execution setup failed: {rollback_err}"
                    );
                }
                server.forget_cancel_sender(&self.uuid);
                return Err(Error::from_rcl_error(ret));
            }
        }

        // send response
        let response_msg = T::make_goal_response_msg(true, time);
        let mut response_msg = match WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response,
        >::try_from(&response_msg)
        {
            Ok(response) => response,
            Err(err) => {
                if let Err(rollback_err) =
                    server.set_goal_state(&self.uuid, rcl_action_goal_event_t::GOAL_EVENT_ABORT)
                {
                    log::error!(
                        "could not abort goal after response allocation failed: {rollback_err}"
                    );
                }
                server.forget_cancel_sender(&self.uuid);
                return Err(err);
            }
        };

        self.response.response_sent = true;
        let ret = unsafe {
            rcl_action_send_goal_response(
                server.handle_mut(),
                &mut self.response.request_id,
                response_msg.void_ptr_mut(),
            )
        };
        if ret != RCL_RET_OK as i32 {
            if let Err(rollback_err) =
                server.set_goal_state(&self.uuid, rcl_action_goal_event_t::GOAL_EVENT_ABORT)
            {
                log::error!(
                    "could not abort goal after sending its response failed: {rollback_err}"
                );
            }
            server.forget_cancel_sender(&self.uuid);
            return Err(Error::from_rcl_error(ret));
        }

        let g = ActionServerGoal {
            uuid: self.uuid,
            goal: self.goal,
            server: self.response.server.clone(),
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
        let mut server = server.lock();
        self.response.response_sent = true;
        server.reject_goal_request(&self.uuid, &mut self.response.request_id)
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
        let mut server = server.lock();

        if let Err(err) = server.reject_goal_request(&self.uuid, &mut self.request_id) {
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

impl PendingCancelDelivery {
    fn try_send(
        self, cancel_senders: &mut HashMap<uuid::Uuid, mpsc::Sender<ActionServerCancelRequest>>,
    ) -> Option<Self> {
        let sender = cancel_senders.get_mut(&self.uuid)?;
        match sender.try_send(self.request) {
            Ok(()) => None,
            Err(err) if err.is_full() => Some(Self {
                uuid: self.uuid,
                request: err.into_inner(),
            }),
            Err(_) => None,
        }
    }
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
) -> Vec<uuid::Uuid> {
    let accepted: HashSet<_> = decisions
        .into_iter()
        .filter_map(|(uuid, accept)| accept.then_some(uuid))
        .collect();

    response
        .goals_canceling
        .retain(|goal_info| accepted.contains(&uuid_msg_to_uuid(&goal_info.goal_id)));

    response
        .goals_canceling
        .iter()
        .map(|goal_info| uuid_msg_to_uuid(&goal_info.goal_id))
        .collect()
}

pub struct WrappedActionServer<T>
where
    T: WrappedActionTypeSupport,
{
    pub rcl_handle: rcl_action_server_t,
    pub clock: Arc<Mutex<Clock>>,
    pub goal_request_sender: mpsc::Sender<ActionServerGoalRequest<T>>,
    pub cancel_senders: HashMap<uuid::Uuid, mpsc::Sender<ActionServerCancelRequest>>,
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
    fn flush_pending_cancel_request(&mut self) -> bool {
        let Some(mut pending) = self.pending_cancel_request.take() else {
            return true;
        };

        pending.deliveries = pending
            .deliveries
            .into_iter()
            .filter_map(|delivery| delivery.try_send(&mut self.cancel_senders))
            .collect();
        if pending.deliveries.is_empty() {
            self.active_cancel_requests.push((
                pending.request_id,
                pending.response,
                join_all(pending.decisions),
            ));
            true
        } else {
            self.pending_cancel_request = Some(pending);
            false
        }
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

    fn now(&mut self) -> Result<builtin_interfaces::msg::Time> {
        let mut clock = self.clock.lock();
        let mut now = 0;
        let ret = unsafe { rcl_clock_get_now(clock.clock_handle.as_mut(), &mut now) };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        time_point_to_builtin(now)
    }

    fn reject_goal_request(
        &mut self, uuid: &uuid::Uuid, request_id: &mut rmw_request_id_t,
    ) -> Result<()> {
        self.forget_cancel_sender(uuid);
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

            let ret = unsafe { rcl_action_notify_goal_done(self.handle()) };
            if ret != RCL_RET_OK as i32 {
                return Err(Error::from_rcl_error(ret));
            }

            Ok(())
        } else {
            Err(Error::RCL_RET_ACTION_GOAL_HANDLE_INVALID)
        }
    }

    fn send_completed_cancel_requests(&mut self) -> Result<()> {
        self.flush_pending_cancel_request();
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
                    canceled.extend(retain_accepted_cancellations(&mut response_msg, decisions));

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

    fn handle_goal_request(&mut self, server: Arc<Mutex<dyn ActionServer_>>) -> Result<()> {
        let request_stream_closed = self.goal_request_sender.is_closed();
        if !request_stream_closed {
            let mut cx = Context::from_waker(noop_waker_ref());
            if self.goal_request_sender.poll_ready(&mut cx) == Poll::Pending {
                return Ok(());
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
            return Ok(());
        }
        let msg = <<<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Request>::from_native(&request_msg);
        let (uuid_msg, goal) = T::destructure_goal_request_msg(msg);
        let uuid = uuid_msg_to_uuid(&uuid_msg);

        let (cancel_sender, cancel_receiver) =
            mpsc::channel::<ActionServerCancelRequest>(self.cancel_request_channel_capacity);
        self.cancel_senders.insert(uuid, cancel_sender);

        let gr: ActionServerGoalRequest<T> = ActionServerGoalRequest {
            uuid,
            goal,
            cancel_requests: cancel_receiver,
            response: GoalRequestResponseGuard {
                server: Arc::downgrade(&server),
                uuid,
                request_id: unsafe { request_id.assume_init() },
                response_sent: false,
            },
        };

        // send out request.
        if let Err(e) = self.goal_request_sender.try_send(gr) {
            // We already hold this server's mutex while polling. Disarm the drop guard and
            // reject directly to avoid trying to lock the same non-reentrant mutex again.
            let mut request = e.into_inner();
            request.response.response_sent = true;
            self.reject_goal_request(&request.uuid, &mut request.response.request_id)?;
        }
        Ok(())
    }

    fn handle_cancel_request(&mut self) -> Result<()> {
        if !self.flush_pending_cancel_request() {
            return Ok(());
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
            return Ok(());
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

        let (deliveries, decisions): (Vec<_>, Vec<_>) = response_msg
            .goals_canceling
            .iter()
            .map(|goal_info| {
                let uuid = uuid_msg_to_uuid(&goal_info.goal_id);
                let (response_sender, response_receiver) = oneshot::channel::<(uuid::Uuid, bool)>();
                (
                    PendingCancelDelivery {
                        uuid,
                        request: ActionServerCancelRequest {
                            uuid,
                            response_sender,
                        },
                    },
                    response_receiver,
                )
            })
            .unzip();

        self.pending_cancel_request = Some(PendingCancelRequest {
            request_id,
            response: response_msg,
            deliveries,
            decisions,
        });
        self.flush_pending_cancel_request();
        Ok(())
    }

    fn handle_goal_expired(&mut self) -> Result<()> {
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
            let uuid = uuid_msg_to_uuid(&gi.goal_id);
            log::debug!("goal expired: {} - {}", uuid, num_expired);
            self.goals.remove(&uuid);
            self.cancel_senders.remove(&uuid);
            self.result_msgs.remove(&uuid);
            self.result_requests.remove(&uuid);
        }
    }

    fn publish_status(&self) -> Result<()> {
        publish_action_status(&self.rcl_handle)
    }

    fn add_goal_handle(&mut self, uuid: uuid::Uuid, goal_handle: *mut rcl_action_goal_handle_t) {
        self.goals
            .insert(uuid, unsafe { OwnedGoalHandle::copy_from(goal_handle) });
    }

    fn forget_cancel_sender(&mut self, uuid: &uuid::Uuid) {
        self.cancel_senders.remove(uuid);
    }

    fn should_remove(&self) -> bool {
        self.goal_request_sender.is_closed()
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
        let goal_info_native =
            WrappedNativeMsg::<action_msgs::msg::GoalInfo>::try_from(&goal_info)?;

        // does this goal exist?
        let goal_exists =
            unsafe { rcl_action_server_goal_exists(&self.rcl_handle, &*goal_info_native) };

        let uuid = uuid_msg_to_uuid(&goal_info.goal_id);

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
    first_error.map_or(Ok(()), Err)
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
    server: Weak<Mutex<dyn ActionServer_>>,
}

unsafe impl<T> Send for ActionServerGoal<T> where T: WrappedActionTypeSupport {}

impl<T: 'static> ActionServerGoal<T>
where
    T: WrappedActionTypeSupport,
{
    /// Return the server-side state of this goal.
    pub fn get_status(&self) -> Result<GoalStatus> {
        let action_server = self
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let action_server = action_server.lock();
        action_server.get_goal_status(&self.uuid)
    }

    /// Start executing a goal accepted with [`ActionServerGoalRequest::accept_deferred`].
    pub fn execute(&self) -> Result<()> {
        let action_server = self
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let result = action_server.lock().execute_goal(&self.uuid);
        result
    }

    pub fn is_cancelling(&self) -> Result<bool> {
        let action_server = self
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
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let mut action_server = action_server.lock();

        // create result message
        let result_msg =
            T::make_result_response_msg(action_msgs::msg::GoalStatus::STATUS_CANCELED as i8, msg);
        let native_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response,
        >::try_from(&result_msg)?;
        complete_goal(
            &mut *action_server,
            self.uuid,
            rcl_action_goal_event_t::GOAL_EVENT_CANCELED,
            Box::new(native_msg),
        )
    }

    pub fn abort(&mut self, msg: T::Result) -> Result<()> {
        // upgrade to actual ref. if still alive
        let action_server = self
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let mut action_server = action_server.lock();

        // create result message
        let result_msg =
            T::make_result_response_msg(action_msgs::msg::GoalStatus::STATUS_ABORTED as i8, msg);
        let native_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response,
        >::try_from(&result_msg)?;
        complete_goal(
            &mut *action_server,
            self.uuid,
            rcl_action_goal_event_t::GOAL_EVENT_ABORT,
            Box::new(native_msg),
        )
    }

    pub fn succeed(&mut self, msg: T::Result) -> Result<()>
    where
        T: WrappedActionTypeSupport,
    {
        // upgrade to actual ref. if still alive
        let action_server = self
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let mut action_server = action_server.lock();

        // create result message
        let result_msg =
            T::make_result_response_msg(action_msgs::msg::GoalStatus::STATUS_SUCCEEDED as i8, msg);
        let native_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response,
        >::try_from(&result_msg)?;
        complete_goal(
            &mut *action_server,
            self.uuid,
            rcl_action_goal_event_t::GOAL_EVENT_SUCCEED,
            Box::new(native_msg),
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
    use futures::{executor::LocalPool, task::LocalSpawnExt, StreamExt};
    use std::time::Duration;

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

        assert_eq!(canceled, vec![accepted_a, accepted_b]);
        assert_eq!(response.goals_canceling.len(), 2);
        assert_eq!(uuid_msg_to_uuid(&response.goals_canceling[0].goal_id), accepted_a);
        assert_eq!(uuid_msg_to_uuid(&response.goals_canceling[1].goal_id), accepted_b);
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
    fn full_cancel_channel_preserves_the_pending_request() {
        let uuid = uuid::Uuid::nil();
        let (sender, mut receiver) = mpsc::channel::<ActionServerCancelRequest>(0);
        let mut cancel_senders = HashMap::from([(uuid, sender)]);
        let (first_response_sender, _first_response_receiver) = oneshot::channel();
        let first = PendingCancelDelivery {
            uuid,
            request: ActionServerCancelRequest {
                uuid,
                response_sender: first_response_sender,
            },
        };
        assert!(first.try_send(&mut cancel_senders).is_none());

        let (second_response_sender, _second_response_receiver) = oneshot::channel();
        let second = PendingCancelDelivery {
            uuid,
            request: ActionServerCancelRequest {
                uuid,
                response_sender: second_response_sender,
            },
        };
        let pending = second.try_send(&mut cancel_senders).unwrap();

        assert_eq!(receiver.next().now_or_never().flatten().unwrap().uuid, uuid);
        assert!(pending.try_send(&mut cancel_senders).is_none());
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
}
