use super::scheduling::Release;
use super::*;
use crate::std_msgs::msg::UInt64;
use std::sync::{mpsc, Condvar, Mutex as StdMutex};

#[tokio::test(flavor = "current_thread")]
async fn reentrant_native_subscription_runs_two_owned_inputs_concurrently_in_one_group() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "reentrant_native_subscription", "").unwrap();
    let mut group = ReentrantCallbackGroup::new(2).unwrap();
    let topic = format!("/reentrant_inputs_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<UInt64>(&topic, QosProfile::default())
        .unwrap();
    let gate = Arc::new((StdMutex::new(false), Condvar::new()));
    let release = Release(gate.clone());
    let (started, start) = mpsc::channel();
    let retired = Arc::new(AtomicUsize::new(0));
    let seen = retired.clone();
    let token = node
        .subscribe_native_reentrant_callback::<UInt64, _>(
            &mut group,
            &topic,
            QosProfile::default(),
            move |message, _| {
                started
                    .send((message.data, std::thread::current().id()))
                    .unwrap();
                let (lock, ready) = &*gate;
                let mut released = lock.lock().unwrap();
                while !*released {
                    released = ready.wait(released).unwrap();
                }
                seen.fetch_add(1, Ordering::Release);
                Ok(())
            },
        )
        .unwrap();
    let matched = publisher.wait_for_subscribers().unwrap();
    node.spin_until_future_complete_with_options(
        matched,
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let mut executor = CallbackExecutor::new(context, 2).unwrap();
    let group_id = executor.add_reentrant_group(group).unwrap();
    executor.add_node(node).unwrap();
    let task = executor.spawn().unwrap();
    assert_eq!(task.worker_count(), 2);
    publisher.publish(&UInt64 { data: 11 }).unwrap();
    publisher.publish(&UInt64 { data: 13 }).unwrap();
    let mut invocations = Vec::new();
    let concurrent = tokio::time::timeout(Duration::from_secs(3), async {
        while invocations.len() != 2 {
            if let Ok(value) = start.try_recv() {
                invocations.push(value);
            }
            tokio::task::yield_now().await;
        }
    })
    .await;
    drop(release);
    let mut executor = task.stop().await.unwrap();
    concurrent.expect("one reentrant handler blocked the next input from its own source");
    assert_ne!(invocations[0].1, invocations[1].1);
    let mut values: Vec<_> = invocations.into_iter().map(|(value, _)| value).collect();
    values.sort_unstable();
    assert_eq!(values, [11, 13]);
    assert_eq!(retired.load(Ordering::Acquire), 2);
    assert_eq!(
        executor
            .reentrant_group_mut(group_id)
            .unwrap()
            .concurrency(),
        2
    );
    assert!(executor.group_mut(group_id).is_err());
    drop(token);
    drop(executor);
}
