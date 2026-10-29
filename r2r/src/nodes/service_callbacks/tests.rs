use super::*;
use crate::{example_interfaces::srv::AddTwoInts, FutureExecutionOptions};
use futures::{FutureExt, StreamExt};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};
mod native_storage;

fn node(name: &str) -> Node {
    Node::create(Context::create().unwrap(), name, "").unwrap()
}
fn name(prefix: &str) -> String {
    format!("/{prefix}_{}", uuid::Uuid::new_v4().simple())
}
fn ready(node: &mut Node, client: &Client<AddTwoInts::Service>) {
    let ready = client.wait_for_service().unwrap();
    node.spin_until_future_complete_with_options(
        ready,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
}

#[test]
fn callback_service_typed_handler_returns_exact_response_and_native_request_identity() {
    let mut node = node("callback_typed");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let observed = seen.clone();
    let clock = node.get_ros_clock();
    let service = node
        .create_service_callback::<AddTwoInts::Service, _>(
            &name("callback_typed"),
            QosProfile::services_default(),
            move |request, info| {
                assert!(clock.try_lock().is_some(), "typed callback held managed clock lock");
                observed.lock().push(info);
                Ok(AddTwoInts::Response {
                    sum: request.a + request.b,
                })
            },
        )
        .unwrap();
    let service_name = service.service_name(&node).unwrap();
    let client = node
        .create_client::<AddTwoInts::Service>(&service_name, QosProfile::services_default())
        .unwrap();
    ready(&mut node, &client);
    for a in [1, -2, 3000] {
        let response = client
            .request_with_info(&AddTwoInts::Request { a, b: 13 })
            .unwrap();
        let response = node
            .spin_until_future_complete_with_options(
                response,
                Duration::from_secs(3),
                FutureExecutionOptions::wake_driven(),
            )
            .unwrap();
        assert_eq!(response.response.sum, a + 13);
        assert_eq!(seen.lock().last().unwrap().sequence_number, response.info.sequence_number);
    }
    assert_eq!(seen.lock().len(), 3);
    assert!(node.services.is_empty(), "callback created a Rust request stream service");
    assert_eq!(node.service_callbacks.as_ref().unwrap().len(), 1);
    assert_eq!(
        service
            .get_request_subscription_actual_qos(&node)
            .unwrap()
            .reliability,
        QosProfile::services_default().reliability
    );
}

#[test]
fn callback_service_errors_and_panics_consume_only_taken_request_and_allow_later_progress() {
    let mut node = node("callback_outcomes");
    let service = node
        .create_service_native_callback::<AddTwoInts::Service, _>(
            &name("callback_outcomes"),
            QosProfile::services_default(),
            |request, response, _| {
                response.sum = -999;
                match request.a {
                    1 => Err(Error::RCL_RET_INVALID_ARGUMENT),
                    2 => panic!("intentional native service callback panic"),
                    _ => {
                        response.sum = request.a + request.b;
                        Ok(())
                    }
                }
            },
        )
        .unwrap();
    let client = node
        .create_client::<AddTwoInts::Service>(
            &service.service_name(&node).unwrap(),
            QosProfile::services_default(),
        )
        .unwrap();
    ready(&mut node, &client);
    for a in [1, 2] {
        let mut response = Box::pin(client.request(&AddTwoInts::Request { a, b: 13 }).unwrap());
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            assert!(Instant::now() < deadline);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                node.try_spin_once(Duration::from_millis(20))
            }));
            if a == 1 && matches!(result, Ok(Err(Error::RCL_RET_INVALID_ARGUMENT))) {
                break;
            }
            if a == 2 && result.is_err() {
                break;
            }
            result.unwrap().unwrap();
        }
        assert!(response.as_mut().now_or_never().is_none());
        assert!(service.service_name(&node).is_ok());
    }
    let response = client
        .request(&AddTwoInts::Request { a: 3, b: 17 })
        .unwrap();
    assert_eq!(
        node.spin_until_future_complete_with_options(
            response,
            Duration::from_secs(3),
            FutureExecutionOptions::wake_driven()
        )
        .unwrap()
        .sum,
        20
    );
}

#[test]
fn callback_service_endpoint_drop_and_foreign_destruction_preserve_unrelated_services() {
    struct Capture(Arc<AtomicUsize>);
    impl Drop for Capture {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
    let mut node = node("callback_cleanup");
    let dropped = Arc::new(AtomicUsize::new(0));
    let capture = Capture(dropped.clone());
    let callback = node
        .create_service_callback::<AddTwoInts::Service, _>(
            &name("callback_cleanup"),
            QosProfile::default(),
            move |request, _| {
                let _keep_alive = &capture;
                Ok(AddTwoInts::Response {
                    sum: request.a + request.b,
                })
            },
        )
        .unwrap();
    let sibling = node
        .create_service_callback::<AddTwoInts::Service, _>(
            &name("callback_sibling"),
            QosProfile::default(),
            |request, _| {
                Ok(AddTwoInts::Response {
                    sum: request.a + request.b,
                })
            },
        )
        .unwrap();
    let ordinary = node
        .create_service::<AddTwoInts::Service>(&name("callback_ordinary"), QosProfile::default())
        .unwrap();
    let mut foreign = Node::create(node.context.clone(), "callback_foreign", "").unwrap();
    assert!(matches!(callback.service_name(&foreign), Err(Error::RCL_RET_SERVICE_INVALID)));
    foreign.destroy_callback_service(callback);
    assert_eq!(dropped.load(Ordering::Relaxed), 0);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(dropped.load(Ordering::Relaxed), 1);
    assert!(sibling.service_name(&node).is_ok());
    assert!(ordinary.service_name(&node).is_ok());
    node.destroy_callback_service(sibling);
    assert_eq!(node.service_callbacks.as_ref().unwrap().len(), 0);
    assert_eq!(node.services.len(), 1);
}

#[test]
fn callback_service_bounded_dispatch_keeps_other_callback_and_stream_services_progressing() {
    let mut node = node("callback_fairness");
    let qos = QosProfile::services_default().keep_last(32);
    let slow_count = Arc::new(AtomicUsize::new(0));
    let count = slow_count.clone();
    let slow = node
        .create_service_callback_with_options::<AddTwoInts::Service, _>(
            &name("callback_bounded"),
            qos.clone(),
            CallbackServiceOptions::default().max_requests_per_spin(2),
            move |request, _| {
                count.fetch_add(1, Ordering::Relaxed);
                Ok(AddTwoInts::Response {
                    sum: request.a + request.b,
                })
            },
        )
        .unwrap();
    let healthy_count = Arc::new(AtomicUsize::new(0));
    let count = healthy_count.clone();
    let healthy = node
        .create_service_callback::<AddTwoInts::Service, _>(
            &name("callback_healthy"),
            qos.clone(),
            move |request, _| {
                count.fetch_add(1, Ordering::Relaxed);
                Ok(AddTwoInts::Response {
                    sum: request.a + request.b,
                })
            },
        )
        .unwrap();
    let mut ordinary = node
        .create_service::<AddTwoInts::Service>(&name("callback_stream"), qos.clone())
        .unwrap();
    let slow_client = node
        .create_client::<AddTwoInts::Service>(&slow.service_name(&node).unwrap(), qos.clone())
        .unwrap();
    let healthy_client = node
        .create_client::<AddTwoInts::Service>(&healthy.service_name(&node).unwrap(), qos.clone())
        .unwrap();
    let stream_client = node
        .create_client::<AddTwoInts::Service>(&ordinary.service_name(&node).unwrap(), qos)
        .unwrap();
    ready(&mut node, &slow_client);
    ready(&mut node, &healthy_client);
    ready(&mut node, &stream_client);
    let requests: Vec<_> = (0..7)
        .flat_map(|a| {
            [&slow_client, &healthy_client, &stream_client]
                .map(|client| client.request(&AddTwoInts::Request { a, b: 11 }).unwrap())
        })
        .collect();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut stream_count = 0;
    while slow_count.load(Ordering::Relaxed) < 7
        || healthy_count.load(Ordering::Relaxed) < 7
        || stream_count < 7
    {
        assert!(Instant::now() < deadline);
        let before = slow_count.load(Ordering::Relaxed);
        node.try_spin_once(Duration::from_millis(20)).unwrap();
        assert!(slow_count.load(Ordering::Relaxed) - before <= 2);
        while let Some(request) = ordinary.next().now_or_never().flatten() {
            stream_count += 1;
            let sum = request.message.a + request.message.b;
            request.respond(AddTwoInts::Response { sum }).unwrap();
        }
    }
    let results = node
        .spin_until_future_complete_with_options(
            futures::future::try_join_all(requests),
            Duration::from_secs(3),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    assert_eq!(results.len(), 21);
    for (index, response) in results.iter().enumerate() {
        assert_eq!(response.sum, (index / 3) as i64 + 11);
    }
}

#[test]
fn callback_service_shared_executor_keeps_callback_stream_and_action_service_offsets_disjoint() {
    use crate::example_interfaces::action::Fibonacci;
    let context = Context::create().unwrap();
    let mut server = Node::create(context.clone(), "callback_shared_server", "").unwrap();
    let mut client_node = Node::create(context.clone(), "callback_shared_client", "").unwrap();
    let service = server
        .create_service_native_callback::<AddTwoInts::Service, _>(
            &name("callback_shared"),
            QosProfile::default(),
            |request, response, _| {
                response.sum = request.a + request.b;
                Ok(())
            },
        )
        .unwrap();
    let client = client_node
        .create_client::<AddTwoInts::Service>(
            &service.service_name(&server).unwrap(),
            QosProfile::default(),
        )
        .unwrap();
    let action_name = name("callback_shared_action");
    let mut goals = server
        .create_action_server::<Fibonacci::Action>(&action_name)
        .unwrap();
    let action = client_node
        .create_action_client::<Fibonacci::Action>(&action_name)
        .unwrap();
    let service_ready = client.wait_for_service().unwrap();
    let action_ready = action.wait_for_action_server().unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(server).unwrap();
    executor.add_node(client_node).unwrap();
    executor
        .spin_until_future_complete_with_options(
            async {
                let server = async {
                    let request = goals.next().await.unwrap();
                    let (mut goal, _cancel) = request.accept()?;
                    goal.succeed(Fibonacci::Result {
                        sequence: vec![0, 1, 1, 2],
                    })
                };
                let application = async {
                    service_ready.await?;
                    action_ready.await?;
                    let sum = client.request(&AddTwoInts::Request { a: 12, b: 30 })?;
                    let goal = action.send_goal_request(Fibonacci::Goal { order: 4 })?;
                    let (sum, (_, result, _feedback)) = futures::try_join!(sum, goal)?;
                    assert_eq!(sum.sum, 42);
                    let (status, result) = result.await?;
                    assert_eq!(status, crate::GoalStatus::Succeeded);
                    assert_eq!(result.sequence, [0, 1, 1, 2]);
                    Ok::<_, Error>(())
                };
                futures::try_join!(server, application)?;
                Ok(())
            },
            Duration::from_secs(3),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
}

#[test]
fn callback_service_zero_options_and_invalid_names_leave_registry_and_native_counts_clean() {
    let mut node = node("callback_creation_failures");
    let result = node.create_service_callback_with_options::<AddTwoInts::Service, _>(
        "valid_name",
        QosProfile::default(),
        CallbackServiceOptions::default().max_requests_per_spin(0),
        |_, _| Ok(AddTwoInts::Response { sum: 0 }),
    );
    assert!(matches!(result, Err(Error::RCL_RET_INVALID_ARGUMENT)));
    assert!(node.service_callbacks.is_none());
    let result = node.create_service_callback::<AddTwoInts::Service, _>(
        "bad\0name",
        QosProfile::default(),
        |_, _| Ok(AddTwoInts::Response { sum: 0 }),
    );
    assert!(result.is_err());
    assert!(node.service_callbacks.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn callback_service_background_drop_wakes_current_executor_and_releases_captures() {
    struct Capture(Arc<AtomicUsize>);
    impl Drop for Capture {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Release);
        }
    }
    let mut node = node("callback_background");
    let dropped = Arc::new(AtomicUsize::new(0));
    let capture = Capture(dropped.clone());
    let service = node
        .create_service_callback::<AddTwoInts::Service, _>(
            &name("callback_background"),
            QosProfile::default(),
            move |request, _| {
                let _keep_alive = &capture;
                Ok(AddTwoInts::Response {
                    sum: request.a + request.b,
                })
            },
        )
        .unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    let old_guard = node.future_wake_guard.clone().unwrap();
    let mut executor = Executor::new(node.context.clone());
    let id = executor.add_node(node).unwrap();
    executor.spin_once(Duration::ZERO).unwrap();
    let guard = future_executor::FutureDriver::cached_future_guard(&executor).unwrap();
    let old_count = old_guard.triggers.load(Ordering::Acquire);
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while !guard.waiting.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    drop(service);
    tokio::time::timeout(Duration::from_secs(3), async {
        while dropped.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(old_guard.triggers.load(Ordering::Acquire), old_count);
    assert!(guard.triggers.load(Ordering::Acquire) >= 1);
    let mut recovered = task.stop().await.unwrap();
    assert_eq!(
        recovered
            .node_mut(id)
            .unwrap()
            .service_callbacks
            .as_ref()
            .unwrap()
            .len(),
        0
    );
}
