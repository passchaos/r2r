use super::*;
use futures::{FutureExt, StreamExt};
use std::{
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use crate::{example_interfaces::action::Fibonacci, ActionGoalOptions};

static TRACKED_GOAL_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
static GOAL_REQUEST_CREATED: AtomicUsize = AtomicUsize::new(0);
static GOAL_REQUEST_DESTROYED: AtomicUsize = AtomicUsize::new(0);
static GOAL_RESPONSE_CREATED: AtomicUsize = AtomicUsize::new(0);
static GOAL_RESPONSE_DESTROYED: AtomicUsize = AtomicUsize::new(0);
static RESULT_REQUEST_CREATED: AtomicUsize = AtomicUsize::new(0);
static RESULT_REQUEST_DESTROYED: AtomicUsize = AtomicUsize::new(0);
static FAIL_RESULT_REQUEST_COPY: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct TrackedGoal {
    order: i32,
    fail_copy: bool,
}

unsafe impl WrappedTypesupport for TrackedGoal {
    type CStruct = <Fibonacci::Goal as WrappedTypesupport>::CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t {
        Fibonacci::Goal::get_ts()
    }

    fn create_msg() -> *mut Self::CStruct {
        Fibonacci::Goal::create_msg()
    }

    fn destroy_msg(message: *mut Self::CStruct) {
        Fibonacci::Goal::destroy_msg(message);
    }

    fn from_native(message: &Self::CStruct) -> Self {
        Self {
            order: Fibonacci::Goal::from_native(message).order,
            fail_copy: false,
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        Fibonacci::Goal { order: self.order }.copy_to_native(message);
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct TrackedGoalRequest {
    goal_id: unique_identifier_msgs::msg::UUID,
    goal: TrackedGoal,
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
            goal: TrackedGoal {
                order: request.goal.order,
                fail_copy: false,
            },
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        self.try_copy_to_native(message)
            .expect("tracked action goal request copy failed");
    }

    fn try_copy_to_native(&self, message: &mut Self::CStruct) -> Result<()> {
        if self.goal.fail_copy {
            Err(Error::RCL_RET_ERROR)
        } else {
            Fibonacci::SendGoal::Request {
                goal_id: self.goal_id.clone(),
                goal: Fibonacci::Goal {
                    order: self.goal.order,
                },
            }
            .try_copy_to_native(message)
        }
    }
}

#[derive(Clone, Debug)]
struct TrackedSendGoal;

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
        let response = Fibonacci::SendGoal::Response::from_native(message);
        Self {
            accepted: response.accepted,
            stamp: response.stamp,
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        Fibonacci::SendGoal::Response {
            accepted: self.accepted,
            stamp: self.stamp.clone(),
        }
        .copy_to_native(message);
    }
}

unsafe impl WrappedServiceTypeSupport for TrackedSendGoal {
    type Request = TrackedGoalRequest;
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

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
struct TrackedResultRequest {
    goal_id: unique_identifier_msgs::msg::UUID,
    fail_copy: bool,
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
            fail_copy: false,
        }
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        self.try_copy_to_native(message)
            .expect("tracked action result request copy failed");
    }

    fn try_copy_to_native(&self, message: &mut Self::CStruct) -> Result<()> {
        if self.fail_copy {
            Err(Error::RCL_RET_ERROR)
        } else {
            Fibonacci::GetResult::Request {
                goal_id: self.goal_id.clone(),
            }
            .try_copy_to_native(message)
        }
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
    type Goal = TrackedGoal;
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
    ) -> TrackedGoalResponse {
        TrackedGoalResponse { accepted, stamp }
    }

    fn make_feedback_msg(
        goal_id: unique_identifier_msgs::msg::UUID, feedback: Self::Feedback,
    ) -> Self::FeedbackMessage {
        Fibonacci::Action::make_feedback_msg(goal_id, feedback)
    }

    fn make_result_request_msg(goal_id: unique_identifier_msgs::msg::UUID) -> TrackedResultRequest {
        TrackedResultRequest {
            goal_id,
            fail_copy: FAIL_RESULT_REQUEST_COPY.swap(false, Ordering::Relaxed),
        }
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
        response: TrackedGoalResponse,
    ) -> (bool, builtin_interfaces::msg::Time) {
        (response.accepted, response.stamp)
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

fn wait_for_server(node: &mut Node, client: &ActionClient<TrackedAction>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !client.is_available(node).unwrap() {
        assert!(Instant::now() < deadline, "action server discovery timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn send_and_reject(
    node: &mut Node, server: &mut crate::ActionServer<Fibonacci::Action>,
    client: &ActionClient<TrackedAction>, order: i32,
) {
    let mut response = Box::pin(
        client
            .send_goal_request_with_options(
                TrackedGoal {
                    order,
                    fail_copy: false,
                },
                ActionGoalOptions::fire_and_forget(),
            )
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut rejected = false;
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if !rejected {
            if let Some(request) = server.next().now_or_never().flatten() {
                assert_eq!(request.goal.order, order);
                request.reject().unwrap();
                rejected = true;
            }
        }
        if let Some(response) = response.as_mut().now_or_never() {
            assert!(matches!(response, Err(Error::RCL_RET_ACTION_GOAL_REJECTED)));
            return;
        }
        assert!(Instant::now() < deadline, "action goal response timed out");
    }
}

#[test]
fn typed_action_client_reuses_goal_request_storage_and_recovers_from_copy_failure() {
    let _lock = TRACKED_GOAL_LOCK.lock();
    GOAL_REQUEST_CREATED.store(0, Ordering::Relaxed);
    GOAL_REQUEST_DESTROYED.store(0, Ordering::Relaxed);
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "typed_action_goal_request_reuse", "").unwrap();
    let action_name = format!("/typed_action_goal_request_reuse_{}", uuid::Uuid::new_v4().simple());
    let mut server = node
        .create_action_server::<Fibonacci::Action>(&action_name)
        .unwrap();
    let client = node
        .create_action_client::<TrackedAction>(&action_name)
        .unwrap();
    let client_clone = client.clone();
    wait_for_server(&mut node, &client);

    send_and_reject(&mut node, &mut server, &client, 7);
    assert_eq!(GOAL_REQUEST_CREATED.load(Ordering::Relaxed), 1);
    send_and_reject(&mut node, &mut server, &client_clone, 11);
    assert_eq!(GOAL_REQUEST_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(GOAL_REQUEST_DESTROYED.load(Ordering::Relaxed), 0);

    assert!(matches!(
        client.send_goal_request_with_options(
            TrackedGoal {
                order: 13,
                fail_copy: true,
            },
            ActionGoalOptions::fire_and_forget(),
        ),
        Err(Error::RCL_RET_ERROR)
    ));
    assert_eq!(GOAL_REQUEST_DESTROYED.load(Ordering::Relaxed), 1);

    send_and_reject(&mut node, &mut server, &client_clone, 17);
    assert_eq!(GOAL_REQUEST_CREATED.load(Ordering::Relaxed), 2);
    node.destroy_action_client(client).unwrap();
    assert_eq!(GOAL_REQUEST_DESTROYED.load(Ordering::Relaxed), 1);
    drop(client_clone);
    assert_eq!(GOAL_REQUEST_DESTROYED.load(Ordering::Relaxed), 2);
}

fn complete_tracked_goal(
    node: &mut Node, server: &mut crate::ActionServer<Fibonacci::Action>,
    client: &ActionClient<TrackedAction>, order: i32,
) -> Vec<i32> {
    let mut response = Box::pin(
        client
            .send_goal_request_with_options(
                TrackedGoal {
                    order,
                    fail_copy: false,
                },
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
            return result.sequence;
        }
        assert!(Instant::now() < deadline, "action result timed out");
    }
}

fn reject_result_request_copy(
    node: &mut Node, server: &mut crate::ActionServer<Fibonacci::Action>,
    client: &ActionClient<TrackedAction>, order: i32,
) {
    FAIL_RESULT_REQUEST_COPY.store(true, Ordering::Relaxed);
    let mut response = Box::pin(
        client
            .send_goal_request_with_options(
                TrackedGoal {
                    order,
                    fail_copy: false,
                },
                ActionGoalOptions::default().track_feedback(false),
            )
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut server_goal = None;
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if server_goal.is_none() {
            if let Some(request) = server.next().now_or_never().flatten() {
                assert_eq!(request.goal.order, order);
                server_goal = Some(request.accept().unwrap().0);
            }
        }
        if let Some(response) = response.as_mut().now_or_never() {
            assert!(matches!(response, Err(Error::RCL_RET_ERROR)));
            server_goal
                .unwrap()
                .abort(Fibonacci::Result::default())
                .unwrap();
            return;
        }
        assert!(Instant::now() < deadline, "failed result request did not complete");
    }
}

fn lifecycle_native_addresses(client: &ActionClient<TrackedAction>) -> (usize, usize) {
    let result_request = client
        .result_request_buffer
        .inspect_available(WrappedNativeMsg::void_ptr)
        .expect("result request buffer should be returned after send")
        as usize;
    let state = client.client.upgrade().unwrap();
    let goal_response = state
        .lock()
        .goal_response_native_buffer
        .inspect_available(WrappedNativeMsg::void_ptr)
        .expect("goal response buffer should be returned after detached decode")
        as usize;
    (result_request, goal_response)
}

#[test]
fn typed_action_client_reuses_result_request_and_goal_response_storage() {
    let _lock = TRACKED_GOAL_LOCK.lock();
    for counter in [
        &GOAL_REQUEST_CREATED,
        &GOAL_REQUEST_DESTROYED,
        &GOAL_RESPONSE_CREATED,
        &GOAL_RESPONSE_DESTROYED,
        &RESULT_REQUEST_CREATED,
        &RESULT_REQUEST_DESTROYED,
    ] {
        counter.store(0, Ordering::Relaxed);
    }
    FAIL_RESULT_REQUEST_COPY.store(false, Ordering::Relaxed);

    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "typed_action_lifecycle_native_reuse", "").unwrap();
    let action_name =
        format!("/typed_action_lifecycle_native_reuse_{}", uuid::Uuid::new_v4().simple());
    let mut server = node
        .create_action_server::<Fibonacci::Action>(&action_name)
        .unwrap();
    let client = node
        .create_action_client::<TrackedAction>(&action_name)
        .unwrap();
    let client_clone = client.clone();
    wait_for_server(&mut node, &client);

    assert_eq!(complete_tracked_goal(&mut node, &mut server, &client, 7), [7; 32]);
    let first_addresses = lifecycle_native_addresses(&client);
    assert_eq!(GOAL_RESPONSE_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(RESULT_REQUEST_CREATED.load(Ordering::Relaxed), 1);

    assert_eq!(complete_tracked_goal(&mut node, &mut server, &client_clone, 11), [11; 32]);
    assert_eq!(lifecycle_native_addresses(&client_clone), first_addresses);
    assert_eq!(GOAL_RESPONSE_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(RESULT_REQUEST_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(RESULT_REQUEST_DESTROYED.load(Ordering::Relaxed), 0);

    reject_result_request_copy(&mut node, &mut server, &client, 13);
    assert_eq!(GOAL_RESPONSE_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(RESULT_REQUEST_DESTROYED.load(Ordering::Relaxed), 1);

    assert_eq!(complete_tracked_goal(&mut node, &mut server, &client_clone, 17), [17; 32]);
    assert_eq!(GOAL_RESPONSE_CREATED.load(Ordering::Relaxed), 1);
    assert_eq!(RESULT_REQUEST_CREATED.load(Ordering::Relaxed), 2);
    assert_eq!(lifecycle_native_addresses(&client_clone).1, first_addresses.1);

    node.destroy_action_client(client).unwrap();
    assert_eq!(GOAL_RESPONSE_DESTROYED.load(Ordering::Relaxed), 1);
    assert_eq!(RESULT_REQUEST_DESTROYED.load(Ordering::Relaxed), 1);
    drop(client_clone);
    assert_eq!(RESULT_REQUEST_DESTROYED.load(Ordering::Relaxed), 2);
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual typed action result-request/goal-response native reuse allocation stage; LD_PRELOAD probe required"]
fn benchmark_typed_action_lifecycle_fresh_vs_reused_native_storage() {
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
    let result_request =
        Fibonacci::Action::make_result_request_msg(unique_identifier_msgs::msg::UUID {
            uuid: [7; 16],
        });
    let goal_response = Fibonacci::Action::make_goal_response_msg(
        true,
        builtin_interfaces::msg::Time {
            sec: 42,
            nanosec: 17,
        },
    );

    let mut fresh_step = || {
        let request =
            WrappedNativeMsg::<Fibonacci::GetResult::Request>::try_from(&result_request).unwrap();
        let response =
            WrappedNativeMsg::<Fibonacci::SendGoal::Response>::try_from(&goal_response).unwrap();
        assert_eq!(request.goal_id.uuid, [7; 16]);
        let decoded = Fibonacci::SendGoal::Response::from_native(&response);
        assert!(decoded.accepted);
        assert_eq!(decoded.stamp.sec, 42);
        std::hint::black_box((request, decoded));
    };
    for _ in 0..1000 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let request_buffer = NativeBuffer::default();
    let response_buffer = NativeBuffer::default();
    drop(
        request_buffer
            .checkout(|| WrappedNativeMsg::try_from(&result_request))
            .unwrap(),
    );
    drop(
        response_buffer
            .checkout(|| WrappedNativeMsg::try_from(&goal_response))
            .unwrap(),
    );
    let mut reused_step = || {
        let mut request = request_buffer
            .checkout(|| panic!("typed action result request slot was not recycled"))
            .unwrap();
        result_request.try_copy_to_native(&mut request).unwrap();
        let mut response = response_buffer
            .checkout(|| panic!("typed action goal response slot was not recycled"))
            .unwrap();
        goal_response.try_copy_to_native(&mut response).unwrap();
        assert_eq!(request.goal_id.uuid, [7; 16]);
        let decoded = Fibonacci::SendGoal::Response::from_native(&response);
        drop((request, response));
        assert!(decoded.accepted);
        assert_eq!(decoded.stamp.sec, 42);
        std::hint::black_box(decoded);
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
                "boundary":"typed action result-request preparation plus goal-response fill/decode; no RCL/DDS",
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

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual typed action goal request native reuse allocation stage; LD_PRELOAD probe required"]
fn benchmark_typed_action_goal_request_fresh_vs_reused_native_preparation() {
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
    let source = Fibonacci::Action::make_goal_request_msg(
        unique_identifier_msgs::msg::UUID { uuid: [7; 16] },
        Fibonacci::Goal { order: 42 },
    );

    let mut fresh_step = || {
        let native = WrappedNativeMsg::<Fibonacci::SendGoal::Request>::try_from(&source).unwrap();
        assert_eq!(native.goal.order, 42);
        std::hint::black_box(native.goal_id.uuid);
    };
    for _ in 0..1000 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let buffer = NativeBuffer::default();
    drop(
        buffer
            .checkout(|| WrappedNativeMsg::<Fibonacci::SendGoal::Request>::try_from(&source))
            .unwrap(),
    );
    let mut reused_step = || {
        let mut native = buffer
            .checkout(|| panic!("typed action goal request slot was not recycled"))
            .unwrap();
        source.try_copy_to_native(&mut native).unwrap();
        assert_eq!(native.goal.order, 42);
        std::hint::black_box(native.goal_id.uuid);
    };
    for _ in 0..1000 {
        reused_step();
    }
    let (reused, reused_elapsed_ns) = measure(&probe, &mut reused_step);

    let allocations = |stats: &allocation_probe::NativeStats| {
        stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls
    };
    assert!(allocations(&fresh) - allocations(&reused) >= ITERATIONS as u64);
    assert!(fresh.free_calls - reused.free_calls >= ITERATIONS as u64);
    assert!(fresh.requested_bytes > reused.requested_bytes);
    for (storage, stats, elapsed_ns) in [
        ("fresh_native", &fresh, fresh_elapsed_ns),
        ("reused_native", &reused, reused_elapsed_ns),
    ] {
        println!(
            "{}",
            serde_json::json!({
                "boundary":"typed action goal request Rust-to-native preparation and validation; no RCL/DDS",
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
