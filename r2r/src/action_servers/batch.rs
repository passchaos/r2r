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
                    feedback_buffer: request.feedback_buffer,
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

fn goal_batch_server<'a>(
    mut goals: impl Iterator<Item = (&'a Weak<Mutex<dyn ActionServer_>>, uuid::Uuid)>,
) -> Result<Option<Arc<Mutex<dyn ActionServer_>>>> {
    let Some((first, first_uuid)) = goals.next() else {
        return Ok(None);
    };
    let server = first
        .upgrade()
        .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
    let mut identities = HashSet::with_capacity(goals.size_hint().0 + 1);
    identities.insert(first_uuid);
    for (owner, uuid) in goals {
        let current = owner
            .upgrade()
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        if !Arc::ptr_eq(&server, &current) || !identities.insert(uuid) {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
    }
    Ok(Some(server))
}

impl<T: 'static> ActionServerGoal<T>
where
    T: WrappedActionTypeSupport,
{
    fn batch_server(goals: &[Self]) -> Result<Option<Arc<Mutex<dyn ActionServer_>>>> {
        goal_batch_server(goals.iter().map(|goal| (&goal.lifecycle.server, goal.uuid)))
    }

    pub(super) fn batch_server_refs(
        goals: &[&Self],
    ) -> Result<Option<Arc<Mutex<dyn ActionServer_>>>> {
        goal_batch_server(goals.iter().map(|goal| (&goal.lifecycle.server, goal.uuid)))
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

    pub(super) fn execute_batch_refs(goals: &[&Self]) -> Result<()> {
        let Some(server) = Self::batch_server_refs(goals)? else {
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

    pub(super) fn complete_batch_refs_on_server(
        server: &Arc<Mutex<dyn ActionServer_>>, goals: &[&Self], results: Vec<T::Result>,
        status: i8, event: rcl_action_goal_event_t,
    ) -> Result<()> {
        if goals.len() != results.len() {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
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

#[cfg(all(test, r2r__example_interfaces__action__Fibonacci))]
mod tests {
    use super::*;
    use crate::{example_interfaces, Context};
    use futures::FutureExt;
    use std::time::Duration;

    struct ForeignRequest;

    impl ErasedGoalRequest for ForeignRequest {
        fn action_name(&self) -> Result<String> {
            unreachable!()
        }

        fn into_any(self: Box<Self>) -> Box<dyn Any + Send> {
            self
        }

        fn accept(
            self: Box<Self>, _execute: bool, _goal: Result<serde_json::Value>,
        ) -> Result<(ActionServerGoalUntyped, ActionServerCancelStreamUntyped)> {
            unreachable!()
        }

        fn accept_batch(
            self: Box<Self>, _execute: bool, _goal: Result<serde_json::Value>,
            _rest: Vec<ActionServerGoalRequestUntyped>,
        ) -> Result<Vec<(ActionServerGoalUntyped, ActionServerCancelStreamUntyped)>> {
            unreachable!()
        }

        fn reject(self: Box<Self>) -> Result<()> {
            Ok(())
        }
    }

    fn spin_once(node: &mut Node) {
        match node.try_spin_once(Duration::from_millis(10)) {
            Ok(()) | Err(Error::RCL_RET_TIMEOUT) => {}
            Err(error) => panic!("runtime-typed batch spin failed: {error}"),
        }
    }

    fn take_request(
        node: &mut Node, requests: &mut ActionServerUntyped,
    ) -> ActionServerGoalRequestUntyped {
        for _ in 0..100 {
            spin_once(node);
            if let Some(request) = requests.next().now_or_never().flatten() {
                return request;
            }
        }
        panic!("runtime-typed action request did not arrive")
    }

    #[test]
    fn runtime_typed_batches_preserve_json_and_reject_mixed_erased_types() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "runtime_typed_batch_test", "").unwrap();

        let mut example_requests = node
            .create_action_server_untyped(
                "/runtime_typed_batch_example",
                "example_interfaces/action/Fibonacci",
            )
            .unwrap();
        let example_client = node
            .create_action_client::<example_interfaces::action::Fibonacci::Action>(
                "/runtime_typed_batch_example",
            )
            .unwrap();
        let example_response = example_client
            .send_goal_request(example_interfaces::action::Fibonacci::Goal { order: 1 })
            .unwrap();

        let example_request = take_request(&mut node, &mut example_requests);
        let foreign_request = ActionServerGoalRequestUntyped {
            uuid: uuid::Uuid::new_v4(),
            goal: Ok(serde_json::json!({ "order": 2 })),
            inner: Box::new(ForeignRequest),
        };
        assert!(matches!(
            ActionServerGoalRequestUntyped::accept_batch(vec![example_request, foreign_request]),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        drop(example_response);

        assert!(ActionServerGoalRequestUntyped::accept_batch(Vec::new())
            .unwrap()
            .is_empty());
        assert!(ActionServerGoalUntyped::execute_batch(&[]).is_ok());
        let mut no_goals = Vec::<ActionServerGoalUntyped>::new();
        assert!(ActionServerGoalUntyped::succeed_batch(&mut no_goals, Vec::new()).is_ok());
        assert!(matches!(
            ActionServerGoalUntyped::succeed_batch(
                &mut no_goals,
                vec![serde_json::json!({ "sequence": [] })]
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
    }

    #[test]
    fn runtime_typed_batches_preserve_deferred_feedback_results_and_error_ordering() {
        let context = Context::create().unwrap();
        let mut node = Node::create(context, "runtime_typed_batch_flow", "").unwrap();
        let name = format!("/runtime_typed_batch_{}", uuid::Uuid::new_v4().simple());
        let mut requests = node
            .create_action_server_untyped(&name, "example_interfaces/action/Fibonacci")
            .unwrap();
        let client = node
            .create_action_client::<example_interfaces::action::Fibonacci::Action>(&name)
            .unwrap();

        let mut responses = (1..=3)
            .map(|order| {
                Box::pin(
                    client
                        .send_goal_request(example_interfaces::action::Fibonacci::Goal { order })
                        .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let mut incoming = Vec::new();
        while incoming.len() != 3 {
            incoming.push(take_request(&mut node, &mut requests));
        }
        let mut goals = ActionServerGoalRequestUntyped::accept_deferred_batch(incoming)
            .unwrap()
            .into_iter()
            .map(|(goal, _)| goal)
            .collect::<Vec<_>>();
        for (index, goal) in goals.iter().enumerate() {
            assert_eq!(goal.goal.as_ref().unwrap()["order"], index as i32 + 1);
            assert_eq!(goal.get_status().unwrap(), GoalStatus::Accepted);
        }

        let invalid = (0..goals.len())
            .map(|_| serde_json::json!({ "sequence": "invalid" }))
            .collect();
        assert!(matches!(
            ActionServerGoalUntyped::succeed_batch(&mut goals, invalid),
            Err(Error::SerdeError { .. })
        ));
        assert!(goals
            .iter()
            .all(|goal| goal.get_status().unwrap() == GoalStatus::Accepted));
        ActionServerGoalUntyped::execute_batch(&goals).unwrap();
        assert!(goals
            .iter()
            .all(|goal| goal.get_status().unwrap() == GoalStatus::Executing));

        let mut observations = Vec::new();
        for response in &mut responses {
            loop {
                spin_once(&mut node);
                if let Some(response) = response.as_mut().now_or_never() {
                    observations.push(response.unwrap());
                    break;
                }
            }
        }
        for (index, goal) in goals.iter().enumerate() {
            goal.publish_feedback(serde_json::json!({
                "sequence": vec![index as i32 + 1; 128]
            }))
            .unwrap();
        }
        for (index, (_, _, feedback)) in observations.iter_mut().enumerate() {
            loop {
                spin_once(&mut node);
                if let Some(message) = feedback.next().now_or_never().flatten() {
                    assert_eq!(message.sequence, vec![index as i32 + 1; 128]);
                    break;
                }
            }
        }
        ActionServerGoalUntyped::succeed_batch(
            &mut goals,
            (1..=3)
                .map(|order| serde_json::json!({ "sequence": [order] }))
                .collect(),
        )
        .unwrap();
        for (index, (_, result, feedback)) in observations.iter_mut().enumerate() {
            loop {
                spin_once(&mut node);
                if let Some(result) = result.now_or_never() {
                    let (status, result) = result.unwrap();
                    assert_eq!(status, GoalStatus::Succeeded);
                    assert_eq!(result.sequence, [index as i32 + 1]);
                    break;
                }
            }
            for _ in 0..100 {
                spin_once(&mut node);
                if matches!(feedback.next().now_or_never(), Some(None)) {
                    break;
                }
            }
            assert!(matches!(feedback.next().now_or_never(), Some(None)));
        }

        let mut abort_response = Box::pin(
            client
                .send_goal_request(example_interfaces::action::Fibonacci::Goal { order: 4 })
                .unwrap(),
        );
        let abort_request = take_request(&mut node, &mut requests);
        let mut abort_goals = ActionServerGoalRequestUntyped::accept_batch(vec![abort_request])
            .unwrap()
            .into_iter()
            .map(|(goal, _)| goal)
            .collect::<Vec<_>>();
        let (_, abort_result, _) = loop {
            spin_once(&mut node);
            if let Some(response) = abort_response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        ActionServerGoalUntyped::abort_batch(
            &mut abort_goals,
            vec![serde_json::json!({ "sequence": [4] })],
        )
        .unwrap();
        let mut abort_result = Box::pin(abort_result);
        loop {
            spin_once(&mut node);
            if let Some(result) = abort_result.as_mut().now_or_never() {
                let (status, result) = result.unwrap();
                assert_eq!(status, GoalStatus::Aborted);
                assert_eq!(result.sequence, [4]);
                break;
            }
        }

        let mut cancel_response = Box::pin(
            client
                .send_goal_request(example_interfaces::action::Fibonacci::Goal { order: 5 })
                .unwrap(),
        );
        let cancel_request = take_request(&mut node, &mut requests);
        let (mut cancel_goals, mut server_cancels): (Vec<_>, Vec<_>) =
            ActionServerGoalRequestUntyped::accept_batch(vec![cancel_request])
                .unwrap()
                .into_iter()
                .unzip();
        let (client_goal, cancel_result, _) = loop {
            spin_once(&mut node);
            if let Some(response) = cancel_response.as_mut().now_or_never() {
                break response.unwrap();
            }
        };
        let mut cancel_ack = Box::pin(client_goal.cancel().unwrap());
        let server_cancel = loop {
            spin_once(&mut node);
            if let Some(cancel) = server_cancels[0].next().now_or_never().flatten() {
                break cancel;
            }
        };
        server_cancel.accept();
        while !cancel_goals[0].is_cancelling().unwrap() {
            spin_once(&mut node);
        }
        ActionServerGoalUntyped::cancel_batch(
            &mut cancel_goals,
            vec![serde_json::json!({ "sequence": [5] })],
        )
        .unwrap();
        let mut cancel_result = Box::pin(cancel_result);
        let mut acknowledged = None;
        let mut canceled = None;
        for _ in 0..100 {
            spin_once(&mut node);
            acknowledged = acknowledged.or_else(|| cancel_ack.as_mut().now_or_never());
            canceled = canceled.or_else(|| cancel_result.as_mut().now_or_never());
            if acknowledged.is_some() && canceled.is_some() {
                break;
            }
        }
        acknowledged.unwrap().unwrap();
        let (status, result) = canceled.unwrap().unwrap();
        assert_eq!(status, GoalStatus::Canceled);
        assert_eq!(result.sequence, [5]);
    }
}
