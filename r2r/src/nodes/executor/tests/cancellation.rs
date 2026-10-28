use super::*;

#[test]
fn cancellation_before_first_spin_is_sticky_and_does_not_allocate_a_guard() {
    let context = Context::create().unwrap();
    let mut executor = Executor::new(context.clone());
    let handle = executor.handle();
    assert!(handle.cancel().unwrap());
    assert!(handle.is_cancelled());
    let result = executor.spin_once(Duration::from_secs(2));
    assert!(matches!(result, Err(Error::ExecutorCancelled)));
    let polled = Cell::new(false);
    let result = executor.spin_until_future_complete(
        async {
            polled.set(true);
            Ok(17)
        },
        Duration::ZERO,
    );
    assert!(matches!(result, Err(Error::ExecutorCancelled)));
    assert!(!polled.get());
    assert!(executor.future_wake_guard.is_none());
    executor.spin().unwrap();
    assert!(context.is_valid());
    executor.reset_cancel();
    assert!(!handle.is_cancelled());
    assert_eq!(
        executor
            .spin_until_future_complete(async { Ok(17) }, Duration::ZERO)
            .unwrap(),
        17
    );
}

#[test]
fn cancellation_during_initial_poll_is_checked_before_native_guard_creation() {
    let mut executor = Executor::new(Context::create().unwrap());
    let handle = executor.handle();
    let future = futures::future::poll_fn(move |_| {
        assert!(handle.cancel().unwrap());
        std::task::Poll::Pending::<Result<()>>
    });
    let result = executor.spin_until_future_complete_with_options(
        future,
        Duration::from_secs(2),
        FutureExecutionOptions::wake_driven(),
    );
    assert!(matches!(result, Err(Error::ExecutorCancelled)));
    assert!(executor.future_wake_guard.is_none());
}

#[test]
fn cancellation_interrupts_native_wait_without_context_shutdown_or_polling() {
    let context = Context::create().unwrap();
    let mut executor = Executor::new(context.clone());
    let guard = guard(&mut executor);
    let observation = guard.clone();
    let handle = executor.handle();
    let producer = thread::spawn(move || {
        wait_for_native_wait(&observation);
        handle.cancel().unwrap();
    });
    let result = executor.spin_until_future_complete_with_options(
        futures::future::pending::<Result<()>>(),
        Duration::from_secs(2),
        FutureExecutionOptions::wake_driven(),
    );
    producer.join().unwrap();
    assert!(matches!(result, Err(Error::ExecutorCancelled)));
    assert_eq!(guard.wait_return.load(Ordering::Acquire), RCL_RET_OK as i32);
    assert!(guard.wait_timeout_ns.load(Ordering::Acquire) > 10_000_000);
    assert!(!guard.active.load(Ordering::Acquire));
    assert!(context.is_valid());
}

#[test]
fn repeated_concurrent_cancellation_requests_coalesce_to_one_native_trigger() {
    let mut executor = Executor::new(Context::create().unwrap());
    let guard = guard(&mut executor);
    let observation = guard.clone();
    let handle = executor.handle();
    let producer = thread::spawn(move || {
        wait_for_native_wait(&observation);
        handle.cancel().unwrap();
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let handle = handle.clone();
                thread::spawn(move || {
                    for _ in 0..1000 {
                        assert!(handle.cancel().unwrap());
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
    });
    let result = executor.spin_until_future_complete_with_options(
        futures::future::pending::<Result<()>>(),
        Duration::from_secs(2),
        FutureExecutionOptions::wake_driven(),
    );
    producer.join().unwrap();
    assert!(matches!(result, Err(Error::ExecutorCancelled)));
    assert_eq!(guard.triggers.load(Ordering::Acquire), 1);
}

#[test]
fn continuous_spin_stops_and_reuses_the_same_nodes_and_native_guard() {
    let context = Context::create().unwrap();
    let mut node = node(&context, "executor_cancel_reuse");
    let mut timer = node.create_wall_timer(Duration::from_millis(5)).unwrap();
    let mut executor = Executor::new(context.clone());
    let id = executor.add_node(node).unwrap();
    let guard = guard(&mut executor);
    let observation = guard.clone();
    let handle = executor.handle();
    let producer = thread::spawn(move || {
        wait_for_native_wait(&observation);
        handle.cancel().unwrap();
    });
    executor.spin().unwrap();
    producer.join().unwrap();
    assert!(executor.node_mut(id).is_ok());
    assert!(context.is_valid());
    executor.reset_cancel();
    executor
        .spin_until_future_complete_with_options(
            async {
                timer.tick().await?;
                Ok(())
            },
            Duration::from_secs(2),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    assert!(Arc::ptr_eq(executor.future_wake_guard.as_ref().unwrap(), &guard));
    executor.node_mut(id).unwrap().destroy_timer(timer);
}

#[test]
fn expired_handles_neither_keep_context_alive_nor_cancel_another_executor() {
    let context = Context::create().unwrap();
    let weak = Arc::downgrade(&context.context_handle);
    let mut executor = Executor::new(context);
    executor.spin_once(Duration::ZERO).unwrap();
    let handle = executor.handle();
    drop(executor);
    assert!(weak.upgrade().is_none());
    assert!(!handle.cancel().unwrap());
    let mut other = Executor::new(Context::create().unwrap());
    assert!(!handle.cancel().unwrap());
    assert!(!other.is_cancelled());
    assert_eq!(
        other
            .spin_until_future_complete(async { Ok(23) }, Duration::ZERO)
            .unwrap(),
        23
    );
}

#[test]
fn a_pending_operation_can_be_cancelled_again_after_reset() {
    let mut executor = Executor::new(Context::create().unwrap());
    let guard = guard(&mut executor);
    for _ in 0..3 {
        executor.reset_cancel();
        let observation = guard.clone();
        let handle = executor.handle();
        let producer = thread::spawn(move || {
            wait_for_native_wait(&observation);
            handle.cancel().unwrap();
        });
        let result = executor.spin_until_future_complete_with_options(
            futures::future::pending::<Result<()>>(),
            Duration::from_secs(2),
            FutureExecutionOptions::wake_driven(),
        );
        producer.join().unwrap();
        assert!(matches!(result, Err(Error::ExecutorCancelled)));
    }
    assert_eq!(guard.triggers.load(Ordering::Acquire), 3);
}
