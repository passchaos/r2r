//! Late observer registration must wake an already waiting native executor.

use super::*;
use crate::clients::Client_;

#[tokio::test(flavor = "current_thread")]
async fn late_service_and_publisher_observers_progress_after_native_wait_has_started() {
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_late_wait");
    let service_name = format!("/background_late_{}", uuid::Uuid::new_v4().simple());
    let _service = owned
        .create_service::<AddTwoInts::Service>(&service_name, QosProfile::services_default())
        .unwrap();
    let client = owned
        .create_client::<AddTwoInts::Service>(&service_name, QosProfile::services_default())
        .unwrap();
    let publisher = owned
        .create_publisher::<crate::std_msgs::msg::Int32>(
            &format!("{service_name}/data"),
            QosProfile::default(),
        )
        .unwrap();
    let _subscription = owned
        .subscribe::<crate::std_msgs::msg::Int32>(
            &format!("{service_name}/data"),
            QosProfile::default(),
        )
        .unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(owned).unwrap();
    executor
        .spin_until_future_complete_with_options(
            async {
                futures::try_join!(client.wait_for_service()?, publisher.wait_for_subscribers()?)?;
                Ok(())
            },
            Duration::from_secs(5),
            crate::FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    let guard = executor.future_wake_guard.clone().unwrap();
    let task = executor.spawn().unwrap();
    // Discovery is complete; observe a quiescent native wait before creating
    // new observer queues. No ROS data/timer event is used to wake them.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !guard.waiting.load(std::sync::atomic::Ordering::Acquire) {
        assert!(std::time::Instant::now() < deadline);
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    let result = tokio::time::timeout(Duration::from_millis(200), async {
        futures::try_join!(
            client.wait_for_service().unwrap(),
            publisher.wait_for_subscribers().unwrap()
        )
    })
    .await;
    let _ = task.stop().await.unwrap();
    result
        .expect("late observers did not wake the native wait")
        .unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn late_untyped_action_matching_and_ack_observers_share_the_native_wake() {
    use crate::example_interfaces::action::Fibonacci;
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_late_untyped");
    let name = format!("/background_untyped_{}", uuid::Uuid::new_v4().simple());
    let _service = owned
        .create_service::<AddTwoInts::Service>(
            &format!("{name}/sum"),
            QosProfile::services_default(),
        )
        .unwrap();
    let client = owned
        .create_client_untyped(
            &format!("{name}/sum"),
            "example_interfaces/srv/AddTwoInts",
            QosProfile::services_default(),
        )
        .unwrap();
    let publisher = owned
        .create_publisher_untyped(
            &format!("{name}/data"),
            "std_msgs/msg/Int32",
            QosProfile::default(),
        )
        .unwrap();
    let _subscription = owned
        .subscribe::<crate::std_msgs::msg::Int32>(&format!("{name}/data"), QosProfile::default())
        .unwrap();
    let _requests = owned
        .create_action_server::<Fibonacci::Action>(&format!("{name}/action"))
        .unwrap();
    let typed_action = owned
        .create_action_client::<Fibonacci::Action>(&format!("{name}/action"))
        .unwrap();
    let untyped_action = owned
        .create_action_client_untyped(
            &format!("{name}/action"),
            "example_interfaces/action/Fibonacci",
        )
        .unwrap();
    let mut executor = Executor::new(context);
    executor.add_node(owned).unwrap();
    executor
        .spin_until_future_complete_with_options(
            async {
                futures::try_join!(
                    client.wait_for_service()?,
                    publisher.wait_for_subscribers()?,
                    typed_action.wait_for_action_server()?,
                    untyped_action.wait_for_action_server()?
                )?;
                Ok(())
            },
            Duration::from_secs(5),
            crate::FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    let guard = executor.future_wake_guard.clone().unwrap();
    let task = executor.spawn().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !guard.waiting.load(std::sync::atomic::Ordering::Acquire) {
        assert!(std::time::Instant::now() < deadline);
        tokio::task::yield_now().await;
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    let result = tokio::time::timeout(Duration::from_millis(200), async {
        futures::try_join!(
            client.wait_for_service().unwrap(),
            publisher.wait_for_subscribers().unwrap(),
            typed_action.wait_for_action_server().unwrap(),
            untyped_action.wait_for_action_server().unwrap()
        )
    })
    .await;
    let ack = tokio::time::timeout(
        Duration::from_millis(200),
        publisher.wait_for_all_acked_async().unwrap(),
    )
    .await;
    let _ = task.stop().await.unwrap();
    result
        .expect("late untyped/action observers did not progress")
        .unwrap();
    match ack.expect("late ACK observer did not progress") {
        Ok(()) | Err(Error::RCL_RET_UNSUPPORTED) => {}
        Err(error) => panic!("unexpected ACK error: {error}"),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn dropping_a_pre_spawn_observer_wakes_cleanup_without_a_poll_timer() {
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_observer_cancel");
    let client = owned
        .create_client::<AddTwoInts::Service>(
            &format!("/missing_background_{}", uuid::Uuid::new_v4().simple()),
            QosProfile::services_default(),
        )
        .unwrap();
    let pending = client.wait_for_service().unwrap();
    let mut executor = Executor::new(context.clone());
    let id = executor.add_node(owned).unwrap();
    let guard = FutureGuard::new(&context).unwrap();
    executor.cache_future_guard(guard.clone());
    let task = executor.spawn().unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !guard.waiting.load(std::sync::atomic::Ordering::Acquire) {
        assert!(std::time::Instant::now() < deadline);
        tokio::task::yield_now().await;
    }
    assert_eq!(
        guard
            .wait_timeout_ns
            .load(std::sync::atomic::Ordering::Acquire),
        i64::MAX
    );
    let before = guard.triggers.load(std::sync::atomic::Ordering::Relaxed);
    drop(pending);
    assert!(guard.triggers.load(std::sync::atomic::Ordering::Relaxed) > before);
    let mut recovered = task.stop().await.unwrap();
    recovered.reset_cancel();
    recovered.spin_once(Duration::ZERO).unwrap();
    assert!(!client
        .client
        .upgrade()
        .unwrap()
        .lock()
        .has_pending_availability());
    assert_eq!(recovered.node_mut(id).unwrap().timer_count(), 0);
}
