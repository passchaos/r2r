use super::*;
use futures::{FutureExt, StreamExt};
use std::time::{Duration, Instant};

use crate::{example_interfaces::action::Fibonacci, ActionGoalOptions};

fn wait_for_server(node: &mut Node, client: &ActionClient<Fibonacci::Action>) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !client.is_available(node).unwrap() {
        assert!(Instant::now() < deadline, "action server discovery timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn complete_goal(
    node: &mut Node, server: &mut crate::ActionServer<Fibonacci::Action>,
    client: &ActionClient<Fibonacci::Action>, order: i32, elements: usize,
) -> Vec<i32> {
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
        assert!(Instant::now() < deadline, "goal response timed out");
    };
    server_goal
        .unwrap()
        .succeed(Fibonacci::Result {
            sequence: vec![order; elements],
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

fn cached_address(client: &ActionClient<Fibonacci::Action>) -> usize {
    let state = client.client.upgrade().unwrap();
    let address = state
        .lock()
        .result_response_native_buffer
        .inspect_available(WrappedNativeMsg::void_ptr)
        .expect("result response buffer should be returned after detached decode");
    address as usize
}

#[test]
fn typed_action_client_reuses_result_response_storage_behind_owned_results() {
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "typed_action_result_response_reuse", "").unwrap();
    let action_name =
        format!("/typed_action_result_response_reuse_{}", uuid::Uuid::new_v4().simple());
    let mut server = node
        .create_action_server::<Fibonacci::Action>(&action_name)
        .unwrap();
    let client = node
        .create_action_client::<Fibonacci::Action>(&action_name)
        .unwrap();
    let stale_clone = client.clone();
    wait_for_server(&mut node, &client);

    let first = complete_goal(&mut node, &mut server, &client, 7, 4096);
    let first_address = cached_address(&client);
    assert_eq!(first, [7; 4096]);

    let second = complete_goal(&mut node, &mut server, &client, 11, 64);
    let second_address = cached_address(&client);
    assert_eq!(second_address, first_address);
    assert_eq!(first, [7; 4096]);
    assert_eq!(second, [11; 64]);

    node.destroy_action_client(client).unwrap();
    assert!(matches!(
        stale_clone.is_available(&node),
        Err(Error::RCL_RET_ACTION_CLIENT_INVALID)
    ));
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual typed action result response native reuse allocation stage; LD_PRELOAD probe required"]
fn benchmark_typed_action_result_response_fresh_vs_reused_native_decode() {
    use crate::allocation_probe;
    const ITERATIONS: usize = 10_000;
    const ELEMENTS: usize = 4096;

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
    let source = Fibonacci::Action::make_result_response_msg(
        action_msgs::msg::GoalStatus::STATUS_SUCCEEDED as i8,
        Fibonacci::Result {
            sequence: vec![42; ELEMENTS],
        },
    );

    let mut fresh_step = || {
        let native = WrappedNativeMsg::<Fibonacci::GetResult::Response>::try_from(&source).unwrap();
        let decoded = Fibonacci::GetResult::Response::from_native(&native);
        let (status, result) = Fibonacci::Action::destructure_result_response_msg(decoded);
        assert_eq!(status, action_msgs::msg::GoalStatus::STATUS_SUCCEEDED as i8);
        assert_eq!(result.sequence.len(), ELEMENTS);
        std::hint::black_box(result.sequence);
    };
    for _ in 0..1000 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let buffer = NativeBuffer::default();
    drop(
        buffer
            .checkout(|| WrappedNativeMsg::<Fibonacci::GetResult::Response>::try_from(&source))
            .unwrap(),
    );
    let mut reused_step = || {
        let mut native = buffer
            .checkout(|| panic!("typed action result response slot was not recycled"))
            .unwrap();
        source.try_copy_to_native(&mut native).unwrap();
        let decoded = Fibonacci::GetResult::Response::from_native(&native);
        drop(native);
        let (status, result) = Fibonacci::Action::destructure_result_response_msg(decoded);
        assert_eq!(status, action_msgs::msg::GoalStatus::STATUS_SUCCEEDED as i8);
        assert_eq!(result.sequence.len(), ELEMENTS);
        std::hint::black_box(result.sequence);
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
    assert!(
        fresh.requested_bytes - reused.requested_bytes
            >= ITERATIONS as u64 * ELEMENTS as u64 * std::mem::size_of::<i32>() as u64
    );
    for (storage, stats, elapsed_ns) in [
        ("fresh_native", &fresh, fresh_elapsed_ns),
        ("reused_native", &reused, reused_elapsed_ns),
    ] {
        println!(
            "{}",
            serde_json::json!({
                "boundary":"typed action result response native fill, owned decode and validation; no RCL/DDS",
                "native_storage":storage,
                "iterations":ITERATIONS,
                "elements":ELEMENTS,
                "elapsed_ns":elapsed_ns,
                "allocations":allocations(stats),
                "frees":stats.free_calls,
                "requested_bytes":stats.requested_bytes
            })
        );
    }
}
