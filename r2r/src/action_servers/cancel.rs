//! Lazy indexed cancellation work for fanout and concurrent request backlogs.

use super::*;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    task::{Wake, Waker},
};

pub(super) mod delivery;
#[cfg(test)]
mod tests;

type DecisionResult = std::result::Result<(uuid::Uuid, bool), oneshot::Canceled>;
type DecisionReceiver = oneshot::Receiver<(uuid::Uuid, bool)>;
pub(crate) type CancelSenders =
    HashMap<uuid::Uuid, Arc<Mutex<Option<mpsc::Sender<ActionServerCancelRequest>>>>>;
pub(crate) type WakeOwner = Weak<Mutex<crate::notifications::NotificationQueue<uuid::Uuid>>>;

pub(super) fn current_owner_waker(owner: WakeOwner) -> Waker {
    struct CurrentOwnerWake(WakeOwner);
    impl Wake for CurrentOwnerWake {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            let Some(owner) = self.0.upgrade() else {
                return;
            };
            let wake = owner.lock().wake();
            if let Some(wake) = wake {
                wake.wake();
            }
        }
    }
    Waker::from(Arc::new(CurrentOwnerWake(owner)))
}

pub struct ActiveCancelRequest {
    pub(super) request_id: rmw_request_id_t,
    pub(super) response: action_msgs::srv::CancelGoal::Response,
    pub(super) pending_decisions: VecDeque<DecisionReceiver>,
    pub(super) completed_decisions: Vec<DecisionResult>,
}

#[cfg(test)]
pub(super) fn poll_cancel_decisions(
    request: &mut ActiveCancelRequest,
) -> Option<Vec<DecisionResult>> {
    poll_cancel_decisions_with_wake(request, None)
}

pub(super) fn poll_cancel_decisions_with_wake(
    request: &mut ActiveCancelRequest, wake: Option<&Waker>,
) -> Option<Vec<DecisionResult>> {
    let mut context = Context::from_waker(wake.unwrap_or(noop_waker_ref()));
    let count = request
        .pending_decisions
        .len()
        .min(MAX_ACTION_SERVER_REQUESTS_PER_SPIN);
    for _ in 0..count {
        let mut decision = request.pending_decisions.pop_front().unwrap();
        match Pin::new(&mut decision).poll(&mut context) {
            Poll::Ready(result) => request.completed_decisions.push(result),
            Poll::Pending => request.pending_decisions.push_back(decision),
        }
    }
    request
        .pending_decisions
        .is_empty()
        .then(|| std::mem::take(&mut request.completed_decisions))
}

pub(super) fn poll_bounded_queue<T, R>(
    queue: &mut VecDeque<T>, mut poll: impl FnMut(&mut T) -> Option<R>,
) -> Vec<(T, R)> {
    let mut ready = Vec::new();
    for _ in 0..queue.len().min(MAX_ACTION_SERVER_REQUESTS_PER_SPIN) {
        let mut item = queue.pop_front().unwrap();
        if let Some(result) = poll(&mut item) {
            ready.push((item, result));
        } else {
            queue.push_back(item);
        }
    }
    ready
}

pub(super) struct ReadyQueue {
    entries: VecDeque<usize>,
    queued: Vec<bool>,
    notify: Waker,
}

impl ReadyQueue {
    pub(super) fn new(count: usize, notify: Waker) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            entries: (0..count).collect(),
            queued: vec![true; count],
            notify,
        }))
    }

    pub(super) fn take(&mut self) -> Vec<usize> {
        let count = self.entries.len().min(MAX_ACTION_SERVER_REQUESTS_PER_SPIN);
        self.entries
            .drain(..count)
            .inspect(|index| self.queued[*index] = false)
            .collect()
    }

    pub(super) fn has_ready(&self) -> bool {
        !self.entries.is_empty()
    }

    pub(super) fn push(&mut self, index: usize) -> bool {
        if std::mem::replace(&mut self.queued[index], true) {
            false
        } else {
            self.entries.push_back(index);
            true
        }
    }

    pub(super) fn waker(queue: &Arc<Mutex<Self>>, index: usize) -> Waker {
        Waker::from(Arc::new(EntryWake {
            queue: Arc::downgrade(queue),
            index,
            notified: AtomicBool::new(false),
        }))
    }

    pub(super) fn notify(&self) -> Waker {
        self.notify.clone()
    }
}

struct EntryWake {
    queue: Weak<Mutex<ReadyQueue>>,
    index: usize,
    notified: AtomicBool,
}
impl Wake for EntryWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        if self.notified.swap(true, Ordering::AcqRel) {
            return;
        }
        let Some(queue) = self.queue.upgrade() else {
            return;
        };
        let wake = {
            let mut queue = queue.lock();
            queue.push(self.index).then(|| queue.notify())
        };
        if let Some(wake) = wake {
            wake.wake();
        }
    }
}

#[derive(Clone, Copy)]
struct RequestId {
    index: usize,
    generation: u64,
}
struct RequestSlot {
    generation: u64,
    active: bool,
    queued: bool,
}
struct RequestReadiness {
    entries: VecDeque<RequestId>,
    slots: Vec<RequestSlot>,
    wake: Option<Waker>,
}

impl RequestReadiness {
    fn push(&mut self, id: RequestId) -> bool {
        let Some(slot) = self.slots.get_mut(id.index) else {
            return false;
        };
        if !slot.active
            || slot.generation != id.generation
            || std::mem::replace(&mut slot.queued, true)
        {
            return false;
        }
        self.entries.push_back(id);
        true
    }
}

struct RequestWake {
    queue: Weak<Mutex<RequestReadiness>>,
    id: RequestId,
}
impl Wake for RequestWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        let Some(queue) = self.queue.upgrade() else {
            return;
        };
        let wake = {
            let mut queue = queue.lock();
            queue.push(self.id).then(|| queue.wake.clone()).flatten()
        };
        if let Some(wake) = wake {
            wake.wake();
        }
    }
}

struct OwnerWake(Weak<Mutex<RequestReadiness>>);
impl Wake for OwnerWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        let Some(queue) = self.0.upgrade() else {
            return;
        };
        let wake = queue.lock().wake.clone();
        if let Some(wake) = wake {
            wake.wake();
        }
    }
}

struct BulkRequest {
    request: ActiveCancelRequest,
    decisions: Vec<Option<DecisionReceiver>>,
    remaining: usize,
    ready: Arc<Mutex<ReadyQueue>>,
}

struct BulkQueue {
    requests: Vec<Option<BulkRequest>>,
    free: Vec<usize>,
    count: usize,
    ready: Arc<Mutex<RequestReadiness>>,
}

type CompletedCancel = (ActiveCancelRequest, Vec<DecisionResult>);

impl BulkQueue {
    fn new(wake: Option<Waker>) -> Self {
        Self {
            requests: Vec::new(),
            free: Vec::new(),
            count: 0,
            ready: Arc::new(Mutex::new(RequestReadiness {
                entries: VecDeque::new(),
                slots: Vec::new(),
                wake,
            })),
        }
    }

    fn push(&mut self, mut request: ActiveCancelRequest) {
        let id = {
            let mut ready = self.ready.lock();
            let index = if let Some(index) = self.free.pop() {
                index
            } else {
                let index = self.requests.len();
                self.requests.push(None);
                ready.slots.push(RequestSlot {
                    generation: 0,
                    active: false,
                    queued: false,
                });
                index
            };
            let slot = &mut ready.slots[index];
            slot.generation = slot
                .generation
                .checked_add(1)
                .expect("cancel request generation exhausted");
            slot.active = true;
            slot.queued = false;
            let id = RequestId {
                index,
                generation: slot.generation,
            };
            ready.push(id);
            id
        };
        let decisions: Vec<_> = request.pending_decisions.drain(..).map(Some).collect();
        let remaining = decisions.len();
        let notify = Waker::from(Arc::new(RequestWake {
            queue: Arc::downgrade(&self.ready),
            id,
        }));
        self.requests[id.index] = Some(BulkRequest {
            request,
            decisions,
            remaining,
            ready: ReadyQueue::new(remaining, notify),
        });
        self.count += 1;
    }

    fn take_ready(&mut self) -> (Vec<CompletedCancel>, bool) {
        let count = self
            .ready
            .lock()
            .entries
            .len()
            .min(MAX_ACTION_SERVER_REQUESTS_PER_SPIN);
        let mut completed = Vec::new();
        for _ in 0..count {
            let id = {
                let mut ready = self.ready.lock();
                let Some(id) = ready.entries.pop_front() else {
                    break;
                };
                let slot = &mut ready.slots[id.index];
                if !slot.active || slot.generation != id.generation {
                    continue;
                }
                slot.queued = false;
                id
            };
            let request = self.requests[id.index].as_mut().unwrap();
            let indices = request.ready.lock().take();
            for index in indices {
                let Some(decision) = request.decisions[index].as_mut() else {
                    continue;
                };
                // Already completed decisions require no readiness allocation.
                let mut context = Context::from_waker(noop_waker_ref());
                let result = match Pin::new(&mut *decision).poll(&mut context) {
                    Poll::Ready(result) => Poll::Ready(result),
                    Poll::Pending => {
                        let wake = ReadyQueue::waker(&request.ready, index);
                        Pin::new(&mut *decision).poll(&mut Context::from_waker(&wake))
                    }
                };
                if let Poll::Ready(result) = result {
                    request.decisions[index] = None;
                    request.remaining -= 1;
                    request.request.completed_decisions.push(result);
                }
            }
            if request.remaining == 0 {
                let mut request = self.requests[id.index].take().unwrap();
                self.ready.lock().slots[id.index].active = false;
                self.free.push(id.index);
                self.count -= 1;
                let results = std::mem::take(&mut request.request.completed_decisions);
                completed.push((request.request, results));
            } else if request.ready.lock().has_ready() {
                self.ready.lock().push(id);
            }
        }
        let more = !self.ready.lock().entries.is_empty();
        (completed, more)
    }
}

#[derive(Default)]
pub(crate) struct ActiveCancelQueue {
    small: crate::notifications::NotificationQueue<ActiveCancelRequest>,
    bulk: Option<Box<BulkQueue>>,
}

impl ActiveCancelQueue {
    pub(super) fn set_wake(&mut self, wake: Option<Waker>) {
        self.small.set_wake(wake.clone());
        if let Some(bulk) = &self.bulk {
            bulk.ready.lock().wake = wake;
        }
    }

    pub(super) fn wake(&self) -> Option<Waker> {
        self.small.wake()
    }

    fn ensure_bulk(&mut self) -> &mut BulkQueue {
        if self.bulk.is_none() {
            let mut bulk = BulkQueue::new(self.small.wake());
            for request in self.small.drain(..) {
                bulk.push(request);
            }
            self.bulk = Some(Box::new(bulk));
        }
        self.bulk.as_mut().unwrap()
    }

    pub(super) fn delivery_wake(&mut self) -> Waker {
        Waker::from(Arc::new(OwnerWake(Arc::downgrade(&self.ensure_bulk().ready))))
    }

    pub(super) fn push_back(&mut self, request: ActiveCancelRequest) {
        if self.bulk.is_some()
            || request.pending_decisions.len() > MAX_ACTION_SERVER_REQUESTS_PER_SPIN
            || self.small.len() >= MAX_ACTION_SERVER_REQUESTS_PER_SPIN
        {
            self.ensure_bulk().push(request);
        } else {
            self.small.push_back(request);
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.bulk
            .as_ref()
            .map_or_else(|| self.small.is_empty(), |bulk| bulk.count == 0)
    }

    pub(super) fn take_ready(&mut self) -> (Vec<CompletedCancel>, bool) {
        if let Some(bulk) = &mut self.bulk {
            bulk.take_ready()
        } else {
            let wake = self.small.wake();
            (
                poll_bounded_queue(&mut self.small, |request| {
                    poll_cancel_decisions_with_wake(request, wake.as_ref())
                }),
                false,
            )
        }
    }

    pub(super) fn release_idle_bulk(&mut self) {
        if self.is_empty() {
            self.bulk = None;
        }
    }

    #[cfg(test)]
    pub(super) fn has_runnable(&self) -> bool {
        self.bulk
            .as_ref()
            .is_some_and(|bulk| !bulk.ready.lock().entries.is_empty())
    }
}
