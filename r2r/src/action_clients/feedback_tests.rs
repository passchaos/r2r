//! Native retry notifications must follow feedback capacity recovery.

#[cfg(test)]
mod overflow;

use super::*;
use futures::{
    task::{waker, ArcWake},
    FutureExt, StreamExt,
};

struct Count(AtomicUsize);
impl ArcWake for Count {
    fn wake_by_ref(this: &Arc<Self>) {
        this.0.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn full_feedback_parks_until_capacity_recovery_then_notifies_native_retry() {
    let active = Arc::new(Mutex::new(crate::notifications::NotificationQueue::default()));
    let notifier = Arc::new(ActionClientMaintenanceNotifier::new(active.clone()));
    notifier.set_identity(42);
    let pending = Arc::new(Mutex::new(PendingFeedbackQueue::new(notifier)));
    let count = Arc::new(Count(AtomicUsize::new(0)));
    active.lock().set_wake(Some(waker(count.clone())));
    let uuid = uuid::Uuid::new_v4();
    let (sender, mut receiver) = mpsc::channel(0);
    let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
    let channels = HashMap::from([(uuid, channel.clone())]);
    for value in [1, 2] {
        let completion =
            feedback_completion_with_queue(channel.clone(), value, Some((uuid, pending.clone())))
                .unwrap();
        (completion.notify)().unwrap();
    }
    active.lock().clear();
    let before = count.0.load(Ordering::Relaxed);
    assert!(matches!(
        take_ready_pending_feedback(&pending, &channels),
        PendingFeedbackAction::Backpressured
    ));
    assert!(active.lock().is_empty(), "blocked feedback self-notified instead of waiting");
    assert_eq!(count.0.load(Ordering::Relaxed), before);
    assert_eq!(receiver.next().now_or_never(), Some(Some(1)));
    assert_eq!(
        active.lock().iter().copied().collect::<Vec<_>>(),
        [42],
        "capacity recovery did not notify native execution"
    );
    let PendingFeedbackAction::Deliver(completion) =
        take_ready_pending_feedback(&pending, &channels)
    else {
        panic!("pending feedback was not retried");
    };
    (completion.notify)().unwrap();
    assert_eq!(receiver.next().now_or_never(), Some(Some(2)));
    assert!(channel.lock().pending.is_none());
    assert!(pending.lock().is_empty());
}

#[test]
fn forty_blocked_goals_park_after_bounded_scans_and_only_ready_goal_retries() {
    let active = Arc::new(Mutex::new(crate::notifications::NotificationQueue::default()));
    let notifier = Arc::new(ActionClientMaintenanceNotifier::new(active.clone()));
    notifier.set_identity(42);
    let queue = Arc::new(Mutex::new(PendingFeedbackQueue::new(notifier)));
    let count = Arc::new(Count(AtomicUsize::new(0)));
    active.lock().set_wake(Some(waker(count.clone())));
    let mut channels = HashMap::new();
    let mut receivers = Vec::new();
    for value in 1..=40_u32 {
        let uuid = uuid::Uuid::from_u128(value as u128);
        let (sender, receiver) = mpsc::channel(0);
        let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
        for payload in [0, value] {
            let ready = feedback_completion_with_queue(
                channel.clone(),
                payload,
                Some((uuid, queue.clone())),
            )
            .unwrap();
            (ready.notify)().unwrap();
        }
        channels.insert(uuid, channel);
        receivers.push(receiver);
    }
    for remaining in [24, 8, 0] {
        active.lock().clear();
        let (ready, continuation) = take_ready_pending_feedback_with_progress(&queue, &channels);
        assert!(matches!(ready, PendingFeedbackAction::Backpressured));
        assert_eq!(queue.lock().goals.len(), remaining);
        assert_eq!(continuation, remaining > 0);
        assert_eq!(!active.lock().is_empty(), remaining > 0);
    }
    active.lock().clear();
    let before = count.0.load(Ordering::Relaxed);
    for _ in 0..5 {
        let (_, continuation) = take_ready_pending_feedback_with_progress(&queue, &channels);
        assert!(!continuation);
    }
    assert_eq!(count.0.load(Ordering::Relaxed), before);
    assert_eq!(queue.lock().blocked_count(), 40);
    assert!(queue.lock().goals.is_empty());
    assert_eq!(receivers[39].next().now_or_never(), Some(Some(0)));
    assert_eq!(
        queue.lock().goals.iter().copied().collect::<Vec<_>>(),
        [uuid::Uuid::from_u128(40)]
    );
    let (PendingFeedbackAction::Deliver(ready), continuation) =
        take_ready_pending_feedback_with_progress(&queue, &channels)
    else {
        panic!("ready goal was not retried");
    };
    assert!(!continuation);
    (ready.notify)().unwrap();
    assert_eq!(receivers[39].next().now_or_never(), Some(Some(40)));
    assert_eq!(queue.lock().blocked_count(), 39);
    assert!(channels[&uuid::Uuid::from_u128(1)].lock().pending.is_some());
}

#[test]
fn detached_sender_restoration_preserves_and_reschedules_reentrant_feedback() {
    let uuid = uuid::Uuid::new_v4();
    let queue = Arc::new(Mutex::new(PendingFeedbackQueue::default()));
    let (sender, mut receiver) = mpsc::channel(1);
    let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
    let sender = channel.lock().sender.take().unwrap();
    let first = feedback::detached_feedback_completion(
        channel.clone(),
        sender,
        1,
        Some((uuid, queue.clone())),
    );
    (feedback_completion_with_queue(channel.clone(), 2, Some((uuid, queue.clone())))
        .unwrap()
        .notify)()
    .unwrap();
    let channels = HashMap::from([(uuid, channel.clone())]);
    let _ = take_ready_pending_feedback(&queue, &channels);
    assert!(queue.lock().is_empty());
    (first.notify)().unwrap();
    assert_eq!(
        channel
            .lock()
            .pending
            .as_ref()
            .and_then(PendingFeedback::ready_ref),
        Some(&2)
    );
    let PendingFeedbackAction::Deliver(second) = take_ready_pending_feedback(&queue, &channels)
    else {
        panic!("restored pending payload was erased");
    };
    (second.notify)().unwrap();
    assert_eq!(receiver.next().now_or_never(), Some(Some(1)));
    assert_eq!(receiver.next().now_or_never(), Some(Some(2)));
    assert!(queue.lock().is_empty());
}

#[test]
fn stale_capacity_wake_cannot_resurrect_closed_feedback_or_keep_owner_alive() {
    let active = Arc::new(Mutex::new(crate::notifications::NotificationQueue::default()));
    let notifier = Arc::new(ActionClientMaintenanceNotifier::new(active.clone()));
    notifier.set_identity(42);
    let queue = Arc::new(Mutex::new(PendingFeedbackQueue::new(notifier)));
    let weak = Arc::downgrade(&queue);
    let uuid = uuid::Uuid::new_v4();
    let (sender, mut receiver) = mpsc::channel(0);
    let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
    let channels = HashMap::from([(uuid, channel.clone())]);
    for value in [1, 2] {
        (feedback_completion_with_queue(channel.clone(), value, Some((uuid, queue.clone())))
            .unwrap()
            .notify)()
        .unwrap();
    }
    let _ = take_ready_pending_feedback(&queue, &channels);
    (close_feedback_completion(channel).notify)().unwrap();
    active.lock().clear();
    assert_eq!(receiver.next().now_or_never(), Some(Some(1)));
    // A cached channel waker may outlive closing the sender. Its bounded
    // stale retry must be discarded without restoring the erased payload.
    assert!(matches!(
        take_ready_pending_feedback(&queue, &channels),
        PendingFeedbackAction::None
    ));
    assert!(queue.lock().is_empty());
    drop(queue);
    assert!(weak.upgrade().is_none());
}
