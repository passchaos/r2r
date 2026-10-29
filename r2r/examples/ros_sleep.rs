//! ROS-time sleeps compose with wall timers in one wake-driven executor.
#[cfg(r2r__rosgraph_msgs__msg__Clock)]
fn main() -> r2r::Result<()> {
    use r2r::{
        builtin_interfaces::msg::Time, rosgraph_msgs::msg::Clock, FutureExecutionOptions,
        QosProfile,
    };
    use std::time::Duration;
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "ros_sleep_example", "")?;
    node.get_time_source().enable_sim_time(&mut node)?;
    let clock = node.create_publisher::<Clock>("/clock", QosProfile::default())?;
    let ready = clock.wait_for_subscribers()?;
    node.spin_until_future_complete_with_options(
        ready,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )?;
    let sleep = node.sleep_until_ros(3_000_000_000)?;
    let mut wall = node.create_wall_timer(Duration::from_millis(10))?;
    node.spin_until_future_complete_with_options(
        async {
            let source = async {
                for sec in [1, 2, 1, 3] {
                    wall.tick().await?;
                    clock.publish(&Clock {
                        clock: Time { sec, nanosec: 0 },
                    })?;
                }
                Ok::<_, r2r::Error>(())
            };
            futures::try_join!(source, sleep)?;
            Ok(())
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )?;
    assert_eq!(node.now_ns()?, 3_000_000_000);
    let mut canceled = node.sleep_for_ros(Duration::from_secs(1))?;
    assert!(canceled.cancel());
    assert!(matches!(
        futures::executor::block_on(canceled),
        Err(r2r::Error::RosSleepCancelled)
    ));
    println!("ROS sleep reached three simulated seconds after a backwards jump; independent cancellation passed");
    Ok(())
}

#[cfg(not(r2r__rosgraph_msgs__msg__Clock))]
fn main() {
    println!("ros_sleep example requires rosgraph_msgs/msg/Clock");
}
