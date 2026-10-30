use super::*;
use crate::LifecycleNode;

#[test]
fn callback_group_lifecycle_timer_rejects_old_selection_and_preserves_one_shot_manual_state() {
    let context = Context::create().unwrap();
    let mut node = LifecycleNode::create(context, "callback_group_lifecycle_epoch", "").unwrap();
    let clock = node.get_ros_clock();
    clock.lock().enable_ros_time_override(100_000_000).unwrap();
    let mut group = CallbackGroup::new();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let timer = node
        .create_timer_callback_in_group_with_options(
            &mut group,
            Duration::from_millis(10),
            CallbackTimerOptions::default().oneshot(true),
            move || {
                seen.fetch_add(1, Ordering::Release);
                Ok(())
            },
        )
        .unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(timer.is_canceled().unwrap());
    assert_eq!(node.prepare_spin(Duration::ZERO).unwrap().capacities.timers, 0);
    node.configure().unwrap();
    node.activate().unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(timer.next_call_time_ns().unwrap(), 110_000_000);
    clock.lock().set_ros_time_override(110_000_000).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    node.deactivate().unwrap();
    clock.lock().set_ros_time_override(20_000_000).unwrap();
    node.activate().unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(timer.next_call_time_ns().unwrap(), 30_000_000);
    clock.lock().set_ros_time_override(30_000_000).unwrap();
    let wake = futures::task::noop_waker();
    assert!(group
        .poll_callbacks(&mut TaskContext::from_waker(&wake))
        .is_pending());
    assert_eq!(
        count.load(Ordering::Acquire),
        0,
        "obsolete activation consumed a new ready timer"
    );
    assert!(!timer.is_canceled().unwrap());
    assert_eq!(timer.next_call_time_ns().unwrap(), 30_000_000);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(group
        .poll_callbacks(&mut TaskContext::from_waker(&wake))
        .is_pending());
    assert_eq!(count.load(Ordering::Acquire), 1);
    assert!(timer.is_canceled().unwrap());
    node.deactivate().unwrap();
    clock.lock().set_ros_time_override(50_000_000).unwrap();
    node.activate().unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(group
        .poll_callbacks(&mut TaskContext::from_waker(&wake))
        .is_pending());
    assert_eq!(count.load(Ordering::Acquire), 1, "reactivation restarted a consumed one-shot");
    timer.reset().unwrap();
    assert_eq!(timer.next_call_time_ns().unwrap(), 60_000_000);
    clock.lock().set_ros_time_override(60_000_000).unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(group
        .poll_callbacks(&mut TaskContext::from_waker(&wake))
        .is_pending());
    assert_eq!(count.load(Ordering::Acquire), 2);
    drop(node);
    assert!(!timer.is_valid());
}

#[test]
fn callback_group_lifecycle_timer_autostart_false_requires_manual_reset_after_activation() {
    let context = Context::create().unwrap();
    let mut node = LifecycleNode::create(context, "callback_group_lifecycle_manual", "").unwrap();
    let mut group = CallbackGroup::new();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let timer = node
        .create_wall_timer_callback_in_group_with_options(
            &mut group,
            Duration::ZERO,
            CallbackTimerOptions::default()
                .autostart(false)
                .oneshot(true),
            move || {
                seen.fetch_add(1, Ordering::Release);
                Ok(())
            },
        )
        .unwrap();
    node.configure().unwrap();
    node.activate().unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    let wake = futures::task::noop_waker();
    assert!(group
        .poll_callbacks(&mut TaskContext::from_waker(&wake))
        .is_pending());
    assert_eq!(count.load(Ordering::Acquire), 0);
    assert!(timer.is_canceled().unwrap());
    timer.reset().unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(group
        .poll_callbacks(&mut TaskContext::from_waker(&wake))
        .is_pending());
    assert_eq!(count.load(Ordering::Acquire), 1);
    timer.cancel().unwrap();
    node.deactivate().unwrap();
    node.activate().unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert!(group
        .poll_callbacks(&mut TaskContext::from_waker(&wake))
        .is_pending());
    assert_eq!(count.load(Ordering::Acquire), 1);
}

#[tokio::test(flavor = "current_thread")]
async fn callback_group_lifecycle_native_services_control_timer_after_owned_node_transfer() {
    use crate::lifecycle_msgs::{msg::Transition, srv::ChangeState};
    let context = Context::create().unwrap();
    let mut sensor =
        LifecycleNode::create(context.clone(), "callback_group_managed_sensor", "").unwrap();
    let mut manager = Node::create(context.clone(), "callback_group_managed_manager", "").unwrap();
    let mut group = CallbackGroup::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let timer = sensor
        .create_wall_timer_callback_in_group(&mut group, Duration::from_millis(2), move || {
            seen.fetch_add(1, Ordering::Release);
            Ok(())
        })
        .unwrap();
    let client = manager
        .create_client::<ChangeState::Service>(
            "/callback_group_managed_sensor/change_state",
            QosProfile::services_default(),
        )
        .unwrap();
    let mut executor = CallbackExecutor::new(context, 2).unwrap();
    executor.add_node(sensor.into_node()).unwrap();
    executor.add_node(manager).unwrap();
    executor.add_group(group).unwrap();
    let transition = |id| {
        let request = client
            .request(&ChangeState::Request {
                transition: Transition {
                    id,
                    label: Default::default(),
                },
            })
            .unwrap();
        async move {
            assert!(
                tokio::time::timeout(Duration::from_secs(3), request)
                    .await
                    .unwrap()
                    .unwrap()
                    .success
            );
        }
    };
    let task = executor.spawn().unwrap();
    tokio::time::timeout(Duration::from_secs(3), client.wait_for_service().unwrap())
        .await
        .unwrap()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert_eq!(calls.load(Ordering::Acquire), 0);
    transition(Transition::TRANSITION_CONFIGURE as u8).await;
    transition(Transition::TRANSITION_ACTIVATE as u8).await;
    let active = tokio::time::timeout(Duration::from_secs(3), async {
        while calls.load(Ordering::Acquire) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await;
    transition(Transition::TRANSITION_DEACTIVATE as u8).await;
    let mut executor = task.stop().await.unwrap();
    active.unwrap();
    let before = calls.load(Ordering::Acquire);
    executor.reset_cancel();
    let task = executor.spawn().unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(calls.load(Ordering::Acquire), before);
    transition(Transition::TRANSITION_ACTIVATE as u8).await;
    let active = tokio::time::timeout(Duration::from_secs(3), async {
        while calls.load(Ordering::Acquire) == before {
            tokio::task::yield_now().await;
        }
    })
    .await;
    transition(Transition::TRANSITION_ACTIVE_SHUTDOWN as u8).await;
    let executor = task.stop().await.unwrap();
    active.unwrap();
    assert!(timer.is_canceled().unwrap());
    drop(executor);
}
