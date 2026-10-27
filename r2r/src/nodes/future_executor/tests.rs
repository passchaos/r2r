use super::*;
use futures::{channel::oneshot, future, StreamExt};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    thread,
};

fn node(name: &str) -> Node {
    Node::create(Context::create().unwrap(), name, "").unwrap()
}
fn install_guard(node: &mut Node) -> Arc<FutureGuard> {
    let guard = FutureGuard::new(&node.context).unwrap();
    node.future_wake_guard = Some(guard.clone());
    guard
}
fn wait_until_native_wait(guard: &FutureGuard) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !guard.waiting.load(Ordering::Acquire) {
        assert!(Instant::now() < deadline, "node never entered native wait");
        thread::yield_now();
    }
}

#[test]
fn ready_zero_timeout_and_non_send_values_do_not_create_wake_storage() {
    let mut node = node("future_ready_test");
    let value = Rc::new(42);
    let result = node
        .spin_until_future_complete(future::ready(Ok(value.clone())), Duration::ZERO)
        .unwrap();
    assert!(Rc::ptr_eq(&value, &result));
    assert!(node.future_wake_guard.is_none());
    assert!(matches!(
        node.spin_until_future_complete(future::pending::<Result<()>>(), Duration::ZERO),
        Err(Error::RCL_RET_TIMEOUT)
    ));
    assert!(node.future_wake_guard.is_none());
    assert!(matches!(
        node.spin_until_future_complete(
            future::ready(Err::<(), _>(Error::RCL_RET_INVALID_ARGUMENT)),
            Duration::ZERO
        ),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
}

#[test]
fn options_and_periodic_poll_only_future_preserve_progress() {
    assert_eq!(FutureExecutionOptions::default().poll_interval, Some(Duration::from_millis(10)));
    assert_eq!(FutureExecutionOptions::wake_driven().poll_interval, None);
    let mut node = node("future_periodic_test");
    assert!(matches!(
        node.spin_until_future_complete_with_options(
            future::ready(Ok(())),
            Duration::ZERO,
            FutureExecutionOptions::default().poll_interval(Duration::ZERO)
        ),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    let polls = Rc::new(Cell::new(0));
    let result = node
        .spin_until_future_complete_with_options(
            future::poll_fn(|_| {
                polls.set(polls.get() + 1);
                if polls.get() == 3 {
                    Poll::Ready(Ok(19))
                } else {
                    Poll::Pending
                }
            }),
            Duration::from_secs(1),
            FutureExecutionOptions::default().poll_interval(Duration::from_millis(2)),
        )
        .unwrap();
    assert_eq!(result, 19);
    assert_eq!(polls.get(), 3);
    assert!(!node
        .future_wake_guard
        .as_ref()
        .unwrap()
        .active
        .load(Ordering::Acquire));
    node.try_spin_once(Duration::ZERO).unwrap();
}

#[test]
fn external_future_wake_interrupts_native_wait_without_periodic_polling() {
    let mut node = node("future_external_wake_test");
    let guard = install_guard(&mut node);
    let (sender, receiver) = oneshot::channel();
    let observed = guard.clone();
    let producer = thread::spawn(move || {
        wait_until_native_wait(&observed);
        sender.send(73).unwrap();
    });
    let result = node
        .spin_until_future_complete_with_options(
            async { receiver.await.map_err(|_| Error::RCL_RET_ERROR) },
            Duration::from_secs(3),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    producer.join().unwrap();
    assert_eq!(result, 73);
    assert_eq!(guard.wait_return.load(Ordering::Acquire), RCL_RET_OK as i32);
    assert_eq!(guard.triggers.load(Ordering::Acquire), 1);
}

#[test]
fn wakes_during_poll_are_not_lost_and_still_drain_ros() {
    let mut node = node("future_poll_wake_test");
    let polls = Cell::new(0);
    let result = node
        .spin_until_future_complete_with_options(
            future::poll_fn(|context| {
                polls.set(polls.get() + 1);
                match polls.get() {
                    1 => Poll::Pending,
                    2 => {
                        context.waker().wake_by_ref();
                        Poll::Pending
                    }
                    _ => Poll::Ready(Ok(31)),
                }
            }),
            Duration::from_secs(1),
            FutureExecutionOptions::wake_driven(),
        )
        .unwrap();
    assert_eq!(result, 31);
    let guard = node.future_wake_guard.as_ref().unwrap();
    assert_eq!(guard.triggers.load(Ordering::Acquire), 0);
    assert_eq!(guard.wait_return.load(Ordering::Acquire), RCL_RET_TIMEOUT as i32);
}

#[test]
fn concurrent_notifications_coalesce_into_one_native_trigger() {
    let node = node("future_coalesce_test");
    let guard = FutureGuard::new(&node.context).unwrap();
    let wake = Arc::new(FutureWake {
        guard: Arc::downgrade(&guard),
        flags: AtomicU8::new(ARMED | WAITING),
        error: AtomicI32::new(0),
    });
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let waker = Waker::from(wake.clone());
            thread::spawn(move || {
                for _ in 0..1000 {
                    waker.wake_by_ref();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    assert_eq!(guard.triggers.load(Ordering::Acquire), 1);
    assert_eq!(wake.error.load(Ordering::Acquire), 0);
}

#[test]
fn completed_wakers_disarm_and_do_not_retain_native_resources() {
    let mut node = node("future_stale_wake_test");
    let polls = Cell::new(0);
    let saved = RefCell::new(None);
    node.spin_until_future_complete_with_options(
        future::poll_fn(|context| {
            polls.set(polls.get() + 1);
            if polls.get() == 1 {
                Poll::Pending
            } else {
                saved.replace(Some(context.waker().clone()));
                Poll::Ready(Ok(()))
            }
        }),
        Duration::from_secs(1),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let weak = Arc::downgrade(node.future_wake_guard.as_ref().unwrap());
    let stale = saved.borrow_mut().take().unwrap();
    for _ in 0..16 {
        stale.wake_by_ref();
    }
    assert_eq!(
        node.future_wake_guard
            .as_ref()
            .unwrap()
            .triggers
            .load(Ordering::Acquire),
        0
    );
    drop(node);
    assert!(weak.upgrade().is_none());
    stale.wake_by_ref();
}

#[test]
fn panic_disarms_wakers_and_preserves_node_reuse() {
    let mut node = node("future_panic_test");
    let polls = Cell::new(0);
    let saved = RefCell::new(None);
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        node.spin_until_future_complete_with_options(
            future::poll_fn(|context| -> Poll<Result<()>> {
                polls.set(polls.get() + 1);
                if polls.get() == 1 {
                    Poll::Pending
                } else {
                    saved.replace(Some(context.waker().clone()));
                    panic!("future panic oracle")
                }
            }),
            Duration::from_secs(1),
            FutureExecutionOptions::wake_driven(),
        )
    }));
    assert!(panicked.is_err());
    assert!(!node
        .future_wake_guard
        .as_ref()
        .unwrap()
        .active
        .load(Ordering::Acquire));
    saved.borrow().as_ref().unwrap().wake_by_ref();
    assert_eq!(
        node.spin_until_future_complete(future::ready(Ok(43)), Duration::ZERO)
            .unwrap(),
        43
    );
    node.try_spin_once(Duration::ZERO).unwrap();
}

#[test]
fn self_woken_future_cannot_starve_a_service_request() {
    use crate::example_interfaces::srv::AddTwoInts;
    let mut node = node("future_ros_fairness_test");
    let name = format!("/future_fairness_{}", uuid::Uuid::new_v4().simple());
    let mut service = node
        .create_service::<AddTwoInts::Service>(&name, crate::QosProfile::default())
        .unwrap();
    let client = node
        .create_client::<AddTwoInts::Service>(&name, crate::QosProfile::default())
        .unwrap();
    node.spin_until_future_complete_with_options(
        client.wait_for_service().unwrap(),
        Duration::from_secs(2),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let response = client.request(&AddTwoInts::Request { a: 3, b: 6 }).unwrap();
    let done = Cell::new(false);
    let work = async {
        let server = async {
            let request = service.next().await.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
            let sum = request.message.a + request.message.b;
            request.respond(AddTwoInts::Response { sum })?;
            Ok::<_, Error>(())
        };
        let consumer = async {
            let response = response.await?;
            assert_eq!(response.sum, 9);
            done.set(true);
            Ok::<_, Error>(())
        };
        let busy = future::poll_fn(|context| {
            if done.get() {
                Poll::Ready(Ok::<_, Error>(()))
            } else {
                context.waker().wake_by_ref();
                Poll::Pending
            }
        });
        futures::try_join!(server, consumer, busy)?;
        Ok(())
    };
    node.spin_until_future_complete_with_options(
        work,
        Duration::from_secs(2),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
}

#[test]
fn service_appearance_wakes_availability_without_periodic_polling() {
    use crate::example_interfaces::srv::AddTwoInts;
    let mut node = node("future_availability_test");
    let name = format!("/future_available_{}", uuid::Uuid::new_v4().simple());
    let client = node
        .create_client::<AddTwoInts::Service>(&name, crate::QosProfile::default())
        .unwrap();
    let context = node.context.clone();
    let guard = install_guard(&mut node);
    let (stop, stopped) = std::sync::mpsc::channel();
    let provider = thread::spawn(move || {
        wait_until_native_wait(&guard);
        let mut server = Node::create(context, "future_availability_provider", "").unwrap();
        let _service = server
            .create_service::<AddTwoInts::Service>(&name, crate::QosProfile::default())
            .unwrap();
        let _ = stopped.recv_timeout(Duration::from_secs(5));
    });
    let result = node.spin_until_future_complete_with_options(
        client.wait_for_service().unwrap(),
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    );
    let _ = stop.send(());
    provider.join().unwrap();
    result.unwrap();
}

#[test]
fn shutdown_interrupts_a_notification_driven_wait() {
    const CHILD: &str = "R2R_FUTURE_WAKE_SHUTDOWN_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "nodes::future_executor::tests::shutdown_interrupts_a_notification_driven_wait",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let mut node = node("future_shutdown_test");
    let context = node.context.clone();
    let guard = install_guard(&mut node);
    let observed = guard.clone();
    let shutdown = thread::spawn(move || {
        wait_until_native_wait(&observed);
        context.shutdown().unwrap();
    });
    let result = node.spin_until_future_complete_with_options(
        future::pending::<Result<()>>(),
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    );
    shutdown.join().unwrap();
    assert!(matches!(result, Err(Error::RCL_RET_NOT_INIT)));
    assert!(!guard.active.load(Ordering::Acquire));
}
