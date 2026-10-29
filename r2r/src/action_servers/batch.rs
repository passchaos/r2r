//! Atomic status snapshots for explicit groups of action goals.

use super::*;

impl<T: 'static> ActionServerGoalRequest<T>
where
    T: WrappedActionTypeSupport,
{
    fn batch_server(requests: &[Self]) -> Result<Option<Arc<Mutex<dyn ActionServer_>>>> {
        let Some(first) = requests.first() else {
            return Ok(None);
        };
        let server = first
            .response
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let mut identities = HashSet::with_capacity(requests.len());
        for request in requests {
            let current = request
                .response
                .server
                .upgrade()
                .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
            if !Arc::ptr_eq(&server, &current) || !identities.insert(request.uuid) {
                return Err(Error::RCL_RET_INVALID_ARGUMENT);
            }
        }
        Ok(Some(server))
    }

    /// Accept several deferred requests and publish one final accepted-status snapshot.
    ///
    /// Requests must belong to one server and UUIDs must be unique. Clocks and response messages
    /// are prepared before the server lock. If native acceptance fails after a prefix was sent,
    /// the accepted prefix is abandoned through its normal cancel/result cleanup and remaining
    /// requests are rejected; the original error is returned.
    pub fn accept_deferred_batch(
        requests: Vec<Self>,
    ) -> Result<Vec<(ActionServerGoal<T>, ActionServerCancelStream)>> {
        Self::accept_batch_with_execution(requests, false)
    }

    /// Accept and immediately execute several requests with one final status snapshot.
    ///
    /// Feedback must still be published only after applications know that goal observers are
    /// installed. This matches [`Self::accept`] state semantics while avoiding one complete
    /// reliable status array per request.
    pub fn accept_batch(
        requests: Vec<Self>,
    ) -> Result<Vec<(ActionServerGoal<T>, ActionServerCancelStream)>> {
        Self::accept_batch_with_execution(requests, true)
    }

    fn accept_batch_with_execution(
        requests: Vec<Self>, execute: bool,
    ) -> Result<Vec<(ActionServerGoal<T>, ActionServerCancelStream)>> {
        let Some(server) = Self::batch_server(&requests)? else {
            return Ok(Vec::new());
        };
        let mut prepared = VecDeque::with_capacity(requests.len());
        for request in requests {
            let time = {
                let mut clock = request.clock.lock();
                let mut now = 0;
                let ret = unsafe { rcl_clock_get_now(clock.clock_handle.as_mut(), &mut now) };
                if ret != RCL_RET_OK as i32 {
                    return Err(Error::from_rcl_error(ret));
                }
                time_point_to_builtin(now)?
            };
            let goal_info = action_msgs::msg::GoalInfo {
                goal_id: unique_identifier_msgs::msg::UUID {
                    uuid: *request.uuid.as_bytes(),
                },
                stamp: time.clone(),
            };
            let native_goal_info =
                WrappedNativeMsg::<action_msgs::msg::GoalInfo>::try_from(&goal_info)?;
            let response = T::make_goal_response_msg(true, time);
            let response = WrappedNativeMsg::<
                <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response,
            >::try_from(&response)?;
            prepared.push_back((request, native_goal_info, response));
        }

        let mut accepted = Vec::with_capacity(prepared.len());
        let mut failure = None;
        let mut failed_request = None;
        let mut server_guard = server.lock();
        while let Some((mut request, native_goal_info, mut response)) = prepared.pop_front() {
            let goal_handle = unsafe {
                rcl_action_accept_new_goal(server_guard.handle_mut(), &*native_goal_info)
            };
            if goal_handle.is_null() {
                failure = Some((Error::RCL_RET_ACTION_GOAL_HANDLE_INVALID, Vec::new()));
                failed_request = Some(request);
                break;
            }
            server_guard.add_goal_handle(request.uuid, goal_handle);
            if execute {
                let ret = unsafe {
                    rcl_action_update_goal_state(
                        goal_handle,
                        rcl_action_goal_event_t::GOAL_EVENT_EXECUTE,
                    )
                };
                if ret != RCL_RET_OK as i32 {
                    let mut completions = Vec::new();
                    if let Err(err) =
                        abort_goal_after_failed_accept(&mut *server_guard, &request.uuid)
                    {
                        log::error!("could not abort batched goal after execution failure: {err}");
                    }
                    server_guard.forget_cancel_sender(&request.uuid, &mut completions);
                    failure = Some((Error::from_rcl_error(ret), completions));
                    failed_request = Some(request);
                    break;
                }
            }
            request.response.response_sent = true;
            let ret = unsafe {
                rcl_action_send_goal_response(
                    server_guard.handle_mut(),
                    &mut request.response.request_id,
                    response.void_ptr_mut(),
                )
            };
            if ret != RCL_RET_OK as i32 {
                let mut completions = Vec::new();
                if let Err(err) = abort_goal_after_failed_accept(&mut *server_guard, &request.uuid)
                {
                    log::error!("could not abort batched goal after response failure: {err}");
                }
                server_guard.forget_cancel_sender(&request.uuid, &mut completions);
                failure = Some((Error::from_rcl_error(ret), completions));
                failed_request = Some(request);
                break;
            }
            accepted.push((
                ActionServerGoal {
                    uuid: request.uuid,
                    goal: request.goal,
                    lifecycle: Arc::new(ServerGoalLifecycle {
                        server: request.response.server.clone(),
                        uuid: request.uuid,
                        action_type: PhantomData,
                    }),
                },
                request.cancel_requests,
            ));
        }

        if failure.is_none() {
            if let Err(err) = server_guard.publish_status() {
                log::error!("could not publish batched initial action goal status: {err}");
            }
        }
        drop(server_guard);
        if let Some((err, completions)) = failure {
            drop(failed_request);
            drop(prepared);
            drop(accepted);
            return finish_action_server_operation(Err(err), completions).map(|()| unreachable!());
        }
        Ok(accepted)
    }
}

pub(super) fn execute_goal_batch(
    server: &mut dyn ActionServer_, uuids: &[uuid::Uuid],
) -> Result<()> {
    let (changed, transition) =
        server.set_goal_states(uuids, rcl_action_goal_event_t::GOAL_EVENT_EXECUTE);
    let mut first_error = transition.err();
    if changed != 0 {
        if let Err(err) = server.publish_status() {
            first_error.get_or_insert(err);
        }
    }
    first_error.map_or(Ok(()), Err)
}

pub(super) fn complete_goal_batch(
    server: &mut dyn ActionServer_, goals: Vec<(uuid::Uuid, Box<dyn VoidPtr>)>,
    event: rcl_action_goal_event_t,
) -> Result<()> {
    let uuids = goals.iter().map(|(uuid, _)| *uuid).collect::<Vec<_>>();
    let (completed, transition) = server.set_goal_states(&uuids, event);
    let mut first_error = transition.err();
    for (uuid, result) in goals.into_iter().take(completed) {
        if let Err(err) = server.add_result(uuid, result) {
            first_error.get_or_insert(err);
        }
    }
    if completed != 0 {
        if let Err(err) = server.publish_status() {
            first_error.get_or_insert(err);
        }
        if let Err(err) = server.notify_goal_done() {
            first_error.get_or_insert(err);
        }
    }
    first_error.map_or(Ok(()), Err)
}

impl<T: 'static> ActionServerGoal<T>
where
    T: WrappedActionTypeSupport,
{
    fn batch_server(goals: &[Self]) -> Result<Option<Arc<Mutex<dyn ActionServer_>>>> {
        let Some(first) = goals.first() else {
            return Ok(None);
        };
        let server = first
            .lifecycle
            .server
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        let mut identities = HashSet::with_capacity(goals.len());
        for goal in goals {
            let current = goal
                .lifecycle
                .server
                .upgrade()
                .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
            if !Arc::ptr_eq(&server, &current) || !identities.insert(goal.uuid) {
                return Err(Error::RCL_RET_INVALID_ARGUMENT);
            }
        }
        Ok(Some(server))
    }

    /// Start several deferred goals with one final reliable status snapshot.
    ///
    /// All goals must belong to the same server and UUIDs must be unique. Native state changes
    /// happen under one server lock. If a transition fails after an earlier transition, the
    /// successfully changed prefix is still published before the first error is returned.
    pub fn execute_batch(goals: &[Self]) -> Result<()> {
        let Some(server) = Self::batch_server(goals)? else {
            return Ok(());
        };
        let uuids = goals.iter().map(|goal| goal.uuid).collect::<Vec<_>>();
        let result = execute_goal_batch(&mut *server.lock(), &uuids);
        result
    }

    fn complete_batch(
        goals: &mut [Self], results: Vec<T::Result>, status: i8, event: rcl_action_goal_event_t,
    ) -> Result<()> {
        if goals.len() != results.len() {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let Some(server) = Self::batch_server(goals)? else {
            return Ok(());
        };
        let prepared = goals
            .iter()
            .zip(results)
            .map(|(goal, result)| {
                let message = T::make_result_response_msg(status, result);
                WrappedNativeMsg::<
                    <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response,
                >::try_from(&message)
                .map(|native| (goal.uuid, Box::new(native) as Box<dyn VoidPtr>))
            })
            .collect::<Result<Vec<_>>>()?;
        let result = complete_goal_batch(&mut *server.lock(), prepared, event);
        result
    }

    /// Succeed several goals while publishing one final reliable status snapshot.
    ///
    /// Result payloads are fully prepared before the server lock is acquired. Each goal keeps its
    /// normal result response and retention behavior; redundant intermediate status snapshots and
    /// expiration-timer notifications are coalesced. See [`Self::execute_batch`] for validation
    /// and partial-failure behavior.
    pub fn succeed_batch(goals: &mut [Self], results: Vec<T::Result>) -> Result<()> {
        Self::complete_batch(
            goals,
            results,
            action_msgs::msg::GoalStatus::STATUS_SUCCEEDED as i8,
            rcl_action_goal_event_t::GOAL_EVENT_SUCCEED,
        )
    }

    /// Abort several goals while publishing one final reliable status snapshot.
    pub fn abort_batch(goals: &mut [Self], results: Vec<T::Result>) -> Result<()> {
        Self::complete_batch(
            goals,
            results,
            action_msgs::msg::GoalStatus::STATUS_ABORTED as i8,
            rcl_action_goal_event_t::GOAL_EVENT_ABORT,
        )
    }

    /// Complete several canceling goals while publishing one final reliable status snapshot.
    pub fn cancel_batch(goals: &mut [Self], results: Vec<T::Result>) -> Result<()> {
        Self::complete_batch(
            goals,
            results,
            action_msgs::msg::GoalStatus::STATUS_CANCELED as i8,
            rcl_action_goal_event_t::GOAL_EVENT_CANCELED,
        )
    }
}
