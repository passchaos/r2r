use crate::{CallbackReturn, CallbackTimerOptions, Context, Error, LifecycleNode, Node};

mod controls;
mod execution;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

fn node(name: &str) -> LifecycleNode {
    let node = LifecycleNode::create(
        Context::create().unwrap(),
        &format!("{name}_{}", uuid::Uuid::new_v4().simple()),
        "",
    )
    .unwrap();
    node.get_ros_clock()
        .lock()
        .enable_ros_time_override(0)
        .unwrap();
    node
}

fn time(node: &LifecycleNode, ms: i64) {
    node.get_ros_clock()
        .lock()
        .set_ros_time_override(ms * 1_000_000)
        .unwrap();
}
fn spin(node: &mut LifecycleNode) {
    if let Err(error) = node.try_spin_once(Duration::ZERO) {
        let native = unsafe { r2r_rcl::rcutils_get_error_string() };
        panic!(
            "{error:?}: {}",
            unsafe { std::ffi::CStr::from_ptr(native.str_.as_ptr()) }.to_string_lossy()
        );
    }
}
fn active(node: &mut LifecycleNode) {
    assert_eq!(node.configure().unwrap(), CallbackReturn::Success);
    assert_eq!(node.activate().unwrap(), CallbackReturn::Success);
    spin(node);
}

#[test]
fn lifecycle_callback_timer_inactive_registration_is_natively_paused() {
    let mut node = node("lc_callback_inactive");
    let count = Arc::new(AtomicUsize::new(0));
    let observed = count.clone();
    let timer = node
        .create_timer_callback(Duration::from_millis(10), move || {
            observed.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .unwrap();
    assert!(
        timer.is_canceled(&node).unwrap(),
        "inactive lifecycle callback timer started its native schedule"
    );
    node.get_ros_clock()
        .lock()
        .set_ros_time_override(100_000_000)
        .unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 0);
    assert_eq!(node.configure().unwrap(), CallbackReturn::Success);
    node.get_ros_clock()
        .lock()
        .set_ros_time_override(200_000_000)
        .unwrap();
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(count.load(Ordering::Relaxed), 0);
}
