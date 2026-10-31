use super::*;
use crate::{example_interfaces::action::Fibonacci, GoalStatus};
use futures::StreamExt;
use std::sync::mpsc;

#[test]
fn typed_action_lifecycle_runs_as_mutually_exclusive_group_tasks() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "group_action_tasks", "").unwrap();
    let name = format!("/group_action_tasks_{}", uuid::Uuid::new_v4().simple());
    let mut requests = node
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = node
        .create_action_client::<Fibonacci::Action>(&name)
        .unwrap();
    let mut group = CallbackGroup::new();
    let group_id = group.id();
    let (outcome, received) = mpsc::sync_channel(1);

    let server_task = group.spawn_task(async move {
        let request = requests
            .next()
            .await
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        if request.goal.order != 8 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let (mut goal, _cancels) = request.accept_deferred()?;
        goal.execute()?;
        goal.publish_feedback(Fibonacci::Feedback {
            sequence: vec![1, 1, 2, 3, 5, 8],
        })?;
        goal.succeed(Fibonacci::Result {
            sequence: vec![1, 1, 2, 3, 5, 8, 13],
        })?;
        Ok(())
    });
    let client_task = group.spawn_task(async move {
        let (_goal, result, mut feedback) = client
            .send_goal_request(Fibonacci::Goal { order: 8 })?
            .await?;
        let feedback = feedback
            .next()
            .await
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let result = result.await?;
        outcome
            .send((feedback.sequence, result))
            .map_err(|_| Error::RCL_RET_ERROR)?;
        Ok(())
    });

    let mut executor = CallbackExecutor::new(context, 2).unwrap();
    executor.add_node(node).unwrap();
    executor.add_group(group).unwrap();
    let task = executor.spawn().unwrap();
    let (feedback, (status, result)) = received.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(feedback, [1, 1, 2, 3, 5, 8]);
    assert_eq!(status, GoalStatus::Succeeded);
    assert_eq!(result.sequence, [1, 1, 2, 3, 5, 8, 13]);

    let mut executor = futures::executor::block_on(task.stop()).unwrap();
    assert!(server_task.is_finished());
    assert!(client_task.is_finished());
    assert!(executor.group_mut(group_id).unwrap().is_empty());
}

#[test]
fn stopped_executor_retains_pending_group_task_for_reset_and_restart() {
    let context = Context::create().unwrap();
    let mut group = CallbackGroup::new();
    let group_id = group.id();
    let (release, released) = futures::channel::oneshot::channel();
    let (outcome, received) = mpsc::sync_channel(1);
    let registration = group.spawn_task(async move {
        released.await.map_err(|_| Error::RCL_RET_ERROR)?;
        outcome.send(()).map_err(|_| Error::RCL_RET_ERROR)?;
        Ok(())
    });
    let mut executor = CallbackExecutor::new(context, 1).unwrap();
    executor.add_group(group).unwrap();

    let task = executor.spawn().unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let mut executor = futures::executor::block_on(task.stop()).unwrap();
    assert!(!registration.is_finished());
    assert!(!executor.group_mut(group_id).unwrap().is_empty());

    executor.reset_cancel();
    release.send(()).unwrap();
    let task = executor.spawn().unwrap();
    received.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut executor = futures::executor::block_on(task.stop()).unwrap();
    assert!(registration.is_finished());
    assert!(executor.group_mut(group_id).unwrap().is_empty());
}

#[test]
fn typed_action_lifecycle_runs_in_bounded_reentrant_task_lanes() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "reentrant_action_tasks", "").unwrap();
    let name = format!("/reentrant_action_tasks_{}", uuid::Uuid::new_v4().simple());
    let mut requests = node
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = node
        .create_action_client::<Fibonacci::Action>(&name)
        .unwrap();
    let mut group = ReentrantCallbackGroup::new(2).unwrap();
    let group_id = group.id();
    let (outcome, received) = mpsc::sync_channel(1);

    let server_task = group.spawn_task(async move {
        let request = requests
            .next()
            .await
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?;
        if request.goal.order != 13 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let (mut goal, _cancels) = request.accept_deferred()?;
        goal.execute()?;
        goal.publish_feedback(Fibonacci::Feedback {
            sequence: vec![1, 1, 2, 3, 5, 8, 13],
        })?;
        goal.succeed(Fibonacci::Result {
            sequence: vec![1, 1, 2, 3, 5, 8, 13, 21],
        })?;
        Ok(())
    });
    let client_task = group.spawn_task(async move {
        let (_goal, result, mut feedback) = client
            .send_goal_request(Fibonacci::Goal { order: 13 })?
            .await?;
        let feedback = feedback
            .next()
            .await
            .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
        let result = result.await?;
        outcome
            .send((feedback.sequence, result))
            .map_err(|_| Error::RCL_RET_ERROR)?;
        Ok(())
    });

    let mut executor = CallbackExecutor::new(context, 3).unwrap();
    executor.add_node(node).unwrap();
    executor.add_reentrant_group(group).unwrap();
    let task = executor.spawn().unwrap();
    let (feedback, (status, result)) = received.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(feedback, [1, 1, 2, 3, 5, 8, 13]);
    assert_eq!(status, GoalStatus::Succeeded);
    assert_eq!(result.sequence, [1, 1, 2, 3, 5, 8, 13, 21]);

    let mut executor = futures::executor::block_on(task.stop()).unwrap();
    assert!(server_task.is_finished());
    assert!(client_task.is_finished());
    assert!(executor.reentrant_group_mut(group_id).unwrap().is_empty());
}

#[test]
fn stopped_executor_retains_pending_reentrant_task_for_reset_and_restart() {
    let context = Context::create().unwrap();
    let mut group = ReentrantCallbackGroup::new(2).unwrap();
    let group_id = group.id();
    let (release, released) = futures::channel::oneshot::channel();
    let (outcome, received) = mpsc::sync_channel(1);
    let registration = group.spawn_task(async move {
        released.await.map_err(|_| Error::RCL_RET_ERROR)?;
        outcome.send(()).map_err(|_| Error::RCL_RET_ERROR)?;
        Ok(())
    });
    let mut executor = CallbackExecutor::new(context, 2).unwrap();
    executor.add_reentrant_group(group).unwrap();

    let task = executor.spawn().unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let mut executor = futures::executor::block_on(task.stop()).unwrap();
    assert!(!registration.is_finished());
    assert!(!executor.reentrant_group_mut(group_id).unwrap().is_empty());

    executor.reset_cancel();
    release.send(()).unwrap();
    let task = executor.spawn().unwrap();
    received.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut executor = futures::executor::block_on(task.stop()).unwrap();
    assert!(registration.is_finished());
    assert!(executor.reentrant_group_mut(group_id).unwrap().is_empty());
}
