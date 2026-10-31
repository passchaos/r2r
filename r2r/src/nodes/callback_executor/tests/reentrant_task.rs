use super::*;
use std::sync::{Condvar, Mutex as StdMutex};

struct Capture(Arc<AtomicUsize>);
impl Drop for Capture {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Release);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn reentrant_group_task_panic_cancels_native_wait_before_aggregate_poll_and_releases_captures(
) {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "reentrant_group_task_panic", "").unwrap();
    let mut group = ReentrantCallbackGroup::new(2).unwrap();
    let id = group.id();
    let retired = Arc::new(AtomicUsize::new(0));
    let capture = Capture(retired.clone());
    let _timer = node
        .create_wall_timer_reentrant_callback_with_options(
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
    executor.add_reentrant_group(group).unwrap();
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
async fn reentrant_group_tasks_overlap_only_up_to_the_lane_bound() {
    let context = Context::create().unwrap();
    let mut group = ReentrantCallbackGroup::new(2).unwrap();
    let group_id = group.id();
    let entered = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new((StdMutex::new(false), Condvar::new()));
    let mut registrations = Vec::new();
    for _ in 0..3 {
        let entered = entered.clone();
        let gate = gate.clone();
        registrations.push(group.spawn_task(futures::future::poll_fn(move |_| {
            entered.fetch_add(1, Ordering::AcqRel);
            let (lock, ready) = &*gate;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = ready.wait(released).unwrap();
            }
            Poll::Ready(Ok(()))
        })));
    }
    assert_eq!(
        group
            .lanes
            .iter()
            .map(|lane| lane.len())
            .collect::<Vec<_>>(),
        [2, 1]
    );

    let mut executor = CallbackExecutor::new(context, 3).unwrap();
    executor.add_reentrant_group(group).unwrap();
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while entered.load(Ordering::Acquire) < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(entered.load(Ordering::Acquire), 2);

    {
        let (lock, ready) = &*gate;
        *lock.lock().unwrap() = true;
        ready.notify_all();
    }
    tokio::time::timeout(Duration::from_secs(3), async {
        while !registrations.iter().all(CallbackGroupTask::is_finished) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(entered.load(Ordering::Acquire), 3);

    let mut executor = task.stop().await.unwrap();
    assert!(executor.reentrant_group_mut(group_id).unwrap().is_empty());
}
