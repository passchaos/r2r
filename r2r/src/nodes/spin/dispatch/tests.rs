use super::*;
use crate::{
    example_interfaces::{action::Fibonacci, srv::AddTwoInts},
    std_msgs::msg::Int32,
};
use futures::{FutureExt, StreamExt};
use std::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Instant,
};

mod actions;
mod client_allocation;

/// Rebuild the actual native wait set without dispatching until every requested family is
/// ready. This freezes one readiness snapshot; a later spin cannot rescue a skipped family.
fn dispatch_all_ready(node: &mut Node) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        assert!(Instant::now() < deadline, "native fixture never became jointly ready");
        let preparation = node.prepare_spin(Duration::from_millis(10))?;
        let (wait_set, mut entities) = node.wait_entities();
        {
            let mut context = entities.context.context_handle.lock();
            wait_set.prepare(preparation.capacities, context.as_mut())?;
        }
        let offsets = entities.add_ordinary(&mut wait_set.handle, &preparation)?;
        entities.add_actions(&mut wait_set.handle)?;
        wait_set.refresh_event_layout(entities.context)?;
        let ret = unsafe { rcl_wait(&mut wait_set.handle, 10_000_000) };
        if ret == RCL_RET_TIMEOUT as i32 {
            continue;
        }
        check_rcl_ret(ret)?;
        let all_subscriptions = (0..entities.subscribers.len()).all(|i| unsafe {
            !(*wait_set.handle.subscriptions.add(offsets.subscribers + i)).is_null()
        });
        let timer = unsafe { !(*wait_set.handle.timers.add(offsets.timers)).is_null() };
        let client_response = unsafe { !(*wait_set.handle.clients.add(offsets.clients)).is_null() };
        let ordinary_service =
            unsafe { !(*wait_set.handle.services.add(offsets.services)).is_null() };
        let callbacks = (0..2).all(|i| unsafe {
            !(*wait_set.handle.services.add(offsets.service_callbacks + i)).is_null()
        });
        let mut goal = false;
        let mut cancel = false;
        let mut result = false;
        let mut expired = false;
        check_rcl_ret(unsafe {
            rcl_action_server_wait_set_get_entities_ready(
                &wait_set.handle,
                entities.action_servers[0].lock().handle(),
                &mut goal,
                &mut cancel,
                &mut result,
                &mut expired,
            )
        })?;
        if all_subscriptions && timer && client_response && ordinary_service && callbacks && goal {
            return entities.dispatch(&wait_set.handle, offsets);
        }
    }
}

fn ready_dispatch_fixture(subscription_error: bool) {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "ready_dispatch_owner", "").unwrap();
    let mut peer = Node::create(context, "ready_dispatch_peer", "").unwrap();
    let prefix = format!("/ready_dispatch_{}", uuid::Uuid::new_v4().simple());
    let mut response_service = peer
        .create_service::<AddTwoInts::Service>(
            &format!("{prefix}/response"),
            QosProfile::services_default(),
        )
        .unwrap();
    let response_client = node
        .create_client::<AddTwoInts::Service>(
            &format!("{prefix}/response"),
            QosProfile::services_default(),
        )
        .unwrap();
    let bad_count = Arc::new(AtomicUsize::new(0));
    let healthy_count = Arc::new(AtomicUsize::new(0));
    let count = bad_count.clone();
    let _bad_subscription = node
        .subscribe_native_callback::<Int32, _>(
            &format!("{prefix}/data"),
            QosProfile::default(),
            move |message, _| {
                assert_eq!(message.data, 37);
                count.fetch_add(1, Ordering::Relaxed);
                if subscription_error {
                    Err(Error::RCL_RET_INVALID_ARGUMENT)
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
    // Resolve client availability through preparation only. Dispatching the owner here
    // would pre-fill the timer channel and weaken the same-snapshot timer assertion.
    let mut available = Box::pin(response_client.wait_for_service().unwrap());
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        node.prepare_spin(Duration::ZERO).unwrap();
        if let Some(result) = available.as_mut().now_or_never() {
            result.unwrap();
            break;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let response = response_client
        .request_with_info(&AddTwoInts::Request { a: 23, b: 19 })
        .unwrap();
    let request = peer
        .spin_until_future_complete_with_options(
            async {
                response_service
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_SERVICE_INVALID)
            },
            Duration::from_secs(3),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    let response_sequence = request.request_info().sequence_number;
    request.respond(AddTwoInts::Response { sum: 42 }).unwrap();
    let count = healthy_count.clone();
    let _healthy_subscription = node
        .subscribe_native_callback::<Int32, _>(
            &format!("{prefix}/data"),
            QosProfile::default(),
            move |message, _| {
                assert_eq!(message.data, 37);
                count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap();
    let publisher = peer
        .create_publisher::<Int32>(&format!("{prefix}/data"), QosProfile::default())
        .unwrap();
    let mut timer = node.create_wall_timer(Duration::from_millis(1)).unwrap();
    let mut ordinary = node
        .create_service::<AddTwoInts::Service>(
            &format!("{prefix}/ordinary"),
            QosProfile::services_default(),
        )
        .unwrap();
    let bad_service_count = Arc::new(AtomicUsize::new(0));
    let good_service_count = Arc::new(AtomicUsize::new(0));
    let count = bad_service_count.clone();
    let _bad_service = node
        .create_service_native_callback::<AddTwoInts::Service, _>(
            &format!("{prefix}/bad"),
            QosProfile::services_default(),
            move |request, _, _| {
                assert_eq!(request.a, 19);
                count.fetch_add(1, Ordering::Relaxed);
                Err(Error::RCL_RET_ERROR)
            },
        )
        .unwrap();
    let count = good_service_count.clone();
    let _good_service = node
        .create_service_callback::<AddTwoInts::Service, _>(
            &format!("{prefix}/good"),
            QosProfile::services_default(),
            move |request, _| {
                count.fetch_add(1, Ordering::Relaxed);
                Ok(AddTwoInts::Response {
                    sum: request.a + request.b,
                })
            },
        )
        .unwrap();
    let mut action_requests = node
        .create_action_server::<Fibonacci::Action>(&format!("{prefix}/action"))
        .unwrap();
    let action = peer
        .create_action_client::<Fibonacci::Action>(&format!("{prefix}/action"))
        .unwrap();
    let mut clients = Vec::new();
    for suffix in ["ordinary", "bad", "good"] {
        clients.push(
            peer.create_client::<AddTwoInts::Service>(
                &format!("{prefix}/{suffix}"),
                QosProfile::services_default(),
            )
            .unwrap(),
        );
    }
    peer.spin_until_future_complete_with_options(
        async {
            publisher.wait_for_subscribers()?.await?;
            action.wait_for_action_server()?.await?;
            for client in &clients {
                client.wait_for_service()?.await?;
            }
            Ok(())
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let _pending = clients
        .iter()
        .map(|client| {
            client
                .request(&AddTwoInts::Request { a: 19, b: 23 })
                .unwrap()
        })
        .collect::<Vec<_>>();
    let _pending_goal = action
        .send_goal_request(Fibonacci::Goal { order: 9 })
        .unwrap();
    publisher.publish(&Int32 { data: 37 }).unwrap();
    let outcome = dispatch_all_ready(&mut node);
    assert!(matches!(outcome, Err(Error::RCL_RET_INVALID_ARGUMENT)) == subscription_error);
    if !subscription_error {
        assert!(matches!(outcome, Err(Error::RCL_RET_ERROR)));
    }
    assert_eq!(bad_count.load(Ordering::Relaxed), 1);
    assert_eq!(healthy_count.load(Ordering::Relaxed), 1);
    assert!(
        timer.tick().now_or_never().is_some(),
        "ready timer was skipped after callback error"
    );
    let response = response
        .now_or_never()
        .expect("ready client response was skipped")
        .unwrap();
    assert_eq!(response.response.sum, 42);
    assert_eq!(response.info.sequence_number, response_sequence);
    let request = ordinary
        .next()
        .now_or_never()
        .flatten()
        .expect("ready ordinary service was skipped");
    assert_eq!(request.message.a + request.message.b, 42);
    assert_eq!(bad_service_count.load(Ordering::Relaxed), 1);
    assert_eq!(good_service_count.load(Ordering::Relaxed), 1);
    let request = action_requests
        .next()
        .now_or_never()
        .flatten()
        .expect("ready action request was skipped after service error");
    assert_eq!(request.goal.order, 9);
}

#[test]
fn ready_dispatch_subscription_error_preserves_all_other_ready_families() {
    ready_dispatch_fixture(true);
}

#[test]
fn ready_dispatch_service_error_preserves_action_requests() {
    ready_dispatch_fixture(false);
}
