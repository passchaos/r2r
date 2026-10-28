//! Feedback retries scheduled by goal-specific channel readiness.

use super::*;
use std::task::Waker;

pub(crate) enum PendingFeedbackAction {
    None,
    Backpressured,
    Deliver(ActionClientCompletion),
}

#[derive(Default)]
pub(crate) struct PendingFeedbackQueue {
    pub(super) goals: VecDeque<uuid::Uuid>,
    pub(super) queued: HashSet<uuid::Uuid>,
    maintenance_notifier: Option<Arc<ActionClientMaintenanceNotifier>>,
    pub(super) blocked: usize,
}

impl PendingFeedbackQueue {
    pub(crate) fn new(notifier: Arc<ActionClientMaintenanceNotifier>) -> Self {
        Self {
            goals: VecDeque::new(),
            queued: HashSet::new(),
            maintenance_notifier: Some(notifier),
            blocked: 0,
        }
    }

    // Only runnable identities are queued. Registered sender wakers own
    // blocked retry counts, so closing a goal cannot leave stale pending IDs.
    pub(super) fn push(&mut self, uuid: uuid::Uuid) -> bool {
        if self.queued.insert(uuid) {
            self.goals.push_back(uuid);
            true
        } else {
            false
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.queued.is_empty() && self.blocked == 0
    }
}

pub(super) fn enqueue(queue: &Arc<Mutex<PendingFeedbackQueue>>, uuid: uuid::Uuid) {
    let notifier = {
        let mut queue = queue.lock();
        if queue.push(uuid) {
            queue.maintenance_notifier.clone()
        } else {
            None
        }
    };
    if let Some(notifier) = notifier {
        notifier.notify();
    }
}

struct FeedbackReady {
    queue: Weak<Mutex<PendingFeedbackQueue>>,
    uuid: uuid::Uuid,
    notified: AtomicBool,
}

impl std::task::Wake for FeedbackReady {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        // Each readiness registration may queue one retry. Replacement on the
        // next pending poll re-arms it; repeated wakes cannot grow the queue.
        if self.notified.swap(true, Ordering::AcqRel) {
            return;
        }
        let Some(queue) = self.queue.upgrade() else {
            return;
        };
        let notifier = {
            let mut queue = queue.lock();
            queue.blocked -= 1;
            if queue.push(self.uuid) {
                queue.maintenance_notifier.clone()
            } else {
                None
            }
        };
        if let Some(notifier) = notifier {
            notifier.notify();
        }
    }
}

impl Drop for FeedbackReady {
    fn drop(&mut self) {
        // A sender that closes without becoming writable drops its cached
        // waker. Release that registration without retaining an abandoned UUID.
        if !self.notified.swap(true, Ordering::AcqRel) {
            if let Some(queue) = self.queue.upgrade() {
                queue.lock().blocked -= 1;
            }
        }
    }
}

pub(crate) struct FeedbackChannel<T> {
    pub(crate) sender: Option<mpsc::Sender<T>>,
    pub(crate) pending: Option<T>,
    pub(crate) closing: bool,
}

pub(crate) type FeedbackSender<T> = Arc<Mutex<FeedbackChannel<T>>>;

impl<T> FeedbackChannel<T> {
    pub(crate) fn new(sender: mpsc::Sender<T>) -> Self {
        Self {
            sender: Some(sender),
            pending: None,
            closing: false,
        }
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.closing || self.sender.as_ref().is_some_and(mpsc::Sender::is_closed)
    }
}

#[cfg(test)]
pub(super) fn feedback_completion<T: Send + 'static>(
    channel: FeedbackSender<T>, feedback: T,
) -> Option<ActionClientCompletion> {
    feedback_completion_with_queue(channel, feedback, None)
}

pub(crate) fn feedback_completion_with_queue<T: Send + 'static>(
    channel: FeedbackSender<T>, feedback: T,
    pending_queue: Option<(uuid::Uuid, Arc<Mutex<PendingFeedbackQueue>>)>,
) -> Option<ActionClientCompletion> {
    let mut locked = channel.lock();
    let Some(sender) = locked.sender.take() else {
        let queued = locked.pending.is_none();
        locked.pending.get_or_insert(feedback);
        drop(locked);
        if queued {
            if let Some((uuid, queue)) = pending_queue {
                enqueue(&queue, uuid);
            }
        }
        return None;
    };
    drop(locked);
    Some(detached_feedback_completion(channel, sender, feedback, pending_queue))
}

#[cfg(test)]
pub(super) fn take_ready_pending_feedback<T: Send + 'static>(
    queue: &Arc<Mutex<PendingFeedbackQueue>>, channels: &HashMap<uuid::Uuid, FeedbackSender<T>>,
) -> PendingFeedbackAction {
    take_ready_pending_feedback_with_progress(queue, channels).0
}

pub(crate) fn take_ready_pending_feedback_with_progress<T: Send + 'static>(
    queue: &Arc<Mutex<PendingFeedbackQueue>>, channels: &HashMap<uuid::Uuid, FeedbackSender<T>>,
) -> (PendingFeedbackAction, bool) {
    let count = queue.lock().goals.len().min(MAX_ACTION_FEEDBACK_PER_SPIN);
    for _ in 0..count {
        let uuid = {
            let mut queue = queue.lock();
            let Some(uuid) = queue.goals.pop_front() else {
                break;
            };
            queue.queued.remove(&uuid);
            uuid
        };
        let Some(channel) = channels.get(&uuid) else {
            queue.lock().queued.remove(&uuid);
            continue;
        };
        let mut locked = channel.lock();
        if locked.pending.is_none() {
            queue.lock().queued.remove(&uuid);
            continue;
        }
        let Some(sender) = locked.sender.as_mut() else {
            // Restoration schedules its retained payload. Do not poll or
            // repeatedly wake while another completion owns the sender.
            queue.lock().queued.remove(&uuid);
            continue;
        };
        queue.lock().blocked += 1;
        let waker = Waker::from(Arc::new(FeedbackReady {
            queue: Arc::downgrade(queue),
            uuid,
            notified: AtomicBool::new(false),
        }));
        let mut context = Context::from_waker(&waker);
        if sender.poll_ready(&mut context).is_pending() {
            continue;
        }
        let payload = locked.pending.take().unwrap();
        let sender = locked.sender.take().unwrap();
        queue.lock().queued.remove(&uuid);
        drop(locked);
        let continuation = continue_ready(queue);
        return (
            PendingFeedbackAction::Deliver(detached_feedback_completion(
                channel.clone(),
                sender,
                payload,
                Some((uuid, queue.clone())),
            )),
            continuation,
        );
    }
    let continuation = continue_ready(queue);
    let action = if queue.lock().is_empty() {
        PendingFeedbackAction::None
    } else {
        PendingFeedbackAction::Backpressured
    };
    (action, continuation)
}

fn continue_ready(queue: &Arc<Mutex<PendingFeedbackQueue>>) -> bool {
    let (more, notifier) = {
        let queue = queue.lock();
        let more = !queue.goals.is_empty();
        (more, more.then(|| queue.maintenance_notifier.clone()).flatten())
    };
    if let Some(notifier) = notifier {
        notifier.notify();
    }
    more
}

fn detached_feedback_completion<T: Send + 'static>(
    channel: FeedbackSender<T>, mut sender: mpsc::Sender<T>, feedback: T,
    pending_queue: Option<(uuid::Uuid, Arc<Mutex<PendingFeedbackQueue>>)>,
) -> ActionClientCompletion {
    ActionClientCompletion {
        notify: Box::new(move || {
            let pending = match sender.try_send(feedback) {
                Ok(()) => None,
                Err(error) if error.is_full() => Some(error.into_inner()),
                Err(_) => None,
            };
            let mut locked = channel.lock();
            if locked.closing {
                drop(locked);
                drop(sender);
                drop(pending);
            } else {
                locked.sender = Some(sender);
                // A reentrant arrival while the sender was detached may have
                // left one payload. A successful send must not erase it.
                if pending.is_some() {
                    locked.pending = pending;
                }
                let queued = locked.pending.is_some();
                drop(locked);
                if queued {
                    if let Some((uuid, queue)) = pending_queue {
                        enqueue(&queue, uuid);
                    }
                }
            }
            Ok(true)
        }),
        cleanup_goal_on_failure: None,
    }
}

pub(crate) fn close_feedback_completion<T: Send + 'static>(
    channel: FeedbackSender<T>,
) -> ActionClientCompletion {
    ActionClientCompletion {
        notify: Box::new(move || {
            let (sender, pending) = {
                let mut locked = channel.lock();
                locked.closing = true;
                (locked.sender.take(), locked.pending.take())
            };
            drop(sender);
            drop(pending);
            Ok(true)
        }),
        cleanup_goal_on_failure: None,
    }
}

#[cfg(test)]
#[test]
fn readiness_notifications_unlock_queue_and_coalesce_per_registration() {
    struct Observe {
        queue: Weak<Mutex<PendingFeedbackQueue>>,
        count: AtomicUsize,
    }
    impl std::task::Wake for Observe {
        fn wake(self: Arc<Self>) {
            let queue = self.queue.upgrade().unwrap();
            assert_eq!(
                queue
                    .try_lock()
                    .expect("capacity notification held the queue lock")
                    .goals
                    .len(),
                1
            );
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }
    let active = Arc::new(Mutex::new(crate::notifications::NotificationQueue::default()));
    let notifier = Arc::new(ActionClientMaintenanceNotifier::new(active.clone()));
    notifier.set_identity(42);
    let queue = Arc::new(Mutex::new(PendingFeedbackQueue::new(notifier)));
    let count = Arc::new(Observe {
        queue: Arc::downgrade(&queue),
        count: AtomicUsize::new(0),
    });
    active.lock().set_wake(Some(Waker::from(count.clone())));
    queue.lock().blocked = 1;
    let ready = Arc::new(FeedbackReady {
        queue: Arc::downgrade(&queue),
        uuid: uuid::Uuid::nil(),
        notified: AtomicBool::new(false),
    });
    let wake = Waker::from(ready.clone());
    wake.wake_by_ref();
    wake.wake_by_ref();
    assert_eq!(count.count.load(Ordering::Relaxed), 1);
    assert_eq!(queue.lock().blocked, 0);
    drop(wake);
    drop(ready);
    assert_eq!(queue.lock().blocked, 0);
}
