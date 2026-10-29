//! Large cancellation cohorts must remain runnable without native traffic.

use super::*;

#[tokio::test(flavor = "current_thread")]
async fn availability_cancel_backlog_ack_observers_restore_idle_wait_after_completion() {
    use futures::FutureExt;
    use std::sync::atomic::Ordering;
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "availability_ack_backlog");
    let prefix = format!("/availability_ack_{}", uuid::Uuid::new_v4().simple());
    let typed = owned
        .create_publisher::<crate::std_msgs::msg::Int32>(
            &format!("{prefix}/typed"),
            QosProfile::default(),
        )
        .unwrap();
    let untyped = owned
        .create_publisher_untyped(
            &format!("{prefix}/untyped"),
            "std_msgs/msg/Int32",
            QosProfile::default(),
        )
        .unwrap();
    let natives = [
        typed.handle.upgrade().unwrap(),
        untyped.handle.upgrade().unwrap(),
    ];
    for native in &natives {
        native
            .ack_query_override
            .store(r2r_rcl::RCL_RET_TIMEOUT as i32, Ordering::Release);
    }
    let mut canceled = Vec::new();
    for _ in 0..512 {
        canceled.push(typed.wait_for_all_acked_async().unwrap().boxed());
        canceled.push(untyped.wait_for_all_acked_async().unwrap().boxed());
    }
    let live = vec![
        typed.wait_for_all_acked_async().unwrap().boxed(),
        untyped.wait_for_all_acked_async().unwrap().boxed(),
    ];
    let mut executor = Executor::new(context.clone());
    executor.add_node(owned).unwrap();
    let guard = FutureGuard::new(&context).unwrap();
    executor.cache_future_guard(guard.clone());
    drop(canceled);
    let task = executor.spawn().unwrap();
    let cleared = tokio::time::timeout(Duration::from_secs(3), async {
        while natives
            .iter()
            .any(|native| native.waiter_counts() != (0, 1))
        {
            tokio::task::yield_now().await;
        }
        while !guard.waiting.load(Ordering::Acquire)
            || guard.wait_timeout_ns.load(Ordering::Acquire) != 10_000_000
        {
            tokio::task::yield_now().await;
        }
    })
    .await;
    if cleared.is_err() {
        let _ = task.stop().await.unwrap();
        panic!("ACK cancellation backlog did not clear while preserving live observers");
    }
    // No native notification accompanies this query change. The remaining ACK observers
    // must complete through their own bounded cadence, then remove that cadence entirely.
    for native in &natives {
        native
            .ack_query_override
            .store(r2r_rcl::RCL_RET_OK as i32, Ordering::Release);
    }
    let observed =
        tokio::time::timeout(Duration::from_secs(3), futures::future::try_join_all(live)).await;
    let idle = tokio::time::timeout(Duration::from_secs(3), async {
        while !guard.waiting.load(Ordering::Acquire)
            || guard.wait_timeout_ns.load(Ordering::Acquire) != i64::MAX
        {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let _ = task.stop().await.unwrap();
    assert_eq!(observed.unwrap().unwrap().len(), 2);
    idle.expect("completed ACK observers retained their native polling cadence");
    assert!(natives
        .iter()
        .all(|native| native.waiter_counts() == (0, 0)));
}

#[tokio::test(flavor = "current_thread")]
async fn availability_cancel_backlog_continues_while_a_live_observer_remains_pending() {
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "availability_cancel_backlog");
    let client = owned
        .create_client::<AddTwoInts::Service>(
            &format!("/availability_cancel_backlog_{}", uuid::Uuid::new_v4().simple()),
            QosProfile::services_default(),
        )
        .unwrap();
    let canceled = (0..512)
        .map(|_| client.wait_for_service().unwrap())
        .collect::<Vec<_>>();
    let live = client.wait_for_service().unwrap();
    let native = client.client.upgrade().unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(owned).unwrap();
    // Drain creation/discovery readiness before cancellation, so graph traffic does not
    // accidentally supply the continuation missing from a bounded cleanup queue.
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        executor.spin_once(Duration::from_millis(20)).unwrap();
        if executor
            .future_wake_guard
            .as_ref()
            .unwrap()
            .wait_return
            .load(std::sync::atomic::Ordering::Acquire)
            == r2r_rcl::RCL_RET_TIMEOUT as i32
        {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
    }
    drop(canceled);
    assert_eq!(native.lock().poll_available_channels.len(), 513);
    let task = executor.spawn().unwrap();
    let cleared = tokio::time::timeout(Duration::from_secs(2), async {
        while native.lock().poll_available_channels.len() != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let recovered = task.stop().await.unwrap();
    drop(live);
    drop(recovered);
    cleared.expect("bounded availability cancellation stopped before clearing its backlog");
}

#[tokio::test(flavor = "current_thread")]
async fn availability_cancel_backlog_releases_an_already_dropped_native_client() {
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "availability_orphan_backlog");
    let client = owned
        .create_client::<AddTwoInts::Service>(
            &format!("/availability_orphan_backlog_{}", uuid::Uuid::new_v4().simple()),
            QosProfile::services_default(),
        )
        .unwrap();
    let canceled = (0..512)
        .map(|_| client.wait_for_service().unwrap())
        .collect::<Vec<_>>();
    let native = client.client.clone();
    let mut executor = Executor::new(context);
    executor.add_node(owned).unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        executor.spin_once(Duration::from_millis(20)).unwrap();
        if executor
            .future_wake_guard
            .as_ref()
            .unwrap()
            .wait_return
            .load(std::sync::atomic::Ordering::Acquire)
            == r2r_rcl::RCL_RET_TIMEOUT as i32
        {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
    }
    drop(client);
    drop(canceled);
    let task = executor.spawn().unwrap();
    let cleared = tokio::time::timeout(Duration::from_secs(2), async {
        while native.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let recovered = task.stop().await.unwrap();
    drop(recovered);
    cleared.expect("availability cleanup left a dropped native client registered");
}

#[tokio::test(flavor = "current_thread")]
async fn availability_cancel_backlog_handles_all_endpoint_kinds_then_returns_to_idle_wait() {
    use crate::example_interfaces::action::Fibonacci;
    use futures::FutureExt;
    use std::sync::atomic::Ordering;
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "availability_kind_matrix");
    let prefix = format!("/availability_kinds_{}", uuid::Uuid::new_v4().simple());
    let typed = owned
        .create_client::<AddTwoInts::Service>(
            &format!("{prefix}/typed"),
            QosProfile::services_default(),
        )
        .unwrap();
    let untyped = owned
        .create_client_untyped(
            &format!("{prefix}/untyped"),
            "example_interfaces/srv/AddTwoInts",
            QosProfile::services_default(),
        )
        .unwrap();
    let action = owned
        .create_action_client::<Fibonacci::Action>(&format!("{prefix}/action"))
        .unwrap();
    let json_action = owned
        .create_action_client_untyped(
            &format!("{prefix}/action"),
            "example_interfaces/action/Fibonacci",
        )
        .unwrap();
    let publisher = owned
        .create_publisher::<crate::std_msgs::msg::Int32>(
            &format!("{prefix}/published"),
            QosProfile::default(),
        )
        .unwrap();
    let subscription = owned
        .subscribe::<crate::std_msgs::msg::Int32>(
            &format!("{prefix}/subscribed"),
            QosProfile::default(),
        )
        .unwrap();
    let mut canceled = Vec::new();
    for _ in 0..512 {
        canceled.push(typed.wait_for_service().unwrap().boxed());
        canceled.push(untyped.wait_for_service().unwrap().boxed());
        canceled.push(action.wait_for_action_server().unwrap().boxed());
        canceled.push(json_action.wait_for_action_server().unwrap().boxed());
        canceled.push(publisher.wait_for_subscribers().unwrap().boxed());
        canceled.push(
            subscription
                .wait_for_publishers(&mut owned)
                .unwrap()
                .boxed(),
        );
    }
    let live = vec![
        typed.wait_for_service().unwrap().boxed(),
        untyped.wait_for_service().unwrap().boxed(),
        action.wait_for_action_server().unwrap().boxed(),
        json_action.wait_for_action_server().unwrap().boxed(),
        publisher.wait_for_subscribers().unwrap().boxed(),
        subscription
            .wait_for_publishers(&mut owned)
            .unwrap()
            .boxed(),
    ];
    let typed_native = typed.client.upgrade().unwrap();
    let untyped_native = untyped.client.upgrade().unwrap();
    let action_native = owned.action_clients.clone();
    let publisher_native = publisher.handle.upgrade().unwrap();
    let subscription_probe = owned
        .subscriber(subscription.id)
        .unwrap()
        .match_state()
        .unwrap()
        .waiters
        .cancellation_probe();
    let mut executor = Executor::new(context.clone());
    let id = executor.add_node(owned).unwrap();
    let guard = FutureGuard::new(&context).unwrap();
    executor.cache_future_guard(guard.clone());
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        executor.spin_once(Duration::from_millis(20)).unwrap();
        if guard.wait_return.load(Ordering::Acquire) == r2r_rcl::RCL_RET_TIMEOUT as i32 {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
    }
    // Include graph-only cancellation debris with no live sender; it must remain maintenance
    // work until its token queue is empty rather than depending on another graph change.
    let owned = executor.node_mut(id).unwrap();
    let graph_probe = owned.graph_change_waiters.cancellation_probe();
    let graph = (0..512)
        .map(|_| owned.wait_for_graph_change())
        .collect::<Vec<_>>();
    drop(graph);
    drop(canceled);
    let task = executor.spawn().unwrap();
    let cleared = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let counts = [
                typed_native.lock().poll_available_channels.len(),
                untyped_native.lock().poll_available_channels.len(),
                action_native[0].lock().availability_waiter_count(),
                action_native[1].lock().availability_waiter_count(),
                publisher_native.waiter_counts().0,
            ];
            if counts.iter().all(|&count| count == 1)
                && subscription_probe() == 0
                && graph_probe() == 0
            {
                break;
            }
            tokio::task::yield_now().await;
        }
        while !guard.waiting.load(Ordering::Acquire)
            || guard.wait_timeout_ns.load(Ordering::Acquire) != i64::MAX
        {
            tokio::task::yield_now().await;
        }
    })
    .await;
    if cleared.is_err() {
        let _ = task.stop().await.unwrap();
        panic!("endpoint cancellation queues did not clear and return to an indefinite idle wait");
    }
    let mut peer = node(&context, "availability_healthy_peer");
    let _typed_service = peer
        .create_service::<AddTwoInts::Service>(
            &format!("{prefix}/typed"),
            QosProfile::services_default(),
        )
        .unwrap();
    let _json_service = peer
        .create_service::<AddTwoInts::Service>(
            &format!("{prefix}/untyped"),
            QosProfile::services_default(),
        )
        .unwrap();
    let _action_server = peer
        .create_action_server::<Fibonacci::Action>(&format!("{prefix}/action"))
        .unwrap();
    let _subscription = peer
        .subscribe::<crate::std_msgs::msg::Int32>(
            &format!("{prefix}/published"),
            QosProfile::default(),
        )
        .unwrap();
    let _publisher = peer
        .create_publisher::<crate::std_msgs::msg::Int32>(
            &format!("{prefix}/subscribed"),
            QosProfile::default(),
        )
        .unwrap();
    let observed =
        tokio::time::timeout(Duration::from_secs(3), futures::future::try_join_all(live)).await;
    let _ = task.stop().await.unwrap();
    assert_eq!(observed.unwrap().unwrap().len(), 6);
}

#[tokio::test(flavor = "current_thread")]
async fn availability_cancel_backlog_releases_untyped_action_and_publisher_endpoint_kinds() {
    use crate::example_interfaces::action::Fibonacci;
    use futures::FutureExt;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "availability_orphan_matrix");
    let prefix = format!("/availability_orphans_{}", uuid::Uuid::new_v4().simple());
    let typed = owned
        .create_client::<AddTwoInts::Service>(
            &format!("{prefix}/typed"),
            QosProfile::services_default(),
        )
        .unwrap();
    let untyped = owned
        .create_client_untyped(
            &format!("{prefix}/untyped"),
            "example_interfaces/srv/AddTwoInts",
            QosProfile::services_default(),
        )
        .unwrap();
    let action = owned
        .create_action_client::<Fibonacci::Action>(&format!("{prefix}/action"))
        .unwrap();
    let json_action = owned
        .create_action_client_untyped(
            &format!("{prefix}/action"),
            "example_interfaces/action/Fibonacci",
        )
        .unwrap();
    let publisher = owned
        .create_publisher::<crate::std_msgs::msg::Int32>(
            &format!("{prefix}/published"),
            QosProfile::default(),
        )
        .unwrap();
    let mut canceled = Vec::new();
    for _ in 0..512 {
        canceled.push(typed.wait_for_service().unwrap().boxed());
        canceled.push(untyped.wait_for_service().unwrap().boxed());
        canceled.push(action.wait_for_action_server().unwrap().boxed());
        canceled.push(json_action.wait_for_action_server().unwrap().boxed());
        canceled.push(publisher.wait_for_subscribers().unwrap().boxed());
    }
    let typed_native = typed.client.clone();
    let untyped_native = untyped.client.clone();
    let action_native: Vec<_> = owned.action_clients.iter().map(Arc::downgrade).collect();
    let publisher_native = publisher.handle.clone();
    let mut executor = Executor::new(context.clone());
    let id = executor.add_node(owned).unwrap();
    let guard = FutureGuard::new(&context).unwrap();
    executor.cache_future_guard(guard.clone());
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    loop {
        executor.spin_once(Duration::from_millis(20)).unwrap();
        if guard.wait_return.load(Ordering::Acquire) == r2r_rcl::RCL_RET_TIMEOUT as i32 {
            break;
        }
        assert!(std::time::Instant::now() < deadline);
    }
    drop((typed, untyped, action, json_action, publisher));
    drop(canceled);
    let task = executor.spawn().unwrap();
    let cleared = tokio::time::timeout(Duration::from_secs(3), async {
        while typed_native.upgrade().is_some()
            || untyped_native.upgrade().is_some()
            || publisher_native.upgrade().is_some()
            || action_native
                .iter()
                .any(|native| native.upgrade().is_some())
        {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let mut recovered = task.stop().await.unwrap();
    let owned = recovered.node_mut(id).unwrap();
    assert!(owned.clients.is_empty());
    assert!(owned.action_clients.is_empty());
    assert!(owned.pubs.is_empty());
    cleared.expect("canceled availability retained an already dropped endpoint kind");
}
