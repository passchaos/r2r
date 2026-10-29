use super::*;
use futures::{FutureExt, StreamExt};
use std::sync::atomic::{AtomicUsize, Ordering};

fn snapshot(count: i32) -> QosEvent {
    QosEvent::OfferedDeadlineMissed(OfferedDeadlineMissedStatus {
        total_count: count,
        total_count_change: 1,
    })
}

fn stream() -> (Arc<EventChannel>, QosEventStream) {
    let channel = Arc::new(EventChannel::default());
    let notifications = Arc::new(Mutex::new(Default::default()));
    let stream = QosEventStream::new(uuid::Uuid::new_v4(), channel.clone(), notifications);
    (channel, stream)
}

#[test]
fn qos_event_slot_backpressures_until_consumption_and_rearms_only_live_streams() {
    let (channel, mut stream) = stream();
    assert!(stream.next().now_or_never().is_none());
    assert!(channel.can_take());
    channel.deliver(snapshot(1));
    assert!(!channel.can_take());
    assert!(stream.notifications.lock().is_empty());
    assert!(matches!(stream.next().now_or_never(), Some(Some(Ok(value))) if value == snapshot(1)));
    assert!(channel.can_take());
    assert_eq!(stream.notifications.lock().pop_front(), Some(stream.id));
    channel.deliver(snapshot(2));
    channel.close(Error::RCL_RET_PUBLISHER_INVALID);
    assert!(matches!(stream.next().now_or_never(), Some(Some(Ok(value))) if value == snapshot(2)));
    assert!(matches!(
        stream.next().now_or_never(),
        Some(Some(Err(Error::RCL_RET_PUBLISHER_INVALID)))
    ));
    assert!(matches!(stream.next().now_or_never(), Some(None)));
    assert!(stream.notifications.lock().is_empty());
}

#[test]
fn qos_event_delivery_and_closure_wake_after_releasing_the_slot() {
    struct Reenter {
        channel: Arc<EventChannel>,
        count: AtomicUsize,
    }
    impl std::task::Wake for Reenter {
        fn wake(self: Arc<Self>) {
            assert!(self.channel.0.try_lock().is_some(), "event waker ran under slot lock");
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }
    let (channel, mut stream) = stream();
    let observer = Arc::new(Reenter {
        channel: channel.clone(),
        count: AtomicUsize::new(0),
    });
    let waker = Waker::from(observer.clone());
    let mut cx = Context::from_waker(&waker);
    assert!(Pin::new(&mut stream).poll_next(&mut cx).is_pending());
    channel.deliver(snapshot(1));
    assert_eq!(observer.count.load(Ordering::Relaxed), 1);
    assert!(Pin::new(&mut stream).poll_next(&mut cx).is_ready());
    assert!(Pin::new(&mut stream).poll_next(&mut cx).is_pending());
    channel.close(Error::RCL_RET_PUBLISHER_INVALID);
    channel.close(Error::RCL_RET_ERROR);
    assert_eq!(observer.count.load(Ordering::Relaxed), 2);
}

#[test]
fn qos_event_stream_drop_is_targeted_and_prevents_further_native_takes() {
    let (channel, stream) = stream();
    let id = stream.id;
    let notifications = stream.notifications.clone();
    channel.deliver(snapshot(1));
    drop(stream);
    assert!(channel.receiver_dropped());
    assert!(!channel.can_take());
    channel.deliver(snapshot(2));
    channel.close(Error::RCL_RET_PUBLISHER_INVALID);
    assert!(channel.0.lock().pending.is_none());
    assert_eq!(notifications.lock().drain(..).collect::<Vec<_>>(), [id]);
}

#[test]
fn qos_event_waker_replacement_and_stream_drop_retire_outside_the_slot_lock() {
    struct OnDrop {
        channel: Arc<EventChannel>,
    }
    impl std::task::Wake for OnDrop {
        fn wake(self: Arc<Self>) {}
    }
    impl Drop for OnDrop {
        fn drop(&mut self) {
            assert!(self.channel.0.try_lock().is_some(), "waker retirement held event slot lock");
        }
    }
    let (channel, mut stream) = stream();
    let first = Waker::from(Arc::new(OnDrop {
        channel: channel.clone(),
    }));
    assert!(Pin::new(&mut stream)
        .poll_next(&mut Context::from_waker(&first))
        .is_pending());
    drop(first);
    let second = Waker::from(Arc::new(OnDrop { channel }));
    assert!(Pin::new(&mut stream)
        .poll_next(&mut Context::from_waker(&second))
        .is_pending());
    drop(second);
    drop(stream);
}
