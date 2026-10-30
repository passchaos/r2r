use super::*;
use crate::example_interfaces::srv::AddTwoInts;

#[tokio::test(flavor = "current_thread")]
async fn callback_group_native_service_preserves_request_identity_and_response_storage() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "callback_group_native_service", "").unwrap();
    let mut group = CallbackGroup::new();
    let name = format!("/grouped_service_{}", uuid::Uuid::new_v4().simple());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let captures = seen.clone();
    let service = node
        .create_service_native_callback_in_group::<AddTwoInts::Service, _>(
            &mut group,
            &name,
            QosProfile::services_default(),
            move |request, response, info| {
                captures
                    .lock()
                    .push((info.sequence_number, response.msg as usize));
                response.sum = request.a + request.b;
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(service.service_name().unwrap(), name);
    service.get_request_subscription_actual_qos().unwrap();
    service.get_response_publisher_actual_qos().unwrap();
    let client = node
        .create_client::<AddTwoInts::Service>(&name, QosProfile::services_default())
        .unwrap();
    let mut executor = CallbackExecutor::new(context, 2).unwrap();
    let id = executor.add_node(node).unwrap();
    executor.add_group(group).unwrap();
    let task = executor.spawn().unwrap();
    let application = tokio::time::timeout(Duration::from_secs(5), async {
        client.wait_for_service().unwrap().await.unwrap();
        for a in 0..32 {
            let response = client
                .request_with_info(&AddTwoInts::Request { a, b: 11 })
                .unwrap()
                .await
                .unwrap();
            assert_eq!(response.response.sum, a + 11);
            assert_eq!(response.info.sequence_number, seen.lock().last().unwrap().0);
        }
    })
    .await;
    let mut executor = task.stop().await.unwrap();
    application.unwrap();
    let captures = seen.lock();
    assert_eq!(captures.len(), 32);
    assert!(captures
        .iter()
        .all(|(_, address)| *address == captures[0].1));
    drop(captures);
    executor
        .node_mut(id)
        .unwrap()
        .destroy_grouped_service(service);
    assert_eq!(
        executor
            .node_mut(id)
            .unwrap()
            .prepare_spin(Duration::ZERO)
            .unwrap()
            .capacities
            .services,
        0
    );
}

#[tokio::test(flavor = "current_thread")]
async fn callback_group_service_handler_can_query_its_endpoint_without_holding_native_mutex() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "callback_group_service_reentry", "").unwrap();
    let mut group = CallbackGroup::new();
    let name = format!("/grouped_reentry_{}", uuid::Uuid::new_v4().simple());
    let slot = Arc::new(Mutex::new(None::<Arc<GroupedService>>));
    let current = slot.clone();
    let expected = name.clone();
    let service = Arc::new(
        node.create_service_callback_in_group::<AddTwoInts::Service, _>(
            &mut group,
            &name,
            QosProfile::services_default(),
            move |request, _| {
                let service = current.lock().clone().unwrap();
                assert_eq!(service.service_name()?, expected);
                Ok(AddTwoInts::Response {
                    sum: request.a + request.b,
                })
            },
        )
        .unwrap(),
    );
    *slot.lock() = Some(service.clone());
    let client = node
        .create_client::<AddTwoInts::Service>(&name, QosProfile::services_default())
        .unwrap();
    let mut executor = CallbackExecutor::new(context, 2).unwrap();
    executor.add_node(node).unwrap();
    executor.add_group(group).unwrap();
    let task = executor.spawn().unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), async {
        client.wait_for_service().unwrap().await.unwrap();
        client
            .request(&AddTwoInts::Request { a: 20, b: 22 })
            .unwrap()
            .await
            .unwrap()
    })
    .await;
    let executor = task.stop().await.unwrap();
    assert_eq!(result.unwrap().sum, 42);
    slot.lock().take();
    drop(service);
    drop(executor);
}

#[test]
fn callback_group_service_invalid_capacity_does_not_create_endpoint_or_group_handler() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "callback_group_service_invalid", "").unwrap();
    let mut group = CallbackGroup::new();
    assert!(matches!(
        node.create_service_native_callback_in_group_with_options::<AddTwoInts::Service, _>(
            &mut group,
            "/invalid_group_service",
            QosProfile::services_default(),
            CallbackQueueOptions::default().capacity(0),
            |_, _, _| Ok(()),
        ),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert!(node.grouped_callbacks.is_none());
    assert!(group.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn callback_group_service_finalization_during_handler_prevents_late_native_response() {
    use super::scheduling::Release;
    use futures::channel::oneshot;
    use std::sync::{Condvar, Mutex as StdMutex};
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "callback_group_service_finalize", "").unwrap();
    let mut group = CallbackGroup::new();
    let name = format!("/grouped_finalize_{}", uuid::Uuid::new_v4().simple());
    let gate = Arc::new((StdMutex::new(false), Condvar::new()));
    let release = Release(gate.clone());
    let (started, start) = oneshot::channel();
    let mut started = Some(started);
    let service = node
        .create_service_callback_in_group::<AddTwoInts::Service, _>(
            &mut group,
            &name,
            QosProfile::services_default(),
            move |request, _| {
                if let Some(started) = started.take() {
                    let _ = started.send(());
                }
                let (lock, ready) = &*gate;
                let mut released = lock.lock().unwrap();
                while !*released {
                    released = ready.wait(released).unwrap();
                }
                Ok(AddTwoInts::Response {
                    sum: request.a + request.b,
                })
            },
        )
        .unwrap();
    let native_valid = service.native_validity_probe();
    let client = node
        .create_client::<AddTwoInts::Service>(&name, QosProfile::services_default())
        .unwrap();
    let mut executor = CallbackExecutor::new(context, 2).unwrap();
    executor.add_node(node).unwrap();
    executor.add_group(group).unwrap();
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(3), client.wait_for_service().unwrap())
        .await
        .unwrap()
        .unwrap();
    let mut response = client
        .request(&AddTwoInts::Request { a: 20, b: 22 })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), start)
        .await
        .unwrap()
        .unwrap();
    drop(service);
    let finalized = tokio::time::timeout(Duration::from_secs(3), async {
        while native_valid() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    drop(release);
    let replied = tokio::time::timeout(Duration::from_millis(50), &mut response).await;
    drop(response);
    let executor = task.stop().await.unwrap();
    finalized.expect("native service was not finalized while user work held no native mutex");
    assert!(replied.is_err(), "handler sent a reply through its finalized service");
    drop(executor);
}
