use super::*;
use crate::{example_interfaces::action::Fibonacci, GoalStatus};

#[test]
fn ready_fanout_cancel_decisions_continue_before_native_wait() {
    let context = Context::create().unwrap();
    let mut server = Node::create(context.clone(), "fanout_cancel_server", "").unwrap();
    let mut client_node = Node::create(context, "fanout_cancel_client", "").unwrap();
    let name = format!("/fanout_cancel_{}", uuid::Uuid::new_v4().simple());
    let mut requests = server
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = client_node
        .create_action_client::<Fibonacci::Action>(&name)
        .unwrap();
    let cancel = client_node
        .create_client::<crate::action_msgs::srv::CancelGoal::Service>(
            &format!("{name}/_action/cancel_goal"),
            QosProfile::default(),
        )
        .unwrap();
    let mut goals = Vec::new();
    let mut observations = Vec::new();
    for order in 0..40 {
        let response = client.send_goal_request(Fibonacci::Goal { order }).unwrap();
        let request = drive(&mut server, &mut client_node, async {
            requests
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
        })
        .unwrap();
        goals.push(request.accept_deferred().unwrap());
        observations.push(drive(&mut server, &mut client_node, response).unwrap());
    }
    let response = cancel
        .request(&crate::action_msgs::srv::CancelGoal::Request::default())
        .unwrap();
    let mut decisions = Vec::new();
    for (_, stream) in &mut goals {
        decisions.push(
            drive(&mut server, &mut client_node, async {
                stream
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
            })
            .unwrap(),
        );
    }
    for _ in 0..4 {
        server.try_spin_once(Duration::ZERO).unwrap();
        client_node.try_spin_once(Duration::ZERO).unwrap();
    }
    for decision in decisions {
        decision.reject();
    }
    let prepared = server.prepare_spin(Duration::from_millis(250)).unwrap();
    assert_eq!(
        prepared.timeout,
        Duration::ZERO,
        "unexamined cancel decisions entered a long wait"
    );
    let response = drive(&mut server, &mut client_node, response).unwrap();
    assert_eq!(
        response.return_code,
        crate::action_msgs::srv::CancelGoal::Response::ERROR_REJECTED as i8
    );
    assert!(response.goals_canceling.is_empty());
    assert!(goals.iter().all(|(goal, _)| !goal.is_cancelling().unwrap()));
    drop((observations, goals));
}

#[test]
fn same_thread_feedback_capacity_recovery_delivers_without_another_ros_message() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "same_thread_feedback_ready", "").unwrap();
    let name = format!("/same_thread_feedback_{}", uuid::Uuid::new_v4().simple());
    let mut requests = node
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = node
        .create_action_client_with_options::<Fibonacci::Action>(
            &name,
            ActionClientOptions::default().feedback_channel_capacity(0),
        )
        .unwrap();
    let response = client
        .send_goal_request(Fibonacci::Goal { order: 4 })
        .unwrap();
    let ((mut server_goal, _cancel), (_goal, result, mut feedback)) = node
        .spin_until_future_complete_with_options(
            async {
                let accept = async {
                    requests
                        .next()
                        .await
                        .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?
                        .accept_deferred()
                };
                futures::try_join!(accept, response)
            },
            Duration::from_secs(5),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    server_goal.execute().unwrap();
    server_goal
        .publish_feedback(Fibonacci::Feedback { sequence: vec![1] })
        .unwrap();
    server_goal
        .publish_feedback(Fibonacci::Feedback { sequence: vec![2] })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !node.action_clients[0].lock().has_pending_maintenance() {
        assert!(Instant::now() < deadline);
        node.try_spin_once(Duration::from_millis(1)).unwrap();
    }
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(feedback.next().now_or_never().unwrap().unwrap().sequence, [1]);
    let message = node
        .spin_until_future_complete_with_options(
            async {
                feedback
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)
            },
            Duration::from_secs(1),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    assert_eq!(message.sequence, [2]);
    server_goal
        .succeed(Fibonacci::Result {
            sequence: vec![0, 1, 1],
        })
        .unwrap();
    assert_eq!(
        node.spin_until_future_complete_with_options(
            result,
            Duration::from_secs(5),
            FutureExecutionOptions::wake_driven()
        )
        .unwrap()
        .0,
        GoalStatus::Succeeded
    );
}

#[test]
fn cancellation_backlog_does_not_sleep_over_remaining_same_thread_work() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "same_thread_cancel_backlog", "").unwrap();
    let name = format!("/cancel_backlog_{}", uuid::Uuid::new_v4().simple());
    let mut requests = node
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = node
        .create_action_client::<Fibonacci::Action>(&name)
        .unwrap();
    let mut server_goals = Vec::new();
    let mut observed = Vec::new();
    for order in 0..40 {
        let response = client.send_goal_request(Fibonacci::Goal { order }).unwrap();
        let (server_goal, response) = node
            .spin_until_future_complete_with_options(
                async {
                    let accepted = async {
                        requests
                            .next()
                            .await
                            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?
                            .accept_deferred()
                    };
                    futures::try_join!(accepted, response)
                },
                Duration::from_secs(5),
                FutureExecutionOptions::wake_driven(),
            )
            .unwrap();
        server_goals.push(server_goal);
        observed.push(response);
    }
    for _ in 0..20 {
        node.try_spin_once(Duration::ZERO).unwrap();
    }
    let mut handles = Vec::new();
    for (goal, result, feedback) in observed {
        handles.push(goal);
        drop((result, feedback));
    }
    assert!(node.action_clients[0].lock().has_pending_maintenance());
    let preparation = node.prepare_spin(Duration::from_millis(250)).unwrap();
    assert_eq!(preparation.timeout, Duration::ZERO);
    assert!(node.action_clients[0].lock().has_pending_maintenance());
    let started = Instant::now();
    node.try_spin_once(Duration::from_millis(250)).unwrap();
    assert!(started.elapsed() < Duration::from_millis(125));
    for _ in 0..3 {
        node.try_spin_once(Duration::ZERO).unwrap();
    }
    assert!(handles.iter().all(|goal| !goal.is_result_aware().unwrap()));
    assert!(!node.action_clients[0].lock().has_pending_maintenance());
    drop((handles, server_goals));
}

fn drive<T>(
    server: &mut Node, client: &mut Node, future: impl Future<Output = Result<T>>,
) -> Result<T> {
    futures::pin_mut!(future);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(result) = future.as_mut().now_or_never() {
            return result;
        }
        assert!(Instant::now() < deadline, "action test operation timed out");
        server.try_spin_once(Duration::ZERO)?;
        client.try_spin_once(Duration::ZERO)?;
        std::thread::yield_now();
    }
}

#[test]
fn completed_local_cancel_decisions_return_before_native_wait_timeout() {
    let context = Context::create().unwrap();
    let mut server = Node::create(context.clone(), "cancel_progress_server", "").unwrap();
    let mut client_node = Node::create(context, "cancel_progress_client", "").unwrap();
    let name = format!("/cancel_progress_{}", uuid::Uuid::new_v4().simple());
    let mut requests = server
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = client_node
        .create_action_client::<Fibonacci::Action>(&name)
        .unwrap();
    for accept in [true, false] {
        let response = client
            .send_goal_request(Fibonacci::Goal { order: 5 })
            .unwrap();
        let request = drive(&mut server, &mut client_node, async {
            requests
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
        })
        .unwrap();
        let (mut goal, mut cancels) = request.accept().unwrap();
        let (client_goal, result, _feedback) =
            drive(&mut server, &mut client_node, response).unwrap();
        let cancellation = client_goal.cancel().unwrap();
        let cancel_request = drive(&mut server, &mut client_node, async {
            cancels
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
        })
        .unwrap();
        assert_eq!(cancel_request.uuid, goal.uuid);
        // Leave the decision unresolved while draining native readiness, so its local
        // completion is the only new work before the long wait below.
        for _ in 0..4 {
            server.try_spin_once(Duration::ZERO).unwrap();
            client_node.try_spin_once(Duration::ZERO).unwrap();
        }
        assert!(server
            .action_servers_with_pending_cancels
            .contains(&requests.id));
        if accept {
            cancel_request.accept();
        } else {
            cancel_request.reject();
        }
        let started = Instant::now();
        server.try_spin_once(Duration::from_millis(250)).unwrap();
        assert!(
            started.elapsed() < Duration::from_millis(125),
            "completed local cancellation waited for the native timeout"
        );
        assert_eq!(goal.is_cancelling().unwrap(), accept);
        let response = drive(&mut server, &mut client_node, cancellation);
        if accept {
            response.unwrap();
            goal.cancel(Fibonacci::Result {
                sequence: vec![0, 1],
            })
            .unwrap();
        } else {
            assert!(matches!(response, Err(Error::GoalCancelRejected)));
            goal.succeed(Fibonacci::Result {
                sequence: vec![0, 1],
            })
            .unwrap();
        }
        let (status, result) = drive(&mut server, &mut client_node, result).unwrap();
        assert_eq!(
            status,
            if accept {
                GoalStatus::Canceled
            } else {
                GoalStatus::Succeeded
            }
        );
        assert_eq!(result.sequence, vec![0, 1]);
    }
}

#[test]
fn unresolved_cancel_decision_preserves_requested_wait_timeout() {
    let context = Context::create().unwrap();
    let mut server = Node::create(context.clone(), "pending_cancel_server", "").unwrap();
    let mut client_node = Node::create(context, "pending_cancel_client", "").unwrap();
    let name = format!("/pending_cancel_{}", uuid::Uuid::new_v4().simple());
    let mut requests = server
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = client_node
        .create_action_client::<Fibonacci::Action>(&name)
        .unwrap();
    let response = client
        .send_goal_request(Fibonacci::Goal { order: 5 })
        .unwrap();
    let request = drive(&mut server, &mut client_node, async {
        requests
            .next()
            .await
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
    })
    .unwrap();
    let (mut goal, mut cancels) = request.accept().unwrap();
    let (client_goal, result, _feedback) = drive(&mut server, &mut client_node, response).unwrap();
    let cancellation = client_goal.cancel().unwrap();
    let cancel_request = drive(&mut server, &mut client_node, async {
        cancels
            .next()
            .await
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
    })
    .unwrap();
    for _ in 0..4 {
        server.try_spin_once(Duration::ZERO).unwrap();
        client_node.try_spin_once(Duration::ZERO).unwrap();
    }
    let started = Instant::now();
    server.try_spin_once(Duration::from_millis(50)).unwrap();
    assert!(
        started.elapsed() >= Duration::from_millis(25),
        "pending decision caused an unnecessary busy spin"
    );
    assert!(!goal.is_cancelling().unwrap());
    cancel_request.accept();
    server.try_spin_once(Duration::ZERO).unwrap();
    assert!(goal.is_cancelling().unwrap());
    drive(&mut server, &mut client_node, cancellation).unwrap();
    goal.cancel(Fibonacci::Result {
        sequence: vec![0, 1],
    })
    .unwrap();
    drive(&mut server, &mut client_node, result).unwrap();
}

#[test]
fn backpressured_cancel_delivery_preserves_requested_wait_timeout() {
    let context = Context::create().unwrap();
    let mut server = Node::create(context.clone(), "backpressure_cancel_server", "").unwrap();
    let mut client_node = Node::create(context, "backpressure_cancel_client", "").unwrap();
    let name = format!("/backpressure_cancel_{}", uuid::Uuid::new_v4().simple());
    let options = ActionServerOptions::default().cancel_request_channel_capacity(0);
    let mut requests = server
        .create_action_server_with_options::<Fibonacci::Action>(&name, options)
        .unwrap();
    let client = client_node
        .create_action_client::<Fibonacci::Action>(&name)
        .unwrap();
    let response = client
        .send_goal_request(Fibonacci::Goal { order: 5 })
        .unwrap();
    let request = drive(&mut server, &mut client_node, async {
        requests
            .next()
            .await
            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
    })
    .unwrap();
    let (mut goal, mut cancels) = request.accept().unwrap();
    let (client_goal, result, _feedback) = drive(&mut server, &mut client_node, response).unwrap();
    let first = client_goal.cancel().unwrap();
    let second = client_goal.cancel().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        assert!(Instant::now() < deadline, "second cancel delivery did not become backpressured");
        server.try_spin_once(Duration::ZERO).unwrap();
        client_node.try_spin_once(Duration::ZERO).unwrap();
        let index = server.action_server_indexes[&requests.id];
        if server.action_servers[index]
            .lock()
            .has_pending_cancel_delivery()
        {
            break;
        }
        std::thread::yield_now();
    }
    for _ in 0..4 {
        server.try_spin_once(Duration::ZERO).unwrap();
        client_node.try_spin_once(Duration::ZERO).unwrap();
    }
    let started = Instant::now();
    server.try_spin_once(Duration::from_millis(50)).unwrap();
    assert!(
        started.elapsed() >= Duration::from_millis(25),
        "backpressured cancel delivery caused a busy spin"
    );
    assert!(!goal.is_cancelling().unwrap());
    for _ in 0..2 {
        let request = drive(&mut server, &mut client_node, async {
            cancels
                .next()
                .await
                .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
        })
        .unwrap();
        request.reject();
    }
    assert!(matches!(
        drive(&mut server, &mut client_node, first),
        Err(Error::GoalCancelRejected)
    ));
    assert!(matches!(
        drive(&mut server, &mut client_node, second),
        Err(Error::GoalCancelRejected)
    ));
    goal.succeed(Fibonacci::Result {
        sequence: vec![0, 1],
    })
    .unwrap();
    drive(&mut server, &mut client_node, result).unwrap();
}
