//! Managed (lifecycle) node example.
//!
//! The node registers configure/activate/deactivate/cleanup/shutdown
//! callbacks, then drives itself through the full lifecycle with the direct
//! Rust API. While running it also serves the standard lifecycle services, so
//! you can inspect it from another shell:
//!
//! ```sh
//! ros2 lifecycle get /lifecycle_example
//! ros2 lifecycle list /lifecycle_example
//! ros2 lifecycle set /lifecycle_example configure
//! ```

use std::time::Duration;

use r2r::{CallbackReturn, LifecycleNode, LifecycleState};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ctx = r2r::Context::create()?;
    let mut node = LifecycleNode::create(ctx, "lifecycle_example", "")?;

    node.on_configure(Box::new(|previous| {
        println!("on_configure (previous state {previous:?})");
        CallbackReturn::Success
    }))
    .on_activate(Box::new(|_| {
        println!("on_activate");
        CallbackReturn::Success
    }))
    .on_deactivate(Box::new(|_| {
        println!("on_deactivate");
        CallbackReturn::Success
    }))
    .on_cleanup(Box::new(|_| {
        println!("on_cleanup");
        CallbackReturn::Success
    }))
    .on_shutdown(Box::new(|_| {
        println!("on_shutdown");
        CallbackReturn::Success
    }))
    .on_error(Box::new(|previous| {
        println!("on_error while leaving {previous:?}");
        CallbackReturn::Success
    }));

    // Drive the full lifecycle from Rust; each step also publishes
    // transition events and answers external change_state calls identically.
    let steps: [(&str, fn(&LifecycleNode) -> r2r::Result<CallbackReturn>); 5] = [
        ("configure", LifecycleNode::configure),
        ("activate", LifecycleNode::activate),
        ("deactivate", LifecycleNode::deactivate),
        ("cleanup", LifecycleNode::cleanup),
        ("shutdown", LifecycleNode::shutdown),
    ];
    for (name, step) in steps {
        std::thread::sleep(Duration::from_millis(200));
        let outcome = step(&node)?;
        assert_eq!(outcome, CallbackReturn::Success);
        println!("{name} -> state {:?}", node.lifecycle_state());
        // Keep services (including any external ros2 lifecycle call)
        // serviced while the transition settles.
        for _ in 0..5 {
            node.spin_once(Duration::from_millis(20));
        }
    }

    assert_eq!(node.lifecycle_state(), LifecycleState::Finalized);
    println!("lifecycle finished");
    Ok(())
}
