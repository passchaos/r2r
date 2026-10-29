use super::super::*;
use futures::channel::oneshot;

mod errors;
mod ownership;
mod workflows;
use std::sync::{Arc, Condvar, Mutex as StdMutex};

struct Release(Arc<(StdMutex<bool>, Condvar)>);
impl Drop for Release {
    fn drop(&mut self) {
        let (lock, ready) = &*self.0;
        *lock.lock().unwrap() = true;
        ready.notify_all();
    }
}

async fn callback_groups(same_worker: bool) {
    use crate::std_msgs::msg::UInt64;
    let context = Context::create().unwrap();
    let mut slow = Node::create(context.clone(), "parallel_node_slow", "").unwrap();
    let mut healthy = Node::create(context.clone(), "parallel_node_healthy", "").unwrap();
    let blocked = Arc::new((StdMutex::new(false), Condvar::new()));
    let unblock = Release(blocked.clone());
    let (started, start) = oneshot::channel();
    let mut started = Some(started);
    let _slow = slow
        .create_wall_timer_callback_with_options(
            Duration::ZERO,
            CallbackTimerOptions::default().oneshot(true),
            move || {
                if let Some(started) = started.take() {
                    let _ = started.send(std::thread::current().id());
                }
                let (lock, ready) = &*blocked;
                let mut released = lock.lock().unwrap();
                while !*released {
                    released = ready.wait(released).unwrap();
                }
                Ok(())
            },
        )
        .unwrap();
    let (progress, seen) = oneshot::channel();
    let mut progress = Some(progress);
    let topic = format!("/parallel_progress_{}", uuid::Uuid::new_v4().simple());
    let publisher = healthy
        .create_publisher::<UInt64>(&topic, QosProfile::default())
        .unwrap();
    let _healthy = healthy
        .subscribe_native_callback::<UInt64, _>(&topic, QosProfile::default(), move |_, _| {
            if let Some(progress) = progress.take() {
                let _ = progress.send(std::thread::current().id());
            }
            Ok(())
        })
        .unwrap();
    // Complete discovery before the blocking callback can occupy either group.
    let matched = publisher.wait_for_subscribers().unwrap();
    healthy
        .spin_until_future_complete_with_options(
            matched,
            Duration::from_secs(3),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    let mut executor = ParallelExecutor::new(context, 2).unwrap();
    executor.add_node_to_worker(slow, 0).unwrap();
    executor
        .add_node_to_worker(healthy, usize::from(!same_worker))
        .unwrap();
    let task = executor.spawn().unwrap();
    let slow_thread = tokio::time::timeout(Duration::from_secs(3), start)
        .await
        .unwrap()
        .unwrap();
    // Only now make the independent callback ready, while slow is definitely blocked.
    publisher.publish(&UInt64 { data: 7 }).unwrap();
    let mut seen = seen;
    let progressed = tokio::time::timeout(Duration::from_millis(200), &mut seen).await;
    // Release and await ownership before asserting, including the serial baseline's failure.
    drop(unblock);
    let healthy_thread = if same_worker {
        assert!(progressed.is_err(), "same-group dispatch escaped serial ownership");
        tokio::time::timeout(Duration::from_secs(3), seen)
            .await
            .unwrap()
            .unwrap()
    } else {
        progressed
            .expect("long callback stalled the independent node")
            .unwrap()
    };
    let _executor = task.stop().await.unwrap();
    assert_eq!(slow_thread == healthy_thread, same_worker);
}

#[tokio::test(flavor = "current_thread")]
async fn parallel_node_executor_long_callback_does_not_stall_an_independent_node() {
    callback_groups(false).await;
}

#[tokio::test(flavor = "current_thread")]
async fn parallel_node_executor_nodes_in_one_worker_retain_serial_native_dispatch() {
    callback_groups(true).await;
}
