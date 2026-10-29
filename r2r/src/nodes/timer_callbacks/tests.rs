use super::*;
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

mod clocks;
mod controls;
mod execution;

fn node(name: &str) -> Node {
    Node::create(Context::create().unwrap(), name, "").unwrap()
}
fn until(node: &mut Node, done: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !done() {
        assert!(Instant::now() < deadline);
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

#[test]
fn callback_timer_wall_periodic_and_oneshot_share_native_dispatch() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "callback_timer_basic", "").unwrap();
    let periodic = Arc::new(AtomicUsize::new(0));
    let once = Arc::new(AtomicUsize::new(0));
    let seen = periodic.clone();
    let timer = node
        .create_wall_timer_callback(Duration::from_millis(1), move || {
            seen.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .unwrap();
    let seen = once.clone();
    let one = node
        .create_wall_timer_callback_with_options(
            Duration::from_millis(1),
            CallbackTimerOptions::default().oneshot(true),
            move || {
                seen.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    until(&mut node, || periodic.load(Ordering::Relaxed) >= 3);
    assert_eq!(once.load(Ordering::Relaxed), 1);
    assert!(one.is_canceled(&node).unwrap());
    assert!(node.timers.is_empty(), "native callback allocated a tick stream timer");
    assert!(timer.is_steady());
    one.reset(&mut node).unwrap();
    while once.load(Ordering::Relaxed) < 2 {
        assert!(Instant::now() < deadline);
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
    assert!(one.is_canceled(&node).unwrap());
    timer.cancel(&mut node).unwrap();
    assert!(timer.is_canceled(&node).unwrap());
    assert_eq!(
        timer
            .set_period(&mut node, Duration::from_millis(5))
            .unwrap(),
        Duration::from_millis(1)
    );
    assert_eq!(timer.period(&node).unwrap(), Duration::from_millis(5));
}
