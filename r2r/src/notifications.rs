//! Targeted native work queues with an optional background wake.

use parking_lot::Mutex;
use std::{
    collections::VecDeque,
    ops::{Deref, DerefMut},
    sync::Arc,
    task::Waker,
};

/// Wake storage shares the existing queue allocation. Public handles retain
/// one Arc; ordinary nodes need no additional native guard or polling timer.
#[derive(Debug)]
pub(crate) struct NotificationQueue<T> {
    entries: VecDeque<T>,
    wake: Option<Waker>,
}

impl<T> Default for NotificationQueue<T> {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            wake: None,
        }
    }
}

impl<T> NotificationQueue<T> {
    pub(crate) fn set_wake(&mut self, wake: Option<Waker>) {
        self.wake = wake;
    }

    pub(crate) fn wake(&self) -> Option<Waker> {
        self.wake.clone()
    }

    pub(crate) fn has_wake(&self) -> bool {
        self.wake.is_some()
    }

    pub(crate) fn parts(&mut self) -> (&mut VecDeque<T>, Option<&Waker>) {
        (&mut self.entries, self.wake.as_ref())
    }

    pub(crate) fn push_and_take_wake(&mut self, value: T) -> Option<Waker> {
        let was_empty = self.entries.is_empty();
        self.entries.push_back(value);
        was_empty.then(|| self.wake.clone()).flatten()
    }
}

impl<T> Deref for NotificationQueue<T> {
    type Target = VecDeque<T>;
    fn deref(&self) -> &Self::Target {
        &self.entries
    }
}

impl<T> DerefMut for NotificationQueue<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.entries
    }
}

pub(crate) fn enqueue<T>(queue: &Arc<Mutex<NotificationQueue<T>>>, value: T) {
    let wake = queue.lock().push_and_take_wake(value);
    // Native or test wakes may reenter the queue. Publish work under the lock,
    // then notify after releasing it. Bursts coalesce until the next drain.
    if let Some(wake) = wake {
        wake.wake();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Observer {
        queue: Arc<Mutex<NotificationQueue<u32>>>,
        count: AtomicUsize,
    }
    impl std::task::Wake for Observer {
        fn wake(self: Arc<Self>) {
            let queue = self
                .queue
                .try_lock()
                .expect("notification held the queue lock");
            assert!(!queue.is_empty(), "wake preceded visible work");
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn notifications_follow_work_unlock_and_coalesce_until_drain() {
        let queue = Arc::new(Mutex::new(NotificationQueue::default()));
        let observer = Arc::new(Observer {
            queue: queue.clone(),
            count: AtomicUsize::new(0),
        });
        queue.lock().set_wake(Some(Waker::from(observer.clone())));
        enqueue(&queue, 1);
        enqueue(&queue, 2);
        assert_eq!(observer.count.load(Ordering::Relaxed), 1);
        assert_eq!(queue.lock().drain(..).collect::<Vec<_>>(), [1, 2]);
        enqueue(&queue, 3);
        assert_eq!(observer.count.load(Ordering::Relaxed), 2);
        queue.lock().set_wake(None);
        queue.lock().clear();
        enqueue(&queue, 4);
        assert_eq!(observer.count.load(Ordering::Relaxed), 2);
    }
}
