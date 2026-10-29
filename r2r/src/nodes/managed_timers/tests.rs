use crate::nodes::future_executor::{FutureDriver, FutureGuard};
use crate::{CallbackTimerOptions, Context, Executor, LifecycleNode};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

#[tokio::test(flavor = "current_thread")]
async fn lifecycle_callback_timer_external_activation_wakes_current_native_owner_without_a_tick_future(
) {
    let context = Context::create().unwrap();
    let mut managed = LifecycleNode::create(context.clone(), "lc_callback_external", "").unwrap();
    let seen = Arc::new(AtomicUsize::new(0));
    let count = seen.clone();
    let timer = managed
        .create_wall_timer_callback_with_options(
            Duration::from_millis(1),
            CallbackTimerOptions::default().oneshot(true),
            move || {
                count.fetch_add(1, Ordering::Release);
                Ok(())
            },
        )
        .unwrap();
    let owned = managed.into_node();
    let state = owned.managed_timers.as_ref().unwrap().callbacks[0]
        .upgrade()
        .unwrap();
    let mut executor = Executor::new(context.clone());
    let guard = FutureGuard::new(&context).unwrap();
    executor.cache_future_guard(guard.clone());
    executor.add_node(owned).unwrap();
    let task = executor.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !guard.waiting.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline);
        tokio::task::yield_now().await;
    }
    let before = guard.triggers.load(Ordering::Acquire);
    state.set_node_active(true);
    let done = tokio::time::timeout(Duration::from_secs(3), async {
        while seen.load(Ordering::Acquire) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let triggered = guard.triggers.load(Ordering::Acquire) > before;
    let _executor = task.stop().await.unwrap();
    done.expect("external activation left a natively paused callback timer asleep");
    assert!(triggered);
    assert!(timer.is_valid());
}
