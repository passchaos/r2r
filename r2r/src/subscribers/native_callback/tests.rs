use super::*;
use futures::{FutureExt, StreamExt};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Instant,
};

use crate::std_msgs::msg::UInt64MultiArray;

static NATIVE_TEST_LOCK: Mutex<()> = Mutex::new(());
static NATIVE_CREATED: AtomicUsize = AtomicUsize::new(0);
static NATIVE_DESTROYED: AtomicUsize = AtomicUsize::new(0);
static FAIL_NEXT_CREATE: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct TrackedNative(UInt64MultiArray);

// The test wrapper delegates to the exact generated native type and its allocator. It rejects
// any owned Rust decode, so the receive-path assertion covers more than the forwarded values.
unsafe impl WrappedTypesupport for TrackedNative {
    type CStruct = <UInt64MultiArray as WrappedTypesupport>::CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t {
        UInt64MultiArray::get_ts()
    }

    fn create_msg() -> *mut Self::CStruct {
        if FAIL_NEXT_CREATE.swap(false, Ordering::Relaxed) {
            return std::ptr::null_mut();
        }
        let message = UInt64MultiArray::create_msg();
        if !message.is_null() {
            NATIVE_CREATED.fetch_add(1, Ordering::Relaxed);
        }
        message
    }

    fn destroy_msg(message: *mut Self::CStruct) {
        NATIVE_DESTROYED.fetch_add(1, Ordering::Relaxed);
        UInt64MultiArray::destroy_msg(message);
    }

    fn from_native(_message: &Self::CStruct) -> Self {
        panic!("scoped native callback constructed an owned Rust payload");
    }

    fn copy_to_native(&self, message: &mut Self::CStruct) {
        self.0.copy_to_native(message);
    }

    fn try_copy_to_native(&self, message: &mut Self::CStruct) -> Result<()> {
        self.0.try_copy_to_native(message)
    }
}

fn spin_until(node: &mut Node, mut complete: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !complete() {
        assert!(Instant::now() < deadline, "native callback oracle timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
    }
}

fn assert_all_native_storage_retired() {
    assert_eq!(
        NATIVE_CREATED.load(Ordering::Relaxed),
        NATIVE_DESTROYED.load(Ordering::Relaxed),
        "each take must retire its independent native container"
    );
}

#[test]
fn native_callback_options_reject_an_empty_dispatch_batch() {
    assert!(matches!(
        NativeCallbackSubscriptionOptions::default()
            .max_messages_per_spin(0)
            .validate(),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
}

#[test]
fn scoped_native_callback_forwards_without_an_owned_rust_message() {
    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "native_callback_forward", "").unwrap();
    let input = format!("/native_callback_input_{}", uuid::Uuid::new_v4().simple());
    let output = format!("/native_callback_output_{}", uuid::Uuid::new_v4().simple());
    let qos = QosProfile::default().keep_last(16);
    let input_publisher = node
        .create_publisher::<UInt64MultiArray>(&input, qos.clone())
        .unwrap();
    let output_publisher = node
        .create_publisher::<UInt64MultiArray>(&output, qos.clone())
        .unwrap();
    let callback_count = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&callback_count);
    let publisher = output_publisher.clone();
    let callback = node
        .subscribe_native_callback::<UInt64MultiArray, _>(
            &input,
            qos.clone(),
            move |message, _info| {
                count.fetch_add(1, Ordering::Relaxed);
                publisher.publish_native_ref(&message)
            },
        )
        .unwrap();
    let mut output_subscription = node.subscribe::<UInt64MultiArray>(&output, qos).unwrap();
    for _ in 0..100 {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if input_publisher.get_subscription_count().unwrap() == 1
            && output_publisher.get_subscription_count().unwrap() == 1
        {
            break;
        }
    }
    assert_eq!(callback.topic_name(&node).unwrap(), input);
    assert_eq!(callback.get_publisher_count(&node).unwrap(), 1);
    input_publisher
        .publish(&UInt64MultiArray {
            data: vec![7, 11, 13],
            ..Default::default()
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let received = loop {
        assert!(Instant::now() < deadline, "native forwarding oracle timed out");
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(message) = output_subscription.next().now_or_never().flatten() {
            break message;
        }
    };
    assert_eq!(received.data, [7, 11, 13]);
    assert_eq!(callback_count.load(Ordering::Relaxed), 1);
    node.destroy_native_callback_subscription(callback);
    for _ in 0..100 {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if input_publisher.get_subscription_count().unwrap() == 0 {
            break;
        }
    }
    assert_eq!(input_publisher.get_subscription_count().unwrap(), 0);
}

#[test]
fn native_callback_retires_dynamic_storage_after_success_error_and_panic() {
    let _serial = NATIVE_TEST_LOCK.lock();
    let mut node =
        Node::create(crate::Context::create().unwrap(), "native_retirement", "").unwrap();
    let topic = format!("/native_retirement_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<UInt64MultiArray>(&topic, QosProfile::default())
        .unwrap();
    let delivered = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&delivered);
    let callback = node
        .subscribe_native_callback::<TrackedNative, _>(
            &topic,
            QosProfile::default(),
            move |message, _| {
                let index = count.fetch_add(1, Ordering::Relaxed);
                assert_eq!(message.data.size, [4096, 7, 0, 128][index]);
                if message.data.size != 0 {
                    // RCL produced this valid native sequence; inspect it while the view lives.
                    let values =
                        unsafe { std::slice::from_raw_parts(message.data.data, message.data.size) };
                    assert!(values.iter().all(|&value| value == index as u64));
                }
                assert_eq!(
                    NATIVE_CREATED.load(Ordering::Relaxed),
                    NATIVE_DESTROYED.load(Ordering::Relaxed) + 1
                );
                match index {
                    1 => Err(Error::RCL_RET_INVALID_ARGUMENT),
                    2 => panic!("intentional native callback unwind"),
                    _ => Ok(()),
                }
            },
        )
        .unwrap();
    spin_until(&mut node, || publisher.get_subscription_count().unwrap() == 1);
    for (index, size) in [4096, 7, 0, 128].into_iter().enumerate() {
        publisher
            .publish(&UInt64MultiArray {
                data: vec![index as u64; size],
                ..Default::default()
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            assert!(Instant::now() < deadline, "native retirement oracle timed out");
            let outcome =
                catch_unwind(AssertUnwindSafe(|| node.try_spin_once(Duration::from_millis(10))));
            assert_all_native_storage_retired();
            if delivered.load(Ordering::Relaxed) == index + 1 {
                match index {
                    1 => assert!(matches!(outcome, Ok(Err(Error::RCL_RET_INVALID_ARGUMENT)))),
                    2 => assert!(outcome.is_err()),
                    _ => outcome.unwrap().unwrap(),
                }
                break;
            }
            outcome.unwrap().unwrap();
        }
        assert_eq!(callback.get_publisher_count(&node).unwrap(), 1);
    }
    assert_eq!(delivered.load(Ordering::Relaxed), 4);
}

#[test]
fn native_callback_allocation_failure_preserves_the_unread_sample() {
    let _serial = NATIVE_TEST_LOCK.lock();
    let mut node =
        Node::create(crate::Context::create().unwrap(), "native_alloc_failure", "").unwrap();
    let topic = format!("/native_alloc_failure_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<UInt64MultiArray>(&topic, QosProfile::default())
        .unwrap();
    let delivered = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&delivered);
    let callback = node
        .subscribe_native_callback::<TrackedNative, _>(
            &topic,
            QosProfile::default(),
            move |message, _| {
                assert_eq!(message.data.size, 128);
                let values =
                    unsafe { std::slice::from_raw_parts(message.data.data, message.data.size) };
                assert!(values.iter().all(|&value| value == 42));
                count.fetch_add(1, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap();
    spin_until(&mut node, || publisher.get_subscription_count().unwrap() == 1);
    assert_eq!(
        node.subscriber_mut(callback.id)
            .unwrap()
            .handle_incoming()
            .unwrap(),
        SubscriptionHandleResult::Idle
    );
    assert_all_native_storage_retired();
    publisher
        .publish(&UInt64MultiArray {
            data: vec![42; 128],
            ..Default::default()
        })
        .unwrap();
    FAIL_NEXT_CREATE.store(true, Ordering::Relaxed);
    assert!(matches!(
        node.subscriber_mut(callback.id).unwrap().handle_incoming(),
        Err(Error::RCL_RET_BAD_ALLOC)
    ));
    assert_eq!(delivered.load(Ordering::Relaxed), 0);
    assert_all_native_storage_retired();
    spin_until(&mut node, || delivered.load(Ordering::Relaxed) == 1);
    assert_all_native_storage_retired();
}

#[test]
fn native_callback_burst_respects_dispatch_bound_and_other_subscription_progress() {
    use crate::std_msgs::msg::UInt64;

    let mut node = Node::create(crate::Context::create().unwrap(), "native_burst", "").unwrap();
    let topic = format!("/native_burst_{}", uuid::Uuid::new_v4().simple());
    let qos = QosProfile::default().keep_last(32);
    let publisher = node
        .create_publisher::<UInt64>(&topic, qos.clone())
        .unwrap();
    let burst = Arc::new(Mutex::new(Vec::new()));
    let values = Arc::clone(&burst);
    let bounded = node
        .subscribe_native_callback_with_options::<UInt64, _>(
            &topic,
            qos.clone(),
            NativeCallbackSubscriptionOptions::default().max_messages_per_spin(2),
            move |message, _| {
                values.lock().push(message.data);
                Ok(())
            },
        )
        .unwrap();
    let mut ordinary = node
        .subscribe_with_options::<UInt64>(
            &topic,
            qos,
            SubscriptionOptions::default().channel_capacity(32),
        )
        .unwrap();
    spin_until(&mut node, || publisher.get_subscription_count().unwrap() == 2);
    for data in 0..7 {
        publisher.publish(&UInt64 { data }).unwrap();
    }
    publisher
        .wait_for_all_acked(Duration::from_secs(3))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut received = Vec::new();
    let mut first_dispatch = true;
    while burst.lock().len() < 7 || received.len() < 7 {
        assert!(Instant::now() < deadline, "native burst oracle timed out");
        let before = burst.lock().len();
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        let after = burst.lock().len();
        assert!(after - before <= 2, "callback exceeded its dispatch budget");
        while let Some(message) = ordinary.next().now_or_never().flatten() {
            received.push(message.data);
        }
        if first_dispatch {
            assert_eq!(after, 2, "queued burst did not use its bounded batch");
            assert_eq!(received.len(), 7, "ordinary subscription was starved by the burst");
            first_dispatch = false;
        }
    }
    assert_eq!(*burst.lock(), (0..7).collect::<Vec<_>>());
    assert_eq!(received, (0..7).collect::<Vec<_>>());
    assert_eq!(bounded.actual_qos(&node).unwrap().depth, 32);
}

#[test]
fn native_callback_drop_releases_capture_and_cancels_only_its_publisher_wait() {
    struct Capture(Arc<AtomicUsize>);
    impl Drop for Capture {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    use crate::std_msgs::msg::UInt64;

    let mut node = Node::create(crate::Context::create().unwrap(), "native_drop", "").unwrap();
    let topic = format!("/native_drop_{}", uuid::Uuid::new_v4().simple());
    let retired = Arc::new(AtomicUsize::new(0));
    let capture = Capture(Arc::clone(&retired));
    let callback = node
        .subscribe_native_callback::<UInt64, _>(&topic, QosProfile::default(), move |_, _| {
            let _keep_alive = &capture;
            Ok(())
        })
        .unwrap();
    let mut publisher_wait = Box::pin(callback.wait_for_publishers(&mut node).unwrap());
    assert!(publisher_wait.as_mut().now_or_never().is_none());
    let sibling = node
        .subscribe_native_callback::<UInt64, _>(&topic, QosProfile::default(), |_, _| Ok(()))
        .unwrap();
    let id = callback.id;
    drop(callback);
    assert_eq!(retired.load(Ordering::Relaxed), 0);
    node.try_spin_once(Duration::ZERO).unwrap();
    assert_eq!(retired.load(Ordering::Relaxed), 1);
    assert!(node.subscriber(id).is_err());
    assert_eq!(sibling.topic_name(&node).unwrap(), topic);
    assert!(matches!(
        publisher_wait.now_or_never(),
        Some(Err(Error::RCL_RET_SUBSCRIPTION_INVALID))
    ));
}

#[test]
fn native_callback_error_returns_from_spin_without_destroying_the_subscription() {
    use crate::std_msgs::msg::UInt64;

    let context = crate::Context::create().unwrap();
    let mut node = Node::create(context, "native_callback_error", "").unwrap();
    let topic = format!("/native_callback_error_{}", uuid::Uuid::new_v4().simple());
    let publisher = node
        .create_publisher::<UInt64>(&topic, QosProfile::default())
        .unwrap();
    let callback_count = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&callback_count);
    let callback = node
        .subscribe_native_callback::<UInt64, _>(
            &topic,
            QosProfile::default(),
            move |_message, _info| {
                if count.fetch_add(1, Ordering::Relaxed) == 0 {
                    Err(Error::RCL_RET_INVALID_ARGUMENT)
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
    for _ in 0..100 {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if publisher.get_subscription_count().unwrap() == 1 {
            break;
        }
    }
    assert_eq!(publisher.get_subscription_count().unwrap(), 1);

    publisher.publish(&UInt64 { data: 1 }).unwrap();
    let mut observed_callback_error = false;
    for _ in 0..100 {
        match node.try_spin_once(Duration::from_millis(10)) {
            Err(Error::RCL_RET_INVALID_ARGUMENT) => {
                observed_callback_error = true;
                break;
            }
            Ok(()) | Err(Error::RCL_RET_TIMEOUT) => {}
            Err(error) => panic!("unexpected spin error: {error:?}"),
        }
    }
    assert!(observed_callback_error);
    assert_eq!(callback_count.load(Ordering::Relaxed), 1);
    assert_eq!(callback.get_publisher_count(&node).unwrap(), 1);

    publisher.publish(&UInt64 { data: 2 }).unwrap();
    for _ in 0..100 {
        match node.try_spin_once(Duration::from_millis(10)) {
            Ok(()) | Err(Error::RCL_RET_TIMEOUT) => {}
            Err(error) => panic!("unexpected spin error: {error:?}"),
        }
        if callback_count.load(Ordering::Relaxed) == 2 {
            break;
        }
    }
    assert_eq!(callback_count.load(Ordering::Relaxed), 2);
    assert_eq!(publisher.get_subscription_count().unwrap(), 1);
}
