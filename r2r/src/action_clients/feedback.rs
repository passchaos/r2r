//! Feedback retries scheduled by goal-specific channel readiness.

use super::*;
use std::task::Waker;

const COALESCE_FLAG: usize = 1 << (usize::BITS - 1);
const BLOCKED_COUNT_MASK: usize = COALESCE_FLAG - 1;

#[cfg(test)]
#[test]
fn overflow_packed_count_boundaries_preserve_policy_after_empty_and_rejected_updates() {
    let mut queue = PendingFeedbackQueue {
        blocked_and_policy: COALESCE_FLAG,
        ..Default::default()
    };
    assert!(queue.is_empty());
    for _ in 0..3 {
        queue.register_blocked();
    }
    assert_eq!(queue.blocked_count(), 3);
    for _ in 0..3 {
        queue.unregister_blocked();
    }
    assert!(queue.is_empty());
    assert_eq!(queue.overflow_policy(), FeedbackOverflowPolicy::Coalesce);
    for word in [BLOCKED_COUNT_MASK, usize::MAX] {
        queue.blocked_and_policy = word;
        assert!(std::panic::catch_unwind(
            std::panic::AssertUnwindSafe(|| queue.register_blocked())
        )
        .is_err());
        assert_eq!(queue.blocked_and_policy, word);
    }
    queue.blocked_and_policy = COALESCE_FLAG;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| queue.unregister_blocked()))
            .is_err()
    );
    assert_eq!(queue.blocked_and_policy, COALESCE_FLAG);
}

pub(crate) enum PendingFeedbackAction {
    None,
    Backpressured,
    /// Pending retries are blocked, but their policy permits continued native ingress.
    Continue,
    Deliver(ActionClientCompletion),
}

#[derive(Default)]
pub(crate) struct PendingFeedbackQueue {
    pub(super) goals: VecDeque<uuid::Uuid>,
    pub(super) queued: HashSet<uuid::Uuid>,
    maintenance_notifier: Option<Arc<ActionClientMaintenanceNotifier>>,
    // The policy occupies the unused high bit of the bounded registration count,
    // preserving the common queue header. Registration checks the count boundary.
    blocked_and_policy: usize,
}

impl PendingFeedbackQueue {
    #[cfg(test)]
    pub(crate) fn new(notifier: Arc<ActionClientMaintenanceNotifier>) -> Self {
        Self::with_overflow_policy(notifier, FeedbackOverflowPolicy::Backpressure)
    }

    pub(crate) fn with_overflow_policy(
        notifier: Arc<ActionClientMaintenanceNotifier>, overflow_policy: FeedbackOverflowPolicy,
    ) -> Self {
        Self {
            goals: VecDeque::new(),
            queued: HashSet::new(),
            maintenance_notifier: Some(notifier),
            blocked_and_policy: if overflow_policy == FeedbackOverflowPolicy::Coalesce {
                COALESCE_FLAG
            } else {
                0
            },
        }
    }

    pub(crate) fn overflow_policy(&self) -> FeedbackOverflowPolicy {
        if self.blocked_and_policy & COALESCE_FLAG != 0 {
            FeedbackOverflowPolicy::Coalesce
        } else {
            FeedbackOverflowPolicy::Backpressure
        }
    }

    pub(super) fn blocked_count(&self) -> usize {
        self.blocked_and_policy & BLOCKED_COUNT_MASK
    }

    fn register_blocked(&mut self) {
        assert!(
            self.blocked_count() < BLOCKED_COUNT_MASK,
            "feedback registration count overflow"
        );
        self.blocked_and_policy += 1;
    }

    fn unregister_blocked(&mut self) {
        assert!(self.blocked_count() > 0, "feedback registration count underflow");
        self.blocked_and_policy -= 1;
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
        self.queued.is_empty() && self.blocked_count() == 0
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
            queue.unregister_blocked();
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
                queue.lock().unregister_blocked();
            }
        }
    }
}

pub(crate) struct FeedbackChannel<T> {
    pub(crate) sender: Option<mpsc::Sender<T>>,
    pub(crate) pending: Option<T>,
    pub(crate) closing: bool,
    pub(crate) overflow_policy: FeedbackOverflowPolicy,
}

pub(crate) type FeedbackSender<T> = Arc<Mutex<FeedbackChannel<T>>>;

impl<T> FeedbackChannel<T> {
    pub(crate) fn new(sender: mpsc::Sender<T>) -> Self {
        Self {
            sender: Some(sender),
            pending: None,
            closing: false,
            overflow_policy: FeedbackOverflowPolicy::Backpressure,
        }
    }
    pub(crate) fn with_overflow_policy(
        sender: mpsc::Sender<T>, overflow_policy: FeedbackOverflowPolicy,
    ) -> Self {
        Self {
            overflow_policy,
            ..Self::new(sender)
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
    // Construction runs under the native client lock; all delivery and payload
    // retirement happen when the completion runs after that lock is released.
    Some(ActionClientCompletion {
        notify: Box::new(move || {
            let mut locked = channel.lock();
            if locked.closing {
                drop(locked);
                drop(feedback);
                return Ok(true);
            }
            if locked.overflow_policy == FeedbackOverflowPolicy::Coalesce
                && locked.pending.is_some()
            {
                let retired = locked.pending.replace(feedback);
                drop(locked);
                drop(retired);
                return Ok(true);
            }
            let Some(sender) = locked.sender.take() else {
                let queued = locked.pending.is_none();
                let retired = if queued {
                    locked.pending = Some(feedback);
                    None
                } else {
                    Some(feedback)
                };
                drop(locked);
                drop(retired);
                if queued {
                    if let Some((uuid, queue)) = pending_queue {
                        enqueue(&queue, uuid);
                    }
                }
                return Ok(true);
            };
            drop(locked);
            send_feedback(channel, sender, feedback, pending_queue)
        }),
        cleanup_goal_on_failure: None,
    })
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
        queue.lock().register_blocked();
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
    let queue = queue.lock();
    let action = if queue.is_empty() {
        PendingFeedbackAction::None
    } else if queue.overflow_policy() == FeedbackOverflowPolicy::Coalesce {
        PendingFeedbackAction::Continue
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

pub(super) fn detached_feedback_completion<T: Send + 'static>(
    channel: FeedbackSender<T>, sender: mpsc::Sender<T>, feedback: T,
    pending_queue: Option<(uuid::Uuid, Arc<Mutex<PendingFeedbackQueue>>)>,
) -> ActionClientCompletion {
    ActionClientCompletion {
        notify: Box::new(move || send_feedback(channel, sender, feedback, pending_queue)),
        cleanup_goal_on_failure: None,
    }
}

fn send_feedback<T: Send + 'static>(
    channel: FeedbackSender<T>, mut sender: mpsc::Sender<T>, feedback: T,
    pending_queue: Option<(uuid::Uuid, Arc<Mutex<PendingFeedbackQueue>>)>,
) -> Result<bool> {
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
        let retired = pending.and_then(|payload| {
            if locked.overflow_policy == FeedbackOverflowPolicy::Coalesce
                && locked.pending.is_some()
            {
                Some(payload)
            } else {
                locked.pending.replace(payload)
            }
        });
        let queued = locked.pending.is_some();
        drop(locked);
        drop(retired);
        if queued {
            if let Some((uuid, queue)) = pending_queue {
                enqueue(&queue, uuid);
            }
        }
    }
    Ok(true)
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
    queue.lock().register_blocked();
    let ready = Arc::new(FeedbackReady {
        queue: Arc::downgrade(&queue),
        uuid: uuid::Uuid::nil(),
        notified: AtomicBool::new(false),
    });
    let wake = Waker::from(ready.clone());
    wake.wake_by_ref();
    wake.wake_by_ref();
    assert_eq!(count.count.load(Ordering::Relaxed), 1);
    assert_eq!(queue.lock().blocked_count(), 0);
    drop(wake);
    drop(ready);
    assert_eq!(queue.lock().blocked_count(), 0);
}
