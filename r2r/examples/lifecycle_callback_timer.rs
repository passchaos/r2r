//! Native lifecycle services control callback-driven sensor publication on one shared wait.

use futures::StreamExt;
use r2r::{
    lifecycle_msgs::{msg::Transition, srv::ChangeState},
    std_msgs::msg::UInt64,
    Client, Context, Error, Executor, FutureExecutionOptions, LifecycleNode, Node, QosProfile,
    Subscription,
};
use std::{
    sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

fn transition(
    executor: &mut Executor, client: &Client<ChangeState::Service>, id: u8,
) -> r2r::Result<()> {
    let request = client.request(&ChangeState::Request {
        transition: Transition {
            id,
            label: Default::default(),
        },
    })?;
    assert!(
        executor
            .spin_until_future_complete_with_options(
                request,
                Duration::from_secs(3),
                FutureExecutionOptions::wake_driven()
            )?
            .success
    );
    Ok(())
}
fn receive(
    executor: &mut Executor, subscription: &mut Subscription<UInt64>, first: u64,
) -> r2r::Result<()> {
    executor.spin_until_future_complete_with_options(
        async {
            for expected in first..first + 8 {
                let message = subscription
                    .next()
                    .await
                    .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
                assert_eq!(message.data, expected);
            }
            Ok(())
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
}
fn paused(executor: &mut Executor, calls: &AtomicUsize) {
    let before = calls.load(Ordering::Acquire);
    assert!(matches!(
        executor.spin_until_future_complete_with_options(
            futures::future::pending::<r2r::Result<()>>(),
            Duration::from_millis(30),
            FutureExecutionOptions::wake_driven()
        ),
        Err(Error::RCL_RET_TIMEOUT)
    ));
    assert_eq!(calls.load(Ordering::Acquire), before);
}
fn main() -> r2r::Result<()> {
    let context = Context::create()?;
    let mut sensor = LifecycleNode::create(context.clone(), "callback_sensor", "")?;
    let mut manager = Node::create(context.clone(), "callback_sensor_manager", "")?;
    let publisher = sensor.create_publisher::<UInt64>(
        "callback_sensor_values",
        QosProfile::default().keep_last(32),
    )?;
    let mut subscription = manager
        .subscribe::<UInt64>("callback_sensor_values", QosProfile::default().keep_last(32))?;
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let limit = Arc::new(AtomicU64::new(8));
    let target = limit.clone();
    let output = publisher.clone();
    let mut next = 0;
    let timer = sensor.create_wall_timer_callback(Duration::from_millis(2), move || {
        count.fetch_add(1, Ordering::Release);
        if next < target.load(Ordering::Acquire) {
            output.publish(&UInt64 { data: next })?;
            next += 1;
        }
        Ok(())
    })?;
    let client = manager.create_client::<ChangeState::Service>(
        "/callback_sensor/change_state",
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
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )?;
    paused(&mut executor, &calls);
    transition(&mut executor, &client, Transition::TRANSITION_CONFIGURE as u8)?;
    transition(&mut executor, &client, Transition::TRANSITION_ACTIVATE as u8)?;
    receive(&mut executor, &mut subscription, 0)?;
    transition(&mut executor, &client, Transition::TRANSITION_DEACTIVATE as u8)?;
    paused(&mut executor, &calls);
    limit.store(16, Ordering::Release);
    transition(&mut executor, &client, Transition::TRANSITION_ACTIVATE as u8)?;
    receive(&mut executor, &mut subscription, 8)?;
    transition(&mut executor, &client, Transition::TRANSITION_ACTIVE_SHUTDOWN as u8)?;
    paused(&mut executor, &calls);
    assert!(timer.is_canceled(executor.node_mut(sensor_id)?)?);
    timer.destroy(executor.node_mut(sensor_id)?);
    publisher.destroy(executor.node_mut(sensor_id)?);
    println!("validated 16 lifecycle callback publications, two active phases and three native paused phases");
    Ok(())
}
