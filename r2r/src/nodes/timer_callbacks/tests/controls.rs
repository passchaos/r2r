use super::*;

struct Retired {
    context: Context,
    clock: Arc<Mutex<Clock>>,
    count: Arc<AtomicUsize>,
}
impl Drop for Retired {
    fn drop(&mut self) {
        assert!(
            self.context.context_handle.try_lock().is_some(),
            "timer capture retired under context lock"
        );
        assert!(self.clock.try_lock().is_some(), "timer capture retired under clock lock");
        self.count.fetch_add(1, Ordering::Relaxed);
    }
}

fn retired(node: &Node, count: Arc<AtomicUsize>) -> Retired {
    Retired {
        context: node.context.clone(),
        clock: node.ros_clock.clone(),
        count,
    }
}

#[test]
fn callback_timer_control_foreign_ownership_and_native_address_are_stable() {
    let mut node = node("callback_timer_control");
    let mut foreign = Node::create(node.context.clone(), "callback_timer_foreign", "").unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let timer = node
        .create_wall_timer_callback_with_options(
            Duration::ZERO,
            CallbackTimerOptions::default().autostart(false),
            move || {
                seen.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap();
    assert!(timer.is_valid() && timer.is_canceled(&node).unwrap());
    let address = node.callback_timer(timer.identity()).unwrap().handle();
    let mut other = Vec::new();
    for _ in 0..32 {
        other.push(
            node.create_timer_callback_with_options(
                Duration::from_secs(30),
                CallbackTimerOptions::default().autostart(false),
                || Ok(()),
            )
            .unwrap(),
        );
    }
    assert_eq!(node.callback_timer(timer.identity()).unwrap().handle(), address);
    assert!(matches!(timer.cancel(&mut foreign), Err(Error::RCL_RET_TIMER_INVALID)));
    assert!(matches!(timer.reset(&mut foreign), Err(Error::RCL_RET_TIMER_INVALID)));
    assert!(matches!(timer.period(&foreign), Err(Error::RCL_RET_TIMER_INVALID)));
    assert!(matches!(
        timer.time_until_next_call_ns(&node),
        Err(Error::RCL_RET_TIMER_CANCELED)
    ));
    timer.reset(&mut node).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 1);
    timer.cancel(&mut node).unwrap();
    assert_eq!(node.callback_timer(timer.identity()).unwrap().handle(), address);
    let alive = node.callback_timer(timer.identity()).unwrap().alive();
    node.destroy_callback_timer(timer);
    assert!(!alive.load(Ordering::Acquire));
    assert_eq!(node.timer_callbacks.as_ref().unwrap().len(), 32);
    let foreign_token = other.pop().unwrap();
    foreign.destroy_callback_timer(foreign_token);
    assert_eq!(node.timer_callbacks.as_ref().unwrap().len(), 32);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(node.timer_callbacks.as_ref().unwrap().len(), 31);
}

#[test]
fn callback_timer_captures_retire_without_internal_locks_on_failure_drop_and_destroy() {
    let count = Arc::new(AtomicUsize::new(0));
    let mut node = node("callback_timer_retirement");
    let capture = retired(&node, count.clone());
    assert!(matches!(
        node.create_timer_callback(Duration::MAX, move || {
            let _ = &capture;
            Ok(())
        }),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert_eq!(count.load(Ordering::Relaxed), 1);
    assert!(node.timer_callbacks.is_none());
    let capture = retired(&node, count.clone());
    let timer = node
        .create_timer_callback(Duration::from_secs(30), move || {
            let _ = &capture;
            Ok(())
        })
        .unwrap();
    node.destroy_callback_timer(timer);
    assert_eq!(count.load(Ordering::Relaxed), 2);
    let capture = retired(&node, count.clone());
    let timer = node
        .create_wall_timer_callback(Duration::from_secs(30), move || {
            let _ = &capture;
            Ok(())
        })
        .unwrap();
    drop(timer);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 3);
    let capture = retired(&node, count.clone());
    let timer = node
        .create_timer_callback(Duration::from_secs(30), move || {
            let _ = &capture;
            Ok(())
        })
        .unwrap();
    drop(node);
    assert_eq!(count.load(Ordering::Relaxed), 4);
    assert!(!timer.is_valid());
}

#[test]
fn callback_timer_drop_after_native_wait_suppresses_the_ready_callback() {
    let mut node = node("callback_timer_stale_ready");
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let token = node
        .create_wall_timer_callback(Duration::ZERO, move || {
            seen.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .unwrap();
    let preparation = node.prepare_spin(Duration::ZERO).unwrap();
    let context = node.context.clone();
    let (wait_set, mut entities) = node.wait_entities();
    {
        let mut context = context.context_handle.lock();
        wait_set
            .prepare(preparation.capacities, context.as_mut())
            .unwrap();
    }
    let offsets = entities
        .add_ordinary(&mut wait_set.handle, &preparation)
        .unwrap();
    wait_set.refresh_event_layout(&context).unwrap();
    check_rcl_ret(unsafe { rcl_wait(&mut wait_set.handle, 0) }).unwrap();
    drop(token);
    entities.dispatch(&wait_set.handle, offsets).unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 0);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(node.timer_callbacks.as_ref().unwrap().len(), 0);
}
