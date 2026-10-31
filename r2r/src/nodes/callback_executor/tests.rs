use super::*;
use futures::task::{noop_waker, ArcWake};
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(not(feature = "doc-only"))]
mod lifecycle;
#[cfg(not(feature = "doc-only"))]
mod native;
mod reentrant_input;
#[cfg(not(feature = "doc-only"))]
mod reentrant_lifecycle;
#[cfg(not(feature = "doc-only"))]
mod reentrant_native;
mod reentrant_scheduling;
#[cfg(not(feature = "doc-only"))]
mod reentrant_service;
#[cfg(not(feature = "doc-only"))]
mod reentrant_task;
#[cfg(not(feature = "doc-only"))]
mod reentrant_timer;
mod scheduling;
#[cfg(not(feature = "doc-only"))]
mod service;
#[cfg(not(feature = "doc-only"))]
mod task;
#[cfg(all(not(feature = "doc-only"), r2r__example_interfaces__action__Fibonacci))]
mod task_action;
#[cfg(not(feature = "doc-only"))]
mod timer;

#[cfg(not(feature = "doc-only"))]
fn poll_reentrant(
    group: &mut ReentrantCallbackGroup, context: &mut TaskContext<'_>,
) -> Poll<Result<()>> {
    let mut pending = false;
    for lane in &mut group.lanes {
        loop {
            match Pin::new(&mut *lane).poll_next(context) {
                Poll::Pending => {
                    pending = true;
                    break;
                }
                Poll::Ready(None) => break,
                Poll::Ready(Some(Ok(()))) => {}
                Poll::Ready(Some(Err(error))) => return Poll::Ready(Err(error)),
            }
        }
    }
    if pending {
        Poll::Pending
    } else {
        Poll::Ready(Ok(()))
    }
}

#[test]
fn callback_group_input_is_bounded_and_preserves_rejected_payload_ownership() {
    let mut group = CallbackGroup::new();
    let values = Arc::new(Mutex::new(Vec::new()));
    let seen = values.clone();
    let input = group
        .input(2, move |value: Vec<u64>| {
            seen.lock().push(value);
            Ok(())
        })
        .unwrap();
    assert_eq!(group.len(), 1);
    assert!(input.push(vec![1, 2]).is_ok());
    assert!(input.push(vec![3]).is_ok());
    assert!(!input.is_ready());
    let rejected = vec![5, 8, 13];
    let pointer = rejected.as_ptr();
    let returned = input.push(rejected).unwrap_err();
    assert_eq!(returned, [5, 8, 13]);
    assert_eq!(returned.as_ptr(), pointer);
    let wake = noop_waker();
    let mut context = TaskContext::from_waker(&wake);
    assert!(group.poll_callbacks(&mut context).is_pending());
    assert_eq!(&*values.lock(), &[vec![1, 2]]);
    assert!(group.poll_callbacks(&mut context).is_pending());
    assert_eq!(&*values.lock(), &[vec![1, 2], vec![3]]);
    assert!(input.is_ready());
    assert!(input.push(returned).is_ok());
    assert!(group.poll_callbacks(&mut context).is_pending());
    assert_eq!(values.lock().last().unwrap(), &[5, 8, 13]);
    input.close();
    assert!(matches!(group.poll_callbacks(&mut context), Poll::Ready(Ok(()))));
}

struct WakeCount(AtomicUsize);
impl ArcWake for WakeCount {
    fn wake_by_ref(value: &Arc<Self>) {
        value.0.fetch_add(1, Ordering::Release);
    }
}

#[test]
fn callback_group_capacity_transition_and_retirement_wake_the_current_native_owner() {
    let mut group = CallbackGroup::new();
    let input = group.input(1, |_: usize| Ok(())).unwrap();
    let old = Arc::new(WakeCount(AtomicUsize::new(0)));
    let current = Arc::new(WakeCount(AtomicUsize::new(0)));
    input.set_native_wake(Some(futures::task::waker(old.clone())));
    input.set_native_wake(Some(futures::task::waker(current.clone())));
    input.push(7).unwrap();
    assert!(!input.is_ready());
    let wake = noop_waker();
    let mut context = TaskContext::from_waker(&wake);
    assert!(group.poll_callbacks(&mut context).is_pending());
    assert_eq!(old.0.load(Ordering::Acquire), 0);
    assert_eq!(current.0.load(Ordering::Acquire), 1);
    input.push(11).unwrap();
    assert!(group.poll_callbacks(&mut context).is_pending());
    assert_eq!(current.0.load(Ordering::Acquire), 1);
    drop(group);
    assert!(input.is_closed());
    assert_eq!(current.0.load(Ordering::Acquire), 2);
}

#[test]
fn callback_group_source_close_discards_queued_owned_inputs_without_invoking_user_work() {
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let mut group = CallbackGroup::new();
    let input = group
        .input(1, move |_: Vec<u64>| {
            seen.fetch_add(1, Ordering::Release);
            Ok(())
        })
        .unwrap();
    input.push(vec![1, 2, 3]).unwrap();
    input.close();
    let wake = noop_waker();
    assert!(matches!(
        group.poll_callbacks(&mut TaskContext::from_waker(&wake)),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(calls.load(Ordering::Acquire), 0);
    assert!(input.is_closed());
    assert!(group.is_empty());
}

#[test]
fn callback_group_invalid_capacity_has_no_registration_and_close_rejects_owned_input() {
    let mut group = CallbackGroup::new();
    for capacity in [0, usize::MAX] {
        assert!(matches!(
            group.input(capacity, |_: usize| Ok(())),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
    }
    assert!(group.is_empty());
    let input = group.input(1, |_: Vec<u64>| Ok(())).unwrap();
    input.close();
    let value = vec![3, 5, 8];
    let address = value.as_ptr();
    let value = input.push(value).unwrap_err();
    assert_eq!(value.as_ptr(), address);
    assert_eq!(value, [3, 5, 8]);
}

#[test]
fn callback_group_queued_payload_retirement_allows_reentry_into_native_wake_binding() {
    struct Payload {
        producer: std::sync::Weak<Producer<Payload>>,
        retired: Arc<AtomicUsize>,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            let producer = self.producer.upgrade().unwrap();
            assert!(producer.is_closed());
            producer.set_native_wake(None);
            self.retired.fetch_add(1, Ordering::Release);
        }
    }
    let mut group = CallbackGroup::new();
    let input = Arc::new(
        group
            .input(2, |_: Payload| panic!("closed input invoked callback"))
            .unwrap(),
    );
    let retired = Arc::new(AtomicUsize::new(0));
    for _ in 0..2 {
        assert!(input
            .push(Payload {
                producer: Arc::downgrade(&input),
                retired: retired.clone()
            })
            .is_ok());
    }
    input.close();
    let wake = noop_waker();
    assert!(matches!(
        group.poll_callbacks(&mut TaskContext::from_waker(&wake)),
        Poll::Ready(Ok(()))
    ));
    assert_eq!(retired.load(Ordering::Acquire), 2);
}

#[test]
fn callback_group_receiver_close_retires_all_inflight_owned_inputs_before_producer_drop() {
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Release);
        }
    }
    let mut group = CallbackGroup::new();
    let input = Arc::new(
        group
            .input(32, |_: Payload| panic!("unpolled callback ran"))
            .unwrap(),
    );
    let retired = Arc::new(AtomicUsize::new(0));
    let began = Arc::new(std::sync::Barrier::new(2));
    let sender = input.clone();
    let count = retired.clone();
    let barrier = began.clone();
    let sender = std::thread::spawn(move || {
        barrier.wait();
        for _ in 0..20000 {
            if let Err(payload) = sender.push(Payload(count.clone())) {
                drop(payload);
            }
            std::thread::yield_now();
        }
    });
    began.wait();
    std::thread::yield_now();
    drop(group);
    sender.join().unwrap();
    assert!(input.is_closed());
    // Keep the native producer alive. A close/publication race must not leave
    // payloads awaiting its later native endpoint/context destruction.
    assert_eq!(retired.load(Ordering::Acquire), 20000);
    drop(input);
}
