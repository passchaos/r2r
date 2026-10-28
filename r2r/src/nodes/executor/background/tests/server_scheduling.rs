//! Dormant action servers must not be scanned for ordinary service traffic.

use super::*;
use crate::{
    example_interfaces::{action::Fibonacci, srv::AddTwoInts},
    Error, GoalStatus,
};
use std::sync::atomic::Ordering;

#[tokio::test(flavor = "current_thread")]
async fn parked_servers_skip_service_traffic_and_preserve_pre_spawn_ready_identity() {
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "server_ready_identity");
    let prefix = uuid::Uuid::new_v4().simple().to_string();
    let mut servers = Vec::new();
    let mut clients = Vec::new();
    for i in 0..20 {
        let name = format!("/ready_server_{prefix}_{i}");
        servers.push(
            owned
                .create_action_server::<Fibonacci::Action>(&name)
                .unwrap(),
        );
        clients.push(
            owned
                .create_action_client::<Fibonacci::Action>(&name)
                .unwrap(),
        );
    }
    let topic = format!("/ready_service_{prefix}");
    let mut requests = owned
        .create_service::<AddTwoInts::Service>(&topic, QosProfile::default())
        .unwrap();
    let ordinary = owned
        .create_client::<AddTwoInts::Service>(&topic, QosProfile::default())
        .unwrap();
    let notifications = owned.dropped_action_servers.clone();
    let polls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    owned.action_server_poll_probe = Some(polls.clone());
    let native = owned.action_servers.clone();
    let last_id = native.last().unwrap().lock().id();
    let mut executor = Executor::new(context.clone());
    let id = executor.add_node(owned).unwrap();
    let guard = FutureGuard::new(&context).unwrap();
    executor.cache_future_guard(guard.clone());
    let task = executor.spawn().unwrap();
    let mut goals = Vec::new();
    let mut observed = Vec::new();
    for (i, client) in clients.iter().enumerate() {
        tokio::time::timeout(Duration::from_secs(5), client.wait_for_action_server().unwrap())
            .await
            .unwrap()
            .unwrap();
        let response = client
            .send_goal_request(Fibonacci::Goal { order: i as i32 })
            .unwrap();
        let request = tokio::time::timeout(Duration::from_secs(5), servers[i].next())
            .await
            .unwrap()
            .unwrap();
        goals.push(request.accept().unwrap());
        observed.push(
            tokio::time::timeout(Duration::from_secs(5), response)
                .await
                .unwrap()
                .unwrap(),
        );
    }
    let mut responses = clients
        .iter()
        .map(|client| client.cancel_all().unwrap())
        .collect::<Vec<_>>();
    let mut decisions = Vec::new();
    for (_, stream) in &mut goals {
        decisions.push(
            tokio::time::timeout(Duration::from_secs(5), stream.next())
                .await
                .unwrap()
                .unwrap(),
        );
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        while notifications.lock().has_ready() || !guard.waiting.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(native
        .iter()
        .all(|server| server.lock().has_pending_cancel_work()));
    polls.swap(0, Ordering::Relaxed);
    for sequence in 0..32 {
        let response = ordinary
            .request(&AddTwoInts::Request { a: sequence, b: 7 })
            .unwrap();
        let request = tokio::time::timeout(Duration::from_secs(5), requests.next())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(request.message.a, sequence);
        request
            .respond(AddTwoInts::Response { sum: sequence + 7 })
            .unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), response)
                .await
                .unwrap()
                .unwrap()
                .sum,
            sequence + 7
        );
    }
    assert_eq!(
        polls.swap(0, Ordering::Relaxed),
        0,
        "service traffic scanned dormant action servers"
    );
    let mut returned = task.stop().await.unwrap();
    let old_triggers = guard.triggers.load(Ordering::Relaxed);
    let extracted = returned.remove_node(id).unwrap();
    decisions.pop().unwrap().reject();
    assert!(notifications.lock().is_ready(last_id), "ready identity was lost while unbound");
    let mut next = Executor::new(context.clone());
    next.add_node(extracted).unwrap();
    let task = next.spawn().unwrap();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(5), responses.pop().unwrap())
            .await
            .unwrap(),
        Err(Error::GoalCancelRejected)
    ));
    assert_eq!(
        polls.swap(0, Ordering::Relaxed),
        1,
        "one pre-spawn decision scheduled unrelated servers"
    );
    assert_eq!(
        native
            .iter()
            .filter(|server| server.lock().has_pending_cancel_work())
            .count(),
        19
    );
    for decision in decisions {
        decision.reject();
    }
    for response in responses {
        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(5), response)
                .await
                .unwrap(),
            Err(Error::GoalCancelRejected)
        ));
    }
    for (i, (goal, _)) in goals.iter_mut().enumerate() {
        goal.succeed(Fibonacci::Result {
            sequence: vec![i as i32],
        })
        .unwrap();
    }
    for (i, (_goal, result, _feedback)) in observed.into_iter().enumerate() {
        let (status, result) = tokio::time::timeout(Duration::from_secs(5), result)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(status, GoalStatus::Succeeded);
        assert_eq!(result.sequence, [i as i32]);
    }
    task.stop().await.unwrap();
    assert!(context.is_valid());
    assert_eq!(
        guard.triggers.load(Ordering::Relaxed),
        old_triggers,
        "ready identity used the former executor"
    );
}
