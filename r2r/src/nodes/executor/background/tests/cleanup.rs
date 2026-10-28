//! Dropping public entities must wake an otherwise idle native executor.

use super::*;
use std::sync::Arc;
use std::{sync::atomic::Ordering, time::Instant};

#[tokio::test(flavor = "current_thread")]
async fn public_entity_drop_wakes_and_releases_idle_native_endpoints() {
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_entity_drop");
    let topic = format!("/background_drop_{}", uuid::Uuid::new_v4().simple());
    let publisher = owned
        .create_publisher::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default())
        .unwrap();
    let subscription = owned
        .subscribe::<crate::std_msgs::msg::Int32>(&topic, QosProfile::default())
        .unwrap();
    let client = owned
        .create_client::<AddTwoInts::Service>(
            &format!("{topic}/missing"),
            QosProfile::services_default(),
        )
        .unwrap();
    let service = owned
        .create_service::<AddTwoInts::Service>(
            &format!("{topic}/service"),
            QosProfile::services_default(),
        )
        .unwrap();
    let timer = owned.create_wall_timer(Duration::from_secs(60)).unwrap();
    let native_publisher = publisher.handle.clone();
    let native_client = client.client.clone();
    let mut executor = Executor::new(context.clone());
    let id = executor.add_node(owned).unwrap();
    executor
        .spin_until_future_complete_with_options(
            publisher.wait_for_subscribers().unwrap(),
            Duration::from_secs(5),
            crate::FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    let guard = executor.future_wake_guard.clone().unwrap();
    let task = executor.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !guard.waiting.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline);
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    let triggers = guard.triggers.load(Ordering::Relaxed);
    drop(subscription);
    drop(service);
    drop(client);
    drop(publisher);
    drop(timer);
    let wake_observed = guard.triggers.load(Ordering::Relaxed) > triggers;
    let cleanup = tokio::time::timeout(Duration::from_millis(300), async {
        while native_publisher.upgrade().is_some() || native_client.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let mut recovered = task.stop().await.unwrap();
    assert!(wake_observed, "entity destruction did not notify the native wait");
    cleanup.expect("dropped native endpoints remained live until an unrelated event");
    let node = recovered.node_mut(id).unwrap();
    assert_eq!(node.timer_count(), 0);
    assert!(node.subscribers.is_empty());
    assert!(node.services.is_empty());
    assert!(node.clients.is_empty());
    assert!(node.pubs.is_empty());
    assert!(context.is_valid());
}

#[tokio::test(flavor = "current_thread")]
async fn last_pending_response_cancellation_releases_already_dropped_clients() {
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_pending_drop");
    let name = format!("/background_pending_{}", uuid::Uuid::new_v4().simple());
    let mut service = owned
        .create_service::<AddTwoInts::Service>(&name, QosProfile::services_default())
        .unwrap();
    let client = owned
        .create_client::<AddTwoInts::Service>(&name, QosProfile::services_default())
        .unwrap();
    let untyped = owned
        .create_client_untyped(
            &name,
            "example_interfaces/srv/AddTwoInts",
            QosProfile::services_default(),
        )
        .unwrap();
    let typed_native = client.client.clone();
    let untyped_native = untyped.client.clone();
    let mut executor = Executor::new(context);
    executor.add_node(owned).unwrap();
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        futures::try_join!(client.wait_for_service().unwrap(), untyped.wait_for_service().unwrap())
    })
    .await
    .unwrap()
    .unwrap();
    let response = client.request(&AddTwoInts::Request { a: 1, b: 2 }).unwrap();
    let request = tokio::time::timeout(Duration::from_secs(5), service.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(request.message.a, 1);
    drop(client);
    assert!(
        typed_native.upgrade().is_some(),
        "outstanding response must keep the native endpoint registered"
    );
    drop(response);
    let cleanup = tokio::time::timeout(Duration::from_secs(1), async {
        while typed_native.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    let response = untyped.request(serde_json::json!({"a":3,"b":4})).unwrap();
    let second = tokio::time::timeout(Duration::from_secs(5), service.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(second.message.a, 3);
    drop(untyped);
    assert!(untyped_native.upgrade().is_some());
    drop(response);
    let untyped_cleanup = tokio::time::timeout(Duration::from_secs(1), async {
        while untyped_native.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    task.stop().await.unwrap();
    cleanup.expect("typed response cancellation left a native endpoint registered");
    untyped_cleanup.expect("untyped response cancellation left a native endpoint registered");
    drop((request, second));
}

#[tokio::test(flavor = "current_thread")]
async fn action_endpoint_drop_wakes_and_releases_both_client_types_and_server() {
    use crate::example_interfaces::action::Fibonacci;
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_action_drop");
    let name = format!("/background_action_drop_{}", uuid::Uuid::new_v4().simple());
    let requests = owned
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = owned
        .create_action_client::<Fibonacci::Action>(&name)
        .unwrap();
    let untyped = owned
        .create_action_client_untyped(&name, "example_interfaces/action/Fibonacci")
        .unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(owned).unwrap();
    let clients: Vec<_> = executor.nodes[0]
        .node
        .action_clients
        .iter()
        .map(Arc::downgrade)
        .collect();
    let server = Arc::downgrade(&executor.nodes[0].node.action_servers[0]);
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        futures::try_join!(
            client.wait_for_action_server().unwrap(),
            untyped.wait_for_action_server().unwrap()
        )
    })
    .await
    .unwrap()
    .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    drop((client, untyped, requests));
    let cleanup = tokio::time::timeout(Duration::from_secs(1), async {
        while clients.iter().any(|client| client.upgrade().is_some()) || server.upgrade().is_some()
        {
            tokio::task::yield_now().await;
        }
    })
    .await;
    task.stop().await.unwrap();
    cleanup.expect("action endpoint cleanup awaited an unrelated native event");
}

#[tokio::test(flavor = "current_thread")]
async fn delayed_action_cancel_decision_wakes_native_dispatch_and_completes_result() {
    use crate::{example_interfaces::action::Fibonacci, GoalStatus};
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_cancel_decision");
    let name = format!("/background_cancel_{}", uuid::Uuid::new_v4().simple());
    let mut requests = owned
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = owned
        .create_action_client::<Fibonacci::Action>(&name)
        .unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(owned).unwrap();
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(5), client.wait_for_action_server().unwrap())
        .await
        .unwrap()
        .unwrap();
    let response = client
        .send_goal_request(Fibonacci::Goal { order: 3 })
        .unwrap();
    let request = tokio::time::timeout(Duration::from_secs(5), requests.next())
        .await
        .unwrap()
        .unwrap();
    let (mut server_goal, mut cancels) = request.accept_deferred().unwrap();
    let (client_goal, result, feedback) = tokio::time::timeout(Duration::from_secs(5), response)
        .await
        .unwrap()
        .unwrap();
    let cancel = client_goal.cancel().unwrap();
    let request = tokio::time::timeout(Duration::from_secs(5), cancels.next())
        .await
        .unwrap()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await; // no unrelated native event.
    request.accept();
    let outcome = tokio::time::timeout(Duration::from_secs(1), cancel).await;
    if let Ok(Ok(_)) = &outcome {
        assert_eq!(server_goal.get_status().unwrap(), GoalStatus::Canceling);
        server_goal
            .cancel(Fibonacci::Result {
                sequence: vec![0, 1, 1],
            })
            .unwrap();
    }
    let result = tokio::time::timeout(Duration::from_secs(1), result).await;
    task.stop().await.unwrap();
    outcome
        .expect("cancel decision did not wake native maintenance")
        .unwrap();
    let (status, result) = result.expect("canceled result did not complete").unwrap();
    assert_eq!(status, GoalStatus::Canceled);
    assert_eq!(result.sequence, [0, 1, 1]);
    drop(feedback);
}

#[tokio::test(flavor = "current_thread")]
async fn abandoned_action_observers_release_a_dropped_client_without_result_delivery() {
    use crate::example_interfaces::action::Fibonacci;
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_action_observers_drop");
    let name = format!("/background_observers_drop_{}", uuid::Uuid::new_v4().simple());
    let mut requests = owned
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = owned
        .create_action_client::<Fibonacci::Action>(&name)
        .unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(owned).unwrap();
    let native = Arc::downgrade(&executor.nodes[0].node.action_clients[0]);
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(5), client.wait_for_action_server().unwrap())
        .await
        .unwrap()
        .unwrap();
    let response = client
        .send_goal_request(Fibonacci::Goal { order: 3 })
        .unwrap();
    let request = tokio::time::timeout(Duration::from_secs(5), requests.next())
        .await
        .unwrap()
        .unwrap();
    let (server_goal, _cancels) = request.accept_deferred().unwrap();
    let (goal, result, feedback) = tokio::time::timeout(Duration::from_secs(5), response)
        .await
        .unwrap()
        .unwrap();
    drop(client);
    assert!(native.upgrade().is_some());
    tokio::time::sleep(Duration::from_millis(20)).await;
    drop((result, feedback, goal));
    let cleanup = tokio::time::timeout(Duration::from_secs(1), async {
        while native.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    task.stop().await.unwrap();
    cleanup.expect("action observer cancellation did not release its native endpoint");
    drop(server_goal);
}

#[tokio::test(flavor = "current_thread")]
async fn abandoned_untyped_action_observers_release_a_dropped_client_without_result_delivery() {
    use crate::example_interfaces::action::Fibonacci;
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_untyped_observers_drop");
    let name = format!("/background_untyped_observers_drop_{}", uuid::Uuid::new_v4().simple());
    let mut requests = owned
        .create_action_server::<Fibonacci::Action>(&name)
        .unwrap();
    let client = owned
        .create_action_client_untyped(&name, "example_interfaces/action/Fibonacci")
        .unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(owned).unwrap();
    let native = Arc::downgrade(&executor.nodes[0].node.action_clients[0]);
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(5), client.wait_for_action_server().unwrap())
        .await
        .unwrap()
        .unwrap();
    let response = client
        .send_goal_request(serde_json::json!({"order":3}))
        .unwrap();
    let request = tokio::time::timeout(Duration::from_secs(5), requests.next())
        .await
        .unwrap()
        .unwrap();
    let (server_goal, _cancels) = request.accept_deferred().unwrap();
    let (goal, result, feedback) = tokio::time::timeout(Duration::from_secs(5), response)
        .await
        .unwrap()
        .unwrap();
    drop(client);
    assert!(native.upgrade().is_some());
    tokio::time::sleep(Duration::from_millis(20)).await;
    drop((result, feedback, goal));
    let cleanup = tokio::time::timeout(Duration::from_secs(1), async {
        while native.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    task.stop().await.unwrap();
    cleanup.expect("action observer cancellation did not release its native endpoint");
    drop(server_goal);
}

#[tokio::test(flavor = "current_thread")]
async fn transferred_node_cleanup_queues_detach_the_previous_executor_wake() {
    use std::sync::atomic::Ordering;
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_transfer_cleanup");
    let timer = owned.create_wall_timer(Duration::from_secs(60)).unwrap();
    let mut executor = Executor::new(context);
    let id = executor.add_node(owned).unwrap();
    let mut recovered = executor.spawn().unwrap().stop().await.unwrap();
    let guard = recovered.future_wake_guard.clone().unwrap();
    let mut extracted = recovered.remove_node(id).unwrap();
    let active = crate::nodes::executor::ActiveWait::new(guard.clone());
    let before = guard.triggers.load(Ordering::Relaxed);
    drop(timer);
    assert_eq!(guard.triggers.load(Ordering::Relaxed), before);
    extracted.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(extracted.timer_count(), 0);
    drop(active);
}
