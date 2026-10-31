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
    client: &ActionClient<Fibonacci::Action>, order: i32,
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
        assert!(Instant::now() < deadline, "goal response timed out");
    };
    server_goal
        .unwrap()
        .succeed(Fibonacci::Result {
            sequence: vec![order],
        })
        .unwrap();
    let mut result = Box::pin(response.result.unwrap());
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(result) = result.as_mut().now_or_never() {
            let (status, result) = result.unwrap();
            assert_eq!(status, GoalStatus::Succeeded);
            assert_eq!(result.sequence, [order]);
            break;
        }
        assert!(Instant::now() < deadline, "action result timed out");
    }
}

fn wait_for_new_status_take(
    node: &mut Node, client: &ActionClient<Fibonacci::Action>, previous: usize,
) -> usize {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if typed_status_native_take_count() > previous {
            let state = client.client.upgrade().unwrap();
            let address = state
                .lock()
                .status_native_buffer
                .inspect_available(WrappedNativeMsg::void_ptr)
                .expect("status buffer should be returned after detached decode");
            return address as usize;
        }
        assert!(Instant::now() < deadline, "status message timed out");
    }
}

#[test]
fn typed_action_client_reuses_status_take_storage_across_goal_lifecycles() {
    reset_typed_status_native_take_count();
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "typed_action_status_reuse", "").unwrap();
    let action_name = format!("/typed_action_status_reuse_{}", uuid::Uuid::new_v4().simple());
    let mut server = node
        .create_action_server::<Fibonacci::Action>(&action_name)
        .unwrap();
    let client = node
        .create_action_client::<Fibonacci::Action>(&action_name)
        .unwrap();
    let stale_clone = client.clone();
    wait_for_server(&mut node, &client);

    let before = typed_status_native_take_count();
    complete_goal(&mut node, &mut server, &client, 7);
    let first_address = wait_for_new_status_take(&mut node, &client, before);

    let before = typed_status_native_take_count();
    complete_goal(&mut node, &mut server, &client, 11);
    let second_address = wait_for_new_status_take(&mut node, &client, before);
    assert_eq!(second_address, first_address);

    node.destroy_action_client(client).unwrap();
    assert!(matches!(
        stale_clone.is_available(&node),
        Err(Error::RCL_RET_ACTION_CLIENT_INVALID)
    ));
}

fn fixture(count: usize) -> action_msgs::msg::GoalStatusArray {
    action_msgs::msg::GoalStatusArray {
        status_list: (0..count)
            .map(|index| action_msgs::msg::GoalStatus {
                goal_info: action_msgs::msg::GoalInfo {
                    goal_id: unique_identifier_msgs::msg::UUID {
                        uuid: *uuid::Uuid::from_u128(index as u128 + 1).as_bytes(),
                    },
                    stamp: builtin_interfaces::msg::Time {
                        sec: index as i32,
                        nanosec: 123,
                    },
                },
                status: [0, 1, 2, 3, 4, 5, 6][index % 7],
            })
            .collect(),
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual typed action status native reuse allocation stage; LD_PRELOAD probe required"]
fn benchmark_typed_action_status_fresh_vs_reused_native_take_decode() {
    use crate::allocation_probe;
    const ITERATIONS: usize = 10_000;
    const STATUSES: usize = 128;

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
    let source = fixture(STATUSES);

    let mut fresh_step = || {
        let native =
            WrappedNativeMsg::<action_msgs::msg::GoalStatusArray>::try_from(&source).unwrap();
        let updates = status::decode_native(&native).unwrap();
        assert_eq!(updates.len(), STATUSES);
        std::hint::black_box(updates);
    };
    for _ in 0..1000 {
        fresh_step();
    }
    let (fresh, fresh_elapsed_ns) = measure(&probe, &mut fresh_step);

    let buffer = NativeBuffer::default();
    drop(
        buffer
            .checkout(|| WrappedNativeMsg::<action_msgs::msg::GoalStatusArray>::try_from(&source))
            .unwrap(),
    );
    let mut reused_step = || {
        let mut native = buffer
            .checkout(|| panic!("typed action status slot was not recycled"))
            .unwrap();
        source.try_copy_to_native(&mut native).unwrap();
        let updates = status::decode_native(&native).unwrap();
        drop(native);
        assert_eq!(updates.len(), STATUSES);
        std::hint::black_box(updates);
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
                "boundary":"typed action status native fill, direct decode and validation; no RCL/DDS",
                "native_storage":storage,
                "iterations":ITERATIONS,
                "statuses":STATUSES,
                "elapsed_ns":elapsed_ns,
                "allocations":allocations(stats),
                "frees":stats.free_calls,
                "requested_bytes":stats.requested_bytes
            })
        );
    }
}
