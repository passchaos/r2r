use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Capture(Arc<AtomicUsize>);
impl Drop for Capture {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::Release);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn parallel_node_executor_simultaneous_errors_report_the_lowest_worker_after_cleanup() {
    let context = Context::create().unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let retired = Arc::new(AtomicUsize::new(0));
    let mut executor = ParallelExecutor::new(context.clone(), 4).unwrap();
    let mut timers = Vec::new();
    for index in 0..2 {
        let mut node =
            Node::create(context.clone(), &format!("parallel_both_errors_{index}"), "").unwrap();
        let rendezvous = barrier.clone();
        let capture = Capture(retired.clone());
        timers.push(
            node.create_wall_timer_callback_with_options(
                Duration::ZERO,
                CallbackTimerOptions::default().oneshot(true),
                move || {
                    let _ = &capture;
                    rendezvous.wait();
                    Err(if index == 0 {
                        Error::RCL_RET_INVALID_ARGUMENT
                    } else {
                        Error::RCL_RET_ERROR
                    })
                },
            )
            .unwrap(),
        );
        executor.add_node_to_worker(node, 1 + index * 2).unwrap();
    }
    let result = tokio::time::timeout(Duration::from_secs(3), executor.spawn().unwrap())
        .await
        .unwrap();
    assert!(matches!(
        result,
        Err(ParallelExecutorTaskError::Worker {
            worker: 1,
            source: ExecutorTaskError::Native(Error::RCL_RET_INVALID_ARGUMENT),
        })
    ));
    assert_eq!(retired.load(Ordering::Acquire), 2);
    drop(timers);
}

#[tokio::test(flavor = "current_thread")]
async fn parallel_node_executor_worker_failure_cancels_peers_before_the_task_is_polled() {
    let context = Context::create().unwrap();
    let mut bad = Node::create(context.clone(), "parallel_error_bad", "").unwrap();
    let mut peer = Node::create(context.clone(), "parallel_error_peer", "").unwrap();
    let retired = Arc::new(AtomicUsize::new(0));
    let capture = Capture(retired.clone());
    let _bad = bad
        .create_wall_timer_callback_with_options(
            Duration::ZERO,
            CallbackTimerOptions::default().oneshot(true),
            move || {
                let _ = &capture;
                Err(Error::RCL_RET_INVALID_ARGUMENT)
            },
        )
        .unwrap();
    let capture = Capture(retired.clone());
    let _peer = peer
        .create_wall_timer_callback(Duration::from_secs(60), move || {
            let _ = &capture;
            Ok(())
        })
        .unwrap();
    let mut executor = ParallelExecutor::new(context, 2).unwrap();
    executor.add_node(bad).unwrap();
    executor.add_node(peer).unwrap();
    let task = executor.spawn().unwrap();
    let progress = tokio::time::timeout(Duration::from_secs(3), async {
        while !task.is_finished() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    // No poll of task completion took place before observing every native worker finish.
    let result = task.await;
    progress.expect("failed worker left its idle peer running until aggregate polling");
    assert!(matches!(
        result,
        Err(ParallelExecutorTaskError::Worker {
            worker: 0,
            source: ExecutorTaskError::Native(Error::RCL_RET_INVALID_ARGUMENT)
        })
    ));
    assert_eq!(retired.load(Ordering::Acquire), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn parallel_node_executor_worker_panic_releases_every_group_before_returning_error() {
    let context = Context::create().unwrap();
    let mut bad = Node::create(context.clone(), "parallel_panic_bad", "").unwrap();
    let mut peer = Node::create(context.clone(), "parallel_panic_peer", "").unwrap();
    let retired = Arc::new(AtomicUsize::new(0));
    let capture = Capture(retired.clone());
    let _bad = bad
        .create_wall_timer_callback_with_options(
            Duration::ZERO,
            CallbackTimerOptions::default().oneshot(true),
            move || {
                let _ = &capture;
                panic!("intentional parallel worker panic");
            },
        )
        .unwrap();
    let capture = Capture(retired.clone());
    let _peer = peer
        .create_wall_timer_callback(Duration::from_secs(60), move || {
            let _ = &capture;
            Ok(())
        })
        .unwrap();
    let mut executor = ParallelExecutor::new(context, 2).unwrap();
    executor.add_node(bad).unwrap();
    executor.add_node(peer).unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), executor.spawn().unwrap())
        .await
        .unwrap();
    assert!(matches!(
        result,
        Err(ParallelExecutorTaskError::Worker {
            worker: 0,
            source: ExecutorTaskError::ThreadPanicked
        })
    ));
    assert_eq!(retired.load(Ordering::Acquire), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn parallel_node_executor_task_drop_cancels_all_groups_without_blocking_the_caller() {
    let context = Context::create().unwrap();
    let retired = Arc::new(AtomicUsize::new(0));
    let mut executor = ParallelExecutor::new(context.clone(), 2).unwrap();
    let mut tokens = Vec::new();
    for index in 0..2 {
        let mut node =
            Node::create(context.clone(), &format!("parallel_drop_{index}"), "").unwrap();
        let capture = Capture(retired.clone());
        let timer = node
            .create_wall_timer_callback(Duration::from_secs(60), move || {
                let _ = &capture;
                Ok(())
            })
            .unwrap();
        tokens.push(timer);
        executor.add_node(node).unwrap();
    }
    let task = executor.spawn().unwrap();
    drop(task);
    tokio::time::timeout(Duration::from_secs(3), async {
        while retired.load(Ordering::Acquire) != 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    drop(tokens);
}
