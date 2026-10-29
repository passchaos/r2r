use super::*;

#[test]
fn callback_timer_ros_clock_pauses_and_handles_backward_jumps_at_native_deadlines() {
    let mut node = node("callback_timer_ros_clock");
    node.ros_clock
        .lock()
        .enable_ros_time_override(1_000_000_000)
        .unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let clock = node.ros_clock.clone();
    let timer = node
        .create_timer_callback(Duration::from_nanos(10), move || {
            assert!(clock.try_lock().is_some(), "timer handler held managed clock lock");
            seen.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .unwrap();
    assert_eq!(timer.clock_type(), ClockType::RosTime);
    assert!(!timer.is_steady());
    for _ in 0..3 {
        node.try_spin_once(Duration::ZERO).unwrap();
    }
    assert_eq!(count.load(Ordering::Relaxed), 0);
    node.ros_clock
        .lock()
        .set_ros_time_override(timer.next_call_time_ns(&node).unwrap())
        .unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 1);
    node.ros_clock
        .lock()
        .set_ros_time_override(999_999_000)
        .unwrap();
    let next = timer.next_call_time_ns(&node).unwrap();
    node.ros_clock
        .lock()
        .set_ros_time_override(next - 1)
        .unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 1);
    node.ros_clock.lock().set_ros_time_override(next).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 2);
    assert_eq!(timer.time_since_last_call_ns(&node).unwrap(), 0);
}

#[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
#[test]
fn callback_timer_call_info_matches_native_ros_timestamps_and_preserves_address() {
    let mut node = node("callback_timer_info");
    node.ros_clock.lock().enable_ros_time_override(100).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let values = seen.clone();
    let timer = node
        .create_timer_callback_with_info(Duration::from_nanos(20), move |info| {
            values.lock().push(info);
            Ok(())
        })
        .unwrap();
    let address = node.callback_timer(timer.identity()).unwrap().handle();
    for now in [120, 140, 180] {
        let expected = timer.next_call_time_ns(&node).unwrap();
        node.ros_clock.lock().set_ros_time_override(now).unwrap();
        let elapsed = timer.time_since_last_call_ns(&node).unwrap();
        node.try_spin_once(Duration::ZERO).unwrap();
        let tick = *seen.lock().last().unwrap();
        assert_eq!(tick.clock_type, ClockType::RosTime);
        assert_eq!(tick.expected_call_time_ns, expected);
        assert_eq!(tick.actual_call_time_ns, now);
        assert_eq!(tick.elapsed, Duration::from_nanos(elapsed.try_into().unwrap()));
        assert_eq!(node.callback_timer(timer.identity()).unwrap().handle(), address);
    }
    assert_eq!(seen.lock().len(), 3);
    node.ros_clock.lock().set_ros_time_override(80).unwrap();
    let next = timer.next_call_time_ns(&node).unwrap();
    node.ros_clock.lock().set_ros_time_override(next).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(seen.lock().len(), 4);
    assert_eq!(seen.lock().last().unwrap().actual_call_time_ns, next);
}
