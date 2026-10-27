use std::time::{Duration, Instant};

use futures::{FutureExt, StreamExt};
use r2r::{
    example_interfaces::{action::Fibonacci, srv::AddTwoInts},
    ActionClientOptions, ActionGoalOptions, ActionServerOptions, Context, GoalStatus, Node,
    NodeOptions, QosProfile,
};

const DEADLINE: Duration = Duration::from_secs(10);

fn fixture(order: i32) -> Vec<i32> {
    vec![order, order.wrapping_add(1), order.wrapping_add(2)]
}
#[path = "support/action_report.rs"]
mod report;
use report::percentile;
#[path = "support/cohort_control.rs"]
mod cohort_control;
#[path = "support/action_driver.rs"]
mod driver;

fn run_cohort(
    node: &mut Node, role: &str, scenario: usize, dimensions: (usize, usize, usize), name: &str,
    offset: usize,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let (iterations, warmup, total_cohorts) = dimensions;
    let total = iterations + warmup;
    let mut driver = driver::Driver::new()?;
    let confirm_controls = total_cohorts > 1;
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
            node.create_action_server_with_options::<Fibonacci::Action>(name, options)?;
        let mut ready =
            node.create_service::<AddTwoInts::Service>(&format!("{}/ready", name), qos)?;
        for sequence in 0..total {
            if sequence == warmup {
                driver.begin()?;
            }
            let request = driver.drive(node, async {
                requests
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)
            })?;
            let order = (offset + sequence) as i32 + 2;
            assert_eq!(request.goal.order, order);
            let (mut goal, mut cancels) = request.accept_deferred()?;
            let start = driver.drive(node, async {
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
                let acknowledged = driver.drive(node, async {
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
                let cancel = driver.drive(node, async {
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
                    driver.spin(node, Duration::from_millis(100))?;
                }
                goal.cancel(result)?;
            }
            checksum = checksum.wrapping_add((offset + sequence) as u64);
        }
        let probe = driver.end()?;
        for control in [-1, -2] {
            let request = driver.drive(node, async {
                requests
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)
            })?;
            assert_eq!(request.goal.order, control);
            request.reject()?;
        }
        node.destroy_action_server(requests)?;
        node.destroy_service(ready);
        return Ok(
            serde_json::json!({"implementation":"r2r", "role":role,"rmw":rmw,"scenario":scenario,
            "iterations":iterations,"warmup":warmup,"received":total,"checksum":checksum,
            "accepted":total,"feedback":total,"results":total,"canceled":if scenario==1 {total} else {0},"probe":probe}),
        );
    }
    let options = ActionClientOptions::default()
        .goal_service_qos(qos.clone())
        .result_service_qos(qos.clone())
        .cancel_service_qos(qos.clone())
        .feedback_topic_qos(qos.clone())
        .feedback_channel_capacity(16);
    let client = node.create_action_client_with_options::<Fibonacci::Action>(name, options)?;
    let ready = node.create_client::<AddTwoInts::Service>(&format!("{}/ready", name), qos)?;
    let deadline = Instant::now() + DEADLINE;
    while !client.is_available(node)? || !ready.is_available(node)? {
        if Instant::now() >= deadline {
            return Err("action discovery timed out".into());
        }
        driver.spin(node, Duration::from_millis(10))?;
    }
    let mut samples = Vec::with_capacity(iterations);
    let mut acceptance = Vec::with_capacity(iterations);
    let mut cancellation = Vec::with_capacity(iterations);
    let mut measured_start = Instant::now();
    for sequence in 0..total {
        if sequence == warmup {
            driver.begin()?;
            measured_start = Instant::now();
        }
        let order = (offset + sequence) as i32 + 2;
        let before = Instant::now();
        let (goal, result, mut feedback) =
            driver.drive(node, client.send_goal_request(Fibonacci::Goal { order })?)?;
        let accepted_ns = before.elapsed().as_nanos() as u64;
        let started = ready.request(&AddTwoInts::Request {
            a: order as i64,
            b: 0,
        })?;
        assert_eq!(driver.drive(node, started)?.sum, order as i64);
        let feedback_message = driver.drive(node, async {
            feedback
                .next()
                .await
                .ok_or(r2r::Error::RCL_RET_ACTION_CLIENT_INVALID)
        })?;
        assert_eq!(feedback_message.sequence, fixture(order));
        if scenario == 1 {
            let before_cancel = Instant::now();
            driver.drive(node, goal.cancel()?)?;
            if sequence >= warmup {
                cancellation.push(before_cancel.elapsed().as_nanos() as u64);
            }
        } else {
            let acknowledged = ready.request(&AddTwoInts::Request {
                a: order as i64,
                b: 1,
            })?;
            assert_eq!(driver.drive(node, acknowledged)?.sum, order as i64 + 1);
        }
        let (status, result) = driver.drive(node, result)?;
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
        checksum = checksum.wrapping_add((offset + sequence) as u64);
        if sequence >= warmup {
            samples.push(elapsed);
            acceptance.push(accepted_ns);
        }
    }
    let elapsed = measured_start.elapsed().as_nanos();
    let probe = driver.end()?;
    let stop = client.send_goal_request_with_options(
        Fibonacci::Goal { order: -1 },
        ActionGoalOptions::fire_and_forget(),
    )?;
    assert!(matches!(
        driver.drive(node, stop),
        Err(r2r::Error::RCL_RET_ACTION_GOAL_REJECTED)
    ));
    let confirmed = client.send_goal_request_with_options(
        Fibonacci::Goal { order: -2 },
        ActionGoalOptions::fire_and_forget(),
    )?;
    if confirm_controls {
        assert!(matches!(
            driver.drive(node, confirmed),
            Err(r2r::Error::RCL_RET_ACTION_GOAL_REJECTED)
        ));
    } else {
        drop(confirmed);
    }
    Ok(
        serde_json::json!({"implementation":"r2r", "role":role,"rmw":rmw,"scenario":scenario,
        "iterations":iterations,"warmup":warmup,"received":total,"checksum":checksum,"probe":probe,
        "accepted":total,"feedback":total,"results":total,"canceled":if scenario==1 {total} else {0},
        "elapsed_ns":elapsed,"roundtrips_per_second":iterations as f64/(elapsed as f64*1e-9),
        "rtt_ns_p50":percentile(&samples,50),"rtt_ns_p95":percentile(&samples,95),"rtt_ns_p99":percentile(&samples,99),
        "rtt_ns_max":samples.iter().max().unwrap(),"samples_ns":samples,
        "stage_samples_ns":{"acceptance":acceptance,"cancellation":cancellation},
        "acceptance_ns_p50":percentile(&acceptance,50),
        "cancellation_ns_p50":if scenario==1 {Some(percentile(&cancellation,50))} else {None}}),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(5..=6).contains(&args.len()) {
        return Err("usage: action_roundtrip <ping|echo> <scenario> <iterations-per-cohort> <warmup-per-cohort> <action-name> [cohorts]".into());
    }
    let role = &args[0];
    let scenario: usize = args[1].parse()?;
    let iterations: usize = args[2].parse()?;
    let warmup: usize = args[3].parse()?;
    let cohorts: usize = args
        .get(5)
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(1);
    let total = iterations
        .checked_add(warmup)
        .and_then(|value| value.checked_mul(cohorts))
        .ok_or("iteration overflow")?;
    if !matches!(role.as_str(), "ping" | "echo")
        || scenario > 1
        || iterations == 0
        || !(1..=1024).contains(&cohorts)
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
    let process_start = Instant::now();
    let control = cohort_control::Control::load()?;
    if let Some(control) = &control {
        control.ready(role)?;
    }
    let mut reports = Vec::with_capacity(cohorts);
    for index in 0..cohorts {
        if let Some(control) = &control {
            control.start(index)?;
        }
        let name = if cohorts == 1 {
            args[4].clone()
        } else {
            format!("{}_cohort_{index}", args[4])
        };
        reports.push(run_cohort(
            &mut node,
            role,
            scenario,
            (iterations, warmup, cohorts),
            &name,
            index * (iterations + warmup),
        )?);
        if let Some(control) = &control {
            control.done(index, role)?;
        }
    }
    if let Some(control) = &control {
        control.finish()?;
    }
    let process_elapsed = process_start.elapsed().as_nanos() as u64;
    let mut result = report::aggregate(reports, iterations, warmup);
    if cohorts > 1 {
        result["cohort_process_elapsed_ns"] = serde_json::json!(process_elapsed);
    }
    println!("{result}");
    Ok(())
}
