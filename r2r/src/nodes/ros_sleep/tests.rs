use super::*;
use futures::FutureExt;
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    task::Wake,
    time::Instant,
};

fn node(name: &str) -> Node {
    Node::create(Context::create().unwrap(), name, "").unwrap()
}

fn await_sleep(node: &mut Node, sleep: RosSleep) -> Result<()> {
    node.spin_until_future_complete_with_options(
        sleep,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
}

#[test]
fn ros_sleep_immediate_deadlines_allocate_no_native_scheduler_and_validate_overflow() {
    let mut node = node("ros_sleep_immediate");
    let sleep = node.sleep_until_ros(i64::MIN).unwrap();
    assert!(matches!(sleep.now_or_never(), Some(Ok(()))));
    assert!(matches!(
        node.sleep_for_ros(Duration::ZERO).unwrap().now_or_never(),
        Some(Ok(()))
    ));
    assert!(node.ros_sleeps.is_none());
    assert!(node.future_wake_guard.is_none());
    assert!(node.timers.is_empty());
    assert!(matches!(
        node.sleep_for_ros(Duration::MAX),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
}

#[test]
fn ros_sleep_system_backed_deadline_uses_one_native_timer_and_never_finishes_early() {
    let mut node = node("ros_sleep_system");
    let first = node.sleep_for_ros(Duration::from_millis(30)).unwrap();
    let deadline = first.deadline_ns();
    let later = node.sleep_for_ros(Duration::from_millis(60)).unwrap();
    let started = Instant::now();
    await_sleep(&mut node, first).unwrap();
    assert!(node.now_ns().unwrap() >= deadline);
    assert!(started.elapsed() >= Duration::from_millis(20));
    await_sleep(&mut node, later).unwrap();
    assert_eq!(node.wait_set.capacities.unwrap().timers, 1);
    assert!(node.timers.is_empty());
    assert!(node
        .ros_sleeps
        .as_ref()
        .unwrap()
        .shared
        .state
        .lock()
        .deadlines
        .is_empty());
    assert!(node.ros_sleeps.as_ref().unwrap().armed.is_none());
    let preparation = node.prepare_spin(Duration::ZERO).unwrap();
    assert_eq!(
        preparation.capacities.timers, 0,
        "completed sleeps kept canceled timers in native waits"
    );
    node.try_spin_once(Duration::ZERO).unwrap();
}

#[test]
fn ros_sleep_cancellation_removes_deadline_immediately_and_does_not_cancel_reused_slot() {
    fn is_send<T: Send>() {}
    is_send::<RosSleep>();
    let mut node = node("ros_sleep_cancel");
    let mut first = node.sleep_for_ros(Duration::from_secs(30)).unwrap();
    let shared = first.shared.as_ref().unwrap().clone();
    assert!(first.cancel());
    assert!(!first.cancel());
    assert!(shared.state.lock().deadlines.is_empty());
    let later = node.sleep_for_ros(Duration::from_millis(10)).unwrap();
    assert!(matches!((&mut first).now_or_never(), Some(Err(Error::RosSleepCancelled))));
    drop(first);
    assert_eq!(shared.state.lock().deadlines.len(), 1);
    await_sleep(&mut node, later).unwrap();
    let forgotten = node.sleep_for_ros(Duration::from_secs(30)).unwrap();
    drop(forgotten);
    assert!(shared.state.lock().deadlines.is_empty());
}

#[test]
fn ros_sleep_system_head_cancellation_rearms_the_remaining_deadline() {
    let mut node = node("ros_sleep_rearm");
    let mut first = node.sleep_for_ros(Duration::from_secs(30)).unwrap();
    let second = node.sleep_for_ros(Duration::from_secs(60)).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(node.ros_sleeps.as_ref().unwrap().armed, Some(first.deadline_ns()));
    assert!(first.cancel());
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(node.ros_sleeps.as_ref().unwrap().armed, Some(second.deadline_ns()));
    drop(second);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(node.ros_sleeps.as_ref().unwrap().armed.is_none());
    assert_eq!(node.prepare_spin(Duration::ZERO).unwrap().capacities.timers, 0);
}

#[test]
fn ros_sleep_node_drop_and_completion_retire_outside_clock_and_queue_locks() {
    struct Reenter {
        clock: Arc<Mutex<Clock>>,
        context: Context,
        shared: Arc<Shared>,
        count: AtomicUsize,
    }
    impl Wake for Reenter {
        fn wake(self: Arc<Self>) {
            assert!(self.clock.try_lock().is_some(), "ROS sleep wake held clock lock");
            assert!(
                self.context.context_handle.try_lock().is_some(),
                "ROS sleep wake held context lock"
            );
            assert!(self.shared.state.try_lock().is_some(), "ROS sleep wake held deadline lock");
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }
    let mut node = node("ros_sleep_drop");
    let mut sleep = node.sleep_for_ros(Duration::from_secs(30)).unwrap();
    let shared = sleep.shared.as_ref().unwrap().clone();
    let observer = Arc::new(Reenter {
        clock: node.get_ros_clock(),
        context: node.context.clone(),
        shared,
        count: AtomicUsize::new(0),
    });
    let waker = Waker::from(observer.clone());
    assert!(Pin::new(&mut sleep)
        .poll(&mut TaskContext::from_waker(&waker))
        .is_pending());
    drop(node);
    assert_eq!(observer.count.load(Ordering::Relaxed), 1);
    assert!(matches!(sleep.now_or_never(), Some(Err(Error::RCL_RET_NODE_INVALID))));
}

#[test]
fn ros_sleep_context_shutdown_completes_after_context_unlock() {
    const CHILD: &str = "R2R_ROS_SLEEP_SHUTDOWN_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "nodes::ros_sleep::tests::ros_sleep_context_shutdown_completes_after_context_unlock", "--test-threads=1"])
            .env(CHILD, "1").status().unwrap();
        assert!(status.success());
        return;
    }
    struct Reenter(Context);
    impl Wake for Reenter {
        fn wake(self: Arc<Self>) {
            assert!(
                self.0.context_handle.try_lock().is_some(),
                "ROS sleep shutdown wake held context lock"
            );
        }
    }
    let mut node = node("ros_sleep_shutdown");
    let mut sleep = node.sleep_for_ros(Duration::from_secs(30)).unwrap();
    let waker = Waker::from(Arc::new(Reenter(node.context.clone())));
    assert!(Pin::new(&mut sleep)
        .poll(&mut TaskContext::from_waker(&waker))
        .is_pending());
    node.context.shutdown().unwrap();
    assert!(matches!(sleep.now_or_never(), Some(Err(Error::RCL_RET_NOT_INIT))));
    assert!(matches!(node.sleep_for_ros(Duration::ZERO), Err(Error::RCL_RET_NOT_INIT)));
}

#[cfg(r2r__rosgraph_msgs__msg__Clock)]
mod simulated;
