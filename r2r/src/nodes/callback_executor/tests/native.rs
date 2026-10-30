use super::scheduling::Release;
use super::*;
use crate::std_msgs::msg::UInt64;
use std::{
    sync::{mpsc, Condvar, Mutex as StdMutex},
    thread,
};

#[test]
fn callback_group_native_wait_and_callbacks_share_two_workers_for_one_node() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "callback_groups_one_node", "").unwrap();
    let mut slow = CallbackGroup::new();
    let mut healthy = CallbackGroup::new();
    let blocked = Arc::new((StdMutex::new(false), Condvar::new()));
    let release = Release(blocked.clone());
    let (started, start) = mpsc::channel();
    let (progress, seen) = mpsc::channel();
    let slow_topic = format!("/callback_group_slow_{}", uuid::Uuid::new_v4().simple());
    let slow_publisher = node
        .create_publisher::<UInt64>(&slow_topic, QosProfile::default())
        .unwrap();
    let _slow = node
        .subscribe_native_callback_in_group_with_options::<UInt64, _>(
            &mut slow,
            &slow_topic,
            QosProfile::default(),
            CallbackQueueOptions::default().capacity(1),
            move |message, _| {
                assert_eq!(message.data, 1);
                started.send(thread::current().id()).unwrap();
                let (lock, ready) = &*blocked;
                let mut released = lock.lock().unwrap();
                while !*released {
                    released = ready.wait(released).unwrap();
                }
                Ok(())
            },
        )
        .unwrap();
    let topic = format!("/callback_group_native_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<UInt64>(&topic, QosProfile::default())
        .unwrap();
    let _healthy = node
        .subscribe_native_callback_in_group::<UInt64, _>(
            &mut healthy,
            &topic,
            QosProfile::default(),
            move |message, _| {
                progress
                    .send((thread::current().id(), message.data))
                    .unwrap();
                Ok(())
            },
        )
        .unwrap();
    let matched = async {
        futures::try_join!(
            publisher.wait_for_subscribers().unwrap(),
            slow_publisher.wait_for_subscribers().unwrap()
        )?;
        Ok(())
    };
    node.spin_until_future_complete_with_options(
        matched,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let mut native = Executor::new(context);
    let node_id = native.add_node(node).unwrap();
    let scheduler = scheduler::Scheduler::new(Some(native), vec![slow, healthy], 2).unwrap();
    let workers: Vec<_> = (0..2)
        .map(|worker| {
            let scheduler = scheduler.clone();
            thread::spawn(move || scheduler.run(worker))
        })
        .collect();
    slow_publisher.publish(&UInt64 { data: 1 }).unwrap();
    let slow_thread = start.recv_timeout(Duration::from_secs(3)).unwrap();
    publisher.publish(&UInt64 { data: 42 }).unwrap();
    let progressed = seen.recv_timeout(Duration::from_millis(200));
    scheduler.cancel().unwrap();
    drop(release);
    for worker in workers {
        worker.join().unwrap().unwrap();
    }
    let (healthy_thread, payload) =
        progressed.expect("one Node's blocked callback stalled its other group");
    assert_eq!(payload, 42);
    assert_ne!(slow_thread, healthy_thread);
    let (native, groups) = scheduler.recover();
    let mut native = native.unwrap();
    assert_eq!(native.len(), 1);
    assert!(native.node_mut(node_id).is_ok());
    assert_eq!(groups.len(), 2);
}

#[test]
fn callback_group_native_precancel_suppresses_even_already_queued_user_work() {
    let context = Context::create().unwrap();
    let mut native = Executor::new(context.clone());
    let id = native
        .add_node(Node::create(context, "callback_group_precancel", "").unwrap())
        .unwrap();
    native.handle().cancel().unwrap();
    let mut group = CallbackGroup::new();
    let input = group
        .input(1, |_: ()| panic!("pre-canceled callback was executed"))
        .unwrap();
    input.push(()).unwrap();
    let scheduler = scheduler::Scheduler::new(Some(native), vec![group], 1).unwrap();
    let worker = scheduler.clone();
    thread::spawn(move || worker.run(0))
        .join()
        .unwrap()
        .unwrap();
    let (native, groups) = scheduler.recover();
    assert!(native.unwrap().node_mut(id).is_ok());
    assert_eq!(groups[0].len(), 1);
    drop(groups);
    assert!(input.is_closed());
}

#[test]
fn callback_group_native_dispatch_error_stops_callback_workers_before_observation() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "callback_group_native_error", "").unwrap();
    let _timer = node
        .create_wall_timer_callback_with_options(
            Duration::ZERO,
            CallbackTimerOptions::default().oneshot(true),
            || Err(Error::RCL_RET_INVALID_ARGUMENT),
        )
        .unwrap();
    let mut native = Executor::new(context);
    native.add_node(node).unwrap();
    let mut group = CallbackGroup::new();
    let input = group.input(1, |_: ()| Ok(())).unwrap();
    let scheduler = scheduler::Scheduler::new(Some(native), vec![group], 2).unwrap();
    let workers: Vec<_> = (0..2)
        .map(|index| {
            let scheduler = scheduler.clone();
            thread::spawn(move || scheduler.run(index))
        })
        .collect();
    let outcomes: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert!(outcomes.iter().any(|outcome| matches!(
        outcome,
        Err(scheduler::Failure::Native(Error::RCL_RET_INVALID_ARGUMENT))
    )));
    drop(scheduler.recover());
    assert!(input.is_closed());
}

#[test]
fn callback_group_native_source_full_ring_omits_waiting_and_preserves_all_unread_samples() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "callback_group_backpressure", "").unwrap();
    let mut group = CallbackGroup::new();
    let topic = format!("/grouped_pressure_{}", uuid::Uuid::new_v4().simple());
    let qos = QosProfile::default().keep_last(64);
    let publisher = node
        .create_publisher::<UInt64>(&topic, qos.clone())
        .unwrap();
    let values = Arc::new(Mutex::new(Vec::new()));
    let seen = values.clone();
    let token = node
        .subscribe_native_callback_in_group_with_options::<UInt64, _>(
            &mut group,
            &topic,
            qos,
            CallbackQueueOptions::default().capacity(2),
            move |message, _| {
                seen.lock().push(message.data);
                Ok(())
            },
        )
        .unwrap();
    let matched = publisher.wait_for_subscribers().unwrap();
    node.spin_until_future_complete_with_options(
        matched,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    for data in 0..32 {
        publisher.publish(&UInt64 { data }).unwrap();
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while node
        .prepare_spin(Duration::ZERO)
        .unwrap()
        .capacities
        .subscriptions
        != 0
    {
        node.try_spin_once(Duration::from_millis(1)).unwrap();
        assert!(std::time::Instant::now() < deadline, "source never filled its two-slot ring");
    }
    assert!(values.lock().is_empty());
    let wake = futures::task::noop_waker();
    assert!(group
        .poll_callbacks(&mut TaskContext::from_waker(&wake))
        .is_pending());
    assert_eq!(values.lock().as_slice(), [0]);
    assert_eq!(
        node.prepare_spin(Duration::ZERO)
            .unwrap()
            .capacities
            .subscriptions,
        1
    );
    while values.lock().len() != 32 {
        node.try_spin_once(Duration::from_millis(1)).unwrap();
        assert!(group
            .poll_callbacks(&mut TaskContext::from_waker(&wake))
            .is_pending());
        assert!(
            std::time::Instant::now() < deadline,
            "capacity return failed to resume unread DDS input"
        );
    }
    assert_eq!(*values.lock(), (0..32).collect::<Vec<_>>());
    drop(token);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(
        node.prepare_spin(Duration::ZERO)
            .unwrap()
            .capacities
            .subscriptions,
        0
    );
    assert!(matches!(
        group.poll_callbacks(&mut TaskContext::from_waker(&wake)),
        Poll::Ready(Ok(()))
    ));
}

#[test]
fn callback_group_native_factory_invalid_queue_rolls_back_group_and_node_registration() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "callback_group_factory_invalid", "").unwrap();
    let mut group = CallbackGroup::new();
    for options in [
        CallbackQueueOptions::default().capacity(0),
        CallbackQueueOptions::default().max_inputs_per_spin(0),
    ] {
        assert!(matches!(
            node.subscribe_native_callback_in_group_with_options::<UInt64, _>(
                &mut group,
                "/invalid_group_source",
                QosProfile::default(),
                options,
                |_, _| Ok(()),
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
    }
    assert!(group.is_empty());
    assert!(node.grouped_callbacks.is_none());
}

#[test]
fn callback_group_subscription_endpoint_options_and_qos_cleanup_keep_native_identity() {
    use crate::SubscriptionEventKind;
    let context = Context::create().unwrap();
    let mut node = Node::create(context, "callback_group_subscription_options", "").unwrap();
    let mut group = CallbackGroup::new();
    let options = GroupedSubscriptionOptions::default()
        .queue(CallbackQueueOptions::default().capacity(3))
        .endpoint(NativeCallbackSubscriptionOptions::default().ignore_local_publications(true));
    let token = node
        .subscribe_native_callback_in_group_with_options::<UInt64, _>(
            &mut group,
            "/grouped_options",
            QosProfile::default(),
            options,
            |_, _| Ok(()),
        )
        .unwrap();
    assert_eq!(token.topic_name(&node).unwrap(), "/grouped_options");
    let source_id = token.source_id();
    let handle = node.subscriber(source_id).unwrap().handle();
    let options = unsafe { rcl_subscription_get_options(handle).as_ref() }.unwrap();
    assert!(options.rmw_subscription_options.ignore_local_publications);
    let mut events = token
        .qos_events(&mut node, SubscriptionEventKind::DeadlineMissed)
        .unwrap();
    node.destroy_grouped_subscription(token);
    assert!(matches!(node.subscriber(source_id), Err(Error::RCL_RET_SUBSCRIPTION_INVALID)));
    assert_eq!(node.prepare_spin(Duration::ZERO).unwrap().capacities.events, 0);
    use futures::StreamExt;
    assert!(matches!(
        events.next().now_or_never(),
        Some(Some(Err(Error::RCL_RET_SUBSCRIPTION_INVALID)))
    ));
    assert!(matches!(events.next().now_or_never(), Some(None)));
}

#[tokio::test(flavor = "current_thread")]
async fn callback_group_owned_task_stops_and_reuses_node_and_handler_state_with_weak_controls() {
    use futures::StreamExt;
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "callback_group_owned_task", "").unwrap();
    let mut group = CallbackGroup::new();
    let topic = format!("/callback_group_owned_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<UInt64>(&topic, QosProfile::default())
        .unwrap();
    let (sender, mut received) = futures::channel::mpsc::unbounded();
    let mut ordinal = 0;
    let token = node
        .subscribe_native_callback_in_group::<UInt64, _>(
            &mut group,
            &topic,
            QosProfile::default(),
            move |message, _| {
                sender
                    .unbounded_send((message.data, ordinal))
                    .map_err(|_| Error::RCL_RET_ERROR)?;
                ordinal += 1;
                Ok(())
            },
        )
        .unwrap();
    let ready = publisher.wait_for_subscribers().unwrap();
    node.spin_until_future_complete_with_options(
        ready,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let mut executor = CallbackExecutor::new(context, 2).unwrap();
    let group_id = executor.add_group(group).unwrap();
    let node_id = executor.add_node(node).unwrap();
    let control = executor.handle();
    for (data, ordinal) in [(40, 0), (42, 1)] {
        let task = executor.spawn().unwrap();
        assert_eq!(task.worker_count(), 2);
        publisher.publish(&UInt64 { data }).unwrap();
        let result = tokio::time::timeout(Duration::from_secs(3), received.next()).await;
        executor = task.stop().await.unwrap();
        assert_eq!(result.unwrap().unwrap(), (data, ordinal));
        assert!(executor.is_cancelled());
        assert!(executor.node_mut(node_id).is_ok());
        assert_eq!(executor.group_mut(group_id).unwrap().len(), 1);
        executor.reset_cancel();
    }
    assert!(token.is_valid());
    drop(token);
    drop(executor);
    assert!(!control.cancel().unwrap());
}

#[test]
fn callback_group_owned_executor_rejects_missing_group_and_foreign_native_sources() {
    let context = Context::create().unwrap();
    assert!(matches!(
        CallbackExecutor::new(context.clone(), 0),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    let mut node = Node::create(context.clone(), "callback_group_missing_owner", "").unwrap();
    let mut group = CallbackGroup::new();
    let token = node
        .subscribe_native_callback_in_group::<UInt64, _>(
            &mut group,
            "/missing_group",
            QosProfile::default(),
            |_, _| Ok(()),
        )
        .unwrap();
    let mut executor = CallbackExecutor::new(context.clone(), 2).unwrap();
    executor.add_node(node).unwrap();
    assert!(matches!(
        executor.spawn(),
        Err(CallbackExecutorTaskError::Native(Error::RCL_RET_INVALID_ARGUMENT))
    ));
    drop(token);
    drop(group);

    let mut external = Node::create(context.clone(), "callback_group_foreign_owner", "").unwrap();
    let mut group = CallbackGroup::new();
    let token = external
        .subscribe_native_callback_in_group::<UInt64, _>(
            &mut group,
            "/foreign_group",
            QosProfile::default(),
            |_, _| Ok(()),
        )
        .unwrap();
    let mut executor = CallbackExecutor::new(context, 2).unwrap();
    executor.add_group(group).unwrap();
    assert!(matches!(
        executor.spawn(),
        Err(CallbackExecutorTaskError::Native(Error::RCL_RET_INVALID_ARGUMENT))
    ));
    drop(token);
    drop(external);
}
