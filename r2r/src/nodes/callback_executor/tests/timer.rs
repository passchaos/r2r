use super::*;

#[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
#[test]
fn callback_group_timer_advances_on_worker_invocation_with_exact_native_clock_metadata() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "callback_group_timer_info", "").unwrap();
    let clock = node.get_ros_clock();
    clock.lock().enable_ros_time_override(100).unwrap();
    let mut group = CallbackGroup::new();
    let ticks = Arc::new(Mutex::new(Vec::new()));
    let seen = ticks.clone();
    let now = clock.clone();
    let timer = node
        .create_timer_callback_in_group_with_info(
            &mut group,
            Duration::from_nanos(20),
            move |tick| {
                assert!(
                    now.try_lock().is_some(),
                    "user timer callback held the managed clock mutex"
                );
                seen.lock().push(tick);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(timer.next_call_time_ns().unwrap(), 120);
    clock.lock().set_ros_time_override(120).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    // Selection must not advance the native timer or repeatedly register it.
    assert_eq!(timer.next_call_time_ns().unwrap(), 120);
    assert_eq!(node.prepare_spin(Duration::ZERO).unwrap().capacities.timers, 0);
    clock.lock().set_ros_time_override(180).unwrap();
    let wake = futures::task::noop_waker();
    assert!(group
        .poll_callbacks(&mut TaskContext::from_waker(&wake))
        .is_pending());
    let tick = ticks.lock()[0];
    assert_eq!(tick.expected_call_time_ns, 120);
    assert_eq!(tick.actual_call_time_ns, 180);
    assert_eq!(tick.elapsed, Duration::from_nanos(80));
    assert_eq!(timer.next_call_time_ns().unwrap(), 200);
    assert_eq!(node.prepare_spin(Duration::ZERO).unwrap().capacities.timers, 1);
    drop(timer);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(node.prepare_spin(Duration::ZERO).unwrap().capacities.timers, 0);
    assert!(matches!(
        group.poll_callbacks(&mut TaskContext::from_waker(&wake)),
        Poll::Ready(Ok(()))
    ));
}

#[tokio::test(flavor = "current_thread")]
async fn callback_group_timer_external_reset_wakes_native_wait_and_one_shot_is_sticky() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "callback_group_timer_reset", "").unwrap();
    let mut group = CallbackGroup::new();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let timer = node
        .create_wall_timer_callback_in_group_with_options(
            &mut group,
            Duration::from_millis(2),
            CallbackTimerOptions::default()
                .autostart(false)
                .oneshot(true),
            move || {
                seen.fetch_add(1, Ordering::Release);
                Ok(())
            },
        )
        .unwrap();
    assert!(timer.is_steady());
    assert!(timer.is_canceled().unwrap());
    let mut executor = CallbackExecutor::new(context, 2).unwrap();
    executor.add_node(node).unwrap();
    executor.add_group(group).unwrap();
    let task = executor.spawn().unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(count.load(Ordering::Acquire), 0);
    timer.reset().unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        while count.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let mut executor = task.stop().await.unwrap();
    result.unwrap();
    assert_eq!(count.load(Ordering::Acquire), 1);
    assert!(timer.is_canceled().unwrap());
    executor.reset_cancel();
    let task = executor.spawn().unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(count.load(Ordering::Acquire), 1);
    timer.reset().unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        while count.load(Ordering::Acquire) == 1 {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let executor = task.stop().await.unwrap();
    result.unwrap();
    assert_eq!(count.load(Ordering::Acquire), 2);
    assert_eq!(timer.period().unwrap(), Duration::from_millis(2));
    assert_eq!(timer.set_period(Duration::from_millis(3)).unwrap(), Duration::from_millis(2));
    drop(executor);
    assert!(!timer.is_valid());
    assert!(matches!(timer.reset(), Err(Error::RCL_RET_TIMER_INVALID)));
}

#[test]
fn callback_group_timer_cancel_after_selection_skips_callback_and_can_be_reset() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "callback_group_timer_selected_cancel", "").unwrap();
    let mut group = CallbackGroup::new();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let timer = node
        .create_wall_timer_callback_in_group_with_options(
            &mut group,
            Duration::ZERO,
            CallbackTimerOptions::default().oneshot(true),
            move || {
                seen.fetch_add(1, Ordering::Release);
                Ok(())
            },
        )
        .unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    timer.cancel().unwrap();
    let wake = futures::task::noop_waker();
    assert!(group
        .poll_callbacks(&mut TaskContext::from_waker(&wake))
        .is_pending());
    assert_eq!(count.load(Ordering::Acquire), 0);
    timer.reset().unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(group
        .poll_callbacks(&mut TaskContext::from_waker(&wake))
        .is_pending());
    assert_eq!(count.load(Ordering::Acquire), 1);
    assert!(timer.is_canceled().unwrap());
}

#[test]
fn callback_group_timer_invalid_period_leaves_no_native_or_group_registration() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "callback_group_timer_invalid", "").unwrap();
    let mut group = CallbackGroup::new();
    assert!(node
        .create_wall_timer_callback_in_group(&mut group, Duration::MAX, || Ok(()))
        .is_err());
    assert!(group.is_empty());
    assert!(node.grouped_callbacks.is_none());
}
