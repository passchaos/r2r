//! One Node keeps control RPC and native forwarding responsive during slow work.
use futures::{channel::oneshot, StreamExt};
use r2r::{
    example_interfaces::srv::AddTwoInts, std_msgs::msg::UInt64, CallbackExecutor, CallbackGroup,
    CallbackTimerOptions, Context, Node, QosProfile,
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
    let mut node = Node::create(context.clone(), "callback_group_application", "")?;
    let mut perception = CallbackGroup::new();
    let mut control = CallbackGroup::new();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let release = Release(gate.clone());
    let (started, start) = oneshot::channel();
    let mut started = Some(started);
    let slow = node.create_wall_timer_callback_in_group_with_options(
        &mut perception,
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
    let input = node.create_publisher::<UInt64>("/grouped_input", QosProfile::default())?;
    let output = node.create_publisher::<UInt64>("/grouped_output", QosProfile::default())?;
    let forward = output.clone();
    let subscription = node.subscribe_native_callback_in_group::<UInt64, _>(
        &mut control,
        "/grouped_input",
        QosProfile::default(),
        move |message, _| forward.publish_native_ref(&message),
    )?;
    let mut received = node.subscribe::<UInt64>("/grouped_output", QosProfile::default())?;
    let service = node.create_service_native_callback_in_group::<AddTwoInts::Service, _>(
        &mut control,
        "/grouped_sum",
        QosProfile::services_default(),
        |request, response, _| {
            response.sum = request.a + request.b;
            Ok(())
        },
    )?;
    let client =
        node.create_client::<AddTwoInts::Service>("/grouped_sum", QosProfile::services_default())?;
    let mut executor = CallbackExecutor::new(context, 2)?;
    let node_id = executor.add_node(node)?;
    let perception_id = executor.add_group(perception)?;
    executor.add_group(control)?;
    let task = executor.spawn()?;
    assert_eq!(task.worker_count(), 2);
    let application = tokio::time::timeout(Duration::from_secs(5), async {
        start.await?;
        futures::try_join!(
            input.wait_for_subscribers()?,
            output.wait_for_subscribers()?,
            client.wait_for_service()?
        )?;
        for value in 0..32 {
            input.publish(&UInt64 { data: value })?;
            let response = client.request(&AddTwoInts::Request {
                a: value as i64,
                b: 11,
            })?;
            let message = async {
                received
                    .next()
                    .await
                    .ok_or(r2r::Error::RCL_RET_SUBSCRIPTION_INVALID)
            };
            let (response, message) = futures::try_join!(response, message)?;
            assert_eq!(response.sum, value as i64 + 11);
            assert_eq!(message.data, value);
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    })
    .await;
    drop(release);
    let mut executor = task.stop().await?;
    application??;
    assert!(executor.group_mut(perception_id)?.len() == 1);
    executor.reset_cancel();
    let task = executor.spawn()?;
    let reused = tokio::time::timeout(Duration::from_secs(3), async {
        client.request(&AddTwoInts::Request { a: 20, b: 22 })?.await
    })
    .await;
    let mut executor = task.stop().await?;
    assert_eq!(reused??.sum, 42);
    executor.node_mut(node_id)?.destroy_grouped_service(service);
    drop(slow);
    drop(subscription);
    drop(executor.remove_node(node_id)?);
    println!("validated 32 native forwards and control RPCs on one Node while another group was blocked, then stopped and reused the two-worker executor");
    Ok(())
}
