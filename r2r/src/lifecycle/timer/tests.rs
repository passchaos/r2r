use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;

use futures::{
    task::{waker_ref, ArcWake},
    Future, FutureExt,
};

use super::*;
use crate::{CallbackReturn, ClockType, Context, Error, FutureExecutionOptions};

fn managed(name: &str) -> LifecycleNode {
    LifecycleNode::create(
        Context::create().unwrap(),
        &format!("{name}_{}", uuid::Uuid::new_v4().simple()),
        "",
    )
    .unwrap()
}

fn simulated(name: &str) -> LifecycleNode {
    let node = managed(name);
    node.get_ros_clock()
        .lock()
        .enable_ros_time_override(0)
        .unwrap();
    node
}

fn set_time(node: &LifecycleNode, milliseconds: i64) {
    node.get_ros_clock()
        .lock()
        .set_ros_time_override(milliseconds * 1_000_000)
        .unwrap();
}

fn spin(node: &mut LifecycleNode) {
    node.try_spin_once(Duration::ZERO).unwrap();
}

fn activate(node: &mut LifecycleNode) {
    assert_eq!(node.configure().unwrap(), CallbackReturn::Success);
    assert_eq!(node.activate().unwrap(), CallbackReturn::Success);
    spin(node);
}

#[test]
fn inactive_native_timer_does_not_call_when_ros_time_is_overdue() {
    let mut node = simulated("lc_timer_inactive");
    let mut timer = node.create_timer(Duration::from_millis(10)).unwrap();
    assert_eq!(timer.clock_type(), ClockType::RosTime);
    assert!(!timer.is_steady());
    set_time(&node, 100);
    spin(&mut node);
    assert!(timer
        .state
        .with_timer(|ordinary| ordinary.is_canceled(&node))
        .unwrap());
    assert_eq!(timer.time_since_last_call_ns(&node).unwrap(), 100_000_000);
    assert!(timer.tick().now_or_never().is_none());
    node.configure().unwrap();
    set_time(&node, 200);
    spin(&mut node);
    assert_eq!(timer.time_since_last_call_ns(&node).unwrap(), 200_000_000);
    assert!(timer.tick().now_or_never().is_none());
}

#[test]
fn activation_and_between_spin_reactivation_discard_old_ticks_and_restart_period() {
    let mut node = simulated("lc_timer_generations");
    let mut timer = node.create_timer(Duration::from_millis(10)).unwrap();
    activate(&mut node);
    assert!(!timer.is_canceled(&node).unwrap());
    set_time(&node, 9);
    spin(&mut node);
    assert!(timer.tick().now_or_never().is_none());
    set_time(&node, 10);
    spin(&mut node);
    assert_eq!(timer.tick().now_or_never().unwrap().unwrap(), Duration::from_millis(10));
    set_time(&node, 20);
    spin(&mut node); // Leave a tick in the native timer's Rust queue.
    node.deactivate().unwrap();
    node.activate().unwrap(); // No spin between these transitions.
    assert!(timer.tick().now_or_never().is_none());
    assert!(timer.is_canceled(&node).unwrap());
    assert!(!timer.is_ready(&node).unwrap());
    assert!(matches!(timer.next_call_time_ns(&node), Err(Error::RCL_RET_TIMER_CANCELED)));
    spin(&mut node); // Reconcile the latest generation and reset at t=20ms.
    assert_eq!(timer.next_call_time_ns(&node).unwrap(), 30_000_000);
    assert!(timer.tick().now_or_never().is_none());
    set_time(&node, 29);
    spin(&mut node);
    assert!(timer.tick().now_or_never().is_none());
    set_time(&node, 30);
    spin(&mut node);
    assert_eq!(timer.tick().now_or_never().unwrap().unwrap(), Duration::from_millis(10));
}

#[test]
fn manual_reset_discards_buffered_ticks_and_cancel_survives_node_activation() {
    let mut node = simulated("lc_timer_manual");
    let mut timer = node.create_timer(Duration::from_millis(10)).unwrap();
    let mut reference = Node::create_timer_with_options(
        &mut node,
        Duration::from_millis(10),
        TimerOptions::default().autostart(false),
    )
    .unwrap();
    activate(&mut node);
    reference.reset(&mut node).unwrap();
    set_time(&node, 10);
    spin(&mut node);
    assert_eq!(reference.tick().now_or_never().unwrap().unwrap(), Duration::from_millis(10));
    timer.reset(&mut node).unwrap();
    reference.reset(&mut node).unwrap();
    assert_eq!(timer.next_call_time_ns(&node).unwrap(), 20_000_000);
    assert!(timer.tick().now_or_never().is_none());
    timer.cancel(&mut node).unwrap();
    reference.cancel(&mut node).unwrap();
    node.deactivate().unwrap();
    node.activate().unwrap();
    set_time(&node, 100);
    spin(&mut node);
    assert!(timer.is_canceled(&node).unwrap());
    assert!(timer.tick().now_or_never().is_none());
    timer.reset(&mut node).unwrap();
    reference.reset(&mut node).unwrap();
    set_time(&node, 110);
    spin(&mut node);
    // RCL preserves elapsed time since the last actual call through reset.
    let expected = reference.tick().now_or_never().unwrap().unwrap();
    assert_eq!(expected, Duration::from_millis(100));
    assert_eq!(timer.tick().now_or_never().unwrap().unwrap(), expected);
}

#[test]
fn autostart_false_stays_paused_when_created_on_an_active_node() {
    let mut node = simulated("lc_timer_no_autostart");
    activate(&mut node);
    let mut timer = node
        .create_timer_with_options(
            Duration::from_millis(10),
            TimerOptions::default().autostart(false).channel_capacity(0),
        )
        .unwrap();
    set_time(&node, 100);
    spin(&mut node);
    assert!(timer.is_canceled(&node).unwrap());
    assert!(timer.tick().now_or_never().is_none());
    timer.reset(&mut node).unwrap();
    assert_eq!(timer.next_call_time_ns(&node).unwrap(), 110_000_000);
    set_time(&node, 110);
    spin(&mut node);
    assert_eq!(timer.tick().now_or_never().unwrap().unwrap(), Duration::from_millis(110));
}

#[test]
fn activation_failures_and_error_recovery_leave_native_timer_paused() {
    let mut node = simulated("lc_timer_failures");
    let mut timer = node.create_timer(Duration::from_millis(10)).unwrap();
    node.configure().unwrap();
    node.on_activate(Box::new(|_| CallbackReturn::failure("activation refused")));
    assert!(matches!(node.activate().unwrap(), CallbackReturn::Failure(_)));
    set_time(&node, 100);
    spin(&mut node);
    assert!(timer
        .state
        .with_timer(|ordinary| ordinary.is_canceled(&node))
        .unwrap());
    assert!(timer.tick().now_or_never().is_none());
    node.on_activate(Box::new(|_| CallbackReturn::Success));
    node.activate().unwrap();
    spin(&mut node);
    node.on_deactivate(Box::new(|_| CallbackReturn::error("recover")));
    assert!(matches!(node.deactivate().unwrap(), CallbackReturn::Error(_)));
    assert_eq!(node.lifecycle_state(), LifecycleState::Unconfigured);
    set_time(&node, 200);
    spin(&mut node);
    assert!(timer
        .state
        .with_timer(|ordinary| ordinary.is_canceled(&node))
        .unwrap());
    assert!(timer.tick().now_or_never().is_none());
    node.shutdown().unwrap();
    spin(&mut node);
    assert!(timer.is_canceled(&node).unwrap());
}

#[test]
fn failed_deactivation_preserves_the_active_deadline() {
    let mut node = simulated("lc_timer_deactivate_failure");
    let mut timer = node.create_timer(Duration::from_millis(10)).unwrap();
    activate(&mut node);
    set_time(&node, 5);
    node.on_deactivate(Box::new(|_| CallbackReturn::failure("stay active")));
    assert!(matches!(node.deactivate().unwrap(), CallbackReturn::Failure(_)));
    spin(&mut node);
    assert_eq!(node.lifecycle_state(), LifecycleState::Active);
    assert_eq!(timer.next_call_time_ns(&node).unwrap(), 10_000_000);
    set_time(&node, 10);
    spin(&mut node);
    assert_eq!(timer.tick().now_or_never().unwrap().unwrap(), Duration::from_millis(10));
}

#[test]
fn wall_timer_waits_for_activation_through_native_future_execution() {
    let mut node = managed("lc_timer_wall");
    let mut timer = node.create_wall_timer(Duration::from_millis(5)).unwrap();
    assert!(timer.is_steady());
    let options = FutureExecutionOptions::wake_driven();
    assert!(matches!(
        node.spin_until_future_complete_with_options(
            timer.tick(),
            Duration::from_millis(20),
            options
        ),
        Err(Error::RCL_RET_TIMEOUT)
    ));
    activate(&mut node);
    let elapsed = node
        .spin_until_future_complete_with_options(timer.tick(), Duration::from_secs(1), options)
        .unwrap();
    assert!(elapsed >= Duration::from_millis(5));
    node.deactivate().unwrap();
    assert!(matches!(
        node.spin_until_future_complete_with_options(
            timer.tick(),
            Duration::from_millis(20),
            options
        ),
        Err(Error::RCL_RET_TIMEOUT)
    ));
}

struct WakeCount(AtomicUsize);
impl ArcWake for WakeCount {
    fn wake_by_ref(this: &Arc<Self>) {
        this.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn inactive_tick_observer_wakes_when_its_node_is_destroyed() {
    let mut node = managed("lc_timer_drop_node");
    let mut timer = node.create_wall_timer(Duration::from_secs(60)).unwrap();
    let mut tick = Box::pin(timer.tick());
    let count = Arc::new(WakeCount(AtomicUsize::new(0)));
    let waker = waker_ref(&count);
    let mut context = std::task::Context::from_waker(&waker);
    assert!(tick.as_mut().poll(&mut context).is_pending());
    drop(node);
    assert!(count.0.load(Ordering::SeqCst) > 0);
    assert!(matches!(
        tick.as_mut().poll(&mut context),
        std::task::Poll::Ready(Err(Error::RCL_RET_TIMER_INVALID))
    ));
}

#[test]
fn timer_destruction_and_drop_preserve_other_native_timers() {
    let mut node = managed("lc_timer_drop");
    let removed = node.create_wall_timer(Duration::from_secs(60)).unwrap();
    let last = node.create_timer(Duration::from_secs(62)).unwrap();
    assert_eq!(node.timer_count(), 2);
    removed.destroy(&mut node);
    assert_eq!(node.timer_count(), 1);
    assert_eq!(last.period(&node).unwrap(), Duration::from_secs(62));
    drop(last);
    spin(&mut node);
    assert_eq!(node.timer_count(), 0);
    node.configure().unwrap();
    node.activate().unwrap();
    spin(&mut node);
}

#[test]
fn foreign_controls_and_invalid_options_preserve_timer_state() {
    let mut node = managed("lc_timer_foreign");
    let mut other =
        crate::Node::create(Context::create().unwrap(), "lc_timer_foreign_other", "").unwrap();
    activate(&mut node);
    let timer = node.create_wall_timer(Duration::from_secs(60)).unwrap();
    assert!(matches!(timer.cancel(&mut other), Err(Error::RCL_RET_TIMER_INVALID)));
    assert!(!timer.is_canceled(&node).unwrap());
    timer.cancel(&mut node).unwrap();
    assert!(matches!(timer.reset(&mut other), Err(Error::RCL_RET_TIMER_INVALID)));
    assert!(timer.is_canceled(&node).unwrap());
    assert!(matches!(timer.period(&other), Err(Error::RCL_RET_TIMER_INVALID)));
    assert!(matches!(
        timer.set_period(&mut node, Duration::MAX),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert_eq!(timer.period(&node).unwrap(), Duration::from_secs(60));
    assert_eq!(
        timer
            .set_period(&mut node, Duration::from_secs(10))
            .unwrap(),
        Duration::from_secs(60)
    );
    assert!(matches!(
        node.create_wall_timer_with_options(
            Duration::ZERO,
            TimerOptions::default().channel_capacity(usize::MAX)
        ),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert_eq!(node.timer_count(), 1);
}

#[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
#[test]
fn tick_info_preserves_current_ros_clock_deadline_and_elapsed_time() {
    let mut node = simulated("lc_timer_info");
    let mut timer = node.create_timer(Duration::from_millis(10)).unwrap();
    activate(&mut node);
    set_time(&node, 12);
    spin(&mut node);
    let info = timer.tick_with_info().now_or_never().unwrap().unwrap();
    assert_eq!(info.expected_call_time_ns, 10_000_000);
    assert_eq!(info.actual_call_time_ns, 12_000_000);
    assert_eq!(info.elapsed, Duration::from_millis(12));
    assert_eq!(info.clock_type, ClockType::RosTime);
}

#[test]
fn reactivation_after_backward_ros_time_jump_starts_at_the_new_clock_time() {
    let mut node = simulated("lc_timer_backward_clock");
    let mut timer = node.create_timer(Duration::from_millis(10)).unwrap();
    activate(&mut node);
    set_time(&node, 10);
    spin(&mut node);
    timer.tick().now_or_never().unwrap().unwrap();
    node.deactivate().unwrap();
    spin(&mut node);
    set_time(&node, 100);
    set_time(&node, 0);
    node.activate().unwrap();
    spin(&mut node);
    assert_eq!(timer.next_call_time_ns(&node).unwrap(), 10_000_000);
    set_time(&node, 9);
    spin(&mut node);
    assert!(timer.tick().now_or_never().is_none());
    set_time(&node, 10);
    spin(&mut node);
    assert!(timer.tick().now_or_never().unwrap().is_ok());
}

#[test]
fn shared_executor_drives_service_transitions_and_managed_wall_ticks() {
    use crate::lifecycle_msgs::{msg::Transition, srv::ChangeState};
    let context = Context::create().unwrap();
    let mut node = LifecycleNode::create(
        context.clone(),
        &format!("lc_timer_executor_{}", uuid::Uuid::new_v4().simple()),
        "",
    )
    .unwrap();
    let mut timer = node.create_wall_timer(Duration::from_millis(5)).unwrap();
    let mut manager = Node::create(context.clone(), "lc_timer_executor_manager", "").unwrap();
    let client = manager
        .create_client::<ChangeState::Service>(
            &format!("{}/change_state", node.fully_qualified_name().unwrap()),
            crate::QosProfile::services_default(),
        )
        .unwrap();
    let mut executor = crate::Executor::new(context);
    let node_id = executor.add_node(node.into_node()).unwrap();
    executor.add_node(manager).unwrap();
    let options = FutureExecutionOptions::wake_driven();
    executor
        .spin_until_future_complete_with_options(
            client.wait_for_service().unwrap(),
            Duration::from_secs(5),
            options,
        )
        .unwrap();
    for id in [
        Transition::TRANSITION_CONFIGURE as u8,
        Transition::TRANSITION_ACTIVATE as u8,
    ] {
        let request = client
            .request(&ChangeState::Request {
                transition: Transition {
                    id,
                    label: Default::default(),
                },
            })
            .unwrap();
        assert!(
            executor
                .spin_until_future_complete_with_options(request, Duration::from_secs(5), options)
                .unwrap()
                .success
        );
    }
    assert!(
        executor
            .spin_until_future_complete_with_options(timer.tick(), Duration::from_secs(1), options)
            .unwrap()
            >= Duration::from_millis(5)
    );
    let request = client
        .request(&ChangeState::Request {
            transition: Transition {
                id: Transition::TRANSITION_DEACTIVATE as u8,
                label: Default::default(),
            },
        })
        .unwrap();
    assert!(
        executor
            .spin_until_future_complete_with_options(request, Duration::from_secs(5), options)
            .unwrap()
            .success
    );
    assert!(matches!(
        executor.spin_until_future_complete_with_options(
            timer.tick(),
            Duration::from_millis(20),
            options
        ),
        Err(Error::RCL_RET_TIMEOUT)
    ));
    assert!(timer
        .state
        .with_timer(|ordinary| ordinary.is_canceled(executor.node_mut(node_id).unwrap()))
        .unwrap());
    timer.destroy(executor.node_mut(node_id).unwrap());
}

#[cfg(r2r__rosgraph_msgs__msg__Clock)]
#[test]
fn managed_ros_timer_follows_native_clock_topic_while_paused_and_after_reset() {
    use crate::rosgraph_msgs::msg::Clock as ClockMessage;
    let context = Context::create().unwrap();
    let mut node = LifecycleNode::create(
        context.clone(),
        &format!("lc_timer_clock_{}", uuid::Uuid::new_v4().simple()),
        "",
    )
    .unwrap();
    node.get_time_source().enable_sim_time(&mut node).unwrap();
    let clock = node.get_ros_clock();
    let mut timer = node.create_timer(Duration::from_millis(10)).unwrap();
    activate(&mut node);
    let mut source = Node::create(context.clone(), "lc_timer_clock_source", "").unwrap();
    let publisher = source
        .create_publisher::<ClockMessage>("/clock", crate::QosProfile::default())
        .unwrap();
    let mut executor = crate::Executor::new(context);
    let id = executor.add_node(node.into_node()).unwrap();
    executor.add_node(source).unwrap();
    let options = FutureExecutionOptions::wake_driven();
    executor
        .spin_until_future_complete_with_options(
            publisher.wait_for_subscribers().unwrap(),
            Duration::from_secs(5),
            options,
        )
        .unwrap();
    publisher
        .publish(&ClockMessage {
            clock: crate::builtin_interfaces::msg::Time {
                sec: 0,
                nanosec: 10_000_000,
            },
        })
        .unwrap();
    assert_eq!(
        executor
            .spin_until_future_complete_with_options(timer.tick(), Duration::from_secs(5), options)
            .unwrap(),
        Duration::from_millis(10)
    );
    assert_eq!(clock.lock().get_now_ns().unwrap(), 10_000_000);
    timer.cancel(executor.node_mut(id).unwrap()).unwrap();
    publisher
        .publish(&ClockMessage {
            clock: crate::builtin_interfaces::msg::Time {
                sec: 0,
                nanosec: 100_000_000,
            },
        })
        .unwrap();
    assert!(matches!(
        executor.spin_until_future_complete_with_options(
            timer.tick(),
            Duration::from_millis(100),
            options
        ),
        Err(Error::RCL_RET_TIMEOUT)
    ));
    assert_eq!(clock.lock().get_now_ns().unwrap(), 100_000_000);
    timer.reset(executor.node_mut(id).unwrap()).unwrap();
    assert_eq!(
        timer
            .next_call_time_ns(executor.node_mut(id).unwrap())
            .unwrap(),
        110_000_000
    );
    publisher
        .publish(&ClockMessage {
            clock: crate::builtin_interfaces::msg::Time {
                sec: 0,
                nanosec: 110_000_000,
            },
        })
        .unwrap();
    assert_eq!(
        executor
            .spin_until_future_complete_with_options(timer.tick(), Duration::from_secs(5), options)
            .unwrap(),
        Duration::from_millis(100)
    );
}
