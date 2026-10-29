//! Controlled native /clock trace for independent Rust/C++ clock sleep verification.
#[cfg(r2r__rosgraph_msgs__msg__Clock)]
fn main() -> r2r::Result<()> {
    use futures::{FutureExt, StreamExt};
    use r2r::{
        builtin_interfaces::msg::Time, rosgraph_msgs::msg::Clock, FutureExecutionOptions,
        QosProfile,
    };
    use std::{
        io::{self, Write},
        time::{Duration, Instant},
    };
    let mut node = r2r::Node::create(r2r::Context::create()?, "rust_ros_sleep_oracle", "")?;
    node.get_time_source().enable_sim_time(&mut node)?;
    let clock = node.get_ros_clock();
    let publisher = node.create_publisher::<Clock>("/clock", QosProfile::default())?;
    let ready = publisher.wait_for_subscribers()?;
    node.spin_until_future_complete_with_options(
        ready,
        Duration::from_secs(5),
        FutureExecutionOptions::wake_driven(),
    )?;
    let (input, mut commands) = futures::channel::mpsc::unbounded::<String>();
    let reader = std::thread::spawn(move || {
        use std::io::BufRead;
        for line in io::stdin().lock().lines() {
            let line = line.unwrap();
            let finish = line == "finish";
            if input.unbounded_send(line).is_err() || finish {
                break;
            }
        }
    });
    let mut sleep: Option<r2r::RosSleep> = None;
    let mut state = "idle";
    println!("{{\"kind\":\"ready\",\"implementation\":\"r2r\"}}");
    io::stdout().flush().unwrap();
    loop {
        let command = node.spin_until_future_complete_with_options(
            async { commands.next().await.ok_or(r2r::Error::RCL_RET_ERROR) },
            Duration::from_secs(20),
            FutureExecutionOptions::wake_driven(),
        )?;
        let words: Vec<_> = command.split_whitespace().collect();
        match words[0] {
            "clock" => {
                let time: i64 = words[1].parse().unwrap();
                let sec = i32::try_from(time.div_euclid(1_000_000_000)).unwrap();
                let nanosec = time.rem_euclid(1_000_000_000) as u32;
                publisher.publish(&Clock {
                    clock: Time { sec, nanosec },
                })?;
                let deadline = Instant::now() + Duration::from_secs(5);
                while clock.lock().get_now_ns()? != time {
                    assert!(Instant::now() < deadline, "native clock sample did not arrive");
                    node.try_spin_once(Duration::from_millis(20))?;
                }
            }
            "sleep" => {
                assert!(sleep.is_none());
                sleep = Some(node.sleep_until_ros(words[1].parse().unwrap())?);
                state = "pending";
            }
            "pause" => node.try_spin_once(Duration::from_millis(50))?,
            "cancel" => {
                assert!(sleep.as_mut().unwrap().cancel());
            }
            "finish" => {}
            _ => panic!("invalid clock trace command"),
        }
        if let Some(wait) = &mut sleep {
            if let Some(result) = wait.now_or_never() {
                state = match result {
                    Ok(()) => "complete",
                    Err(r2r::Error::RosSleepCancelled) => "canceled",
                    Err(error) => return Err(error),
                };
                sleep = None;
            }
        }
        println!(
            "{}",
            serde_json::json!({"command":command, "now_ns":clock.lock().get_now_ns()?, "state":state})
        );
        io::stdout().flush().unwrap();
        if command == "finish" {
            break;
        }
    }
    reader.join().unwrap();
    Ok(())
}

#[cfg(not(r2r__rosgraph_msgs__msg__Clock))]
fn main() {
    std::process::exit(2);
}
