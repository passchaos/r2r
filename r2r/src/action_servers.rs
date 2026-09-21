use futures::{
    channel::{mpsc, oneshot},
    future::{join_all, FutureExt, JoinAll},
    stream::Stream,
    task::noop_waker_ref,
};
use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    mem::MaybeUninit,
    sync::{Arc, Mutex, Weak},
    task::{Context, Poll},
};

use crate::{
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
    fn handle_goal_request(&mut self, server: Arc<Mutex<dyn ActionServer_>>);
    fn send_completed_cancel_requests(&mut self);
    fn handle_cancel_request(&mut self);
    fn handle_result_request(&mut self);
    fn handle_goal_expired(&mut self);
    fn publish_status(&self);
    fn set_goal_state(
        &mut self, uuid: &uuid::Uuid, new_state: rcl_action_goal_event_t,
    ) -> Result<()>;
    fn add_result(&mut self, uuid: uuid::Uuid, msg: Box<dyn VoidPtr>);
    fn cancel_goal(&mut self, uuid: &uuid::Uuid);
    fn is_cancelling(&self, uuid: &uuid::Uuid) -> Result<bool>;
    fn add_goal_handle(&mut self, uuid: uuid::Uuid, goal_handle: *mut rcl_action_goal_handle_t);
    fn forget_cancel_sender(&mut self, uuid: &uuid::Uuid);
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
    server: Weak<Mutex<dyn ActionServer_>>,
    request_id: rmw_request_id_t,
}

unsafe impl<T> Send for ActionServerGoalRequest<T> where T: WrappedActionTypeSupport {}

impl<T: 'static> ActionServerGoalRequest<T>
where
    T: WrappedActionTypeSupport,
{
    /// Accept the goal request and become a ServerGoal.
    /// Returns a handle to the goal and a stream on which cancel requests can be received.
    pub fn accept(
        mut self,
    ) -> Result<(ActionServerGoal<T>, impl Stream<Item = ActionServerCancelRequest> + Unpin)> {
        let uuid_msg = unique_identifier_msgs::msg::UUID {
            uuid: self.uuid.as_bytes().to_vec(),
        };
        let server = self
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let mut server = server.lock().unwrap();
        let time = match server.now() {
            Ok(time) => time,
            Err(err) => {
                server.forget_cancel_sender(&self.uuid);
                return Err(err);
            }
        };
        let goal_info = action_msgs::msg::GoalInfo {
            goal_id: uuid_msg,
            stamp: time.clone(),
        };
        let native_goal_info = WrappedNativeMsg::<action_msgs::msg::GoalInfo>::from(&goal_info);

        let goal_handle: *mut rcl_action_goal_handle_t =
            unsafe { rcl_action_accept_new_goal(server.handle_mut(), &*native_goal_info) };
        if goal_handle.is_null() {
            server.forget_cancel_sender(&self.uuid);
            return Err(Error::RCL_RET_ACTION_GOAL_HANDLE_INVALID);
        }

        let ret = unsafe {
            rcl_action_update_goal_state(goal_handle, rcl_action_goal_event_t::GOAL_EVENT_EXECUTE)
        };
        if ret != RCL_RET_OK as i32 {
            unsafe {
                rcl_action_goal_handle_fini(goal_handle);
            }
            server.forget_cancel_sender(&self.uuid);
            return Err(Error::from_rcl_error(ret));
        }

        // send response
        let response_msg = T::make_goal_response_msg(true, time);
        let mut response_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response,
        >::from(&response_msg);

        let ret = unsafe {
            rcl_action_send_goal_response(
                server.handle_mut(),
                &mut self.request_id,
                response_msg.void_ptr_mut(),
            )
        };
        if ret != RCL_RET_OK as i32 {
            unsafe {
                rcl_action_goal_handle_fini(goal_handle);
            }
            server.forget_cancel_sender(&self.uuid);
            return Err(Error::from_rcl_error(ret));
        }

        server.publish_status();

        let g = ActionServerGoal {
            uuid: self.uuid,
            goal: self.goal,
            server: self.server,
        };

        // server.goals.insert(g.uuid.clone(), goal_handle);
        server.add_goal_handle(g.uuid, goal_handle);

        Ok((g, self.cancel_requests))
    }

    /// reject the goal request and be consumed in the process
    pub fn reject(mut self) -> Result<()> {
        let time = builtin_interfaces::msg::Time::default();
        let server = self
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let mut server = server.lock().unwrap();

        let response_msg = T::make_goal_response_msg(false, time);
        let mut response_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response,
        >::from(&response_msg);

        let ret = unsafe {
            rcl_action_send_goal_response(
                server.handle_mut(),
                &mut self.request_id,
                response_msg.void_ptr_mut(),
            )
        };
        server.forget_cancel_sender(&self.uuid);
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        Ok(())
    }
}
pub type ActiveCancelRequest = (
    rmw_request_id_t,
    action_msgs::srv::CancelGoal::Response,
    JoinAll<oneshot::Receiver<(uuid::Uuid, bool)>>,
);

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
    pub active_cancel_requests: Vec<ActiveCancelRequest>,
    pub goals: HashMap<uuid::Uuid, *mut rcl_action_goal_handle_t>,
    pub result_msgs: HashMap<uuid::Uuid, Box<dyn VoidPtr>>,
    pub result_requests: HashMap<uuid::Uuid, Vec<rmw_request_id_t>>,
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
        let mut clock = self.clock.lock().unwrap();
        let mut now = 0;
        let ret = unsafe { rcl_clock_get_now(clock.clock_handle.as_mut(), &mut now) };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        time_point_to_builtin(now)
    }

    fn is_cancelling(&self, uuid: &uuid::Uuid) -> Result<bool> {
        if let Some(handle) = self.goals.get(uuid) {
            let mut state = action_msgs::msg::GoalStatus::STATUS_UNKNOWN as u8;
            let ret = unsafe { rcl_action_goal_handle_get_status(*handle, &mut state) };

            if ret != RCL_RET_OK as i32 {
                log::debug!("action server: Failed to get goal handle state: {}", ret);
                return Err(Error::from_rcl_error(ret));
            }
            return Ok(state == action_msgs::msg::GoalStatus::STATUS_CANCELING as u8);
        }
        Err(Error::RCL_RET_ACTION_GOAL_HANDLE_INVALID)
    }

    fn cancel_goal(&mut self, uuid: &uuid::Uuid) {
        if let Some(handle) = self.goals.get(uuid) {
            let ret = unsafe {
                rcl_action_update_goal_state(
                    *handle,
                    rcl_action_goal_event_t::GOAL_EVENT_CANCEL_GOAL,
                )
            };

            if ret != RCL_RET_OK as i32 {
                log::debug!("action server: could not cancel goal: {}", Error::from_rcl_error(ret));
            }
        }
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
        let goal_info_native = WrappedNativeMsg::<action_msgs::msg::GoalInfo>::from(&goal_info);

        // does this goal exist?
        let goal_exists =
            unsafe { rcl_action_server_goal_exists(self.handle(), &*goal_info_native) };

        if !goal_exists {
            log::debug!("tried to publish result without a goal");
            return Err(Error::RCL_RET_ACTION_GOAL_HANDLE_INVALID);
        }

        if let Some(handle) = self.goals.get(uuid) {
            let ret = unsafe { rcl_action_update_goal_state(*handle, new_state) };
            if ret != RCL_RET_OK as i32 {
                return Err(Error::from_rcl_error(ret));
            }

            let ret = unsafe { rcl_action_notify_goal_done(self.handle()) };
            if ret != RCL_RET_OK as i32 {
                return Err(Error::from_rcl_error(ret));
            }

            // send out updated statues
            self.publish_status();

            Ok(())
        } else {
            Err(Error::RCL_RET_ACTION_GOAL_HANDLE_INVALID)
        }
    }

    fn send_completed_cancel_requests(&mut self) {
        let mut canceled = vec![];
        let mut responses = vec![];
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

        canceled.iter().for_each(|uuid| self.cancel_goal(uuid));
        if !canceled.is_empty() {
            // at least one goal state changed, publish a new status message
            self.publish_status();
        }

        // send out responses
        for (mut request_id, response_msg) in responses {
            // send out response msg.
            let mut native_msg =
                WrappedNativeMsg::<action_msgs::srv::CancelGoal::Response>::from(&response_msg);
            let ret = unsafe {
                rcl_action_send_cancel_response(
                    &self.rcl_handle,
                    &mut request_id,
                    native_msg.void_ptr_mut(),
                )
            };

            if ret != RCL_RET_OK as i32 {
                log::debug!("action server: could send cancel response. {}", ret);
            }
        }
    }

    fn handle_goal_request(&mut self, server: Arc<Mutex<dyn ActionServer_>>) {
        let request_stream_closed = self.goal_request_sender.is_closed();
        if !request_stream_closed {
            let mut cx = Context::from_waker(noop_waker_ref());
            if self.goal_request_sender.poll_ready(&mut cx) == Poll::Pending {
                return;
            }
        }

        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut request_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Request,
        >::new();
        let ret = unsafe {
            rcl_action_take_goal_request(
                &self.rcl_handle,
                request_id.as_mut_ptr(),
                request_msg.void_ptr_mut(),
            )
        };

        if ret != RCL_RET_OK as i32 {
            // this seems normal if client dies.
            return;
        }
        let msg = <<<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Request>::from_native(&request_msg);
        let (uuid_msg, goal) = T::destructure_goal_request_msg(msg);
        let uuid = uuid_msg_to_uuid(&uuid_msg);

        let (cancel_sender, cancel_receiver) = mpsc::channel::<ActionServerCancelRequest>(10);
        self.cancel_senders.insert(uuid, cancel_sender);

        let gr: ActionServerGoalRequest<T> = ActionServerGoalRequest {
            uuid,
            goal,
            cancel_requests: cancel_receiver,
            server: Arc::downgrade(&server),
            request_id: unsafe { request_id.assume_init() },
        };

        // send out request.
        if let Err(e) = self.goal_request_sender.try_send(gr) {
            let mut request = e.into_inner();
            self.cancel_senders.remove(&request.uuid);
            let response =
                T::make_goal_response_msg(false, builtin_interfaces::msg::Time::default());
            let mut response = WrappedNativeMsg::<
                <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response,
            >::from(&response);
            let ret = unsafe {
                rcl_action_send_goal_response(
                    &self.rcl_handle,
                    &mut request.request_id,
                    response.void_ptr_mut(),
                )
            };
            if ret != RCL_RET_OK as i32 {
                log::error!("could not reject goal after request stream closed: {}", ret);
            }
        }
    }

    fn handle_cancel_request(&mut self) {
        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut request_msg = WrappedNativeMsg::<action_msgs::srv::CancelGoal::Request>::new();
        let ret = unsafe {
            rcl_action_take_cancel_request(
                &self.rcl_handle,
                request_id.as_mut_ptr(),
                request_msg.void_ptr_mut(),
            )
        };

        if ret != RCL_RET_OK as i32 {
            // this seems normal if client dies.
            return;
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
            log::debug!("action server: could not process cancel request. {}", ret);
            return;
        }

        let response_msg =
            action_msgs::srv::CancelGoal::Response::from_native(&cancel_response.0.msg);

        let return_channels = response_msg
            .goals_canceling
            .iter()
            .flat_map(|goal_info| {
                let uuid = uuid_msg_to_uuid(&goal_info.goal_id);
                self.cancel_senders
                    .get_mut(&uuid)
                    .and_then(|cancel_sender| {
                        let (s, r) = oneshot::channel::<(uuid::Uuid, bool)>();
                        let cr = ActionServerCancelRequest {
                            uuid,
                            response_sender: s,
                        };
                        match cancel_sender.try_send(cr) {
                            Err(_) => {
                                log::error!("warning: could not send goal cancellation request");
                                None
                            }
                            _ => Some(r),
                        }
                    })
            })
            .collect::<Vec<_>>();

        // because we want to reply to the caller when all goals have been either accepted or rejected,
        // join the channels into one future that we can poll during spin.
        self.active_cancel_requests
            .push((request_id, response_msg, join_all(return_channels)));
    }

    fn handle_goal_expired(&mut self) {
        let mut goal_info = WrappedNativeMsg::<action_msgs::msg::GoalInfo>::new();

        loop {
            let mut num_expired = 0;
            let ret = unsafe {
                rcl_action_expire_goals(&self.rcl_handle, &mut *goal_info, 1, &mut num_expired)
            };
            if ret != RCL_RET_OK as i32 {
                log::debug!("action server: could not expire goal.");
                return;
            }
            if num_expired == 0 {
                return;
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

    fn publish_status(&self) {
        unsafe {
            let mut status = rcl_action_get_zero_initialized_goal_status_array();
            let ret = rcl_action_get_goal_status_array(&self.rcl_handle, &mut status);
            if ret != RCL_RET_OK as i32 {
                log::debug!(
                    "action server: failed to get goal status array: {}",
                    Error::from_rcl_error(ret)
                );
                rcl_action_goal_status_array_fini(&mut status);
                return;
            }
            let ret = rcl_action_publish_status(
                &self.rcl_handle,
                &status as *const _ as *const std::os::raw::c_void,
            );
            if ret != RCL_RET_OK as i32 {
                log::debug!(
                    "action server: failed to publish status: {}",
                    Error::from_rcl_error(ret)
                );
            }
            rcl_action_goal_status_array_fini(&mut status);
        }
    }

    fn add_goal_handle(&mut self, uuid: uuid::Uuid, goal_handle: *mut rcl_action_goal_handle_t) {
        self.goals.insert(uuid, goal_handle);
    }

    fn forget_cancel_sender(&mut self, uuid: &uuid::Uuid) {
        self.cancel_senders.remove(uuid);
    }

    // bit of a hack...
    fn add_result(&mut self, uuid: uuid::Uuid, mut msg: Box<dyn VoidPtr>) {
        // if there are already requests for this goal, send the result immediately.
        if let Some(rr) = self.result_requests.remove(&uuid) {
            for mut req in rr {
                let ret = unsafe {
                    rcl_action_send_result_response(&self.rcl_handle, &mut req, msg.void_ptr_mut())
                };
                if ret != RCL_RET_OK as i32 {
                    log::debug!(
                        "action server: could send result request response. {}",
                        Error::from_rcl_error(ret)
                    );
                }
            }
        }
        self.result_msgs.insert(uuid, msg);
    }

    fn handle_result_request(&mut self) {
        let mut request_id = MaybeUninit::<rmw_request_id_t>::uninit();
        let mut request_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Request,
        >::new();
        let ret = unsafe {
            rcl_action_take_result_request(
                &self.rcl_handle,
                request_id.as_mut_ptr(),
                request_msg.void_ptr_mut(),
            )
        };

        if ret != RCL_RET_OK as i32 {
            // this seems normal if client dies.
            return;
        }

        let msg = <<<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Request>::from_native(&request_msg);
        let goal_info = action_msgs::msg::GoalInfo {
            goal_id: T::destructure_result_request_msg(msg),
            ..action_msgs::msg::GoalInfo::default()
        };
        let goal_info_native = WrappedNativeMsg::<action_msgs::msg::GoalInfo>::from(&goal_info);

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
            >::from(&msg);
            let ret = unsafe {
                rcl_action_send_result_response(
                    &self.rcl_handle,
                    &mut request_id.assume_init(),
                    response_msg.void_ptr_mut(),
                )
            };
            if ret != RCL_RET_OK as i32 {
                log::debug!(
                    "action server: could not send result request response. {}",
                    Error::from_rcl_error(ret)
                );
            }
        } else if let Some(response_msg) = self.result_msgs.get_mut(&uuid) {
            let ret = unsafe {
                rcl_action_send_result_response(
                    &self.rcl_handle,
                    &mut request_id.assume_init(),
                    response_msg.void_ptr_mut(),
                )
            };
            if ret != RCL_RET_OK as i32 {
                log::debug!(
                    "action server: could not send result request response. {}",
                    Error::from_rcl_error(ret)
                );
            }
        } else {
            // keep request for later when result comes in
            self.result_requests
                .entry(uuid)
                .or_default()
                .push(unsafe { request_id.assume_init() });
        }
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_action_server_fini(&mut self.rcl_handle, node);
        }
    }
}

fn time_point_to_builtin(time: rcl_time_point_value_t) -> Result<builtin_interfaces::msg::Time> {
    let seconds = time.div_euclid(1_000_000_000);
    let nanoseconds = time.rem_euclid(1_000_000_000);
    Ok(builtin_interfaces::msg::Time {
        sec: i32::try_from(seconds).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?,
        nanosec: nanoseconds as u32,
    })
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
    pub fn is_cancelling(&self) -> Result<bool> {
        let action_server = self
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;

        let action_server = action_server.lock().unwrap();
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
        let mut native_msg = WrappedNativeMsg::<T::FeedbackMessage>::from(&feedback_msg);
        let ret = unsafe {
            rcl_action_publish_feedback(
                action_server.lock().unwrap().handle(),
                native_msg.void_ptr_mut(),
            )
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
        let mut action_server = action_server.lock().unwrap();

        action_server.set_goal_state(&self.uuid, rcl_action_goal_event_t::GOAL_EVENT_CANCELED)?;

        // create result message
        let result_msg =
            T::make_result_response_msg(action_msgs::msg::GoalStatus::STATUS_CANCELED as i8, msg);
        let native_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response,
        >::from(&result_msg);
        action_server.add_result(self.uuid, Box::new(native_msg));

        Ok(())
    }

    pub fn abort(&mut self, msg: T::Result) -> Result<()> {
        // upgrade to actual ref. if still alive
        let action_server = self
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let mut action_server = action_server.lock().unwrap();

        action_server.set_goal_state(&self.uuid, rcl_action_goal_event_t::GOAL_EVENT_ABORT)?;

        // create result message
        let result_msg =
            T::make_result_response_msg(action_msgs::msg::GoalStatus::STATUS_ABORTED as i8, msg);
        let native_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response,
        >::from(&result_msg);
        action_server.add_result(self.uuid, Box::new(native_msg));

        Ok(())
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
        let mut action_server = action_server.lock().unwrap();

        action_server.set_goal_state(&self.uuid, rcl_action_goal_event_t::GOAL_EVENT_SUCCEED)?;

        // create result message
        let result_msg =
            T::make_result_response_msg(action_msgs::msg::GoalStatus::STATUS_SUCCEEDED as i8, msg);
        let native_msg = WrappedNativeMsg::<
            <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response,
        >::from(&result_msg);
        action_server.add_result(self.uuid, Box::new(native_msg));

        Ok(())
    }
}

pub fn create_action_server_helper(
    node: &mut rcl_node_t, action_name: &str, clock_handle: *mut rcl_clock_t,
    action_ts: *const rosidl_action_type_support_t,
) -> Result<rcl_action_server_t> {
    let mut server_handle = unsafe { rcl_action_get_zero_initialized_server() };
    let action_name_c_string =
        CString::new(action_name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;

    let result = unsafe {
        let server_options = rcl_action_server_get_default_options();

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
}
