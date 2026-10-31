use super::*;
use futures::{
    stream::StreamExt,
    task::{waker_ref, ArcWake},
    FutureExt,
};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::Arc,
    task::{Context as TaskContext, Poll},
};

fn node(name: &str) -> Node {
    Node::create(
        Context::create().unwrap(),
        &format!("{name}_{}", uuid::Uuid::new_v4().simple()),
        "",
    )
    .unwrap()
}

struct PanicWake;
impl ArcWake for PanicWake {
    fn wake_by_ref(_arc_self: &Arc<Self>) {
        panic!("intentional reset observer waker panic");
    }
}

fn register_panicking_waker(events: &mut TimerResetEvents) {
    let wake = Arc::new(PanicWake);
    let waker = waker_ref(&wake);
    let mut context = TaskContext::from_waker(&waker);
    assert!(matches!(Pin::new(events).poll_next(&mut context), Poll::Pending));
}

#[test]
fn ordinary_timer_reset_events_replay_coalesce_transfer_and_close() {
    let mut node = node("timer_reset_events");
    let timer = node
        .create_wall_timer_with_options(
            Duration::from_secs(30),
            TimerOptions::default().autostart(false),
        )
        .unwrap();

    timer.reset(&mut node).unwrap();
    timer.reset(&mut node).unwrap();
    let mut events = timer.observe_resets(&mut node).unwrap();
    assert!(matches!(timer.observe_resets(&mut node), Err(Error::TimerResetAlreadyObserved)));
    assert_eq!(events.next().now_or_never(), Some(Some(TimerReset { count: 2, total: 2 })));
    assert_eq!(events.pending(), 0);
    assert!(events.next().now_or_never().is_none());

    register_panicking_waker(&mut events);
    assert!(matches!(catch_unwind(AssertUnwindSafe(|| timer.reset(&mut node))), Ok(Ok(()))));
    assert_eq!(events.next().now_or_never(), Some(Some(TimerReset { count: 1, total: 3 })));

    drop(events);
    timer.reset(&mut node).unwrap();
    let mut replacement = timer.observe_resets(&mut node).unwrap();
    assert_eq!(replacement.next().now_or_never(), Some(Some(TimerReset { count: 1, total: 4 })));
    node.destroy_timer(timer);
    assert_eq!(replacement.next().now_or_never(), Some(None));
}

#[test]
fn callback_and_grouped_timers_share_reset_event_contract() {
    let mut node = node("callback_timer_reset_events");
    let callback = node
        .create_wall_timer_callback_with_options(
            Duration::from_secs(30),
            CallbackTimerOptions::default().autostart(false),
            || Ok(()),
        )
        .unwrap();
    callback.reset(&mut node).unwrap();
    let mut callback_events = callback.observe_resets(&mut node).unwrap();
    assert_eq!(
        callback_events.next().now_or_never(),
        Some(Some(TimerReset { count: 1, total: 1 }))
    );
    node.destroy_callback_timer(callback);
    assert_eq!(callback_events.next().now_or_never(), Some(None));

    let mut group = CallbackGroup::new();
    let grouped = node
        .create_wall_timer_callback_in_group_with_options(
            &mut group,
            Duration::from_secs(30),
            CallbackTimerOptions::default().autostart(false),
            || Ok(()),
        )
        .unwrap();
    grouped.reset().unwrap();
    grouped.reset().unwrap();
    let mut grouped_events = grouped.observe_resets().unwrap();
    assert!(matches!(grouped.observe_resets(), Err(Error::TimerResetAlreadyObserved)));
    assert_eq!(
        grouped_events.next().now_or_never(),
        Some(Some(TimerReset { count: 2, total: 2 }))
    );
    drop(grouped);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(grouped_events.next().now_or_never(), Some(None));
}
