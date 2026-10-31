use futures::{executor::LocalPool, task::LocalSpawnExt, StreamExt};
use r2r::{Error, ParameterValue};
use std::time::Duration;

// Run with:
// cargo run --example parameter_callbacks -- --ros-args -r __ns:=/demo
//
// In another shell:
// ros2 param set /demo/parameter_callbacks gain 4
// ros2 param get /demo/parameter_callbacks effective_gain  # 8
// ros2 param set /demo/parameter_callbacks gain 20          # rejected

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = r2r::Context::create()?;
    let mut node = r2r::Node::create(context, "parameter_callbacks", "")?;
    node.declare_parameter("gain", 1_i64)?;
    node.declare_parameter("effective_gain", 2_i64)?;

    // Pre-set callbacks may derive or normalize values. The appended value wins if the input
    // already contained the same name, so the two values commit as one transaction.
    let pre = node.add_pre_set_parameters_callback(|updates| {
        let gain = updates.iter().rev().find_map(|(name, value)| {
            (name == "gain")
                .then_some(value)
                .and_then(|value| match value {
                    ParameterValue::Integer(value) => Some(*value),
                    _ => None,
                })
        });
        if let Some(gain) = gain {
            updates
                .push(("effective_gain".into(), ParameterValue::Integer(gain.saturating_mul(2))));
        }
    });

    // On-set callbacks validate the transformed batch before any value changes.
    let validate = node.add_on_set_parameters_callback(|updates| {
        let gain = updates.iter().find_map(|(name, value)| {
            (name == "gain")
                .then_some(value)
                .and_then(|value| match value {
                    ParameterValue::Integer(value) => Some(*value),
                    _ => None,
                })
        });
        if gain.is_some_and(|gain| !(0..=10).contains(&gain)) {
            return Err(Error::ParameterConstraintViolation {
                name: "gain".into(),
                reason: "gain must be between 0 and 10".into(),
            });
        }
        if gain.is_none() && updates.iter().any(|(name, _)| name == "effective_gain") {
            return Err(Error::ParameterConstraintViolation {
                name: "effective_gain".into(),
                reason: "effective_gain is derived from gain".into(),
            });
        }
        Ok(())
    });

    // Post-set callbacks observe only committed batches and run after parameter locks release.
    let post = node.add_post_set_parameters_callback(|updates| {
        println!("committed parameter transaction: {updates:?}");
    });
    let _callbacks = (pre, validate, post);

    let (handler, mut events) = node.make_parameter_handler()?;
    let mut pool = LocalPool::new();
    pool.spawner().spawn_local(handler)?;
    pool.spawner().spawn_local(async move {
        while let Some((name, value)) = events.next().await {
            println!("parameter event: {name} = {value:?}");
        }
    })?;

    node.set_parameter("gain", 3_i64)?;
    loop {
        node.try_spin_once(Duration::from_millis(100))?;
        pool.run_until_stalled();
    }
}
