use super::*;
use futures::{FutureExt, StreamExt};

#[test]
fn lifecycle_callback_timer_reset_events_include_activation_and_manual_restarts() {
    let mut node = node("lc_callback_reset_events");
    let timer = node
        .create_timer_callback(Duration::from_millis(10), || Ok(()))
        .unwrap();
    let mut events = timer.observe_resets(&mut node).unwrap();
    assert!(events.next().now_or_never().is_none());

    active(&mut node);
    assert_eq!(
        events.next().now_or_never(),
        Some(Some(crate::TimerReset { count: 1, total: 1 }))
    );
    timer.reset(&mut node).unwrap();
    assert_eq!(
        events.next().now_or_never(),
        Some(Some(crate::TimerReset { count: 1, total: 2 }))
    );
    timer.destroy(&mut node);
    assert_eq!(events.next().now_or_never(), Some(None));
}

#[test]
fn lifecycle_callback_timer_generations_restart_deadlines_and_preserve_manual_pause() {
    let mut node = node("lc_callback_generations");
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let timer = node
        .create_timer_callback(Duration::from_millis(10), move || {
            seen.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .unwrap();
    active(&mut node);
    assert_eq!(timer.next_call_time_ns(&node).unwrap(), 10_000_000);
    time(&node, 10);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    time(&node, 20);
    node.deactivate().unwrap();
    node.activate().unwrap();
    assert!(timer.is_canceled(&node).unwrap());
    assert!(matches!(timer.next_call_time_ns(&node), Err(Error::RCL_RET_TIMER_CANCELED)));
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert_eq!(timer.next_call_time_ns(&node).unwrap(), 30_000_000);
    time(&node, 30);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    timer.cancel(&mut node).unwrap();
    node.deactivate().unwrap();
    node.activate().unwrap();
    spin(&mut node);
    time(&node, 100);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 2);
    timer.reset(&mut node).unwrap();
    assert_eq!(timer.next_call_time_ns(&node).unwrap(), 110_000_000);
    time(&node, 110);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 3);
}

#[test]
fn lifecycle_callback_timer_oneshot_completion_survives_reactivation_until_explicit_reset() {
    let mut node = node("lc_callback_once");
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let timer = node
        .create_timer_callback_with_options(
            Duration::from_millis(10),
            CallbackTimerOptions::default().oneshot(true),
            move || {
                seen.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap();
    active(&mut node);
    time(&node, 10);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert!(timer.is_canceled(&node).unwrap());
    node.deactivate().unwrap();
    node.activate().unwrap();
    spin(&mut node);
    time(&node, 100);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    node.deactivate().unwrap();
    spin(&mut node);
    timer.reset(&mut node).unwrap();
    assert!(timer.is_canceled(&node).unwrap());
    node.activate().unwrap();
    spin(&mut node);
    assert_eq!(timer.next_call_time_ns(&node).unwrap(), 110_000_000);
    time(&node, 110);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[test]
fn lifecycle_callback_timer_manual_start_foreign_controls_and_drop_have_owner_boundaries() {
    let mut node = node("lc_callback_controls");
    let mut foreign =
        Node::create(crate::Context::create().unwrap(), "lc_callback_foreign", "").unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let timer = node
        .create_timer_callback_with_options(
            Duration::from_millis(10),
            CallbackTimerOptions::default().autostart(false),
            move || {
                seen.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap();
    active(&mut node);
    assert!(timer.is_canceled(&node).unwrap());
    assert!(matches!(timer.reset(&mut foreign), Err(Error::RCL_RET_TIMER_INVALID)));
    assert!(matches!(timer.cancel(&mut foreign), Err(Error::RCL_RET_TIMER_INVALID)));
    time(&node, 100);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    timer.reset(&mut node).unwrap();
    assert_eq!(
        timer
            .set_period(&mut node, Duration::from_millis(20))
            .unwrap(),
        Duration::from_millis(10)
    );
    assert_eq!(timer.period(&node).unwrap(), Duration::from_millis(20));
    time(&node, 120);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    let weak = Arc::downgrade(&timer.state);
    drop(timer);
    spin(&mut node);
    assert!(weak.upgrade().is_none());
    assert_eq!(Arc::strong_count(&calls), 1);
}

#[test]
fn lifecycle_callback_timer_activation_failure_and_deactivation_failure_match_settled_state() {
    let mut node = node("lc_callback_outcomes");
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let timer = node
        .create_timer_callback(Duration::from_millis(10), move || {
            seen.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .unwrap();
    node.configure().unwrap();
    node.on_activate(Box::new(|_| CallbackReturn::failure("stay inactive")));
    assert!(matches!(node.activate().unwrap(), CallbackReturn::Failure(_)));
    time(&node, 100);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    node.on_activate(Box::new(|_| CallbackReturn::Success));
    node.activate().unwrap();
    spin(&mut node);
    let deadline = timer.next_call_time_ns(&node).unwrap();
    node.on_deactivate(Box::new(|_| CallbackReturn::failure("stay active")));
    assert!(matches!(node.deactivate().unwrap(), CallbackReturn::Failure(_)));
    spin(&mut node);
    assert_eq!(timer.next_call_time_ns(&node).unwrap(), deadline);
    time(&node, 110);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    node.on_deactivate(Box::new(|_| CallbackReturn::error("recover")));
    assert!(matches!(node.deactivate().unwrap(), CallbackReturn::Error(_)));
    spin(&mut node);
    time(&node, 200);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert!(timer.is_canceled(&node).unwrap());
}

#[test]
fn lifecycle_callback_timer_stale_generation_does_not_advance_native_timer_or_consume_oneshot() {
    let mut node = node("lc_callback_stale");
    let slot = Arc::new(parking_lot::Mutex::new(
        None::<std::sync::Weak<super::super::state::CallbackTimerState>>,
    ));
    let observed = slot.clone();
    let _updater = Node::create_timer_callback_with_options(
        &mut node,
        Duration::from_millis(10),
        CallbackTimerOptions::default().oneshot(true),
        move || {
            let state = observed.lock().as_ref().unwrap().upgrade().unwrap();
            state.set_node_active(false);
            state.set_node_active(true);
            Ok(())
        },
    )
    .unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let timer = node
        .create_timer_callback_with_options(
            Duration::from_millis(10),
            CallbackTimerOptions::default().oneshot(true),
            move || {
                seen.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap();
    *slot.lock() = Some(Arc::downgrade(&timer.state));
    active(&mut node);
    time(&node, 10);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_eq!(timer.time_since_last_call_ns(&node).unwrap(), 10_000_000);
    assert!(timer
        .state
        .with_timer(|token| token.is_ready(&node))
        .unwrap());
    spin(&mut node);
    assert_eq!(timer.next_call_time_ns(&node).unwrap(), 20_000_000);
    time(&node, 20);
    spin(&mut node);
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}
