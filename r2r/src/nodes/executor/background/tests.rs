use super::*;

mod cleanup;
mod observers;
mod workflows;
use std::{sync::mpsc, time::Duration};

use crate::example_interfaces::srv::AddTwoInts;
use crate::{Context as RosContext, Node, QosProfile};
use futures::{executor::block_on, StreamExt};

fn node(context: &RosContext, name: &str) -> Node {
    Node::create(context.clone(), &format!("{name}_{}", uuid::Uuid::new_v4().simple()), "").unwrap()
}

fn bounded_join(worker: JoinHandle<()>) {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(worker.join());
    });
    receiver
        .recv_timeout(Duration::from_secs(5))
        .expect("native worker did not stop")
        .unwrap();
}

#[test]
fn task_is_send_and_futures_executor_can_recover_owned_nodes() {
    fn require_send<T: Send>() {}
    require_send::<ExecutorTask>();
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_owned");
    let publisher = owned
        .create_publisher::<crate::std_msgs::msg::Int32>(
            "background_owned_data",
            QosProfile::default(),
        )
        .unwrap();
    let mut executor = Executor::new(context.clone());
    let id = executor.add_node(owned).unwrap();
    let task = executor.spawn().unwrap();
    let handle = task.handle();
    handle.cancel().unwrap();
    let mut recovered = block_on(task).unwrap();
    assert!(context.is_valid());
    assert_eq!(recovered.len(), 1);
    assert!(recovered.is_cancelled());
    recovered.reset_cancel();
    publisher
        .publish(&crate::std_msgs::msg::Int32 { data: 42 })
        .unwrap();
    recovered.node_mut(id).unwrap().destroy_publisher(publisher);
    let task = recovered.spawn().unwrap();
    let mut recovered = block_on(task.stop()).unwrap();
    assert!(recovered
        .remove_node(id)
        .unwrap()
        .name()
        .unwrap()
        .starts_with("background_owned"));
}

#[test]
fn dropping_pending_task_stops_worker_and_releases_native_endpoints() {
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_drop");
    let publisher = owned
        .create_publisher::<crate::std_msgs::msg::Int32>(
            "background_drop_data",
            QosProfile::default(),
        )
        .unwrap();
    let mut executor = Executor::new(context.clone());
    executor.add_node(owned).unwrap();
    let mut task = executor.spawn().unwrap();
    let control = task.handle();
    let worker = task.thread.take().unwrap();
    drop(task);
    bounded_join(worker);
    assert!(!control.cancel().unwrap());
    assert!(context.is_valid());
    assert!(matches!(
        publisher.publish(&crate::std_msgs::msg::Int32 { data: 1 }),
        Err(Error::RCL_RET_PUBLISHER_INVALID)
    ));
}

#[test]
fn cancellation_before_guard_registration_is_persistent() {
    let context = RosContext::create().unwrap();
    let mut executor = Executor::new(context.clone());
    executor
        .add_node(node(&context, "background_precancel"))
        .unwrap();
    executor.handle().cancel().unwrap();
    let recovered = block_on(executor.spawn().unwrap()).unwrap();
    assert!(recovered.is_cancelled());
    assert_eq!(recovered.len(), 1);
}

#[test]
fn native_failure_releases_executor_before_error_completion() {
    let context = RosContext::create().unwrap();
    let mut owned = node(&context, "background_native_error");
    let publisher = owned
        .create_publisher::<crate::std_msgs::msg::Int32>(
            "background_error_data",
            QosProfile::default(),
        )
        .unwrap();
    let mut executor = Executor::new(context.clone());
    executor.add_node(owned).unwrap();
    let handle = executor.handle();
    let (sender, receiver) = oneshot::channel();
    complete(executor, sender, Ok(Err(Error::RCL_RET_ERROR)));
    assert!(matches!(
        block_on(receiver).unwrap(),
        Err(ExecutorTaskError::Native(Error::RCL_RET_ERROR))
    ));
    assert!(!handle.cancel().unwrap());
    assert!(matches!(
        publisher.publish(&crate::std_msgs::msg::Int32 { data: 1 }),
        Err(Error::RCL_RET_PUBLISHER_INVALID)
    ));
}

#[test]
fn unused_completion_receiver_releases_returned_executor_on_drop() {
    let context = RosContext::create().unwrap();
    let mut executor = Executor::new(context.clone());
    executor
        .add_node(node(&context, "background_unobserved"))
        .unwrap();
    let mut task = executor.spawn().unwrap();
    task.cancel().unwrap();
    let worker = task.thread.take().unwrap();
    bounded_join(worker);
    assert!(task.is_finished());
    let control = task.handle();
    assert!(control.is_cancelled());
    drop(task);
    assert!(!control.cancel().unwrap());
    assert!(context.is_valid());
}
