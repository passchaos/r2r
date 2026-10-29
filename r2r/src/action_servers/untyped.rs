//! Typed adapters behind the runtime-typed JSON action API.

use super::*;

impl<T> ErasedGoalRequest for ActionServerGoalRequest<T>
where
    T: 'static + WrappedActionTypeSupport,
{
    fn action_name(&self) -> Result<String> {
        ActionServerGoalRequest::action_name(self)
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any + Send> {
        self
    }

    fn accept(
        self: Box<Self>, execute: bool, goal: Result<serde_json::Value>,
    ) -> Result<(ActionServerGoalUntyped, ActionServerCancelStreamUntyped)> {
        let (server_goal, cancel_requests) = (*self).accept_with_execution(execute)?;
        Ok(erase_server_goal(server_goal, goal, cancel_requests))
    }

    fn accept_batch(
        self: Box<Self>, execute: bool, goal: Result<serde_json::Value>,
        rest: Vec<ActionServerGoalRequestUntyped>,
    ) -> Result<Vec<(ActionServerGoalUntyped, ActionServerCancelStreamUntyped)>> {
        let mut requests = Vec::with_capacity(rest.len() + 1);
        let mut goals = Vec::with_capacity(rest.len() + 1);
        requests.push(*self);
        goals.push(goal);
        for request in rest {
            let ActionServerGoalRequestUntyped { goal, inner, .. } = request;
            let request = inner
                .into_any()
                .downcast::<ActionServerGoalRequest<T>>()
                .map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
            requests.push(*request);
            goals.push(goal);
        }
        let accepted = if execute {
            ActionServerGoalRequest::accept_batch(requests)?
        } else {
            ActionServerGoalRequest::accept_deferred_batch(requests)?
        };
        Ok(accepted
            .into_iter()
            .zip(goals)
            .map(|((server_goal, cancel_requests), goal)| {
                erase_server_goal(server_goal, goal, cancel_requests)
            })
            .collect())
    }

    fn reject(self: Box<Self>) -> Result<()> {
        (*self).reject()
    }
}

fn erase_server_goal<T>(
    server_goal: ActionServerGoal<T>, goal: Result<serde_json::Value>,
    cancel_requests: ActionServerCancelStream,
) -> (ActionServerGoalUntyped, ActionServerCancelStreamUntyped)
where
    T: 'static + WrappedActionTypeSupport,
{
    (
        ActionServerGoalUntyped {
            uuid: server_goal.uuid,
            goal,
            inner: Box::new(server_goal),
        },
        ActionServerCancelStreamUntyped {
            inner: Box::pin(cancel_requests),
        },
    )
}

impl<T> ErasedServerGoal for ActionServerGoal<T>
where
    T: 'static + WrappedActionTypeSupport,
{
    fn clone_box(&self) -> Box<dyn ErasedServerGoal> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
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

    fn execute_batch(&self, rest: &[&dyn ErasedServerGoal]) -> Result<()> {
        let mut goals = Vec::with_capacity(rest.len() + 1);
        goals.push(self);
        for goal in rest {
            goals.push(
                goal.as_any()
                    .downcast_ref::<ActionServerGoal<T>>()
                    .ok_or(Error::RCL_RET_INVALID_ARGUMENT)?,
            );
        }
        ActionServerGoal::execute_batch_refs(&goals)
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

    fn complete_batch(
        &self, rest: &[&dyn ErasedServerGoal], results: Vec<serde_json::Value>,
        terminal: ErasedTerminalState,
    ) -> Result<()> {
        let mut goals = Vec::with_capacity(rest.len() + 1);
        goals.push(self);
        for goal in rest {
            goals.push(
                goal.as_any()
                    .downcast_ref::<ActionServerGoal<T>>()
                    .ok_or(Error::RCL_RET_INVALID_ARGUMENT)?,
            );
        }
        let Some(server) = ActionServerGoal::batch_server_refs(&goals)? else {
            return Ok(());
        };
        let results = results
            .into_iter()
            .map(|result| {
                serde_json::from_value(result).map_err(|err| Error::SerdeError {
                    err: err.to_string(),
                })
            })
            .collect::<Result<Vec<T::Result>>>()?;
        let (status, event) = match terminal {
            ErasedTerminalState::Canceled => (
                action_msgs::msg::GoalStatus::STATUS_CANCELED as i8,
                rcl_action_goal_event_t::GOAL_EVENT_CANCELED,
            ),
            ErasedTerminalState::Aborted => (
                action_msgs::msg::GoalStatus::STATUS_ABORTED as i8,
                rcl_action_goal_event_t::GOAL_EVENT_ABORT,
            ),
            ErasedTerminalState::Succeeded => (
                action_msgs::msg::GoalStatus::STATUS_SUCCEEDED as i8,
                rcl_action_goal_event_t::GOAL_EVENT_SUCCEED,
            ),
        };
        ActionServerGoal::complete_batch_refs_on_server(&server, &goals, results, status, event)
    }
}
