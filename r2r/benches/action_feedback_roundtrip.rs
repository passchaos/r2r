use std::time::{Duration, Instant};

use futures::{FutureExt, StreamExt};
use r2r::{
    example_interfaces::{action::Fibonacci, srv::AddTwoInts},
    ActionClientOptions, ActionGoalOptions, ActionServerOptions, Context, GoalStatus, Node,
    NodeOptions, QosProfile,
};

#[path = "support/action_driver.rs"]
mod driver;

const DEADLINE: Duration = Duration::from_secs(30);

struct Config<'a> {
    elements: usize,
    goal_window: usize,
    feedback_per_goal: usize,
    iterations: usize,
    warmup: usize,
    name: &'a str,
}

fn percentile(samples: &[u64], percent: usize) -> u64 {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[((sorted.len() - 1) * percent).div_ceil(100)]
}

fn payload_value(order: i32, update: usize, index: usize) -> i32 {
    order
        .wrapping_mul(1_000_003)
        .wrapping_add((update as i32).wrapping_mul(65_537))
        .wrapping_add(index as i32)
}

fn payload(order: i32, update: usize, elements: usize) -> Vec<i32> {
    (0..elements)
        .map(|index| payload_value(order, update, index))
        .collect()
}

fn validate_payload(values: &[i32], order: i32, update: usize, elements: usize) {
    assert_eq!(values.len(), elements);
    for (index, value) in values.iter().enumerate() {
        assert_eq!(*value, payload_value(order, update, index));
    }
}

fn wait_for_server(
    node: &mut Node, driver: &mut driver::Driver, client: &r2r::ActionClient<Fibonacci::Action>,
    ready: &r2r::Client<AddTwoInts::Service>,
) -> r2r::Result<()> {
    let deadline = Instant::now() + DEADLINE;
    while !client.is_available(node)? || !ready.is_available(node)? {
        if Instant::now() >= deadline {
            return Err(r2r::Error::RCL_RET_TIMEOUT);
        }
        driver.spin(node, Duration::from_millis(10))?;
    }
    Ok(())
}

fn run(
    node: &mut Node, role: &str, config: Config<'_>,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let Config {
        elements,
        goal_window,
        feedback_per_goal,
        iterations,
        warmup,
        name,
    } = config;
    let total = iterations + warmup;
    let total_goals = total
        .checked_mul(goal_window)
        .ok_or("goal count overflow")?;
    let feedback_count = total_goals
        .checked_mul(feedback_per_goal)
        .ok_or("feedback count overflow")?;
    let depth = goal_window
        .checked_mul(feedback_per_goal)
        .ok_or("history depth overflow")?
        .max(16);
    let qos = QosProfile::services_default().keep_last(depth);
    let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
        .to_str()?;
    let mut driver = driver::Driver::new()?;
    let mut batch_checksum = 0_u64;
    let mut goal_checksum = 0_u64;

    if role == "echo" {
        let options = ActionServerOptions::default()
            .goal_service_qos(qos.clone())
            .result_service_qos(qos.clone())
            .cancel_service_qos(qos.clone())
            .feedback_topic_qos(qos.clone())
            .goal_request_channel_capacity(goal_window)
            .result_timeout(Duration::from_secs(10));
        let mut requests =
            node.create_action_server_with_options::<Fibonacci::Action>(name, options)?;
        let mut ready =
            node.create_service::<AddTwoInts::Service>(&format!("{name}/ready"), qos)?;
        for batch in 0..total {
            if batch == warmup {
                driver.begin()?;
            }
            let mut goals = Vec::with_capacity(goal_window);
            for goal_index in 0..goal_window {
                let request = driver.drive(node, async {
                    requests
                        .next()
                        .await
                        .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)
                })?;
                let order = (batch * goal_window + goal_index + 1) as i32;
                assert_eq!(request.goal.order, order);
                goals.push(request.accept_deferred()?);
            }
            let start = driver.drive(node, async {
                ready
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_SERVICE_INVALID)
            })?;
            assert_eq!((start.message.a, start.message.b), (batch as i64, 0));
            for (goal, _) in &mut goals {
                goal.execute()?;
            }
            start.respond(AddTwoInts::Response { sum: batch as i64 })?;
            for update in 0..feedback_per_goal {
                for (goal_index, (goal, _)) in goals.iter().enumerate() {
                    let order = (batch * goal_window + goal_index + 1) as i32;
                    goal.publish_feedback(Fibonacci::Feedback {
                        sequence: payload(order, update, elements),
                    })?;
                }
            }
            let acknowledged = driver.drive(node, async {
                ready
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_SERVICE_INVALID)
            })?;
            assert_eq!((acknowledged.message.a, acknowledged.message.b), (batch as i64, 1));
            acknowledged.respond(AddTwoInts::Response {
                sum: batch as i64 + 1,
            })?;
            for (goal_index, (mut goal, _)) in goals.into_iter().enumerate() {
                let order = (batch * goal_window + goal_index + 1) as i32;
                goal.succeed(Fibonacci::Result {
                    sequence: vec![order],
                })?;
                goal_checksum = goal_checksum.wrapping_add(order as u64);
            }
            batch_checksum = batch_checksum.wrapping_add(batch as u64);
        }
        let probe = driver.end()?;
        let control = driver.drive(node, async {
            requests
                .next()
                .await
                .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)
        })?;
        assert_eq!(control.goal.order, -1);
        control.reject()?;
        node.destroy_action_server(requests)?;
        node.destroy_service(ready);
        return Ok(serde_json::json!({
            "implementation":"r2r","role":role,"rmw":rmw,"elements":elements,
            "goal_window":goal_window,"feedback_per_goal":feedback_per_goal,
            "iterations":iterations,"warmup":warmup,"received":total,
            "batch_checksum":batch_checksum,"goal_checksum":goal_checksum,
            "accepted":total_goals,"feedback":feedback_count,"results":total_goals,
            "probe":probe
        }));
    }

    let options = ActionClientOptions::default()
        .goal_service_qos(qos.clone())
        .result_service_qos(qos.clone())
        .cancel_service_qos(qos.clone())
        .feedback_topic_qos(qos.clone())
        .feedback_channel_capacity(feedback_per_goal);
    let client = node.create_action_client_with_options::<Fibonacci::Action>(name, options)?;
    let ready = node.create_client::<AddTwoInts::Service>(&format!("{name}/ready"), qos)?;
    wait_for_server(node, &mut driver, &client, &ready)?;
    let mut samples = Vec::with_capacity(iterations);
    let mut feedback_samples = Vec::with_capacity(iterations);
    let mut measured_start = Instant::now();
    for batch in 0..total {
        if batch == warmup {
            driver.begin()?;
            measured_start = Instant::now();
        }
        let before = Instant::now();
        let mut observations = Vec::with_capacity(goal_window);
        for goal_index in 0..goal_window {
            let order = (batch * goal_window + goal_index + 1) as i32;
            let response = client.send_goal_request(Fibonacci::Goal { order })?;
            observations.push(driver.drive(node, response)?);
        }
        let feedback_before = Instant::now();
        let start = ready.request(&AddTwoInts::Request {
            a: batch as i64,
            b: 0,
        })?;
        assert_eq!(driver.drive(node, start)?.sum, batch as i64);
        for update in 0..feedback_per_goal {
            for (goal_index, (_, _, feedback)) in observations.iter_mut().enumerate() {
                let message = driver.drive(node, async {
                    feedback
                        .next()
                        .await
                        .ok_or(r2r::Error::RCL_RET_ACTION_CLIENT_INVALID)
                })?;
                let order = (batch * goal_window + goal_index + 1) as i32;
                validate_payload(&message.sequence, order, update, elements);
            }
        }
        let feedback_elapsed = feedback_before.elapsed().as_nanos() as u64;
        let acknowledged = ready.request(&AddTwoInts::Request {
            a: batch as i64,
            b: 1,
        })?;
        assert_eq!(driver.drive(node, acknowledged)?.sum, batch as i64 + 1);
        for (goal_index, (_goal, result, mut feedback)) in observations.into_iter().enumerate() {
            let (status, result) = driver.drive(node, result)?;
            let order = (batch * goal_window + goal_index + 1) as i32;
            assert_eq!(status, GoalStatus::Succeeded);
            assert_eq!(result.sequence, [order]);
            assert!(feedback.next().now_or_never().flatten().is_none());
            goal_checksum = goal_checksum.wrapping_add(order as u64);
        }
        let elapsed = before.elapsed().as_nanos() as u64;
        batch_checksum = batch_checksum.wrapping_add(batch as u64);
        if batch >= warmup {
            samples.push(elapsed);
            feedback_samples.push(feedback_elapsed);
        }
    }
    let elapsed = measured_start.elapsed().as_nanos() as u64;
    let probe = driver.end()?;
    let stop = client.send_goal_request_with_options(
        Fibonacci::Goal { order: -1 },
        ActionGoalOptions::fire_and_forget(),
    )?;
    assert!(matches!(
        driver.drive(node, stop),
        Err(r2r::Error::RCL_RET_ACTION_GOAL_REJECTED)
    ));
    let measured_feedback = iterations * goal_window * feedback_per_goal;
    Ok(serde_json::json!({
        "implementation":"r2r","role":role,"rmw":rmw,"elements":elements,
        "goal_window":goal_window,"feedback_per_goal":feedback_per_goal,
        "iterations":iterations,"warmup":warmup,"received":total,
        "batch_checksum":batch_checksum,"goal_checksum":goal_checksum,
        "accepted":total_goals,"feedback":feedback_count,"results":total_goals,
        "elapsed_ns":elapsed,"roundtrips_per_second":iterations as f64/(elapsed as f64*1e-9),
        "feedback_messages_per_second":measured_feedback as f64/(elapsed as f64*1e-9),
        "rtt_ns_p50":percentile(&samples,50),"rtt_ns_p95":percentile(&samples,95),
        "rtt_ns_p99":percentile(&samples,99),"rtt_ns_max":samples.iter().max().unwrap(),
        "feedback_ns_p50":percentile(&feedback_samples,50),
        "feedback_ns_p95":percentile(&feedback_samples,95),
        "feedback_ns_p99":percentile(&feedback_samples,99),
        "samples_ns":samples,"feedback_samples_ns":feedback_samples,"probe":probe
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 7 {
        return Err("usage: action_feedback_roundtrip <ping|echo> <elements> <goal-window> <feedback-per-goal> <iterations> <warmup> <action-name>".into());
    }
    let role = &args[0];
    let elements: usize = args[1].parse()?;
    let goal_window: usize = args[2].parse()?;
    let feedback_per_goal: usize = args[3].parse()?;
    let iterations: usize = args[4].parse()?;
    let warmup: usize = args[5].parse()?;
    let total = iterations
        .checked_add(warmup)
        .and_then(|value| value.checked_mul(goal_window))
        .ok_or("goal count overflow")?;
    if !matches!(role.as_str(), "ping" | "echo")
        || elements > 1_000_000
        || !(1..=64).contains(&goal_window)
        || !(1..=256).contains(&feedback_per_goal)
        || goal_window * feedback_per_goal > 4096
        || iterations == 0
        || total > i32::MAX as usize - 1
    {
        return Err("invalid action feedback configuration".into());
    }
    let context = Context::create()?;
    let mut node = Node::create_with_options(
        context,
        &format!("r2r_action_feedback_{role}"),
        "",
        NodeOptions::default().enable_rosout(false),
    )?;
    println!(
        "{}",
        run(
            &mut node,
            role,
            Config {
                elements,
                goal_window,
                feedback_per_goal,
                iterations,
                warmup,
                name: &args[6],
            },
        )?
    );
    Ok(())
}
