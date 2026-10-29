use std::{
    pin::Pin,
    time::{Duration, Instant},
};

use futures::{FutureExt, StreamExt};
use r2r::{
    example_interfaces::{action::Fibonacci, srv::AddTwoInts},
    ActionClientOptions, ActionGoalOptions, ActionServerCancelRequest, ActionServerGoal,
    ActionServerOptions, Context, GoalStatus, Node, NodeOptions, QosProfile,
};

#[path = "support/action_driver.rs"]
mod driver;

const DEADLINE: Duration = Duration::from_secs(30);
type BenchmarkCancelStream =
    Pin<Box<dyn futures::Stream<Item = ActionServerCancelRequest> + Unpin>>;
#[cfg(r2r_bench_action_batches)]
const STATUS_PATH: &str = "batched";
#[cfg(not(r2r_bench_action_batches))]
const STATUS_PATH: &str = "per_goal";

struct Config<'a> {
    elements: usize,
    goal_window: usize,
    feedback_per_goal: usize,
    iterations: usize,
    warmup: usize,
    name: &'a str,
    status_batch_size: usize,
    status_sync: bool,
}

fn status_qos_override() -> Result<(&'static str, Option<QosProfile>), &'static str> {
    match std::env::var("R2R_ACTION_FEEDBACK_STATUS_QOS").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("default") => Ok(("default", None)),
        Ok("reliable_volatile") => {
            Ok(("reliable_volatile", Some(QosProfile::default().keep_last(1).volatile())))
        }
        Ok("best_effort_volatile") => Ok((
            "best_effort_volatile",
            Some(QosProfile::default().keep_last(1).best_effort().volatile()),
        )),
        _ => Err("invalid R2R_ACTION_FEEDBACK_STATUS_QOS"),
    }
}

fn result_timeout_override() -> Result<(u64, Duration), &'static str> {
    let milliseconds = match std::env::var("R2R_ACTION_FEEDBACK_RESULT_TIMEOUT_MS") {
        Err(std::env::VarError::NotPresent) => 10_000,
        Ok(value) => value.parse().map_err(|_| "invalid result timeout")?,
        Err(_) => return Err("invalid result timeout"),
    };
    if milliseconds == 0 || milliseconds > 600_000 {
        return Err("invalid result timeout");
    }
    Ok((milliseconds, Duration::from_millis(milliseconds)))
}

fn status_batch_size_override(goal_window: usize) -> Result<usize, &'static str> {
    let size = match std::env::var("R2R_ACTION_FEEDBACK_STATUS_BATCH_SIZE") {
        Err(std::env::VarError::NotPresent) => goal_window,
        Ok(value) => value.parse().map_err(|_| "invalid status batch size")?,
        Err(_) => return Err("invalid status batch size"),
    };
    if size == 0 || size > goal_window {
        return Err("invalid status batch size");
    }
    Ok(size)
}

fn status_sync_override() -> Result<bool, &'static str> {
    match std::env::var("R2R_ACTION_FEEDBACK_STATUS_SYNC").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("required") => Ok(true),
        Ok("none") => Ok(false),
        _ => Err("invalid R2R_ACTION_FEEDBACK_STATUS_SYNC"),
    }
}

struct ServerStageRecord {
    accept: u64,
    start_wait: u64,
    execute: u64,
    feedback_publish: u64,
    feedback_wait: u64,
    terminal: u64,
}

struct ServerStageSamples {
    accept: Vec<u64>,
    start_wait: Vec<u64>,
    execute: Vec<u64>,
    feedback_publish: Vec<u64>,
    feedback_wait: Vec<u64>,
    terminal: Vec<u64>,
}

impl ServerStageSamples {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            accept: Vec::with_capacity(capacity),
            start_wait: Vec::with_capacity(capacity),
            execute: Vec::with_capacity(capacity),
            feedback_publish: Vec::with_capacity(capacity),
            feedback_wait: Vec::with_capacity(capacity),
            terminal: Vec::with_capacity(capacity),
        }
    }

    fn push(&mut self, record: ServerStageRecord) {
        self.accept.push(record.accept);
        self.start_wait.push(record.start_wait);
        self.execute.push(record.execute);
        self.feedback_publish.push(record.feedback_publish);
        self.feedback_wait.push(record.feedback_wait);
        self.terminal.push(record.terminal);
    }

    fn percentiles(&self, percent: usize) -> serde_json::Value {
        serde_json::json!({
            "accept":percentile(&self.accept,percent),
            "start_wait":percentile(&self.start_wait,percent),
            "execute":percentile(&self.execute,percent),
            "feedback_publish":percentile(&self.feedback_publish,percent),
            "feedback_wait":percentile(&self.feedback_wait,percent),
            "terminal":percentile(&self.terminal,percent)
        })
    }

    fn samples(&self) -> serde_json::Value {
        serde_json::json!({
            "accept":&self.accept,"start_wait":&self.start_wait,"execute":&self.execute,
            "feedback_publish":&self.feedback_publish,"feedback_wait":&self.feedback_wait,
            "terminal":&self.terminal
        })
    }
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

#[cfg(r2r_bench_action_batches)]
fn succeed_goals(
    goals: &mut [ActionServerGoal<Fibonacci::Action>], results: Vec<Fibonacci::Result>,
    status_batch_size: usize,
) -> r2r::Result<()> {
    let mut results = results.into_iter();
    for goals in goals.chunks_mut(status_batch_size) {
        let chunk = results.by_ref().take(goals.len()).collect();
        ActionServerGoal::succeed_batch(goals, chunk)?;
    }
    Ok(())
}

#[cfg(not(r2r_bench_action_batches))]
fn succeed_goals(
    goals: &mut [ActionServerGoal<Fibonacci::Action>], results: Vec<Fibonacci::Result>,
    _status_batch_size: usize,
) -> r2r::Result<()> {
    for (goal, result) in goals.iter_mut().zip(results) {
        goal.succeed(result)?;
    }
    Ok(())
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
        status_batch_size,
        status_sync,
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
    let (status_qos_label, status_qos) = status_qos_override()?;
    let (result_timeout_ms, result_timeout) = result_timeout_override()?;
    let rmw = unsafe { std::ffi::CStr::from_ptr(r2r_rcl::rmw_get_implementation_identifier()) }
        .to_str()?;
    let mut driver = driver::Driver::new()?;
    let mut batch_checksum = 0_u64;
    let mut goal_checksum = 0_u64;

    if role == "echo" {
        let mut options = ActionServerOptions::default()
            .goal_service_qos(qos.clone())
            .result_service_qos(qos.clone())
            .cancel_service_qos(qos.clone())
            .feedback_topic_qos(qos.clone())
            .goal_request_channel_capacity(goal_window)
            .result_timeout(result_timeout);
        if let Some(status_qos) = status_qos.clone() {
            options = options.status_topic_qos(status_qos);
        }
        let mut requests =
            node.create_action_server_with_options::<Fibonacci::Action>(name, options)?;
        let mut ready =
            node.create_service::<AddTwoInts::Service>(&format!("{name}/ready"), qos)?;
        let mut stages = ServerStageSamples::with_capacity(iterations);
        for batch in 0..total {
            if batch == warmup {
                driver.begin()?;
            }
            let accept_before = Instant::now();
            let mut requests_batch = Vec::with_capacity(goal_window);
            for goal_index in 0..goal_window {
                let request = driver.drive(node, async {
                    requests
                        .next()
                        .await
                        .ok_or(r2r::Error::RCL_RET_ACTION_SERVER_INVALID)
                })?;
                let order = (batch * goal_window + goal_index + 1) as i32;
                assert_eq!(request.goal.order, order);
                requests_batch.push(request);
            }
            let goals = accept_requests(requests_batch, status_batch_size)?;
            let accept = accept_before.elapsed().as_nanos() as u64;
            let start_wait_before = Instant::now();
            let start = driver.drive(node, async {
                ready
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_SERVICE_INVALID)
            })?;
            let start_wait = start_wait_before.elapsed().as_nanos() as u64;
            assert_eq!((start.message.a, start.message.b), (batch as i64, 0));
            start.respond(AddTwoInts::Response { sum: batch as i64 })?;
            let mut batch_goals = goals
                .iter()
                .map(|(goal, _)| goal.clone())
                .collect::<Vec<_>>();
            let execute_before = Instant::now();
            // Immediate batch acceptance has already transitioned every goal to Executing.
            let execute = execute_before.elapsed().as_nanos() as u64;
            let feedback_publish_before = Instant::now();
            for update in 0..feedback_per_goal {
                for (goal_index, (goal, _)) in goals.iter().enumerate() {
                    let order = (batch * goal_window + goal_index + 1) as i32;
                    goal.publish_feedback(Fibonacci::Feedback {
                        sequence: payload(order, update, elements),
                    })?;
                }
            }
            let feedback_publish = feedback_publish_before.elapsed().as_nanos() as u64;
            let feedback_wait_before = Instant::now();
            let acknowledged = driver.drive(node, async {
                ready
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_SERVICE_INVALID)
            })?;
            let feedback_wait = feedback_wait_before.elapsed().as_nanos() as u64;
            assert_eq!((acknowledged.message.a, acknowledged.message.b), (batch as i64, 1));
            acknowledged.respond(AddTwoInts::Response {
                sum: batch as i64 + 1,
            })?;
            let terminal_before = Instant::now();
            let mut results = Vec::with_capacity(goal_window);
            for goal_index in 0..goal_window {
                let order = (batch * goal_window + goal_index + 1) as i32;
                results.push(Fibonacci::Result {
                    sequence: vec![order],
                });
                goal_checksum = goal_checksum.wrapping_add(order as u64);
            }
            succeed_goals(&mut batch_goals, results, status_batch_size)?;
            drop(goals);
            let terminal = terminal_before.elapsed().as_nanos() as u64;
            batch_checksum = batch_checksum.wrapping_add(batch as u64);
            if batch >= warmup {
                stages.push(ServerStageRecord {
                    accept,
                    start_wait,
                    execute,
                    feedback_publish,
                    feedback_wait,
                    terminal,
                });
            }
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
            "status_path":STATUS_PATH,
            "status_qos":status_qos_label,
            "result_timeout_ms":result_timeout_ms,
            "status_batch_size":status_batch_size,
            "status_sync":if status_sync {"required"} else {"none"},
            "goal_window":goal_window,"feedback_per_goal":feedback_per_goal,
            "iterations":iterations,"warmup":warmup,"received":total,
            "batch_checksum":batch_checksum,"goal_checksum":goal_checksum,
            "accepted":total_goals,"feedback":feedback_count,"results":total_goals,
            "server_stage_ns_p50":stages.percentiles(50),
            "server_stage_ns_p95":stages.percentiles(95),
            "server_stage_ns_p99":stages.percentiles(99),
            "server_stage_samples_ns":stages.samples(),
            "probe":probe
        }));
    }

    let mut options = ActionClientOptions::default()
        .goal_service_qos(qos.clone())
        .result_service_qos(qos.clone())
        .cancel_service_qos(qos.clone())
        .feedback_topic_qos(qos.clone())
        .feedback_channel_capacity(feedback_per_goal);
    if let Some(status_qos) = status_qos {
        options = options.status_topic_qos(status_qos);
    }
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
        let mut responses = Vec::with_capacity(goal_window);
        for goal_index in 0..goal_window {
            let order = (batch * goal_window + goal_index + 1) as i32;
            responses.push(client.send_goal_request(Fibonacci::Goal { order })?);
        }
        let mut observations = Vec::with_capacity(goal_window);
        for response in responses {
            observations.push(driver.drive(node, response)?);
        }
        if status_sync {
            let status_deadline = Instant::now() + DEADLINE;
            while !observations
                .iter()
                .all(|(goal, _, _)| matches!(goal.get_status(), Ok(GoalStatus::Executing)))
            {
                if Instant::now() >= status_deadline {
                    return Err("executing status deadline expired".into());
                }
                driver.spin(node, Duration::from_millis(100))?;
            }
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
        "status_path":STATUS_PATH,
        "status_qos":status_qos_label,
        "result_timeout_ms":result_timeout_ms,
        "status_batch_size":status_batch_size,
        "status_sync":if status_sync {"required"} else {"none"},
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

#[cfg(r2r_bench_action_batches)]
fn accept_requests(
    mut requests: Vec<r2r::ActionServerGoalRequest<Fibonacci::Action>>, status_batch_size: usize,
) -> r2r::Result<Vec<(ActionServerGoal<Fibonacci::Action>, BenchmarkCancelStream)>> {
    let mut accepted = Vec::with_capacity(requests.len());
    while !requests.is_empty() {
        let tail = requests.split_off(requests.len().min(status_batch_size));
        accepted.extend(
            r2r::ActionServerGoalRequest::accept_batch(requests)?
                .into_iter()
                .map(|(goal, cancels)| (goal, Box::pin(cancels) as BenchmarkCancelStream)),
        );
        requests = tail;
    }
    Ok(accepted)
}

#[cfg(not(r2r_bench_action_batches))]
fn accept_requests(
    requests: Vec<r2r::ActionServerGoalRequest<Fibonacci::Action>>, _status_batch_size: usize,
) -> r2r::Result<Vec<(ActionServerGoal<Fibonacci::Action>, BenchmarkCancelStream)>> {
    requests
        .into_iter()
        .map(|request| {
            request
                .accept()
                .map(|(goal, cancels)| (goal, Box::pin(cancels) as BenchmarkCancelStream))
        })
        .collect()
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
    let status_batch_size = status_batch_size_override(goal_window)?;
    let status_sync = status_sync_override()?;
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
                status_batch_size,
                status_sync,
            },
        )?
    );
    Ok(())
}
