//! Node-owned action-server cleanup and runnable maintenance notifications.

use super::*;
use std::{
    ops::{Deref, DerefMut},
    task::Waker,
};

const MAX_SERVERS_PER_PREPARE: usize = 16;

#[derive(Default)]
struct ReadyServers {
    entries: VecDeque<uuid::Uuid>,
    // The common small queue keeps one optional pointer rather than an inline
    // HashSet header. Its index is allocated only beyond the bounded scan size.
    #[allow(clippy::box_collection)]
    queued: Option<Box<HashSet<uuid::Uuid>>>,
}

#[derive(Default)]
pub(crate) struct ActionServerNotifications {
    dropped: crate::notifications::NotificationQueue<uuid::Uuid>,
    ready: Option<Box<ReadyServers>>,
}

impl ActionServerNotifications {
    fn work_empty(&self) -> bool {
        self.dropped.is_empty()
            && self
                .ready
                .as_ref()
                .is_none_or(|ready| ready.entries.is_empty())
    }

    pub(crate) fn set_wake(&mut self, wake: Option<Waker>) {
        self.dropped.set_wake(wake);
    }

    pub(crate) fn queue_ready(&mut self, id: uuid::Uuid) -> Option<Waker> {
        let empty = self.work_empty();
        let ready = self.ready.get_or_insert_with(Default::default);
        if let Some(queued) = &mut ready.queued {
            if !queued.insert(id) {
                return None;
            }
        } else if ready.entries.contains(&id) {
            return None;
        }
        ready.entries.push_back(id);
        if ready.queued.is_none() && ready.entries.len() > MAX_SERVERS_PER_PREPARE {
            ready.queued = Some(Box::new(ready.entries.iter().copied().collect()));
        }
        empty.then(|| self.dropped.wake()).flatten()
    }

    pub(crate) fn take_ready(&mut self) -> (Vec<uuid::Uuid>, bool, Option<Waker>) {
        let Some(ready) = &mut self.ready else {
            return (Vec::new(), false, None);
        };
        let count = ready.entries.len().min(MAX_SERVERS_PER_PREPARE);
        let entries = ready
            .entries
            .drain(..count)
            .inspect(|id| {
                if let Some(queued) = &mut ready.queued {
                    queued.remove(id);
                }
            })
            .collect();
        let more = !ready.entries.is_empty();
        if !more {
            ready.queued = None;
        }
        let wake = more.then(|| self.dropped.wake()).flatten();
        (entries, more, wake)
    }

    pub(crate) fn remove(&mut self, id: uuid::Uuid) {
        self.dropped.retain(|entry| *entry != id);
        if let Some(ready) = &mut self.ready {
            if let Some(queued) = &mut ready.queued {
                queued.remove(&id);
            }
            ready.entries.retain(|entry| *entry != id);
            if ready.entries.is_empty() {
                ready.queued = None;
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn is_ready(&self, id: uuid::Uuid) -> bool {
        self.ready.as_ref().is_some_and(|ready| {
            ready
                .queued
                .as_ref()
                .map_or_else(|| ready.entries.contains(&id), |queued| queued.contains(&id))
        })
    }

    #[cfg(test)]
    pub(crate) fn has_ready(&self) -> bool {
        self.ready
            .as_ref()
            .is_some_and(|ready| !ready.entries.is_empty())
    }
}

impl Deref for ActionServerNotifications {
    type Target = VecDeque<uuid::Uuid>;
    fn deref(&self) -> &Self::Target {
        &self.dropped
    }
}

impl DerefMut for ActionServerNotifications {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.dropped
    }
}

pub(crate) fn enqueue_ready(queue: &Arc<Mutex<ActionServerNotifications>>, id: uuid::Uuid) {
    let wake = queue.lock().queue_ready(id);
    if let Some(wake) = wake {
        wake.wake();
    }
}

pub(super) fn enqueue_drop(queue: &Arc<Mutex<ActionServerNotifications>>, id: uuid::Uuid) {
    let wake = {
        let mut queue = queue.lock();
        let empty = queue.work_empty();
        queue.dropped.push_back(id);
        empty.then(|| queue.dropped.wake()).flatten()
    };
    if let Some(wake) = wake {
        wake.wake();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::atomic::{AtomicUsize, Ordering},
        task::Wake,
    };

    struct Observe {
        queue: Weak<Mutex<ActionServerNotifications>>,
        count: AtomicUsize,
    }
    impl Wake for Observe {
        fn wake(self: Arc<Self>) {
            assert!(!self
                .queue
                .upgrade()
                .unwrap()
                .try_lock()
                .expect("notification held queue lock")
                .work_empty());
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn ready_server_boundaries_are_bounded_deduplicated_and_notified_after_unlock() {
        for count in [0, 1, 16, 17, 40, 128] {
            let queue = Arc::new(Mutex::new(ActionServerNotifications::default()));
            let observe = Arc::new(Observe {
                queue: Arc::downgrade(&queue),
                count: AtomicUsize::new(0),
            });
            queue.lock().set_wake(Some(Waker::from(observe.clone())));
            assert!(queue.lock().ready.is_none());
            for i in 0..count {
                enqueue_ready(&queue, uuid::Uuid::from_u128(i));
                enqueue_ready(&queue, uuid::Uuid::from_u128(i));
            }
            assert_eq!(observe.count.load(Ordering::Relaxed), usize::from(count > 0));
            let mut total = Vec::new();
            loop {
                let (batch, more, wake) = queue.lock().take_ready();
                assert!(batch.len() <= MAX_SERVERS_PER_PREPARE);
                total.extend(batch);
                assert_eq!(more, total.len() < count as usize);
                if let Some(wake) = wake {
                    wake.wake();
                }
                if !more {
                    break;
                }
            }
            assert_eq!(
                total.iter().map(|id| id.as_u128()).collect::<Vec<_>>(),
                (0..count).collect::<Vec<_>>()
            );
            assert!(queue.lock().work_empty());
        }
    }

    #[test]
    fn cleanup_and_ready_work_coalesce_and_removal_clears_both_lanes() {
        let queue = Arc::new(Mutex::new(ActionServerNotifications::default()));
        let observe = Arc::new(Observe {
            queue: Arc::downgrade(&queue),
            count: AtomicUsize::new(0),
        });
        queue.lock().set_wake(Some(Waker::from(observe.clone())));
        let id = uuid::Uuid::nil();
        enqueue_drop(&queue, id);
        enqueue_ready(&queue, id);
        assert_eq!(observe.count.load(Ordering::Relaxed), 1);
        queue.lock().remove(id);
        assert!(queue.lock().work_empty());
        enqueue_ready(&queue, id);
        assert_eq!(observe.count.load(Ordering::Relaxed), 2);
        assert_eq!(queue.lock().take_ready().0, [id]);
        enqueue_drop(&queue, id);
        assert_eq!(observe.count.load(Ordering::Relaxed), 3);
    }
}
