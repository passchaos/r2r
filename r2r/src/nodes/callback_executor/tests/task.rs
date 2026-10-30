use super::scheduling::Release;
use super::*;
use std::sync::{Condvar, Mutex as StdMutex};

struct Capture(Arc<AtomicUsize>);
impl Drop for Capture {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Release);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn callback_group_task_panic_cancels_native_wait_before_aggregate_poll_and_releases_captures()
{
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "callback_group_task_panic", "").unwrap();
    let mut group = CallbackGroup::new();
    let id = group.id();
    let retired = Arc::new(AtomicUsize::new(0));
    let capture = Capture(retired.clone());
    let _timer = node
        .create_wall_timer_callback_in_group_with_options(
            &mut group,
            Duration::ZERO,
            CallbackTimerOptions::default().oneshot(true),
            move || {
                let _ = &capture;
                panic!("intentional callback group panic");
            },
        )
        .unwrap();
    let mut executor = CallbackExecutor::new(context, 2).unwrap();
    executor.add_node(node).unwrap();
    executor.add_group(group).unwrap();
    let task = executor.spawn().unwrap();
    let control = task.handle();
    let complete = tokio::time::timeout(Duration::from_secs(3), async {
        while !task.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let result = task.await;
    complete.expect("callback panic left a native worker running until task polling");
    assert!(matches!(
        result,
        Err(CallbackExecutorTaskError::Worker {
            source: CallbackWorkerError::Panic,
            ..
        })
    ));
    assert_eq!(retired.load(Ordering::Acquire), 1);
    assert!(!control.cancel().unwrap());
    let _ = id;
}

#[tokio::test(flavor = "current_thread")]
async fn callback_group_task_drop_returns_before_a_blocked_handler_and_retires_it_after_release() {
    use futures::channel::oneshot;
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "callback_group_task_drop", "").unwrap();
    let mut group = CallbackGroup::new();
    let gate = Arc::new((StdMutex::new(false), Condvar::new()));
    let release = Release(gate.clone());
    let retired = Arc::new(AtomicUsize::new(0));
    let capture = Capture(retired.clone());
    let (started, start) = oneshot::channel();
    let mut started = Some(started);
    let _timer = node
        .create_wall_timer_callback_in_group_with_options(
            &mut group,
            Duration::ZERO,
            CallbackTimerOptions::default().oneshot(true),
            move || {
                let _ = &capture;
                if let Some(started) = started.take() {
                    let _ = started.send(());
                }
                let (lock, ready) = &*gate;
                let mut released = lock.lock().unwrap();
                while !*released {
                    released = ready.wait(released).unwrap();
                }
                Ok(())
            },
        )
        .unwrap();
    let mut executor = CallbackExecutor::new(context, 2).unwrap();
    executor.add_node(node).unwrap();
    executor.add_group(group).unwrap();
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(3), start)
        .await
        .unwrap()
        .unwrap();
    let control = task.handle();
    drop(task);
    assert_eq!(retired.load(Ordering::Acquire), 0, "drop joined a still-blocked native handler");
    drop(release);
    tokio::time::timeout(Duration::from_secs(3), async {
        while retired.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(retired.load(Ordering::Acquire), 1);
    tokio::time::timeout(Duration::from_secs(3), async {
        while control.cancel().unwrap() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
