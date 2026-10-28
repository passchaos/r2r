//! A managed sensor pauses its native wall timer between active phases.

use std::time::Duration;

use futures::StreamExt;
use r2r::{
    lifecycle_msgs::{msg::Transition, srv::ChangeState},
    std_msgs::msg::UInt64,
    Client, Context, Error, Executor, FutureExecutionOptions, LifecycleNode, LifecyclePublisher,
    LifecycleTimer, Node, QosProfile, Subscription,
};

fn transition(
    executor: &mut Executor, client: &Client<ChangeState::Service>, id: u8,
) -> r2r::Result<()> {
    let result = client.request(&ChangeState::Request {
        transition: Transition {
            id,
            label: Default::default(),
        },
    })?;
    let response = executor.spin_until_future_complete_with_options(
        result,
        Duration::from_secs(5),
        FutureExecutionOptions::wake_driven(),
    )?;
    assert!(response.success, "lifecycle command {id} failed");
    Ok(())
}

fn samples(
    executor: &mut Executor, timer: &mut LifecycleTimer, publisher: &LifecyclePublisher<UInt64>,
    received: &mut Subscription<UInt64>, first: u64,
) -> r2r::Result<()> {
    executor.spin_until_future_complete_with_options(
        async {
            for value in first..first + 8 {
                timer.tick().await?;
                publisher.publish(&UInt64 { data: value })?;
                let message = received
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
                assert_eq!(message.data, value);
            }
            Ok(())
        },
        Duration::from_secs(5),
        FutureExecutionOptions::wake_driven(),
    )
}

fn paused(executor: &mut Executor, timer: &mut LifecycleTimer) {
    assert!(matches!(
        executor.spin_until_future_complete_with_options(
            timer.tick(),
            Duration::from_millis(30),
            FutureExecutionOptions::wake_driven(),
        ),
        Err(Error::RCL_RET_TIMEOUT)
    ));
}

fn main() -> r2r::Result<()> {
    let context = Context::create()?;
    let mut sensor = LifecycleNode::create(context.clone(), "managed_sensor", "")?;
    let mut manager = Node::create(context.clone(), "managed_sensor_manager", "")?;
    let mut timer = sensor.create_wall_timer(Duration::from_millis(5))?;
    let publisher =
        sensor.create_publisher::<UInt64>("managed_sensor_values", QosProfile::default())?;
    let mut received =
        manager.subscribe::<UInt64>("managed_sensor_values", QosProfile::default())?;
    let client = manager.create_client::<ChangeState::Service>(
        "/managed_sensor/change_state",
        QosProfile::services_default(),
    )?;
    let ready = client.wait_for_service()?;
    let matched = publisher.wait_for_subscribers()?;
    let mut executor = Executor::new(context);
    let sensor_id = executor.add_node(sensor.into_node())?;
    executor.add_node(manager)?;
    executor.spin_until_future_complete_with_options(
        async {
            futures::try_join!(ready, matched)?;
            Ok(())
        },
        Duration::from_secs(5),
        FutureExecutionOptions::wake_driven(),
    )?;
    paused(&mut executor, &mut timer);
    transition(&mut executor, &client, Transition::TRANSITION_CONFIGURE as u8)?;
    transition(&mut executor, &client, Transition::TRANSITION_ACTIVATE as u8)?;
    samples(&mut executor, &mut timer, &publisher, &mut received, 0)?;
    transition(&mut executor, &client, Transition::TRANSITION_DEACTIVATE as u8)?;
    paused(&mut executor, &mut timer);
    transition(&mut executor, &client, Transition::TRANSITION_ACTIVATE as u8)?;
    samples(&mut executor, &mut timer, &publisher, &mut received, 8)?;
    transition(&mut executor, &client, Transition::TRANSITION_ACTIVE_SHUTDOWN as u8)?;
    paused(&mut executor, &mut timer);
    assert!(timer.is_canceled(executor.node_mut(sensor_id)?)?);
    timer.destroy(executor.node_mut(sensor_id)?);
    publisher.destroy(executor.node_mut(sensor_id)?);
    println!("validated 16 timer-driven samples, two active phases and three paused phases");
    Ok(())
}
