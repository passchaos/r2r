//! One native service accepts another request while its first shared handler blocks.
use r2r::{
    example_interfaces::srv::AddTwoInts, CallbackExecutor, Context, Node, QosProfile,
    ReentrantCallbackGroup,
};
use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

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
    let mut node = Node::create(context.clone(), "reentrant_callbacks", "")?;
    let mut group = ReentrantCallbackGroup::new(2)?;
    let name = format!("/reentrant_example_{}", uuid::Uuid::new_v4().simple());
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let release = Release(gate.clone());
    let (started, entered) = futures::channel::oneshot::channel();
    let started = Mutex::new(Some(started));
    let service = node.create_service_reentrant_callback::<AddTwoInts::Service, _>(
        &mut group,
        &name,
        QosProfile::services_default(),
        move |request, _| {
            if request.a == 11 {
                if let Some(started) = started.lock().unwrap().take() {
                    let _ = started.send(());
                }
                let (lock, ready) = &*gate;
                let mut released = lock.lock().unwrap();
                while !*released {
                    released = ready.wait(released).unwrap();
                }
            }
            Ok(AddTwoInts::Response {
                sum: request.a + request.b,
            })
        },
    )?;
    let client =
        node.create_client::<AddTwoInts::Service>(&name, QosProfile::services_default())?;
    // Two invocation lanes plus room for the single owned native wait job.
    let mut executor = CallbackExecutor::new(context, 3)?;
    let node_id = executor.add_node(node)?;
    let group_id = executor.add_reentrant_group(group)?;
    let task = executor.spawn()?;
    let application = tokio::time::timeout(Duration::from_secs(5), async {
        client.wait_for_service()?.await?;
        let first = client.request(&AddTwoInts::Request { a: 11, b: 31 })?;
        entered
            .await
            .expect("service handler was removed before entry");
        let second = client.request(&AddTwoInts::Request { a: 13, b: 41 })?;
        assert_eq!(second.await?.sum, 54);
        drop(release);
        assert_eq!(first.await?.sum, 42);
        Ok::<_, r2r::Error>(())
    })
    .await;
    let mut executor = task.stop().await?;
    application??;
    executor.reset_cancel();
    let (node, group) = executor.node_and_reentrant_group_mut(node_id, group_id)?;
    assert_eq!(group.concurrency(), 2);
    let timer = node.create_wall_timer_reentrant_callback_with_options(
        group,
        Duration::from_millis(1),
        r2r::CallbackTimerOptions::default().oneshot(true),
        || Ok(()),
    )?;
    let task = executor.spawn()?;
    let response = tokio::time::timeout(
        Duration::from_secs(3),
        client.request(&AddTwoInts::Request { a: 20, b: 22 })?,
    )
    .await;
    let executor = task.stop().await?;
    assert_eq!(response??.sum, 42);
    println!("verified same-source concurrent RPC responses and stopped group reuse");
    drop((timer, service, executor));
    Ok(())
}
