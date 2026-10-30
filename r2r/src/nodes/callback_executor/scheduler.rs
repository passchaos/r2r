//! Coalesced readiness tokens transfer exclusive work ownership between workers.

use super::*;
use crossbeam_queue::ArrayQueue;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
    task::{Wake, Waker},
};

const IDLE: u8 = 0;
const QUEUED: u8 = 1;
const RUNNING: u8 = 2;
const NOTIFIED: u8 = 3;
const FINISHED: u8 = 4;

pub(super) enum Work {
    Native(Box<Executor>),
    Group(Box<CallbackGroup>),
}

#[derive(Debug, thiserror::Error)]
pub enum Failure {
    #[error("Native callback scheduler operation failed: {0}")]
    Native(#[source] Error),
    #[error("Callback group {id:?} failed: {source}")]
    Group { id: CallbackGroupId, source: Error },
    #[error("Callback scheduler worker panicked")]
    Panic,
}

#[repr(align(64))]
struct State(AtomicU8);

struct Slot {
    state: State,
    work: Mutex<Option<Work>>,
    wake: std::sync::OnceLock<Waker>,
}

pub(super) struct Scheduler {
    slots: Vec<Slot>,
    ready: ArrayQueue<usize>,
    stopped: AtomicBool,
    remaining: AtomicUsize,
    group_flags: Vec<Arc<AtomicBool>>,
    native_control: Option<ExecutorHandle>,
    native_context: Option<Context>,
    native_wake: Waker,
    worker_wakes: Vec<std::sync::OnceLock<std::thread::Thread>>,
}

struct SlotWake {
    scheduler: std::sync::Weak<Scheduler>,
    index: usize,
}
impl Wake for SlotWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        if let Some(scheduler) = self.scheduler.upgrade() {
            scheduler.notify(self.index);
        }
    }
}

// Work moves into this guard before dispatch. Its calling thread owns the value;
// no slot mutex remains held while polling callbacks or entering a native wait.
struct Owned<'a> {
    slot: &'a Slot,
    work: Option<Work>,
}
impl Drop for Owned<'_> {
    fn drop(&mut self) {
        *self.slot.work.lock() = self.work.take();
    }
}

impl Scheduler {
    pub(super) fn new(
        mut native: Option<Executor>, groups: Vec<CallbackGroup>, workers: usize,
    ) -> Result<Arc<Self>> {
        if workers == 0 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let (native_wake, native_context, native_control) = match &mut native {
            Some(native) => {
                let (wake, context) = native.callback_scheduler_wake()?;
                (wake, Some(context), Some(native.handle()))
            }
            None => (futures::task::noop_waker(), None, None),
        };
        let group_flags = groups.iter().map(|group| group.cancelled.clone()).collect();
        let precancelled = native.as_ref().is_some_and(Executor::is_cancelled)
            || groups
                .iter()
                .any(|group| group.cancelled.load(Ordering::Acquire));
        let work: Vec<_> = native
            .into_iter()
            .map(|native| Work::Native(Box::new(native)))
            .chain(groups.into_iter().map(|group| Work::Group(Box::new(group))))
            .collect();
        let count = work.len();
        let scheduler = Arc::new(Self {
            slots: work
                .into_iter()
                .map(|work| Slot {
                    state: State(AtomicU8::new(QUEUED)),
                    work: Mutex::new(Some(work)),
                    wake: std::sync::OnceLock::new(),
                })
                .collect(),
            ready: ArrayQueue::new(count.max(1)),
            stopped: AtomicBool::new(count == 0 || precancelled),
            remaining: AtomicUsize::new(count),
            group_flags,
            native_control,
            native_context,
            native_wake,
            worker_wakes: (0..workers).map(|_| std::sync::OnceLock::new()).collect(),
        });
        for index in 0..count {
            scheduler.slots[index]
                .wake
                .set(Waker::from(Arc::new(SlotWake {
                    scheduler: Arc::downgrade(&scheduler),
                    index,
                })))
                .expect("callback slot wake initialized twice");
        }
        // Prime callback wakers before an otherwise idle native wait can occupy
        // the only available thread in a one-worker configuration.
        let native_count = usize::from(scheduler.native_control.is_some());
        for index in (native_count..count).chain(0..native_count) {
            scheduler
                .ready
                .push(index)
                .expect("initial ready ring overflow");
        }
        if precancelled {
            scheduler.cancel()?;
        }
        Ok(scheduler)
    }

    pub(super) fn cancel(&self) -> Result<()> {
        self.stopped.store(true, Ordering::Release);
        for flag in &self.group_flags {
            flag.store(true, Ordering::Release);
        }
        let result = self
            .native_control
            .as_ref()
            .map_or(Ok(()), |control| control.cancel().map(|_| ()));
        self.wake_workers();
        result
    }

    fn wake_workers(&self) {
        for worker in &self.worker_wakes {
            if let Some(worker) = worker.get() {
                worker.unpark();
            }
        }
    }

    fn publish(&self, index: usize) {
        // Each slot has at most one queued token. Notifications while running
        // only set NOTIFIED, so capacity equal to slot count is sufficient.
        self.ready
            .push(index)
            .expect("coalesced ready ring overflow");
        // unpark has a persistent token, including when publication precedes
        // park. This avoids a global sleep mutex and a lost empty-check wake.
        self.wake_workers();
        // Callback work can reclaim the thread currently in rcl_wait. The same
        // cached guard stays owned by the exclusive native job across migration.
        if self.native_control.is_some() && index != 0 {
            self.native_wake.wake_by_ref();
        }
    }

    fn notify(&self, index: usize) {
        if self.stopped.load(Ordering::Acquire) {
            return;
        }
        let state = &self.slots[index].state.0;
        loop {
            match state.load(Ordering::Acquire) {
                IDLE => {
                    if state
                        .compare_exchange(IDLE, QUEUED, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        self.publish(index);
                        return;
                    }
                }
                RUNNING => {
                    if state
                        .compare_exchange(RUNNING, NOTIFIED, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        return;
                    }
                }
                QUEUED | NOTIFIED | FINISHED => return,
                _ => unreachable!("invalid callback readiness state"),
            }
        }
    }

    fn next(&self) -> Option<usize> {
        loop {
            if self.stopped.load(Ordering::Acquire) {
                return None;
            }
            if let Some(index) = self.ready.pop() {
                return Some(index);
            }
            std::thread::park();
        }
    }

    fn run_one(self: &Arc<Self>, index: usize) -> std::result::Result<(), Failure> {
        let slot = &self.slots[index];
        slot.state
            .0
            .compare_exchange(QUEUED, RUNNING, Ordering::AcqRel, Ordering::Acquire)
            .expect("ready token did not own its work slot");
        let work = slot
            .work
            .lock()
            .take()
            .expect("ready slot ownership missing");
        let mut owned = Owned {
            slot,
            work: Some(work),
        };
        let mut context = TaskContext::from_waker(slot.wake.get().unwrap());
        let finished = match owned.work.as_mut().unwrap() {
            Work::Native(native) => {
                let valid = self.native_context.as_ref().unwrap().is_valid();
                if !valid {
                    let _ = self.cancel();
                    true
                } else {
                    match native.spin_once(Duration::from_nanos(i64::MAX as u64)) {
                        Ok(()) => {
                            self.notify(index);
                            false
                        }
                        Err(Error::ExecutorCancelled) => {
                            let _ = self.cancel();
                            true
                        }
                        Err(Error::RCL_RET_NOT_INIT)
                            if !self.native_context.as_ref().unwrap().is_valid() =>
                        {
                            let _ = self.cancel();
                            true
                        }
                        Err(error) => return Err(Failure::Native(error)),
                    }
                }
            }
            Work::Group(group) => match group.poll_callbacks(&mut context) {
                Poll::Pending => false,
                Poll::Ready(Ok(())) => true,
                Poll::Ready(Err(source)) => {
                    return Err(Failure::Group {
                        id: group.id,
                        source,
                    })
                }
            },
        };
        drop(owned);
        if finished {
            slot.state.0.store(FINISHED, Ordering::Release);
            if self.remaining.fetch_sub(1, Ordering::AcqRel) == 1 {
                let _ = self.cancel();
            }
        } else {
            match slot
                .state
                .0
                .compare_exchange(RUNNING, IDLE, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => {}
                Err(NOTIFIED) => {
                    slot.state.0.store(QUEUED, Ordering::Release);
                    self.publish(index);
                }
                Err(_) => unreachable!("running work lost exclusive readiness state"),
            }
        }
        Ok(())
    }

    pub(super) fn run(self: &Arc<Self>, worker: usize) -> std::result::Result<(), Failure> {
        self.worker_wakes[worker]
            .set(std::thread::current())
            .expect("worker started twice");
        while let Some(index) = self.next() {
            let result = catch_unwind(AssertUnwindSafe(|| self.run_one(index)))
                .unwrap_or(Err(Failure::Panic));
            if let Err(error) = result {
                let _ = self.cancel();
                return Err(error);
            }
        }
        Ok(())
    }

    /// Call only after collecting every worker; slot values remain exclusively
    /// owned even after cancellation and callback unwinding.
    pub(super) fn recover(&self) -> (Option<Executor>, Vec<CallbackGroup>) {
        let mut native = None;
        let mut groups = Vec::new();
        for slot in &self.slots {
            match slot
                .work
                .lock()
                .take()
                .expect("completed work ownership missing")
            {
                Work::Native(executor) => native = Some(*executor),
                Work::Group(group) => groups.push(*group),
            }
        }
        (native, groups)
    }
}
