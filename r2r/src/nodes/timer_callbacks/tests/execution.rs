use super::*;
use crate::example_interfaces::{action::Fibonacci, srv::AddTwoInts};
use crate::nodes::future_executor::{FutureDriver, FutureGuard};
use futures::{channel::oneshot, StreamExt};

#[test]
fn callback_timer_error_preserves_other_ready_timers_and_can_reset() {
    let mut node = node("callback_timer_errors");
    let bad_count = Arc::new(AtomicUsize::new(0));
    let count = bad_count.clone();
    let bad = node
        .create_wall_timer_callback_with_options(
            Duration::ZERO,
            CallbackTimerOptions::default().oneshot(true),
            move || {
                if count.fetch_add(1, Ordering::Relaxed) == 0 {
                    Err(Error::RCL_RET_INVALID_ARGUMENT)
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
    let good_count = Arc::new(AtomicUsize::new(0));
    let count = good_count.clone();
    let _good = node
        .create_wall_timer_callback_with_options(
            Duration::ZERO,
            CallbackTimerOptions::default().oneshot(true),
            move || {
                count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap();
    assert!(matches!(
        node.try_spin_once(Duration::ZERO),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert_eq!(good_count.load(Ordering::Relaxed), 1);
    assert!(bad.is_canceled(&node).unwrap());
    bad.reset(&mut node).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(bad_count.load(Ordering::Relaxed), 2);
    assert_eq!(good_count.load(Ordering::Relaxed), 1);
    assert!(bad.is_valid());
}

#[test]
fn callback_timer_panic_is_outside_rcl_and_preserves_native_endpoint() {
    let mut node = node("callback_timer_panic");
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let timer = node
        .create_wall_timer_callback_with_options(
            Duration::ZERO,
            CallbackTimerOptions::default().oneshot(true),
            move || {
                if seen.fetch_add(1, Ordering::Relaxed) == 0 {
                    panic!("intentional callback timer unwind");
                }
                Ok(())
            },
        )
        .unwrap();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || node.try_spin_once(Duration::ZERO)
    ))
    .is_err());
    assert!(timer.is_valid() && timer.is_canceled(&node).unwrap());
    timer.reset(&mut node).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 2);
}

#[test]
fn callback_timer_shared_offsets_preserve_stream_sleep_service_and_action_readiness() {
    let context = Context::create().unwrap();
    let mut first = Node::create(context.clone(), "callback_timer_offsets_first", "").unwrap();
    let mut second = Node::create(context.clone(), "callback_timer_offsets_second", "").unwrap();
    let prefix = format!("/callback_timer_offsets_{}", uuid::Uuid::new_v4().simple());
    let _decoy = first
        .create_wall_timer_callback(Duration::from_secs(60), || Ok(()))
        .unwrap();
    let mut ordinary = second.create_wall_timer(Duration::from_millis(1)).unwrap();
    let sleep = second.sleep_for_ros(Duration::from_millis(1)).unwrap();
    let mut requests = second
        .create_action_server::<Fibonacci::Action>(&format!("{prefix}/action"))
        .unwrap();
    let action = first
        .create_action_client::<Fibonacci::Action>(&format!("{prefix}/action"))
        .unwrap();
    let _service = second
        .create_service_callback::<AddTwoInts::Service, _>(
            &format!("{prefix}/sum"),
            QosProfile::services_default(),
            |request, _| {
                Ok(AddTwoInts::Response {
                    sum: request.a + request.b,
                })
            },
        )
        .unwrap();
    let client = first
        .create_client::<AddTwoInts::Service>(
            &format!("{prefix}/sum"),
            QosProfile::services_default(),
        )
        .unwrap();
    let (seen, callback_seen) = oneshot::channel();
    let mut seen = Some(seen);
    let _timer = second
        .create_timer_callback_with_options(
            Duration::from_millis(1),
            CallbackTimerOptions::default().oneshot(true),
            move || {
                if let Some(seen) = seen.take() {
                    let _ = seen.send(());
                }
                Ok(())
            },
        )
        .unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(first).unwrap();
    executor.add_node(second).unwrap();
    executor
        .spin_until_future_complete_with_options(
            async {
                futures::try_join!(client.wait_for_service()?, action.wait_for_action_server()?)?;
                let response = client.request(&AddTwoInts::Request { a: 19, b: 23 })?;
                let accepted = action.send_goal_request(Fibonacci::Goal { order: 8 })?;
                let accept = async {
                    requests
                        .next()
                        .await
                        .ok_or(Error::RCL_RET_ACTION_SERVER_INVALID)?
                        .accept()
                };
                let ((), response, _, _, _) = futures::try_join!(
                    async {
                        callback_seen
                            .await
                            .map_err(|_| Error::RCL_RET_TIMER_INVALID)
                    },
                    response,
                    ordinary.tick(),
                    sleep,
                    async {
                        let _ = futures::try_join!(accepted, accept)?;
                        Ok(())
                    }
                )?;
                assert_eq!(response.sum, 42);
                Ok(())
            },
            Duration::from_secs(3),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn callback_timer_background_drop_uses_current_guard_and_releases_captured_state() {
    let context = Context::create().unwrap();
    let mut owned = Node::create(context.clone(), "callback_timer_background", "").unwrap();
    let old = FutureGuard::new(&context).unwrap();
    owned.cache_future_guard(old.clone());
    let mut executor = Executor::new(context.clone());
    let guard = FutureGuard::new(&context).unwrap();
    executor.cache_future_guard(guard.clone());
    let id = executor.add_node(owned).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    struct Capture(Arc<AtomicUsize>);
    impl Drop for Capture {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Release);
        }
    }
    let capture = Capture(count.clone());
    let token = executor
        .node_mut(id)
        .unwrap()
        .create_wall_timer_callback(Duration::from_secs(60), move || {
            let _ = &capture;
            Ok(())
        })
        .unwrap();
    let task = executor.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while !guard.waiting.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline);
        tokio::task::yield_now().await;
    }
    let before = guard.triggers.load(Ordering::Acquire);
    drop(token);
    let progress = tokio::time::timeout(Duration::from_secs(3), async {
        while count.load(Ordering::Acquire) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let triggered = guard.triggers.load(Ordering::Acquire) > before;
    let mut executor = task.stop().await.unwrap();
    progress.expect("callback timer capture remained owned during idle background execution");
    assert!(triggered);
    assert_eq!(old.triggers.load(Ordering::Acquire), 0);
    assert_eq!(
        executor
            .node_mut(id)
            .unwrap()
            .timer_callbacks
            .as_ref()
            .unwrap()
            .len(),
        0
    );
}

#[test]
fn callback_timer_returned_node_restores_foreground_wake_after_shared_owner_removal() {
    let context = Context::create().unwrap();
    let mut owned = Node::create(context.clone(), "callback_timer_reuse", "").unwrap();
    let original = FutureGuard::new(&context).unwrap();
    owned.cache_future_guard(original.clone());
    let mut executor = Executor::new(context.clone());
    let shared = FutureGuard::new(&context).unwrap();
    executor.cache_future_guard(shared.clone());
    let id = executor.add_node(owned).unwrap();
    let mut owned = executor.remove_node(id).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let capture = calls.clone();
    let timer = owned
        .create_wall_timer_callback(Duration::from_secs(60), move || {
            let _ = &capture;
            Ok(())
        })
        .unwrap();
    let queue = owned.timer_callbacks.as_ref().unwrap().canceled.clone();
    assert!(queue.lock().wake().is_none());
    let observed = original.clone();
    let worker = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !observed.waiting.load(Ordering::Acquire) {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        drop(timer);
    });
    let result = owned.spin_until_future_complete_with_options(
        futures::future::pending::<Result<()>>(),
        Duration::from_millis(150),
        FutureExecutionOptions::wake_driven(),
    );
    worker.join().unwrap();
    assert!(matches!(result, Err(Error::RCL_RET_TIMEOUT)));
    assert_eq!(owned.timer_callbacks.as_ref().unwrap().len(), 0);
    assert!(original.triggers.load(Ordering::Acquire) > 0);
    assert_eq!(shared.triggers.load(Ordering::Acquire), 0);
    assert!(queue
        .lock()
        .wake()
        .unwrap()
        .will_wake(&original.observer_waker()));
    assert_eq!(Arc::strong_count(&calls), 1);
}
