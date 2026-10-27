use std::{
    future::Future,
    time::{Duration, Instant},
};

use futures::{FutureExt, StreamExt};
use r2r::{
    example_interfaces::{action::Fibonacci, srv::AddTwoInts},
    ActionClientOptions, ActionGoalOptions, ActionServerOptions, Context, GoalStatus, Node,
    NodeOptions, QosProfile,
};

const DEADLINE: Duration = Duration::from_secs(10);

fn drive<T>(node: &mut Node, future: impl Future<Output = r2r::Result<T>>) -> r2r::Result<T> {
    futures::pin_mut!(future);
    let deadline = Instant::now() + DEADLINE;
    loop {
        if let Some(result) = future.as_mut().now_or_never() {
            return result;
        }
        if Instant::now() >= deadline {
            return Err(r2r::Error::RCL_RET_TIMEOUT);
        }
        node.try_spin_once(Duration::from_millis(100))?;
    }
}

fn fixture(order: i32) -> Vec<i32> {
    vec![order, order.wrapping_add(1), order.wrapping_add(2)]
}
fn percentile(samples: &[u64], percent: usize) -> u64 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[((sorted.len() - 1) * percent).div_ceil(100)]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err("usage: action_roundtrip <ping|echo> <scenario:0=success,1=cancel> <iterations> <warmup> <action-name>".into());
    }
    let role = &args[0];
    let scenario: usize = args[1].parse()?;
    let iterations: usize = args[2].parse()?;
    let warmup: usize = args[3].parse()?;
    let total = iterations.checked_add(warmup).ok_or("iteration overflow")?;
    if !matches!(role.as_str(), "ping" | "echo")
        || scenario > 1
        || iterations == 0
        || total > i32::MAX as usize - 3
    {
        return Err("invalid action configuration".into());
    }
    let context = Context::create()?;
    let mut node = Node::create_with_options(
        context,
        &format!("r2r_action_{role}"),
        "",
        NodeOptions::default().enable_rosout(false),
    )?;
    let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
        .to_str()?;
    let qos = QosProfile::services_default().keep_last(16);
    let mut checksum = 0_u64;
    if role == "echo" {
        let options = ActionServerOptions::default()
            .goal_service_qos(qos.clone())
            .result_service_qos(qos.clone())
            .cancel_service_qos(qos.clone())
            .feedback_topic_qos(qos.clone())
            .result_timeout(Duration::from_secs(10));
        let mut requests =
            node.create_action_server_with_options::<Fibonacci::Action>(&args[4], options)?;
        let mut ready =
            node.create_service::<AddTwoInts::Service>(&format!("{}/ready", args[4]), qos)?;
        for sequence in 0..total {
            let request = drive(&mut node, async {
                requests
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)
            })?;
            let order = sequence as i32 + 2;
            assert_eq!(request.goal.order, order);
            let (mut goal, mut cancels) = request.accept_deferred()?;
            let start = drive(&mut node, async {
                ready
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_SERVICE_INVALID)
            })?;
            assert_eq!((start.message.a, start.message.b), (order as i64, 0));
            goal.execute()?;
            start.respond(AddTwoInts::Response { sum: order as i64 })?;
            goal.publish_feedback(Fibonacci::Feedback {
                sequence: fixture(order),
            })?;
            let result = Fibonacci::Result {
                sequence: fixture(order),
            };
            if scenario == 0 {
                let acknowledged = drive(&mut node, async {
                    ready
                        .next()
                        .await
                        .ok_or(r2r::Error::RCL_RET_SERVICE_INVALID)
                })?;
                assert_eq!((acknowledged.message.a, acknowledged.message.b), (order as i64, 1));
                acknowledged.respond(AddTwoInts::Response {
                    sum: order as i64 + 1,
                })?;
                goal.succeed(result)?;
            } else {
                let cancel = drive(&mut node, async {
                    cancels
                        .next()
                        .await
                        .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)
                })?;
                assert_eq!(cancel.uuid, goal.uuid);
                cancel.accept();
                let deadline = Instant::now() + DEADLINE;
                while !goal.is_cancelling()? {
                    if Instant::now() >= deadline {
                        return Err("cancel transition timed out".into());
                    }
                    node.try_spin_once(Duration::from_millis(100))?;
                }
                goal.cancel(result)?;
            }
            checksum = checksum.wrapping_add(sequence as u64);
        }
        for control in [-1, -2] {
            let request = drive(&mut node, async {
                requests
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)
            })?;
            assert_eq!(request.goal.order, control);
            request.reject()?;
        }
        println!(
            "{}",
            serde_json::json!({"implementation":"r2r", "role":role,"rmw":rmw,"scenario":scenario,
            "iterations":iterations,"warmup":warmup,"received":total,"checksum":checksum,
            "accepted":total,"feedback":total,"results":total,"canceled":if scenario==1 {total} else {0}})
        );
        return Ok(());
    }
    let options = ActionClientOptions::default()
        .goal_service_qos(qos.clone())
        .result_service_qos(qos.clone())
        .cancel_service_qos(qos.clone())
        .feedback_topic_qos(qos.clone())
        .feedback_channel_capacity(16);
    let client = node.create_action_client_with_options::<Fibonacci::Action>(&args[4], options)?;
    let ready = node.create_client::<AddTwoInts::Service>(&format!("{}/ready", args[4]), qos)?;
    let deadline = Instant::now() + DEADLINE;
    while !client.is_available(&node)? || !ready.is_available(&node)? {
        if Instant::now() >= deadline {
            return Err("action discovery timed out".into());
        }
        node.try_spin_once(Duration::from_millis(10))?;
    }
    let mut samples = Vec::with_capacity(iterations);
    let mut acceptance = Vec::with_capacity(iterations);
    let mut cancellation = Vec::with_capacity(iterations);
    let mut measured_start = Instant::now();
    for sequence in 0..total {
        if sequence == warmup {
            measured_start = Instant::now();
        }
        let order = sequence as i32 + 2;
        let before = Instant::now();
        let (goal, result, mut feedback) =
            drive(&mut node, client.send_goal_request(Fibonacci::Goal { order })?)?;
        let accepted_ns = before.elapsed().as_nanos() as u64;
        let started = ready.request(&AddTwoInts::Request {
            a: order as i64,
            b: 0,
        })?;
        assert_eq!(drive(&mut node, started)?.sum, order as i64);
        let feedback_message = drive(&mut node, async {
            feedback
                .next()
                .await
                .ok_or(r2r::Error::RCL_RET_ACTION_CLIENT_INVALID)
        })?;
        assert_eq!(feedback_message.sequence, fixture(order));
        if scenario == 1 {
            let before_cancel = Instant::now();
            drive(&mut node, goal.cancel()?)?;
            if sequence >= warmup {
                cancellation.push(before_cancel.elapsed().as_nanos() as u64);
            }
        } else {
            let acknowledged = ready.request(&AddTwoInts::Request {
                a: order as i64,
                b: 1,
            })?;
            assert_eq!(drive(&mut node, acknowledged)?.sum, order as i64 + 1);
        }
        let (status, result) = drive(&mut node, result)?;
        let elapsed = before.elapsed().as_nanos() as u64;
        assert_eq!(
            status,
            if scenario == 1 {
                GoalStatus::Canceled
            } else {
                GoalStatus::Succeeded
            }
        );
        assert_eq!(result.sequence, fixture(order));
        // Any extra feedback from this goal is a protocol-oracle failure.
        assert!(feedback.next().now_or_never().flatten().is_none());
        checksum = checksum.wrapping_add(sequence as u64);
        if sequence >= warmup {
            samples.push(elapsed);
            acceptance.push(accepted_ns);
        }
    }
    let elapsed = measured_start.elapsed().as_nanos();
    let stop = client.send_goal_request_with_options(
        Fibonacci::Goal { order: -1 },
        ActionGoalOptions::fire_and_forget(),
    )?;
    assert!(matches!(drive(&mut node, stop), Err(r2r::Error::RCL_RET_ACTION_GOAL_REJECTED)));
    drop(client.send_goal_request_with_options(
        Fibonacci::Goal { order: -2 },
        ActionGoalOptions::fire_and_forget(),
    )?);
    println!(
        "{}",
        serde_json::json!({"implementation":"r2r", "role":role,"rmw":rmw,"scenario":scenario,
        "iterations":iterations,"warmup":warmup,"received":total,"checksum":checksum,
        "accepted":total,"feedback":total,"results":total,"canceled":if scenario==1 {total} else {0},
        "elapsed_ns":elapsed,"roundtrips_per_second":iterations as f64/(elapsed as f64*1e-9),
        "rtt_ns_p50":percentile(&samples,50),"rtt_ns_p95":percentile(&samples,95),"rtt_ns_p99":percentile(&samples,99),
        "rtt_ns_max":samples.iter().max().unwrap(),"samples_ns":samples,
        "stage_samples_ns":{"acceptance":acceptance,"cancellation":cancellation},
        "acceptance_ns_p50":percentile(&acceptance,50),
        "cancellation_ns_p50":if scenario==1 {Some(percentile(&cancellation,50))} else {None}})
    );
    Ok(())
}
