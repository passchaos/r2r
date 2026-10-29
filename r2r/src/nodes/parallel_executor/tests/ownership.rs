use super::*;
use crate::example_interfaces::srv::AddTwoInts;

#[tokio::test(flavor = "current_thread")]
async fn parallel_node_executor_native_wait_sets_and_guards_are_disjoint_and_reused() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let context = Context::create().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut executor = ParallelExecutor::new(context.clone(), 3).unwrap();
    let mut timers = Vec::new();
    for index in 0..3 {
        let mut node =
            Node::create(context.clone(), &format!("parallel_storage_{index}"), "").unwrap();
        let count = calls.clone();
        let timer = node
            .create_wall_timer_callback_with_options(
                Duration::ZERO,
                CallbackTimerOptions::default().oneshot(true),
                move || {
                    count.fetch_add(1, Ordering::Release);
                    Ok(())
                },
            )
            .unwrap();
        timers.push((executor.add_node(node).unwrap(), timer));
    }
    let mut previous = None;
    for cycle in 1..=2 {
        let task = executor.spawn().unwrap();
        let ready = tokio::time::timeout(Duration::from_secs(3), async {
            while calls.load(Ordering::Acquire) < cycle * 3 {
                tokio::task::yield_now().await;
            }
        })
        .await;
        executor = task.stop().await.unwrap();
        ready.unwrap();
        let storage: Vec<_> = executor
            .workers
            .iter()
            .map(Executor::native_wait_identity)
            .collect();
        let waits: std::collections::HashSet<_> = storage.iter().map(|(wait, _)| *wait).collect();
        let guards: std::collections::HashSet<_> =
            storage.iter().map(|(_, guard)| *guard).collect();
        assert!(!waits.contains(&0));
        assert!(!guards.contains(&0));
        assert_eq!(waits.len(), 3, "two workers shared native wait-set storage");
        assert_eq!(guards.len(), 3, "two workers registered the same guard");
        if let Some(previous) = previous.replace(storage.clone()) {
            assert_eq!(storage, previous, "stop/restart replaced cached native storage");
        }
        executor.reset_cancel();
        for (id, timer) in &timers {
            timer.reset(executor.node_mut(*id).unwrap()).unwrap();
        }
    }
}

#[test]
fn parallel_node_executor_configuration_rejects_zero_budget_and_preserves_stable_ids() {
    fn require_send<T: Send>() {}
    fn require_send_sync<T: Send + Sync>() {}
    require_send::<ParallelExecutor>();
    require_send::<ParallelExecutorTask>();
    require_send_sync::<ParallelExecutorHandle>();
    let context = Context::create().unwrap();
    assert!(matches!(
        ParallelExecutor::new(context.clone(), 0),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    let mut executor = ParallelExecutor::new(context.clone(), 3).unwrap();
    assert_eq!(executor.worker_count(), 3);
    assert!(executor.is_empty());
    let first = executor
        .add_node(Node::create(context.clone(), "parallel_config_first", "").unwrap())
        .unwrap();
    let removed = executor
        .add_node(Node::create(context.clone(), "parallel_config_removed", "").unwrap())
        .unwrap();
    let last = executor
        .add_node(Node::create(context.clone(), "parallel_config_last", "").unwrap())
        .unwrap();
    assert_eq!(executor.placements[&first], 0);
    assert_eq!(executor.placements[&removed], 1);
    assert_eq!(executor.placements[&last], 2);
    assert!(matches!(
        executor.add_node_to_worker(
            Node::create(context.clone(), "parallel_bad_index", "").unwrap(),
            3
        ),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    // Context::create shares the live process context. Exercise accepted re-acquisition,
    // rather than pretending it constructs a distinct context while one is alive.
    let reacquired = executor
        .add_node(
            Node::create(Context::create().unwrap(), "parallel_reacquired_context", "").unwrap(),
        )
        .unwrap();
    assert!(executor.node_mut(reacquired).is_ok());
    drop(executor.remove_node(reacquired).unwrap());
    let owned = executor.remove_node(removed).unwrap();
    drop(owned);
    assert!(matches!(executor.node_mut(removed), Err(Error::RCL_RET_NODE_INVALID)));
    assert_eq!(executor.len(), 2);
    assert!(executor.node_mut(last).is_ok());
    assert!(executor.node_mut(first).is_ok());
}

#[tokio::test(flavor = "current_thread")]
async fn parallel_node_executor_worker_budget_reuses_nodes_and_native_guard_ownership_after_stop() {
    let context = Context::create().unwrap();
    let mut executor = ParallelExecutor::new(context.clone(), 4).unwrap();
    let id = executor
        .add_node(Node::create(context, "parallel_reuse", "").unwrap())
        .unwrap();
    let control = executor.handle();
    let mut executor = executor.spawn().unwrap().stop().await.unwrap();
    assert!(executor.is_cancelled());
    executor.reset_cancel();
    assert!(!executor.is_cancelled());
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let seen = count.clone();
    let _timer = executor
        .node_mut(id)
        .unwrap()
        .create_wall_timer_callback_with_options(
            Duration::ZERO,
            CallbackTimerOptions::default().oneshot(true),
            move || {
                seen.fetch_add(1, std::sync::atomic::Ordering::Release);
                Ok(())
            },
        )
        .unwrap();
    let task = executor.spawn().unwrap();
    assert_eq!(task.worker_count(), 1);
    let ready = tokio::time::timeout(Duration::from_secs(3), async {
        while count.load(std::sync::atomic::Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await;
    control.cancel().unwrap();
    let mut executor = task.await.unwrap();
    ready.unwrap();
    assert_eq!(count.load(std::sync::atomic::Ordering::Acquire), 1);
    assert!(executor.node_mut(id).is_ok());
    drop(executor);
    assert!(!control.cancel().unwrap());
}

#[tokio::test(flavor = "current_thread")]
async fn parallel_node_executor_native_service_crosses_distinct_workers_and_retains_response_identity(
) {
    let context = Context::create().unwrap();
    let mut server = Node::create(context.clone(), "parallel_service_server", "").unwrap();
    let mut client_node = Node::create(context.clone(), "parallel_service_client", "").unwrap();
    let name = format!("/parallel_service_{}", uuid::Uuid::new_v4().simple());
    let observed = Arc::new(Mutex::new(Vec::new()));
    let sequence = observed.clone();
    let _service = server
        .create_service_callback::<AddTwoInts::Service, _>(
            &name,
            QosProfile::services_default(),
            move |request, info| {
                sequence.lock().push(info.sequence_number);
                Ok(AddTwoInts::Response {
                    sum: request.a + request.b,
                })
            },
        )
        .unwrap();
    let client = client_node
        .create_client::<AddTwoInts::Service>(&name, QosProfile::services_default())
        .unwrap();
    let mut executor = ParallelExecutor::new(context, 2).unwrap();
    executor.add_node_to_worker(server, 0).unwrap();
    executor.add_node_to_worker(client_node, 1).unwrap();
    let task = executor.spawn().unwrap();
    assert_eq!(task.worker_count(), 2);
    let application = tokio::time::timeout(Duration::from_secs(3), async {
        client.wait_for_service().unwrap().await.unwrap();
        for a in 0..32 {
            let response = client
                .request_with_info(&AddTwoInts::Request { a, b: 11 })
                .unwrap()
                .await
                .unwrap();
            assert_eq!(response.response.sum, a + 11);
            assert_eq!(Some(&response.info.sequence_number), observed.lock().last());
        }
    })
    .await;
    let executor = task.stop().await.unwrap();
    application.unwrap();
    assert_eq!(executor.len(), 2);
}

#[tokio::test(flavor = "current_thread")]
async fn parallel_node_executor_empty_groups_cancel_and_shutdown_without_extra_node_workers() {
    const CHILD: &str = "R2R_PARALLEL_SHUTDOWN_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "nodes::parallel_executor::tests::ownership::parallel_node_executor_empty_groups_cancel_and_shutdown_without_extra_node_workers", "--test-threads=1"])
            .env(CHILD, "1").status().unwrap();
        assert!(status.success());
        return;
    }
    let context = Context::create().unwrap();
    let executor = ParallelExecutor::new(context.clone(), 3).unwrap();
    let task = executor.spawn().unwrap();
    assert_eq!(task.worker_count(), 1);
    let mut executor = task.stop().await.unwrap();
    assert!(executor.is_empty());
    executor.reset_cancel();
    for index in 0..3 {
        executor
            .add_node(
                Node::create(context.clone(), &format!("parallel_shutdown_{index}"), "").unwrap(),
            )
            .unwrap();
    }
    let task = executor.spawn().unwrap();
    assert_eq!(task.worker_count(), 3);
    tokio::time::sleep(Duration::from_millis(20)).await;
    context.shutdown().unwrap();
    let executor = tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(executor.len(), 3);
    assert_eq!(executor.worker_count(), 3);
    assert!(matches!(ParallelExecutor::new(context, 2), Err(Error::RCL_RET_NOT_INIT)));
    assert!(matches!(
        executor.spawn(),
        Err(ParallelExecutorTaskError::Worker {
            worker: 0,
            source: ExecutorTaskError::Native(Error::RCL_RET_NOT_INIT),
        })
    ));
}
