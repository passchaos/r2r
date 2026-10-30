use super::*;

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
