use super::*;
use crate::{example_interfaces::action::Fibonacci, GoalStatus};

#[test]
#[ignore = "manual native preparation-stage benchmark"]
fn benchmark_native_parked_server_prepare() {
    use crate::action_servers::take_action_server_maintenance_poll_count;
    let servers: usize = std::env::var("R2R_PREPARE_SERVERS")
        .unwrap_or_else(|_| "32".into())
        .parse()
        .unwrap();
    let iterations: usize = std::env::var("R2R_PREPARE_ITERATIONS")
        .unwrap_or_else(|_| "20000".into())
        .parse()
        .unwrap();
    let warmup: usize = std::env::var("R2R_PREPARE_WARMUP")
        .unwrap_or_else(|_| "1000".into())
        .parse()
        .unwrap();
    assert!(iterations > 0);
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "prepare_stage_benchmark", "").unwrap();
    let prefix = uuid::Uuid::new_v4().simple().to_string();
    let mut keepalive: Vec<Box<dyn std::any::Any>> = Vec::new();
    for i in 0..servers {
        let name = format!("/prepare_stage_{prefix}_{i}");
        let mut requests = node
            .create_action_server::<Fibonacci::Action>(&name)
            .unwrap();
        let client = node
            .create_action_client::<Fibonacci::Action>(&name)
            .unwrap();
        let response = client
            .send_goal_request(Fibonacci::Goal { order: i as i32 })
            .unwrap();
        let (accepted, observed) = node
            .spin_until_future_complete_with_options(
                async {
                    let accept = async {
                        requests
                            .next()
                            .await
                            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?
                            .accept()
                    };
                    futures::try_join!(accept, response)
                },
                Duration::from_secs(5),
                FutureExecutionOptions::wake_driven(),
            )
            .unwrap();
        let (goal, mut cancels) = accepted;
        let response = client.cancel_all().unwrap();
        let decision = node
            .spin_until_future_complete_with_options(
                async {
                    cancels
                        .next()
                        .await
                        .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
                },
                Duration::from_secs(5),
                FutureExecutionOptions::wake_driven(),
            )
            .unwrap();
        keepalive.push(Box::new((requests, client, goal, cancels, observed, response, decision)));
    }
    for _ in 0..4 {
        node.try_spin_once(Duration::ZERO).unwrap();
    }
    assert_eq!(node.action_servers.len(), servers);
    assert!(node
        .action_servers
        .iter()
        .all(|server| server.lock().has_pending_cancel_work()));
    for _ in 0..warmup {
        node.prepare_spin(Duration::ZERO).unwrap();
    }
    take_action_server_maintenance_poll_count();
    let mut samples = Vec::with_capacity(iterations);
    let started = Instant::now();
    for _ in 0..iterations {
        let start = Instant::now();
        node.prepare_spin(Duration::ZERO).unwrap();
        samples.push(start.elapsed().as_nanos() as u64);
    }
    let elapsed = started.elapsed().as_nanos() as u64;
    let polls = take_action_server_maintenance_poll_count();
    let mut sorted = samples.clone();
    sorted.sort_unstable();
    let percentile = |p: usize| sorted[(iterations * p).div_ceil(100).saturating_sub(1)];
    let process_status = std::fs::read_to_string("/proc/self/status").unwrap();
    let affinity = process_status
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:"))
        .unwrap()
        .trim();
    println!(
        "R2R_PREPARE_STAGE {}",
        serde_json::json!({
            "rmw_identifier": unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }.to_str().unwrap(),
            "cpu_affinity": affinity,
            "servers": servers, "iterations": iterations, "warmup": warmup,
            "p50_ns": percentile(50), "p95_ns": percentile(95), "p99_ns": percentile(99),
            "loop_ns": elapsed, "maintenance_polls": polls, "samples_ns": samples,
            "boundary": "Node::prepare_spin only; optimized lib test profile with TLS poll instrumentation"
        })
    );
}

#[test]
fn external_cancel_decision_interrupts_existing_foreground_guard_wait() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "external_cancel_decision", "").unwrap();
    let name = format!("/external_cancel_{}", uuid::Uuid::new_v4().simple());
    let mut requests = node
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = node
        .create_action_client::<Fibonacci::Action>(&name)
        .unwrap();
    let response = client
        .send_goal_request(Fibonacci::Goal { order: 1 })
        .unwrap();
    let (accepted, observed) = node
        .spin_until_future_complete_with_options(
            async {
                let accept = async {
                    requests
                        .next()
                        .await
                        .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?
                        .accept()
                };
                futures::try_join!(accept, response)
            },
            Duration::from_secs(5),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    let (goal, mut cancels) = accepted;
    let cancellation = client.cancel_all().unwrap();
    let decision = node
        .spin_until_future_complete_with_options(
            async {
                cancels
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
            },
            Duration::from_secs(5),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    for _ in 0..4 {
        node.try_spin_once(Duration::ZERO).unwrap();
    }
    let guard = node.future_wake_guard.as_ref().unwrap().clone();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !guard.waiting.load(std::sync::atomic::Ordering::Acquire) {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            decision.reject();
        });
        let started = Instant::now();
        let response = node.spin_until_future_complete_with_options(
            cancellation,
            Duration::from_secs(1),
            FutureExecutionOptions::wake_driven(),
        );
        assert!(
            matches!(response, Err(Error::GoalCancelRejected)),
            "external decision failed to wake native delivery: {response:?}"
        );
        assert!(started.elapsed() < Duration::from_millis(500));
    });
    drop((goal, observed));
}

#[test]
fn external_cancel_decision_interrupts_direct_shared_executor_wait() {
    use crate::nodes::future_executor::FutureDriver;
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "external_shared_cancel", "").unwrap();
    let name = format!("/shared_cancel_{}", uuid::Uuid::new_v4().simple());
    let mut requests = node
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = node
        .create_action_client::<Fibonacci::Action>(&name)
        .unwrap();
    let response = client
        .send_goal_request(Fibonacci::Goal { order: 1 })
        .unwrap();
    let (accepted, observed) = node
        .spin_until_future_complete_with_options(
            async {
                let accept = async {
                    requests
                        .next()
                        .await
                        .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?
                        .accept()
                };
                futures::try_join!(accept, response)
            },
            Duration::from_secs(5),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    let (goal, mut cancels) = accepted;
    let cancellation = client.cancel_all().unwrap();
    let decision = node
        .spin_until_future_complete_with_options(
            async {
                cancels
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
            },
            Duration::from_secs(5),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    for _ in 0..4 {
        node.try_spin_once(Duration::ZERO).unwrap();
    }
    let mut executor = Executor::new(context);
    executor.add_node(node).unwrap();
    executor.spin_once(Duration::ZERO).unwrap();
    let guard = executor.cached_future_guard().unwrap();
    std::thread::scope(|scope| {
        scope.spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !guard.waiting.load(std::sync::atomic::Ordering::Acquire) {
                assert!(Instant::now() < deadline);
                std::thread::yield_now();
            }
            decision.reject();
        });
        let started = Instant::now();
        executor.spin_once(Duration::from_millis(250)).unwrap();
        assert!(
            started.elapsed() < Duration::from_millis(125),
            "shared wait ignored a ready service identity"
        );
    });
    assert!(matches!(
        executor.spin_until_future_complete_with_options(
            cancellation,
            Duration::from_secs(5),
            FutureExecutionOptions::wake_driven()
        ),
        Err(Error::GoalCancelRejected)
    ));
    drop((goal, observed));
}

#[test]
fn parked_action_servers_skip_unrelated_maintenance_and_wake_selectively() {
    use crate::action_servers::take_action_server_maintenance_poll_count;
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "parked_server_maintenance", "").unwrap();
    let prefix = uuid::Uuid::new_v4().simple().to_string();
    let mut streams = Vec::new();
    let mut clients = Vec::new();
    let mut goals = Vec::new();
    let mut observed = Vec::new();
    for order in 0..20 {
        let name = format!("/parked_{prefix}_{order}");
        let mut requests = node
            .create_action_server::<Fibonacci::Action>(&name)
            .unwrap();
        let client = node
            .create_action_client::<Fibonacci::Action>(&name)
            .unwrap();
        let response = client.send_goal_request(Fibonacci::Goal { order }).unwrap();
        let (accepted, response) = node
            .spin_until_future_complete_with_options(
                async {
                    let accept = async {
                        requests
                            .next()
                            .await
                            .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?
                            .accept()
                    };
                    futures::try_join!(accept, response)
                },
                Duration::from_secs(5),
                FutureExecutionOptions::wake_driven(),
            )
            .unwrap();
        goals.push(accepted);
        observed.push(response);
        streams.push(requests);
        clients.push(client);
    }
    let mut responses = clients
        .iter()
        .map(|client| client.cancel_all().unwrap())
        .collect::<Vec<_>>();
    let mut decisions = Vec::new();
    for (_, stream) in &mut goals {
        decisions.push(
            node.spin_until_future_complete_with_options(
                async {
                    stream
                        .next()
                        .await
                        .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)
                },
                Duration::from_secs(5),
                FutureExecutionOptions::wake_driven(),
            )
            .unwrap(),
        );
    }
    for _ in 0..4 {
        node.try_spin_once(Duration::ZERO).unwrap();
    }
    assert!(node
        .action_servers
        .iter()
        .all(|server| server.lock().has_pending_cancel_work()));
    take_action_server_maintenance_poll_count();
    for _ in 0..4 {
        node.try_spin_once(Duration::ZERO).unwrap();
    }
    assert_eq!(
        take_action_server_maintenance_poll_count(),
        0,
        "blocked servers were rechecked without ready work"
    );
    decisions.pop().unwrap().reject();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(
        take_action_server_maintenance_poll_count(),
        1,
        "one decision woke unrelated servers"
    );
    assert_eq!(
        node.action_servers
            .iter()
            .filter(|server| server.lock().has_pending_cancel_work())
            .count(),
        19
    );
    let rejected = node.spin_until_future_complete_with_options(
        responses.pop().unwrap(),
        Duration::from_secs(5),
        FutureExecutionOptions::wake_driven(),
    );
    assert!(matches!(rejected, Err(Error::GoalCancelRejected)));
    for decision in decisions {
        decision.reject();
    }
    for response in responses {
        assert!(matches!(
            node.spin_until_future_complete_with_options(
                response,
                Duration::from_secs(5),
                FutureExecutionOptions::wake_driven()
            ),
            Err(Error::GoalCancelRejected)
        ));
    }
    for (order, (goal, _)) in goals.iter_mut().enumerate() {
        assert!(!goal.is_cancelling().unwrap());
        goal.succeed(Fibonacci::Result {
            sequence: vec![order as i32],
        })
        .unwrap();
    }
    for (order, (_goal, result, _feedback)) in observed.into_iter().enumerate() {
        let (status, result) = node
            .spin_until_future_complete_with_options(
                result,
                Duration::from_secs(5),
                FutureExecutionOptions::wake_driven(),
            )
            .unwrap();
        assert_eq!(status, GoalStatus::Succeeded);
        assert_eq!(result.sequence, [order as i32]);
    }
}
