//! A canceled-operation queue needs one notification per unfinished batch.

use super::{ActionClientMaintenanceQueue, Arc, Mutex};

#[cfg(test)]
pub(super) fn take_batch<T>(
    queue: &Arc<Mutex<ActionClientMaintenanceQueue<T>>>, limit: usize,
) -> Vec<T> {
    take_batch_with_progress(queue, limit).0
}

pub(super) fn take_batch_with_progress<T>(
    queue: &Arc<Mutex<ActionClientMaintenanceQueue<T>>>, limit: usize,
) -> (Vec<T>, bool) {
    let (entries, continuation, unexamined) = {
        let mut queue = queue.lock();
        let count = queue.entries.len().min(limit);
        let entries = queue.entries.drain(..count).collect();
        // Entries requeued later for a detached feedback sender are retries,
        // not unexamined work. They must not trigger an endless self-wake.
        let continuation = if queue.entries.is_empty() {
            None
        } else {
            queue.notifier.clone()
        };
        let unexamined = !queue.entries.is_empty();
        (entries, continuation, unexamined)
    };
    // The original active-client notification was already drained during
    // preparation. Re-arm after exposing the remaining entries and unlocking.
    if let Some(notifier) = continuation {
        notifier.notify();
    }
    (entries, unexamined)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{action_clients::*, notifications::NotificationQueue};
    use futures::channel::{mpsc, oneshot};
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicUsize, Ordering},
        task::Waker,
    };

    struct QueueWake<T> {
        queue: std::sync::Weak<Mutex<ActionClientMaintenanceQueue<T>>>,
        count: AtomicUsize,
    }
    impl<T: Send> std::task::Wake for QueueWake<T> {
        fn wake(self: Arc<Self>) {
            let queue = self.queue.upgrade().unwrap();
            let queue = queue
                .try_lock()
                .expect("batch continuation held the queue lock");
            assert!(!queue.is_empty(), "notification preceded visible backlog");
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    type QueueFixture<T> = (
        Arc<Mutex<ActionClientMaintenanceQueue<T>>>,
        Arc<Mutex<NotificationQueue<usize>>>,
        Arc<QueueWake<T>>,
    );

    fn queue<T: Send + 'static>() -> QueueFixture<T> {
        let active = Arc::new(Mutex::new(NotificationQueue::default()));
        let notifier = Arc::new(ActionClientMaintenanceNotifier::new(active.clone()));
        notifier.set_identity(42);
        let queue = Arc::new(Mutex::new(ActionClientMaintenanceQueue::new(notifier)));
        let wake = Arc::new(QueueWake {
            queue: Arc::downgrade(&queue),
            count: AtomicUsize::new(0),
        });
        active.lock().set_wake(Some(Waker::from(wake.clone())));
        (queue, active, wake)
    }

    #[test]
    fn unfinished_batches_rearm_after_unlock_and_stop_at_the_last_entry() {
        for count in [0_usize, 1, 16, 17, 40, 128] {
            let (queue, active, wake) = queue();
            // Fill directly: the initial enqueue event has already been
            // consumed by the native owner, as in the reproduced failure.
            queue.lock().entries.extend(0..count);
            let mut seen = Vec::new();
            while !queue.lock().is_empty() {
                active.lock().clear();
                let batch = take_batch(&queue, 16);
                assert!(batch.len() <= 16);
                seen.extend(batch);
                assert_eq!(!active.lock().is_empty(), !queue.lock().is_empty());
            }
            assert_eq!(seen, (0..count).collect::<Vec<_>>());
            assert_eq!(wake.count.load(Ordering::Relaxed), count.div_ceil(16).saturating_sub(1));
            let before = wake.count.load(Ordering::Relaxed);
            assert!(take_batch(&queue, 16).is_empty());
            assert_eq!(wake.count.load(Ordering::Relaxed), before);
        }
        let (queue, active, wake) = queue();
        queue.lock().entries.extend(0..40);
        queue.lock().disable_notifier();
        assert_eq!(take_batch(&queue, 16), (0..16).collect::<Vec<_>>());
        assert_eq!(queue.lock().len(), 24);
        assert!(active.lock().is_empty());
        assert_eq!(wake.count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn result_and_cancel_cleanup_preserve_live_requests_and_process_stale_tokens() {
        let (results, active, _) = queue();
        let mut result_senders = HashMap::new();
        let mut live_result = None;
        for id in 1..=40_u128 {
            let (sender, receiver) = oneshot::channel::<()>();
            if id == 40 {
                live_result = Some(receiver);
            } else {
                drop(receiver);
            }
            result_senders.insert(uuid::Uuid::from_u128(id), sender);
            results.lock().entries.push_back(uuid::Uuid::from_u128(id));
        }
        let mut removed = Vec::new();
        for _ in 0..3 {
            active.lock().clear();
            removed.extend(take_canceled_result_goals(&results, &result_senders));
        }
        assert_eq!(removed, (1..40).map(uuid::Uuid::from_u128).collect::<Vec<_>>());
        assert!(live_result.unwrap().try_recv().unwrap().is_none());
        assert!(results.lock().is_empty());
        assert!(active.lock().is_empty());

        let (cancels, active, _) = queue();
        let mut cancel_senders = HashMap::new();
        let mut live_cancel = None;
        for id in 0..40_i64 {
            let (sender, receiver) = oneshot::channel();
            if id == 39 {
                live_cancel = Some(receiver);
            } else {
                drop(receiver);
            }
            if id != 38 {
                cancel_senders.insert(id, sender);
            }
            cancels.lock().entries.push_back(id);
        }
        let mut removed = Vec::new();
        for _ in 0..3 {
            active.lock().clear();
            removed.extend(take_canceled_cancel_sequences(&cancels, &cancel_senders));
        }
        assert_eq!(removed, (0..38).collect::<Vec<_>>());
        assert!(live_cancel.unwrap().try_recv().unwrap().is_none());
        assert!(cancels.lock().is_empty());
        assert!(active.lock().is_empty());
    }

    #[test]
    fn detached_feedback_retry_is_not_unexamined_work() {
        let (queue, active, wake) = queue();
        let uuid = uuid::Uuid::from_u128(1);
        let (sender, receiver) = mpsc::channel::<u32>(1);
        let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
        let detached = channel.lock().sender.take().unwrap();
        let channels = HashMap::from([(uuid, channel.clone())]);
        queue.lock().entries.push_back(uuid);
        assert!(take_canceled_feedback_goals(&queue, &channels).is_empty());
        assert_eq!(queue.lock().len(), 1);
        assert!(active.lock().is_empty());
        assert_eq!(wake.count.load(Ordering::Relaxed), 0);
        drop(receiver);
        channel.lock().sender = Some(detached);
        assert_eq!(take_canceled_feedback_goals(&queue, &channels), [uuid]);
        assert!(queue.lock().is_empty());
        assert_eq!(wake.count.load(Ordering::Relaxed), 0);
    }
}
