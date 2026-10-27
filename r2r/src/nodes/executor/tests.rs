use super::*;
use futures::{FutureExt, StreamExt};
use std::{cell::Cell, rc::Rc, sync::atomic::Ordering, thread, time::Instant};

mod workflows;

fn node(context: &Context, name: &str) -> Node {
    Node::create_with_options(
        context.clone(),
        name,
        "",
        NodeOptions::default().enable_rosout(false),
    )
    .unwrap()
}

fn guard(executor: &mut Executor) -> Arc<FutureGuard> {
    let guard = FutureGuard::new(&executor.context).unwrap();
    executor.future_wake_guard = Some(guard.clone());
    guard
}

fn wait_for_native_wait(guard: &FutureGuard) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !guard.waiting.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline, "executor did not enter native wait");
        thread::yield_now();
    }
}

#[test]
fn idle_nodes_do_not_add_sequential_waits_before_a_ready_timer() {
    let context = Context::create().unwrap();
    let idle = node(&context, "executor_idle_first");
    let mut ready = node(&context, "executor_timer_last");
    let mut timer = ready.create_wall_timer(Duration::from_millis(10)).unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(idle).unwrap();
    executor.add_node(ready).unwrap();
    let guard = guard(&mut executor);
    let started = Instant::now();
    executor.spin_once(Duration::from_secs(2)).unwrap();
    timer.tick().now_or_never().unwrap().unwrap();
    assert!(started.elapsed() < Duration::from_millis(500));
    assert_eq!(guard.wait_timeout_ns.load(Ordering::Acquire), 2_000_000_000);
    assert_eq!(guard.wait_return.load(Ordering::Acquire), RCL_RET_OK as i32);
}

#[test]
fn empty_executor_waits_for_external_wakes_and_keeps_non_send_futures_local() {
    let context = Context::create().unwrap();
    let mut executor = Executor::new(context);
    let guard = guard(&mut executor);
    let observation = guard.clone();
    let (sender, receiver) = oneshot::channel();
    let producer = thread::spawn(move || {
        wait_for_native_wait(&observation);
        sender.send(41).unwrap();
    });
    let local = Rc::new(Cell::new(0));
    let result = executor
        .spin_until_future_complete_with_options(
            async {
                local.set(receiver.await.map_err(|_| Error::RCL_RET_ERROR)? + 1);
                Ok(local.get())
            },
            Duration::from_secs(2),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    producer.join().unwrap();
    assert_eq!(result, 42);
    assert!(guard.wait_timeout_ns.load(Ordering::Acquire) > 10_000_000);
    assert_eq!(guard.wait_return.load(Ordering::Acquire), RCL_RET_OK as i32);
    assert_eq!(guard.triggers.load(Ordering::Acquire), 1);
}

#[test]
fn ready_and_zero_deadline_futures_do_not_create_native_storage() {
    let mut executor = Executor::new(Context::create().unwrap());
    assert_eq!(
        executor
            .spin_until_future_complete(async { Ok(17) }, Duration::ZERO)
            .unwrap(),
        17
    );
    let result = executor
        .spin_until_future_complete(futures::future::pending::<Result<()>>(), Duration::ZERO);
    assert!(matches!(result, Err(Error::RCL_RET_TIMEOUT)));
    assert!(executor.future_wake_guard.is_none());
    assert!(executor.wait_set.capacities.is_none());
}

#[test]
fn node_ids_survive_swap_removal_and_returned_nodes_can_spin_independently() {
    let context = Context::create().unwrap();
    let mut executor = Executor::new(context.clone());
    let first = executor
        .add_node(node(&context, "executor_remove_first"))
        .unwrap();
    let last = executor
        .add_node(node(&context, "executor_remove_last"))
        .unwrap();
    executor.spin_once(Duration::ZERO).unwrap();
    let mut removed = executor.remove_node(first).unwrap();
    assert_eq!(executor.len(), 1);
    assert!(executor.node_mut(first).is_err());
    assert_eq!(executor.node_mut(last).unwrap().name().unwrap(), "executor_remove_last");
    removed.try_spin_once(Duration::ZERO).unwrap();
    let restored = executor.add_node(removed).unwrap();
    assert_ne!(first, restored);
    executor.remove_node(last).unwrap();
    executor.remove_node(restored).unwrap();
    assert!(executor.is_empty());
}

#[test]
fn dropped_endpoints_resize_the_shared_wait_set_before_waiting() {
    let context = Context::create().unwrap();
    let mut first = node(&context, "executor_cleanup_first");
    let mut last = node(&context, "executor_cleanup_last");
    let topic = format!("/executor_cleanup_{}", uuid::Uuid::new_v4().simple());
    let subscription = first
        .subscribe::<crate::std_msgs::msg::String>(&topic, QosProfile::default())
        .unwrap();
    let timer = last.create_wall_timer(Duration::from_secs(10)).unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(first).unwrap();
    executor.add_node(last).unwrap();
    executor.spin_once(Duration::ZERO).unwrap();
    assert_eq!(executor.wait_set.capacities.unwrap().subscriptions, 1);
    assert_eq!(executor.wait_set.capacities.unwrap().timers, 1);
    drop(subscription);
    drop(timer);
    executor.spin_once(Duration::ZERO).unwrap();
    assert_eq!(executor.wait_set.capacities.unwrap().subscriptions, 0);
    assert_eq!(executor.wait_set.capacities.unwrap().timers, 0);
}

#[test]
fn timed_out_wakers_are_disarmed_and_do_not_retain_the_executor_guard() {
    let mut executor = Executor::new(Context::create().unwrap());
    let saved = Arc::new(Mutex::new(None));
    let capture = saved.clone();
    let pending = futures::future::poll_fn(move |context| {
        *capture.lock() = Some(context.waker().clone());
        std::task::Poll::Pending::<Result<()>>
    });
    let result = executor.spin_until_future_complete_with_options(
        pending,
        Duration::from_millis(20),
        FutureExecutionOptions::wake_driven(),
    );
    assert!(matches!(result, Err(Error::RCL_RET_TIMEOUT)));
    let guard = executor.future_wake_guard.as_ref().unwrap();
    let triggers = guard.triggers.load(Ordering::Acquire);
    saved.lock().as_ref().unwrap().wake_by_ref();
    assert_eq!(guard.triggers.load(Ordering::Acquire), triggers);
    let weak = Arc::downgrade(guard);
    drop(executor);
    assert!(weak.upgrade().is_none());
}

#[test]
fn pending_ack_queries_bound_the_shared_wait_without_extra_events() {
    let context = Context::create().unwrap();
    let mut first = node(&context, "executor_ack_first");
    let publisher = first
        .create_publisher::<crate::std_msgs::msg::String>("executor_ack", QosProfile::default())
        .unwrap();
    let state = publisher.handle.upgrade().unwrap();
    state
        .ack_query_override
        .store(RCL_RET_TIMEOUT as i32, Ordering::Release);
    let acked = publisher.wait_for_all_acked_async().unwrap();
    let mut executor = Executor::new(context.clone());
    executor.add_node(first).unwrap();
    executor
        .add_node(node(&context, "executor_ack_idle"))
        .unwrap();
    let guard = guard(&mut executor);
    let observation = guard.clone();
    let producer = thread::spawn(move || {
        wait_for_native_wait(&observation);
        assert!((0..=10_000_000).contains(&observation.wait_timeout_ns.load(Ordering::Acquire)));
        state
            .ack_query_override
            .store(RCL_RET_OK as i32, Ordering::Release);
    });
    executor
        .spin_until_future_complete_with_options(
            acked,
            Duration::from_secs(1),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    producer.join().unwrap();
}

#[test]
fn shutdown_interrupts_an_empty_executor_native_wait() {
    const MARKER: &str = "R2R_EXECUTOR_SHUTDOWN_CHILD";
    if std::env::var_os(MARKER).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "nodes::executor::tests::shutdown_interrupts_an_empty_executor_native_wait",
                "--test-threads=1",
            ])
            .env(MARKER, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let context = Context::create().unwrap();
    let mut executor = Executor::new(context.clone());
    let guard = guard(&mut executor);
    let producer = thread::spawn(move || {
        wait_for_native_wait(&guard);
        context.shutdown().unwrap();
    });
    let result = executor.spin_once(Duration::from_secs(2));
    producer.join().unwrap();
    assert!(matches!(result, Err(Error::RCL_RET_NOT_INIT)));
}

#[test]
fn capacity_overflow_returns_an_error_before_native_resize() {
    let mut target = WaitSetCapacities {
        subscriptions: usize::MAX,
        ..Default::default()
    };
    let result = add_capacities(
        &mut target,
        WaitSetCapacities {
            subscriptions: 1,
            ..Default::default()
        },
    );
    assert!(matches!(result, Err(Error::RCL_RET_INVALID_ARGUMENT)));
}
