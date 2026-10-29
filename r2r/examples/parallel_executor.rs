//! Keep control RPCs responsive while a separate Node group executes slow work.

use futures::channel::oneshot;
use r2r::{
    example_interfaces::srv::AddTwoInts, CallbackTimerOptions, Context, Node, ParallelExecutor,
    QosProfile,
};
use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

// Ensure the simulated slow callback can return even if the application fails.
struct Release(Arc<(Mutex<bool>, Condvar)>);
impl Drop for Release {
    fn drop(&mut self) {
        let (lock, ready) = &*self.0;
        *lock.lock().unwrap() = true;
        ready.notify_all();
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = Context::create()?;
    let mut perception = Node::create(context.clone(), "parallel_perception", "")?;
    let mut control = Node::create(context.clone(), "parallel_control", "")?;
    let mut application = Node::create(context.clone(), "parallel_application", "")?;
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let release = Release(gate.clone());
    let (started, start) = oneshot::channel();
    let mut started = Some(started);
    let slow = perception.create_wall_timer_callback_with_options(
        Duration::ZERO,
        CallbackTimerOptions::default().oneshot(true),
        move || {
            if let Some(started) = started.take() {
                let _ = started.send(());
            }
            let (lock, ready) = &*gate;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = ready.wait(released).unwrap();
            }
            Ok(())
        },
    )?;
    let service = control.create_service_callback::<AddTwoInts::Service, _>(
        "/parallel_control/sum",
        QosProfile::services_default(),
        |request, _| {
            Ok(AddTwoInts::Response {
                sum: request.a + request.b,
            })
        },
    )?;
    let client = application.create_client::<AddTwoInts::Service>(
        "/parallel_control/sum",
        QosProfile::services_default(),
    )?;
    let mut executor = ParallelExecutor::new(context, 2)?;
    let perception_id = executor.add_node_to_worker(perception, 0)?;
    let control_id = executor.add_node_to_worker(control, 1)?;
    let application_id = executor.add_node_to_worker(application, 1)?;
    let task = executor.spawn()?;
    assert_eq!(task.worker_count(), 2);
    let requests = tokio::time::timeout(Duration::from_secs(5), async {
        start.await?;
        client.wait_for_service()?.await?;
        for a in 0..32 {
            let response = client.request(&AddTwoInts::Request { a, b: 11 })?.await?;
            assert_eq!(response.sum, a + 11);
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    })
    .await;
    // Recover all Nodes before propagating a timeout/application error.
    drop(release);
    let mut executor = task.stop().await?;
    requests??;
    executor.reset_cancel();
    let task = executor.spawn()?;
    let reused = tokio::time::timeout(Duration::from_secs(3), async {
        client.request(&AddTwoInts::Request { a: 20, b: 22 })?.await
    })
    .await;
    let mut executor = task.stop().await?;
    assert_eq!(reused??.sum, 42);
    executor
        .node_mut(perception_id)?
        .destroy_callback_timer(slow);
    executor
        .node_mut(control_id)?
        .destroy_callback_service(service);
    drop(executor.remove_node(perception_id)?);
    drop(executor.remove_node(control_id)?);
    drop(executor.remove_node(application_id)?);
    assert!(executor.is_empty());
    println!("validated 32 control RPCs while another worker was blocked, then stopped and reused all three Nodes");
    Ok(())
}
