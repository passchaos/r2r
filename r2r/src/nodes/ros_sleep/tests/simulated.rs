use super::*;

fn simulated(name: &str, now: i64) -> Node {
    let node = node(name);
    node.ros_clock.lock().enable_ros_time_override(now).unwrap();
    node
}

#[test]
fn ros_sleep_paused_and_backward_simulated_time_keep_absolute_deadlines() {
    let mut node = simulated("ros_sleep_backwards", 100);
    let mut sleep = node.sleep_until_ros(200).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    let started = Instant::now();
    node.try_spin_once(Duration::from_millis(50)).unwrap();
    assert!(started.elapsed() >= Duration::from_millis(35), "paused sleep busy-polled");
    assert!((&mut sleep).now_or_never().is_none());
    for now in [150, 50, 199] {
        node.ros_clock.lock().set_ros_time_override(now).unwrap();
        node.try_spin_once(Duration::ZERO).unwrap();
        assert!((&mut sleep).now_or_never().is_none(), "sleep finished at {now}");
    }
    node.ros_clock.lock().set_ros_time_override(200).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(matches!(sleep.now_or_never(), Some(Ok(()))));
}

#[test]
fn ros_sleep_signed_time_and_relative_overflow_have_explicit_boundaries() {
    let mut node = simulated("ros_sleep_signed", -100);
    let sleep = node.sleep_for_ros(Duration::from_nanos(50)).unwrap();
    assert_eq!(sleep.deadline_ns(), -50);
    node.ros_clock.lock().set_ros_time_override(-50).unwrap();
    await_sleep(&mut node, sleep).unwrap();
    node.ros_clock
        .lock()
        .set_ros_time_override(i64::MAX - 5)
        .unwrap();
    assert!(matches!(
        node.sleep_for_ros(Duration::from_nanos(10)),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert!(matches!(node.sleep_until_ros(i64::MIN).unwrap().now_or_never(), Some(Ok(()))));
}

#[test]
fn ros_sleep_maximum_absolute_simulated_deadline_never_overflows_a_native_ros_timer() {
    let mut node = simulated("ros_sleep_max_deadline", i64::MAX - 1);
    let sleep = node.sleep_until_ros(i64::MAX).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    node.ros_clock
        .lock()
        .set_ros_time_override(i64::MAX)
        .unwrap();
    await_sleep(&mut node, sleep).unwrap();
}

#[test]
fn ros_sleep_time_source_change_interrupts_previous_waits_and_allows_new_domain_waits() {
    let mut node = simulated("ros_sleep_source", 100);
    let mut old = node.sleep_until_ros(200).unwrap();
    assert!((&mut old).now_or_never().is_none());
    node.ros_clock.lock().disable_ros_time_override().unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(matches!(old.now_or_never(), Some(Err(Error::RosTimeSourceChanged))));
    let mut system = node.sleep_for_ros(Duration::from_secs(30)).unwrap();
    node.ros_clock.lock().enable_ros_time_override(500).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(matches!((&mut system).now_or_never(), Some(Err(Error::RosTimeSourceChanged))));
    let new = node.sleep_until_ros(600).unwrap();
    node.ros_clock.lock().set_ros_time_override(600).unwrap();
    await_sleep(&mut node, new).unwrap();
}

#[test]
fn ros_sleep_new_wait_after_time_source_change_survives_old_interruption_cleanup() {
    let mut node = simulated("ros_sleep_new_domain", 100);
    let old = node.sleep_until_ros(200).unwrap();
    node.ros_clock.lock().disable_ros_time_override().unwrap();
    let system = node.sleep_for_ros(Duration::from_millis(20)).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(matches!(old.now_or_never(), Some(Err(Error::RosTimeSourceChanged))));
    await_sleep(&mut node, system).unwrap();
}

#[test]
fn ros_sleep_large_cohort_uses_one_timer_and_bounded_completion_passes() {
    let mut node = simulated("ros_sleep_cohort", 0);
    let sleeps = (0..512)
        .map(|index| node.sleep_until_ros(1000 + index).unwrap())
        .collect::<Vec<_>>();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(node.wait_set.capacities.unwrap().timers, 1);
    node.ros_clock.lock().set_ros_time_override(2000).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    let remaining = node
        .ros_sleeps
        .as_ref()
        .unwrap()
        .shared
        .state
        .lock()
        .deadlines
        .len();
    assert!(
        (512 - 2 * MAX_COMPLETIONS_PER_PASS..=512 - MAX_COMPLETIONS_PER_PASS).contains(&remaining)
    );
    while !node
        .ros_sleeps
        .as_ref()
        .unwrap()
        .shared
        .state
        .lock()
        .deadlines
        .is_empty()
    {
        node.try_spin_once(Duration::ZERO).unwrap();
    }
    assert!(
        matches!(futures::future::try_join_all(sleeps).now_or_never(), Some(Ok(values)) if values.len() == 512)
    );
}

#[test]
fn ros_sleep_shared_native_timer_preserves_ordinary_wall_timer_progress() {
    let mut node = simulated("ros_sleep_wall_progress", 0);
    let mut sleeps = (0..32)
        .map(|_| node.sleep_until_ros(100).unwrap())
        .collect::<Vec<_>>();
    let mut wall = node.create_wall_timer(Duration::from_millis(10)).unwrap();
    node.spin_until_future_complete_with_options(
        async { wall.tick().await.map(|_| ()) },
        Duration::from_secs(1),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    for sleep in &mut sleeps {
        assert!(sleep.now_or_never().is_none());
    }
    assert_eq!(node.wait_set.capacities.unwrap().timers, 2);
}

#[tokio::test(flavor = "current_thread")]
async fn ros_sleep_clock_updates_follow_current_background_and_returned_node_guards() {
    let mut node = simulated("ros_sleep_current_guard", 100);
    let sleep = node.sleep_until_ros(200).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    let old_guard = node.future_wake_guard.clone().unwrap();
    let clock = node.get_ros_clock();
    let mut executor = Executor::new(node.context.clone());
    let id = executor.add_node(node).unwrap();
    executor.spin_once(Duration::ZERO).unwrap();
    let guard =
        super::super::super::future_executor::FutureDriver::cached_future_guard(&executor).unwrap();
    let old_triggers = old_guard.triggers.load(Ordering::Acquire);
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !guard.waiting.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    clock.lock().set_ros_time_override(200).unwrap();
    tokio::time::timeout(Duration::from_secs(3), sleep)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(old_guard.triggers.load(Ordering::Acquire), old_triggers);
    assert!(guard.triggers.load(Ordering::Acquire) >= 1);
    let mut executor = task.stop().await.unwrap();
    executor.reset_cancel();
    let mut returned = executor.remove_node(id).unwrap();
    let stale_triggers = guard.triggers.load(Ordering::Acquire);
    let second = returned.sleep_until_ros(300).unwrap();
    clock.lock().set_ros_time_override(300).unwrap();
    await_sleep(&mut returned, second).unwrap();
    assert_eq!(guard.triggers.load(Ordering::Acquire), stale_triggers);
}

#[test]
fn ros_sleep_real_clock_topic_drives_wait_without_a_periodic_future_timer() {
    let mut node = node("ros_sleep_clock_topic");
    node.get_time_source().enable_sim_time(&mut node).unwrap();
    let clock_publisher = node
        .create_publisher::<crate::rosgraph_msgs::msg::Clock>("/clock", QosProfile::default())
        .unwrap();
    let ready = clock_publisher.wait_for_subscribers().unwrap();
    node.spin_until_future_complete_with_options(
        ready,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let sleep = node.sleep_until_ros(7_000_000_000).unwrap();
    clock_publisher
        .publish(&crate::rosgraph_msgs::msg::Clock {
            clock: crate::builtin_interfaces::msg::Time { sec: 7, nanosec: 0 },
        })
        .unwrap();
    await_sleep(&mut node, sleep).unwrap();
    assert_eq!(node.now_ns().unwrap(), 7_000_000_000);
    assert_eq!(node.wait_set.capacities.unwrap().timers, 1);
}
