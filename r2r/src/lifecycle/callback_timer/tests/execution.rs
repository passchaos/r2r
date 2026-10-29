use super::*;

#[test]
fn lifecycle_callback_timer_user_code_and_retirement_do_not_hold_control_or_context_locks() {
    let context = Context::create().unwrap();
    let mut node = LifecycleNode::create(context.clone(), "lc_callback_reentry", "").unwrap();
    node.get_ros_clock()
        .lock()
        .enable_ros_time_override(0)
        .unwrap();
    let weak = Arc::new(parking_lot::Mutex::new(
        None::<std::sync::Weak<super::super::state::CallbackTimerState>>,
    ));
    struct Capture {
        slot: Arc<
            parking_lot::Mutex<Option<std::sync::Weak<super::super::state::CallbackTimerState>>>,
        >,
        context: Context,
        retired: Arc<AtomicUsize>,
    }
    impl Drop for Capture {
        fn drop(&mut self) {
            assert!(self.context.context_handle.try_lock().is_some());
            self.retired.fetch_add(1, Ordering::Relaxed);
        }
    }
    let retired = Arc::new(AtomicUsize::new(0));
    let capture = Capture {
        slot: weak.clone(),
        context,
        retired: retired.clone(),
    };
    let token = node
        .create_timer_callback_with_options(
            Duration::from_millis(10),
            CallbackTimerOptions::default().oneshot(true),
            move || {
                let state = capture.slot.lock().as_ref().unwrap().upgrade().unwrap();
                assert!(state.with_timer(|_| Ok(())).is_ok());
                assert!(capture.context.context_handle.try_lock().is_some());
                Err(Error::RCL_RET_INVALID_ARGUMENT)
            },
        )
        .unwrap();
    *weak.lock() = Some(Arc::downgrade(&token.state));
    active(&mut node);
    time(&node, 10);
    assert!(matches!(
        node.try_spin_once(Duration::ZERO),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert!(token.is_canceled(&node).unwrap());
    node.deactivate().unwrap();
    node.activate().unwrap();
    spin(&mut node);
    time(&node, 100);
    spin(&mut node);
    token.destroy(&mut node);
    assert_eq!(retired.load(Ordering::Relaxed), 1);
}

#[test]
fn lifecycle_callback_timer_panic_consumes_one_shot_until_reset_and_node_destruction_closes_handle()
{
    let mut node = node("lc_callback_panic");
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let timer = node
        .create_timer_callback_with_options(
            Duration::from_millis(10),
            CallbackTimerOptions::default().oneshot(true),
            move || {
                if seen.fetch_add(1, Ordering::Relaxed) == 0 {
                    panic!("intentional managed timer panic");
                }
                Ok(())
            },
        )
        .unwrap();
    active(&mut node);
    time(&node, 10);
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || node.try_spin_once(Duration::ZERO)
    ))
    .is_err());
    assert!(timer.is_canceled(&node).unwrap());
    node.deactivate().unwrap();
    node.activate().unwrap();
    spin(&mut node);
    time(&node, 100);
    spin(&mut node);
    assert_eq!(count.load(Ordering::Relaxed), 1);
    timer.reset(&mut node).unwrap();
    time(&node, 110);
    spin(&mut node);
    assert_eq!(count.load(Ordering::Relaxed), 2);
    drop(node);
    assert!(!timer.is_valid());
}

#[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
#[test]
fn lifecycle_callback_timer_native_info_and_backward_reactivation_follow_current_clock() {
    let mut node = node("lc_callback_info");
    time(&node, 100);
    let seen = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let values = seen.clone();
    let timer = node
        .create_timer_callback_with_info(Duration::from_millis(10), move |info| {
            values.lock().push(info);
            Ok(())
        })
        .unwrap();
    active(&mut node);
    time(&node, 110);
    spin(&mut node);
    assert_eq!(seen.lock()[0].expected_call_time_ns, 110_000_000);
    assert_eq!(seen.lock()[0].actual_call_time_ns, 110_000_000);
    node.deactivate().unwrap();
    spin(&mut node);
    time(&node, 20);
    node.activate().unwrap();
    spin(&mut node);
    assert_eq!(timer.next_call_time_ns(&node).unwrap(), 30_000_000);
    time(&node, 30);
    spin(&mut node);
    assert_eq!(seen.lock().len(), 2);
    assert_eq!(seen.lock()[1].expected_call_time_ns, 30_000_000);
    assert_eq!(seen.lock()[1].actual_call_time_ns, 30_000_000);
}

#[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
#[test]
fn lifecycle_callback_timer_negative_native_call_matches_plain_timer_error_and_can_recover() {
    let mut node = node("lc_callback_negative");
    let managed_calls = Arc::new(AtomicUsize::new(0));
    let observed = managed_calls.clone();
    let timer = node
        .create_timer_callback_with_info_and_options(
            Duration::from_millis(10),
            CallbackTimerOptions::default().oneshot(true),
            move |_| {
                observed.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap();
    active(&mut node);
    time(&node, -100);
    timer.reset(&mut node).unwrap();
    time(&node, -90);
    let error = node.try_spin_once(Duration::ZERO).unwrap_err();
    assert!(matches!(error, Error::RCL_RET_ERROR));
    assert_eq!(managed_calls.load(Ordering::Relaxed), 0);
    assert!(
        !timer.is_canceled(&node).unwrap(),
        "failed native call consumed managed one-shot state"
    );
    unsafe {
        r2r_rcl::rcutils_reset_error();
    }

    let mut plain =
        Node::create(Context::create().unwrap(), "plain_callback_negative", "").unwrap();
    plain
        .get_ros_clock()
        .lock()
        .enable_ros_time_override(0)
        .unwrap();
    let plain_calls = Arc::new(AtomicUsize::new(0));
    let observed = plain_calls.clone();
    let ordinary = plain
        .create_timer_callback_with_info_and_options(
            Duration::from_millis(10),
            CallbackTimerOptions::default().oneshot(true),
            move |_| {
                observed.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap();
    plain
        .get_ros_clock()
        .lock()
        .set_ros_time_override(-100_000_000)
        .unwrap();
    ordinary.reset(&mut plain).unwrap();
    plain
        .get_ros_clock()
        .lock()
        .set_ros_time_override(-90_000_000)
        .unwrap();
    assert!(matches!(plain.try_spin_once(Duration::ZERO), Err(Error::RCL_RET_ERROR)));
    assert_eq!(plain_calls.load(Ordering::Relaxed), 0);
    assert!(!ordinary.is_canceled(&plain).unwrap());
    unsafe {
        r2r_rcl::rcutils_reset_error();
    }
    time(&node, 100);
    timer.reset(&mut node).unwrap();
    time(&node, 110);
    spin(&mut node);
    assert_eq!(managed_calls.load(Ordering::Relaxed), 1);
    assert!(timer.is_canceled(&node).unwrap());
}
