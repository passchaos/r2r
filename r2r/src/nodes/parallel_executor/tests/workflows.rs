use super::*;

#[tokio::test(flavor = "current_thread")]
async fn parallel_node_executor_precancel_reaches_all_groups_before_native_callbacks() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let context = Context::create().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let mut executor = ParallelExecutor::new(context.clone(), 3).unwrap();
    let mut timers = Vec::new();
    for index in 0..2 {
        let mut node =
            Node::create(context.clone(), &format!("parallel_precancel_{index}"), "").unwrap();
        let count = calls.clone();
        timers.push(
            node.create_wall_timer_callback(Duration::ZERO, move || {
                count.fetch_add(1, Ordering::Release);
                Ok(())
            })
            .unwrap(),
        );
        executor.add_node(node).unwrap();
    }
    let control = executor.handle();
    assert!(control.cancel().unwrap());
    let executor = tokio::time::timeout(Duration::from_secs(3), executor.spawn().unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert!(executor.is_cancelled());
    assert_eq!(executor.len(), 2);
    drop(executor);
    assert!(!control.cancel().unwrap());
    drop(timers);
}

#[cfg(r2r__rosgraph_msgs__msg__Clock)]
#[tokio::test(flavor = "current_thread")]
async fn parallel_node_executor_lifecycle_services_and_native_clock_keep_managed_callbacks_serial()
{
    use crate::{
        builtin_interfaces::msg::Time,
        lifecycle_msgs::{msg::Transition, srv::ChangeState},
        rosgraph_msgs::msg::Clock as ClockMessage,
        std_msgs::msg::Int64,
        LifecycleNode,
    };
    use futures::StreamExt;

    let context = Context::create().unwrap();
    let mut sensor = LifecycleNode::create(context.clone(), "parallel_clock_sensor", "").unwrap();
    let source = sensor.get_time_source();
    source.enable_sim_time(&mut sensor).unwrap();
    let clock = sensor.get_ros_clock();
    let mut manager = Node::create(context.clone(), "parallel_clock_manager", "").unwrap();
    let topic = format!("/parallel_ticks_{}", uuid::Uuid::new_v4().simple());
    let publisher = sensor
        .create_publisher::<Int64>(&topic, QosProfile::default())
        .unwrap();
    let output = publisher.clone();
    let now = clock.clone();
    let timer = sensor
        .create_timer_callback(Duration::from_millis(10), move || {
            output.publish(&Int64 {
                data: now.lock().get_now_ns()?,
            })
        })
        .unwrap();
    let mut ticks = manager
        .subscribe::<Int64>(&topic, QosProfile::default())
        .unwrap();
    let client = manager
        .create_client::<ChangeState::Service>(
            "/parallel_clock_sensor/change_state",
            QosProfile::services_default(),
        )
        .unwrap();
    let clock_publisher = manager
        .create_publisher::<ClockMessage>("/clock", QosProfile::default())
        .unwrap();
    let mut executor = ParallelExecutor::new(context, 2).unwrap();
    let sensor_id = executor.add_node(sensor.into_node()).unwrap();
    executor.add_node(manager).unwrap();
    let task = executor.spawn().unwrap();

    let application = tokio::time::timeout(Duration::from_secs(5), async {
        // Register every observer after spawn to exercise the current per-worker guards.
        futures::try_join!(
            client.wait_for_service().unwrap(),
            publisher.wait_for_subscribers().unwrap(),
            clock_publisher.wait_for_subscribers().unwrap()
        )
        .unwrap();
        let clock = &clock;
        let clock_publisher = &clock_publisher;
        let client = &client;
        let set_time = |nanosec| async move {
            clock_publisher
                .publish(&ClockMessage {
                    clock: Time { sec: 0, nanosec },
                })
                .unwrap();
            while clock.lock().get_now_ns().unwrap() != i64::from(nanosec) {
                tokio::task::yield_now().await;
            }
        };
        let transition = |id| async move {
            assert!(
                client
                    .request(&ChangeState::Request {
                        transition: Transition {
                            id,
                            label: Default::default()
                        },
                    })
                    .unwrap()
                    .await
                    .unwrap()
                    .success
            );
        };
        async fn absent(ticks: &mut crate::Subscription<Int64>) {
            assert!(tokio::time::timeout(Duration::from_millis(30), ticks.next())
                .await
                .is_err());
        }
        set_time(100_000_000).await;
        absent(&mut ticks).await;
        transition(Transition::TRANSITION_CONFIGURE as u8).await;
        transition(Transition::TRANSITION_ACTIVATE as u8).await;
        set_time(110_000_000).await;
        assert_eq!(ticks.next().await.unwrap().data, 110_000_000);
        transition(Transition::TRANSITION_DEACTIVATE as u8).await;
        // A native backwards update during inactive establishes a fresh activation deadline.
        set_time(20_000_000).await;
        absent(&mut ticks).await;
        transition(Transition::TRANSITION_ACTIVATE as u8).await;
        set_time(29_000_000).await;
        absent(&mut ticks).await;
        set_time(30_000_000).await;
        assert_eq!(ticks.next().await.unwrap().data, 30_000_000);
        transition(Transition::TRANSITION_ACTIVE_SHUTDOWN as u8).await;
        set_time(100_000_000).await;
        absent(&mut ticks).await;
    })
    .await;
    let mut executor = task.stop().await.unwrap();
    application.unwrap();
    assert!(timer
        .is_canceled(executor.node_mut(sensor_id).unwrap())
        .unwrap());
    timer.destroy(executor.node_mut(sensor_id).unwrap());
    publisher.destroy(executor.node_mut(sensor_id).unwrap());
}
