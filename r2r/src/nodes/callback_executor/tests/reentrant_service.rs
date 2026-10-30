use super::scheduling::Release;
use super::*;
use crate::example_interfaces::srv::AddTwoInts;
use std::sync::{mpsc, Condvar, Mutex as StdMutex};

#[tokio::test(flavor = "current_thread")]
async fn reentrant_group_service_concurrent_requests_own_independent_reused_response_buffers() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "reentrant_service_buffers", "").unwrap();
    let mut group = ReentrantCallbackGroup::new(2).unwrap();
    let name = format!("/reentrant_rpc_{}", uuid::Uuid::new_v4().simple());
    let gate = Arc::new((StdMutex::new(false), Condvar::new()));
    let release = Release(gate.clone());
    let (started, start) = mpsc::channel();
    let service = node
        .create_service_native_reentrant_callback::<AddTwoInts::Service, _>(
            &mut group,
            &name,
            QosProfile::services_default(),
            move |request, response, info| {
                started
                    .send((request.a, info.sequence_number, response.msg as usize))
                    .unwrap();
                let (lock, ready) = &*gate;
                let mut released = lock.lock().unwrap();
                while !*released {
                    released = ready.wait(released).unwrap();
                }
                response.sum = request.a + request.b;
                Ok(())
            },
        )
        .unwrap();
    let client = node
        .create_client::<AddTwoInts::Service>(&name, QosProfile::services_default())
        .unwrap();
    let mut executor = CallbackExecutor::new(context, 3).unwrap();
    executor.add_node(node).unwrap();
    let group_id = executor.add_reentrant_group(group).unwrap();
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(3), client.wait_for_service().unwrap())
        .await
        .unwrap()
        .unwrap();
    let first = client
        .request_with_info(&AddTwoInts::Request { a: 11, b: 31 })
        .unwrap();
    let second = client
        .request_with_info(&AddTwoInts::Request { a: 13, b: 41 })
        .unwrap();
    let mut inputs = Vec::new();
    let concurrent = tokio::time::timeout(Duration::from_secs(3), async {
        while inputs.len() < 2 {
            if let Ok(input) = start.try_recv() {
                inputs.push(input);
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    drop(release);
    let replies = tokio::time::timeout(Duration::from_secs(3), async {
        (first.await.unwrap(), second.await.unwrap())
    })
    .await;
    let mut executor = task.stop().await.unwrap();
    concurrent.expect("same service serialized shared user bodies");
    assert_ne!(inputs[0].2, inputs[1].2, "native reply storage was shared between lanes");
    let (first, second) = replies.unwrap();
    assert_eq!((first.response.sum, second.response.sum), (42, 54));
    for (a, sequence, _) in &inputs {
        assert_eq!(
            *sequence,
            if *a == 11 {
                first.info.sequence_number
            } else {
                second.info.sequence_number
            }
        );
    }
    assert_eq!(
        executor
            .reentrant_group_mut(group_id)
            .unwrap()
            .concurrency(),
        2
    );
    executor.reset_cancel();
    let task = executor.spawn().unwrap();
    let resumed = tokio::time::timeout(Duration::from_secs(3), async {
        for a in 0..16 {
            assert_eq!(
                client
                    .request(&AddTwoInts::Request { a, b: 7 })
                    .unwrap()
                    .await
                    .unwrap()
                    .sum,
                a + 7
            );
        }
    })
    .await;
    let executor = task.stop().await.unwrap();
    resumed.unwrap();
    let remaining: Vec<_> = start.try_iter().collect();
    assert_eq!(remaining.len(), 16);
    assert!(remaining
        .iter()
        .all(|(_, _, address)| inputs.iter().any(|(_, _, old)| old == address)));
    drop(service);
    drop(executor);
}

#[tokio::test(flavor = "current_thread")]
async fn reentrant_group_service_finalization_during_two_bodies_prevents_both_late_responses() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "reentrant_service_finalize", "").unwrap();
    let mut group = ReentrantCallbackGroup::new(2).unwrap();
    let name = format!("/reentrant_finalize_{}", uuid::Uuid::new_v4().simple());
    let gate = Arc::new((StdMutex::new(false), Condvar::new()));
    let release = Release(gate.clone());
    let (started, start) = mpsc::channel();
    let service = node
        .create_service_reentrant_callback::<AddTwoInts::Service, _>(
            &mut group,
            &name,
            QosProfile::services_default(),
            move |request, _| {
                started.send(()).unwrap();
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
    let mut executor = CallbackExecutor::new(context, 3).unwrap();
    executor.add_node(node).unwrap();
    executor.add_reentrant_group(group).unwrap();
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(3), client.wait_for_service().unwrap())
        .await
        .unwrap()
        .unwrap();
    let first = client.request(&AddTwoInts::Request { a: 1, b: 2 }).unwrap();
    let second = client.request(&AddTwoInts::Request { a: 3, b: 4 }).unwrap();
    let concurrent = tokio::time::timeout(Duration::from_secs(3), async {
        let mut count = 0;
        while count < 2 {
            if start.try_recv().is_ok() {
                count += 1;
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    drop(service);
    let finalized = tokio::time::timeout(Duration::from_secs(3), async {
        while native_valid() {
            tokio::task::yield_now().await;
        }
    })
    .await;
    drop(release);
    let replied =
        tokio::time::timeout(Duration::from_millis(50), async { futures::join!(first, second) })
            .await;
    let executor = task.stop().await.unwrap();
    concurrent.unwrap();
    finalized.expect("service fini waited on user bodies");
    assert!(replied.is_err(), "responses were sent through the finalized endpoint");
    drop(executor);
}

#[test]
fn reentrant_group_service_invalid_capacity_has_no_native_or_lane_registration() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "reentrant_invalid_service", "").unwrap();
    let mut group = ReentrantCallbackGroup::new(2).unwrap();
    assert!(matches!(
        node.create_service_native_reentrant_callback_with_options::<AddTwoInts::Service, _>(
            &mut group,
            "/reentrant_invalid_service",
            QosProfile::services_default(),
            CallbackQueueOptions::default().capacity(0),
            |_, _, _| Ok(())
        ),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    assert!(group.is_empty());
    assert!(node.grouped_callbacks.is_none());
}
