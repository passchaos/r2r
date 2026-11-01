use super::*;
use futures::{FutureExt, StreamExt};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use crate::{example_interfaces::action::Fibonacci, ActionGoalOptions};

static GOAL_RESPONSE_CREATED: AtomicUsize = AtomicUsize::new(0);
static GOAL_RESPONSE_DESTROYED: AtomicUsize = AtomicUsize::new(0);
static BATCH_TEST_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct TrackedGoalResponse {
    accepted: bool,
    stamp: builtin_interfaces::msg::Time,
}

unsafe impl WrappedTypesupport for TrackedGoalResponse {
    type CStruct = <Fibonacci::SendGoal::Response as WrappedTypesupport>::CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t {
        Fibonacci::SendGoal::Response::get_ts()
    }

    fn create_msg() -> *mut Self::CStruct {
        let message = Fibonacci::SendGoal::Response::create_msg();
        if !message.is_null() {
            GOAL_RESPONSE_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }

    fn destroy_msg(message: *mut Self::CStruct) {
        GOAL_RESPONSE_DESTROYED.fetch_add(1, Ordering::Relaxed);
        Fibonacci::SendGoal::Response::destroy_msg(message);
    }

    fn from_native(message: &Self::CStruct) -> Self {
        Self {
            accepted: message.accepted,
            stamp: builtin_interfaces::msg::Time::from_native(&message.stamp),
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        message.accepted = self.accepted;
        self.stamp.copy_to_native(&mut message.stamp);
    }
}

#[derive(Clone, Debug)]
struct TrackedSendGoal;

unsafe impl WrappedServiceTypeSupport for TrackedSendGoal {
    type Request = Fibonacci::SendGoal::Request;
    type Response = TrackedGoalResponse;
    #[cfg(any(
        r2r__ros__distro__iron,
        r2r__ros__distro__jazzy,
        r2r__ros__distro__kilted,
        r2r__ros__distro__lyrical,
        r2r__ros__distro__rolling
    ))]
    type Event = Fibonacci::SendGoal::Event;

    fn get_ts() -> &'static rosidl_service_type_support_t {
        Fibonacci::SendGoal::Service::get_ts()
    }
}

#[derive(Clone, Debug)]
struct TrackedBatchAction;

unsafe impl WrappedActionTypeSupport for TrackedBatchAction {
    type Goal = Fibonacci::Goal;
    type Result = Fibonacci::Result;
    type Feedback = Fibonacci::Feedback;
    type FeedbackMessage = Fibonacci::FeedbackMessage;
    type SendGoal = TrackedSendGoal;
    type GetResult = Fibonacci::GetResult::Service;

    fn get_ts() -> &'static rosidl_action_type_support_t {
        Fibonacci::Action::get_ts()
    }

    fn make_goal_request_msg(
        goal_id: unique_identifier_msgs::msg::UUID, goal: Self::Goal,
    ) -> Fibonacci::SendGoal::Request {
        Fibonacci::SendGoal::Request { goal_id, goal }
    }

    fn make_goal_response_msg(
        accepted: bool, stamp: builtin_interfaces::msg::Time,
    ) -> TrackedGoalResponse {
        TrackedGoalResponse { accepted, stamp }
    }

    fn make_feedback_msg(
        goal_id: unique_identifier_msgs::msg::UUID, feedback: Self::Feedback,
    ) -> Self::FeedbackMessage {
        Fibonacci::FeedbackMessage { goal_id, feedback }
    }

    fn make_result_request_msg(
        goal_id: unique_identifier_msgs::msg::UUID,
    ) -> Fibonacci::GetResult::Request {
        Fibonacci::GetResult::Request { goal_id }
    }

    fn make_result_response_msg(
        status: i8, result: Self::Result,
    ) -> Fibonacci::GetResult::Response {
        Fibonacci::GetResult::Response { status, result }
    }

    fn destructure_goal_request_msg(
        message: Fibonacci::SendGoal::Request,
    ) -> (unique_identifier_msgs::msg::UUID, Self::Goal) {
        (message.goal_id, message.goal)
    }

    fn destructure_goal_response_msg(
        message: TrackedGoalResponse,
    ) -> (bool, builtin_interfaces::msg::Time) {
        (message.accepted, message.stamp)
    }

    fn destructure_feedback_msg(
        message: Self::FeedbackMessage,
    ) -> (unique_identifier_msgs::msg::UUID, Self::Feedback) {
        (message.goal_id, message.feedback)
    }

    fn destructure_result_response_msg(
        message: Fibonacci::GetResult::Response,
    ) -> (i8, Self::Result) {
        (message.status, message.result)
    }

    fn destructure_result_request_msg(
        message: Fibonacci::GetResult::Request,
    ) -> unique_identifier_msgs::msg::UUID {
        message.goal_id
    }
}

fn wait_for_server(node: &mut Node, client: &crate::ActionClient<Fibonacci::Action>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !client.is_available(node).unwrap() {
        assert!(Instant::now() < deadline, "action server discovery timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn run_batch(
    node: &mut Node, server: &mut ActionServer<TrackedBatchAction>,
    client: &crate::ActionClient<Fibonacci::Action>, start: i32,
) {
    let options = ActionGoalOptions::default()
        .track_feedback(false)
        .track_result(false);
    let mut responses = Vec::new();
    for offset in 0..4 {
        responses.push(Box::pin(
            client
                .send_goal_request_with_options(
                    Fibonacci::Goal {
                        order: start + offset,
                    },
                    options,
                )
                .unwrap(),
        ));
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut requests = Vec::new();
    while requests.len() < 4 {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        while requests.len() < 4 {
            match server.next().now_or_never().flatten() {
                Some(request) => requests.push(request),
                None => break,
            }
        }
        assert!(Instant::now() < deadline, "batched goal requests timed out");
    }
    let mut accepted =
        ActionServerGoalRequest::<TrackedBatchAction>::accept_batch(requests).unwrap();
    assert_eq!(accepted.len(), 4);
    for response in responses.iter_mut() {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            node.try_spin_once(Duration::from_millis(10)).unwrap();
            if let Some(response) = response.as_mut().now_or_never() {
                response.unwrap();
                break;
            }
            assert!(Instant::now() < deadline, "batched goal responses timed out");
        }
    }
    for (goal, _cancel) in accepted.iter_mut() {
        goal.succeed(Fibonacci::Result {
            sequence: vec![1; 4],
        })
        .unwrap();
    }
}

#[test]
fn typed_action_server_batch_reuses_goal_response_storage() {
    let _lock = BATCH_TEST_LOCK.lock();
    GOAL_RESPONSE_CREATED.store(0, Ordering::Relaxed);
    GOAL_RESPONSE_DESTROYED.store(0, Ordering::Relaxed);

    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "typed_action_server_batch_reuse", "").unwrap();
    let action_name = format!("/typed_action_server_batch_reuse_{}", uuid::Uuid::new_v4().simple());
    let mut server = node
        .create_action_server::<TrackedBatchAction>(&action_name)
        .unwrap();
    let client = node
        .create_action_client::<Fibonacci::Action>(&action_name)
        .unwrap();
    wait_for_server(&mut node, &client);

    run_batch(&mut node, &mut server, &client, 1);
    assert_eq!(GOAL_RESPONSE_CREATED.load(Ordering::Relaxed), 4);
    assert_eq!(GOAL_RESPONSE_DESTROYED.load(Ordering::Relaxed), 0);

    run_batch(&mut node, &mut server, &client, 5);
    assert_eq!(GOAL_RESPONSE_CREATED.load(Ordering::Relaxed), 4);
    assert_eq!(GOAL_RESPONSE_DESTROYED.load(Ordering::Relaxed), 0);

    node.destroy_action_server(server).unwrap();
    assert_eq!(GOAL_RESPONSE_DESTROYED.load(Ordering::Relaxed), 4);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual typed action server batch native reuse allocation stage; LD_PRELOAD probe required"]
fn benchmark_typed_action_server_batch_fresh_vs_pooled_native_decode() {
    use crate::allocation_probe;
    const ITERATIONS: usize = 10_000;
    const BATCH: usize = 16;

    fn measure(
        probe: &allocation_probe::Probe, mut operation: impl FnMut(),
    ) -> (allocation_probe::NativeStats, u128) {
        let mut stats = allocation_probe::NativeStats::default();
        assert_eq!(unsafe { (probe.begin)() }, 0);
        let begin = Instant::now();
        for _ in 0..ITERATIONS {
            operation();
        }
        let elapsed_ns = begin.elapsed().as_nanos();
        assert_eq!(unsafe { (probe.end)(&mut stats) }, 0);
        assert_eq!(stats.failed_allocations + stats.counter_overflows + stats.size_overflows, 0);
        (stats, elapsed_ns)
    }

    let probe = allocation_probe::Probe::load()
        .unwrap()
        .expect("LD_PRELOAD allocation probe required");
    let goal_info = action_msgs::msg::GoalInfo {
        goal_id: unique_identifier_msgs::msg::UUID { uuid: [7; 16] },
        stamp: builtin_interfaces::msg::Time { sec: 1, nanosec: 2 },
    };
    let response = TrackedGoalResponse {
        accepted: true,
        stamp: goal_info.stamp.clone(),
    };

    let mut fresh_goal_infos = Vec::with_capacity(BATCH);
    let mut fresh_responses = Vec::with_capacity(BATCH);
    let mut fresh_step = || {
        fresh_goal_infos.clear();
        fresh_responses.clear();
        for _ in 0..BATCH {
            fresh_goal_infos.push(
                WrappedNativeMsg::<action_msgs::msg::GoalInfo>::try_from(&goal_info).unwrap(),
            );
            fresh_responses
                .push(WrappedNativeMsg::<TrackedGoalResponse>::try_from(&response).unwrap());
        }
        std::hint::black_box((&fresh_goal_infos, &fresh_responses));
    };
    for _ in 0..100 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let goal_info_pool = NativePool::new(BATCH);
    let response_pool = NativePool::new(BATCH);
    let mut warm_goal_infos = Vec::new();
    let mut warm_responses = Vec::new();
    for _ in 0..BATCH {
        warm_goal_infos.push(
            goal_info_pool
                .checkout(|| WrappedNativeMsg::try_from(&goal_info))
                .unwrap(),
        );
        warm_responses.push(
            response_pool
                .checkout(|| WrappedNativeMsg::try_from(&response))
                .unwrap(),
        );
    }
    drop(warm_goal_infos);
    drop(warm_responses);
    assert_eq!(goal_info_pool.available_len(), BATCH);
    assert_eq!(response_pool.available_len(), BATCH);
    let mut pooled_goal_infos = Vec::with_capacity(BATCH);
    let mut pooled_responses = Vec::with_capacity(BATCH);
    let mut pooled_step = || {
        pooled_goal_infos.clear();
        pooled_responses.clear();
        for _ in 0..BATCH {
            let mut goal_info_msg = goal_info_pool
                .checkout(|| panic!("typed action server goal-info pool was not recycled"))
                .unwrap();
            goal_info.try_copy_to_native(&mut goal_info_msg).unwrap();
            pooled_goal_infos.push(goal_info_msg);
            let mut response_msg = response_pool
                .checkout(|| panic!("typed action server goal-response pool was not recycled"))
                .unwrap();
            response.try_copy_to_native(&mut response_msg).unwrap();
            pooled_responses.push(response_msg);
        }
        std::hint::black_box((&pooled_goal_infos, &pooled_responses));
    };
    for _ in 0..100 {
        pooled_step();
    }
    let (pooled, pooled_elapsed_ns) = measure(&probe, &mut pooled_step);

    let allocations = |stats: &allocation_probe::NativeStats| {
        stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls
    };
    assert!(allocations(&fresh) - allocations(&pooled) >= ITERATIONS as u64 * 2 * BATCH as u64);
    assert!(fresh.free_calls - pooled.free_calls >= ITERATIONS as u64 * 2 * BATCH as u64);
    assert!(
        fresh.requested_bytes - pooled.requested_bytes >= ITERATIONS as u64 * BATCH as u64 * 24
    );
    for (storage, stats, elapsed_ns) in [
        ("fresh_native", &fresh, fresh_elapsed_ns),
        ("pooled_native", &pooled, pooled_elapsed_ns),
    ] {
        println!(
            "{}",
            serde_json::json!({
                "boundary":"typed action server batch goal-info/response native fill; no RCL/DDS",
                "native_storage":storage,
                "iterations":ITERATIONS,
                "batch":BATCH,
                "elapsed_ns":elapsed_ns,
                "allocations":allocations(stats),
                "frees":stats.free_calls,
                "requested_bytes":stats.requested_bytes
            })
        );
    }
}
