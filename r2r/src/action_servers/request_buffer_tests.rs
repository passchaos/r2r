use super::*;
use futures::{FutureExt, StreamExt};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use crate::{example_interfaces::action::Fibonacci, ActionGoalOptions};

static TRACKED_SERVER_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
static GOAL_REQUEST_CREATED: AtomicUsize = AtomicUsize::new(0);
static GOAL_REQUEST_DESTROYED: AtomicUsize = AtomicUsize::new(0);
static RESULT_REQUEST_CREATED: AtomicUsize = AtomicUsize::new(0);
static RESULT_REQUEST_DESTROYED: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct TrackedGoalRequest {
    goal_id: unique_identifier_msgs::msg::UUID,
    goal: Fibonacci::Goal,
}

unsafe impl WrappedTypesupport for TrackedGoalRequest {
    type CStruct = <Fibonacci::SendGoal::Request as WrappedTypesupport>::CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t {
        Fibonacci::SendGoal::Request::get_ts()
    }

    fn create_msg() -> *mut Self::CStruct {
        let message = Fibonacci::SendGoal::Request::create_msg();
        if !message.is_null() {
            GOAL_REQUEST_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }

    fn destroy_msg(message: *mut Self::CStruct) {
        GOAL_REQUEST_DESTROYED.fetch_add(1, Ordering::Relaxed);
        Fibonacci::SendGoal::Request::destroy_msg(message);
    }

    fn from_native(message: &Self::CStruct) -> Self {
        let request = Fibonacci::SendGoal::Request::from_native(message);
        Self {
            goal_id: request.goal_id,
            goal: request.goal,
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        Fibonacci::SendGoal::Request {
            goal_id: self.goal_id.clone(),
            goal: self.goal.clone(),
        }
        .copy_to_native(message);
    }
}

#[derive(Clone, Debug)]
struct TrackedSendGoal;

unsafe impl WrappedServiceTypeSupport for TrackedSendGoal {
    type Request = TrackedGoalRequest;
    type Response = Fibonacci::SendGoal::Response;
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

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct TrackedResultRequest {
    goal_id: unique_identifier_msgs::msg::UUID,
}

unsafe impl WrappedTypesupport for TrackedResultRequest {
    type CStruct = <Fibonacci::GetResult::Request as WrappedTypesupport>::CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t {
        Fibonacci::GetResult::Request::get_ts()
    }

    fn create_msg() -> *mut Self::CStruct {
        let message = Fibonacci::GetResult::Request::create_msg();
        if !message.is_null() {
            RESULT_REQUEST_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }

    fn destroy_msg(message: *mut Self::CStruct) {
        RESULT_REQUEST_DESTROYED.fetch_add(1, Ordering::Relaxed);
        Fibonacci::GetResult::Request::destroy_msg(message);
    }

    fn from_native(message: &Self::CStruct) -> Self {
        Self {
            goal_id: Fibonacci::GetResult::Request::from_native(message).goal_id,
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        Fibonacci::GetResult::Request {
            goal_id: self.goal_id.clone(),
        }
        .copy_to_native(message);
    }
}

#[derive(Clone, Debug)]
struct TrackedGetResult;

unsafe impl WrappedServiceTypeSupport for TrackedGetResult {
    type Request = TrackedResultRequest;
    type Response = Fibonacci::GetResult::Response;
    #[cfg(any(
        r2r__ros__distro__iron,
        r2r__ros__distro__jazzy,
        r2r__ros__distro__kilted,
        r2r__ros__distro__lyrical,
        r2r__ros__distro__rolling
    ))]
    type Event = Fibonacci::GetResult::Event;

    fn get_ts() -> &'static rosidl_service_type_support_t {
        Fibonacci::GetResult::Service::get_ts()
    }
}

#[derive(Clone, Debug)]
struct TrackedAction;

unsafe impl WrappedActionTypeSupport for TrackedAction {
    type Goal = Fibonacci::Goal;
    type Result = Fibonacci::Result;
    type Feedback = Fibonacci::Feedback;
    type FeedbackMessage = Fibonacci::FeedbackMessage;
    type SendGoal = TrackedSendGoal;
    type GetResult = TrackedGetResult;

    fn get_ts() -> &'static rosidl_action_type_support_t {
        Fibonacci::Action::get_ts()
    }

    fn make_goal_request_msg(
        goal_id: unique_identifier_msgs::msg::UUID, goal: Self::Goal,
    ) -> TrackedGoalRequest {
        TrackedGoalRequest { goal_id, goal }
    }

    fn make_goal_response_msg(
        accepted: bool, stamp: builtin_interfaces::msg::Time,
    ) -> Fibonacci::SendGoal::Response {
        Fibonacci::Action::make_goal_response_msg(accepted, stamp)
    }

    fn make_feedback_msg(
        goal_id: unique_identifier_msgs::msg::UUID, feedback: Self::Feedback,
    ) -> Self::FeedbackMessage {
        Fibonacci::Action::make_feedback_msg(goal_id, feedback)
    }

    fn make_result_request_msg(goal_id: unique_identifier_msgs::msg::UUID) -> TrackedResultRequest {
        TrackedResultRequest { goal_id }
    }

    fn make_result_response_msg(
        status: i8, result: Self::Result,
    ) -> Fibonacci::GetResult::Response {
        Fibonacci::Action::make_result_response_msg(status, result)
    }

    fn destructure_goal_request_msg(
        request: TrackedGoalRequest,
    ) -> (unique_identifier_msgs::msg::UUID, Self::Goal) {
        (request.goal_id, request.goal)
    }

    fn destructure_goal_response_msg(
        response: Fibonacci::SendGoal::Response,
    ) -> (bool, builtin_interfaces::msg::Time) {
        Fibonacci::Action::destructure_goal_response_msg(response)
    }

    fn destructure_feedback_msg(
        message: Self::FeedbackMessage,
    ) -> (unique_identifier_msgs::msg::UUID, Self::Feedback) {
        Fibonacci::Action::destructure_feedback_msg(message)
    }

    fn destructure_result_response_msg(
        response: Fibonacci::GetResult::Response,
    ) -> (i8, Self::Result) {
        Fibonacci::Action::destructure_result_response_msg(response)
    }

    fn destructure_result_request_msg(
        request: TrackedResultRequest,
    ) -> unique_identifier_msgs::msg::UUID {
        request.goal_id
    }
}

fn wait_for_server(node: &mut Node, client: &crate::ActionClient<Fibonacci::Action>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !client.is_available(node).unwrap() {
        assert!(Instant::now() < deadline, "action server discovery timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn complete_goal(
    node: &mut Node, server: &mut ActionServer<TrackedAction>,
    client: &crate::ActionClient<Fibonacci::Action>, order: i32,
) {
    let mut response = Box::pin(
        client
            .send_goal_request_with_options(
                Fibonacci::Goal { order },
                ActionGoalOptions::default().track_feedback(false),
            )
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut server_goal = None;
    let response = loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if server_goal.is_none() {
            if let Some(request) = server.next().now_or_never().flatten() {
                assert_eq!(request.goal.order, order);
                server_goal = Some(request.accept().unwrap().0);
            }
        }
        if let Some(response) = response.as_mut().now_or_never() {
            break response.unwrap();
        }
        assert!(Instant::now() < deadline, "action goal response timed out");
    };
    server_goal
        .unwrap()
        .succeed(Fibonacci::Result {
            sequence: vec![order; 32],
        })
        .unwrap();
    let mut result = Box::pin(response.result.unwrap());
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(result) = result.as_mut().now_or_never() {
            let (status, result) = result.unwrap();
            assert_eq!(status, GoalStatus::Succeeded);
            assert_eq!(result.sequence, [order; 32]);
            return;
        }
        assert!(Instant::now() < deadline, "action result timed out");
    }
}

#[test]
fn typed_action_server_reuses_goal_and_result_request_storage() {
    let _lock = TRACKED_SERVER_LOCK.lock();
    for counter in [
        &GOAL_REQUEST_CREATED,
        &GOAL_REQUEST_DESTROYED,
        &RESULT_REQUEST_CREATED,
        &RESULT_REQUEST_DESTROYED,
    ] {
        counter.store(0, Ordering::Relaxed);
    }

    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "typed_action_server_request_reuse", "").unwrap();
    let action_name =
        format!("/typed_action_server_request_reuse_{}", uuid::Uuid::new_v4().simple());
    let mut server = node
        .create_action_server::<TrackedAction>(&action_name)
        .unwrap();
    let client = node
        .create_action_client::<Fibonacci::Action>(&action_name)
        .unwrap();
    wait_for_server(&mut node, &client);

    complete_goal(&mut node, &mut server, &client, 7);
    assert_eq!(GOAL_REQUEST_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(RESULT_REQUEST_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(GOAL_REQUEST_DESTROYED.load(Ordering::Relaxed), 0);
    assert_eq!(RESULT_REQUEST_DESTROYED.load(Ordering::Relaxed), 0);

    complete_goal(&mut node, &mut server, &client, 11);
    assert_eq!(GOAL_REQUEST_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(RESULT_REQUEST_CREATED.load(Ordering::Relaxed), 1);

    node.destroy_action_server(server).unwrap();
    assert_eq!(GOAL_REQUEST_DESTROYED.load(Ordering::Relaxed), 1);
    assert_eq!(RESULT_REQUEST_DESTROYED.load(Ordering::Relaxed), 1);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual typed action server request native reuse allocation stage; LD_PRELOAD probe required"]
fn benchmark_typed_action_server_request_fresh_vs_reused_native_decode() {
    use crate::allocation_probe;
    const ITERATIONS: usize = 10_000;

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
    let goal_request = Fibonacci::Action::make_goal_request_msg(
        unique_identifier_msgs::msg::UUID { uuid: [7; 16] },
        Fibonacci::Goal { order: 42 },
    );
    let result_request =
        Fibonacci::Action::make_result_request_msg(unique_identifier_msgs::msg::UUID {
            uuid: [7; 16],
        });

    let mut fresh_step = || {
        let goal =
            WrappedNativeMsg::<Fibonacci::SendGoal::Request>::try_from(&goal_request).unwrap();
        let result =
            WrappedNativeMsg::<Fibonacci::GetResult::Request>::try_from(&result_request).unwrap();
        assert_eq!(Fibonacci::SendGoal::Request::from_native(&goal).goal.order, 42);
        assert_eq!(
            Fibonacci::GetResult::Request::from_native(&result)
                .goal_id
                .uuid,
            [7; 16]
        );
        std::hint::black_box((goal, result));
    };
    for _ in 0..1000 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let goal_buffer = NativeBuffer::default();
    let result_buffer = NativeBuffer::default();
    drop(
        goal_buffer
            .checkout(|| WrappedNativeMsg::try_from(&goal_request))
            .unwrap(),
    );
    drop(
        result_buffer
            .checkout(|| WrappedNativeMsg::try_from(&result_request))
            .unwrap(),
    );
    let mut reused_step = || {
        let mut goal = goal_buffer
            .checkout(|| panic!("typed action server goal request slot was not recycled"))
            .unwrap();
        goal_request.try_copy_to_native(&mut goal).unwrap();
        let mut result = result_buffer
            .checkout(|| panic!("typed action server result request slot was not recycled"))
            .unwrap();
        result_request.try_copy_to_native(&mut result).unwrap();
        assert_eq!(Fibonacci::SendGoal::Request::from_native(&goal).goal.order, 42);
        assert_eq!(
            Fibonacci::GetResult::Request::from_native(&result)
                .goal_id
                .uuid,
            [7; 16]
        );
        std::hint::black_box((goal, result));
    };
    for _ in 0..1000 {
        reused_step();
    }
    let (reused, reused_elapsed_ns) = measure(&probe, &mut reused_step);

    let allocations = |stats: &allocation_probe::NativeStats| {
        stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls
    };
    assert!(allocations(&fresh) - allocations(&reused) >= ITERATIONS as u64 * 2);
    assert!(fresh.free_calls - reused.free_calls >= ITERATIONS as u64 * 2);
    assert!(fresh.requested_bytes > reused.requested_bytes);
    for (storage, stats, elapsed_ns) in [
        ("fresh_native", &fresh, fresh_elapsed_ns),
        ("reused_native", &reused, reused_elapsed_ns),
    ] {
        println!(
            "{}",
            serde_json::json!({
                "boundary":"typed action server goal/result request native fill and owned decode; no RCL/DDS",
                "native_storage":storage,
                "iterations":ITERATIONS,
                "elapsed_ns":elapsed_ns,
                "allocations":allocations(stats),
                "frees":stats.free_calls,
                "requested_bytes":stats.requested_bytes
            })
        );
    }
}
