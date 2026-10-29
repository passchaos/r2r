//! Shared ROS-clock deadlines, native wakes and caller-owned asynchronous waits.

use super::*;
use std::task::{Context as TaskContext, Poll, Waker};

mod heap;
mod native;
use heap::{Deadlines, Key};

const MAX_COMPLETIONS_PER_PASS: usize = 64;

#[derive(Default)]
struct State {
    deadlines: Deadlines,
    interrupted: VecDeque<heap::Entry>,
    closed: bool,
    dirty: bool,
    wake: Option<Waker>,
}

#[derive(Default)]
struct Shared {
    state: Mutex<State>,
}

impl Shared {
    fn changed(state: &mut State) -> Option<Waker> {
        let wake = (!state.dirty).then(|| state.wake.clone()).flatten();
        state.dirty = true;
        wake
    }
    fn cancel(&self, key: Key) -> bool {
        let (removed, wake) = {
            let mut state = self.state.lock();
            let removed = state.deadlines.remove(key);
            let wake = removed.as_ref().and_then(|_| Self::changed(&mut state));
            (removed, wake)
        };
        let canceled = removed.is_some();
        // Receiver and native wakes may reenter either this queue or the managed clock.
        if let Some(entry) = removed {
            let _ = entry.sender.send(Err(Error::RosSleepCancelled));
        }
        if let Some(wake) = wake {
            wake.wake();
        }
        canceled
    }
    fn jump(&self, jump: TimeJump) {
        if jump.phase != TimeJumpPhase::After {
            return;
        }
        let wake = {
            let mut state = self.state.lock();
            if state.closed || state.deadlines.is_empty() {
                return;
            }
            if matches!(
                jump.clock_change,
                ClockChange::RosTimeActivated | ClockChange::RosTimeDeactivated
            ) {
                let State {
                    deadlines,
                    interrupted,
                    ..
                } = &mut *state;
                deadlines.drain_into(interrupted);
            }
            Self::changed(&mut state)
        };
        // Only the executor's internal native guard is bound here. Application completion is
        // deferred until dispatch releases the clock lock held around native jump callbacks.
        if let Some(wake) = wake {
            wake.wake();
        }
    }
    fn close(&self, error: Error) {
        let entries = self.detach_all();
        for entry in entries {
            let _ = entry.sender.send(Err(error.clone()));
        }
    }
    fn detach_all(&self) -> VecDeque<heap::Entry> {
        let mut state = self.state.lock();
        state.closed = true;
        let State {
            deadlines,
            interrupted,
            ..
        } = &mut *state;
        deadlines.drain_into(interrupted);
        std::mem::take(interrupted)
    }
}

impl crate::context::ShutdownWake for Shared {
    fn wake_shutdown(&self) {}
    fn after_shutdown(&self) {
        self.close(Error::RCL_RET_NOT_INIT);
    }
}

/// A cancellable wait until an absolute signed ROS-clock deadline is reached.
///
/// The owning Node/shared/background executor must be driven. Simulated time can pause this
/// indefinitely; backwards jumps retain the absolute target. A change between system-backed
/// ROS time and simulated ROS time fails with [`Error::RosTimeSourceChanged`]. Dropping this
/// future immediately removes its deadline and wakes the current executor to rearm its one
/// shared native timer. It allocates no per-wait native timer or clock-jump callback.
pub struct RosSleep {
    deadline: i64,
    shared: Option<Arc<Shared>>,
    key: Option<Key>,
    receiver: Option<oneshot::Receiver<Result<()>>>,
    immediate: Option<Result<()>>,
}

impl RosSleep {
    fn ready(deadline: i64) -> Self {
        Self {
            deadline,
            shared: None,
            key: None,
            receiver: None,
            immediate: Some(Ok(())),
        }
    }
    pub fn deadline_ns(&self) -> i64 {
        self.deadline
    }

    /// Cancel this wait independently from other sleeps or the node/context.
    /// Returns false if completion or interruption has already committed.
    pub fn cancel(&mut self) -> bool {
        self.shared
            .as_ref()
            .zip(self.key)
            .is_some_and(|(shared, key)| shared.cancel(key))
    }
}

impl Future for RosSleep {
    type Output = Result<()>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Result<()>> {
        if let Some(result) = self.immediate.take() {
            return Poll::Ready(result);
        }
        let receiver = self
            .receiver
            .as_mut()
            .expect("RosSleep polled after completion");
        match Pin::new(receiver).poll(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                self.key = None;
                self.receiver = None;
                Poll::Ready(result.unwrap_or(Err(Error::RCL_RET_NODE_INVALID)))
            }
        }
    }
}

impl Drop for RosSleep {
    fn drop(&mut self) {
        if let Some((shared, key)) = self.shared.as_ref().zip(self.key.take()) {
            shared.cancel(key);
        }
    }
}

pub(super) struct RosSleeps {
    timer: native::SleepTimer,
    shared: Arc<Shared>,
    clock: Arc<Mutex<Clock>>,
    callback: Option<TimeJumpCallbackId>,
    armed: Option<i64>,
}

impl RosSleeps {
    fn create(node: &Node) -> Result<Self> {
        let clock = node.ros_clock.clone();
        let timer = native::SleepTimer::create(node)?;
        let shared = Arc::new(Shared::default());
        let weak = Arc::downgrade(&shared);
        let callback = {
            let mut clock = clock.lock();
            clock.add_jump_callback(
                TimeJumpThreshold::default()
                    .on_clock_change(true)
                    .min_forward(Duration::from_nanos(1))
                    .min_backward(Duration::from_nanos(1)),
                move |jump| {
                    if let Some(shared) = weak.upgrade() {
                        shared.jump(jump);
                    }
                },
            )
        }?;
        let wake: Arc<dyn crate::context::ShutdownWake> = shared.clone();
        let result = Self {
            shared,
            clock,
            timer,
            callback: Some(callback),
            armed: None,
        };
        let registered = {
            let mut context = node.context.context_handle.lock();
            if !unsafe { rcl_context_is_valid(context.as_mut()) } {
                false
            } else {
                context.register_shutdown_wake(Arc::downgrade(&wake));
                true
            }
        };
        // Drop rollback outside the context lock: native timer and callback cleanup acquire
        // the managed clock mutex, whose normal creation order is clock then context.
        if !registered {
            return Err(Error::RCL_RET_NOT_INIT);
        }
        Ok(result)
    }
    fn register(&self, deadline: i64) -> Result<RosSleep> {
        let clock = self.clock.lock();
        if clock.get_now_ns()? >= deadline {
            return Ok(RosSleep::ready(deadline));
        }
        let (sender, receiver) = oneshot::channel();
        let (key, wake) = {
            let mut state = self.shared.state.lock();
            if state.closed {
                return Err(Error::RCL_RET_NOT_INIT);
            }
            let key = state.deadlines.insert(deadline, sender)?;
            (key, Shared::changed(&mut state))
        };
        drop(clock);
        if let Some(wake) = wake {
            wake.wake();
        }
        Ok(RosSleep {
            deadline,
            shared: Some(self.shared.clone()),
            key: Some(key),
            receiver: Some(receiver),
            immediate: None,
        })
    }
    pub(super) fn has_work(&self) -> bool {
        let state = self.shared.state.lock();
        !state.deadlines.is_empty() || !state.interrupted.is_empty()
    }
    pub(super) fn set_wake(&self, wake: Option<Waker>) {
        self.shared.state.lock().wake = wake;
    }
    pub(super) fn handle(&self) -> *const rcl_timer_t {
        self.timer.handle()
    }

    pub(super) fn sync(&mut self, native_ready: bool) -> Result<bool> {
        // Stable deadlines do not tax unrelated traffic with clock queries or heap scans.
        // A native timer or clock-jump notification makes the next reconciliation necessary.
        if !native_ready && !self.shared.state.lock().dirty {
            return Ok(false);
        }
        let clock = self.clock.lock();
        let now = clock.get_now_ns()?;
        let simulated = clock.ros_time_override_enabled()?;
        let mut completed = Vec::new();
        let (target, continuation) = {
            let mut state = self.shared.state.lock();
            state.dirty = false;
            for _ in 0..MAX_COMPLETIONS_PER_PASS {
                if let Some(entry) = state.interrupted.pop_front() {
                    completed.push((entry.sender, Err(Error::RosTimeSourceChanged)));
                } else if let Some(entry) = state.deadlines.pop_due(now) {
                    completed.push((entry.sender, Ok(())));
                } else {
                    break;
                }
            }
            let continuation = !state.interrupted.is_empty()
                || state
                    .deadlines
                    .first()
                    .is_some_and(|deadline| deadline <= now);
            state.dirty = continuation;
            (state.deadlines.first().filter(|_| !simulated), continuation)
        };
        let timer_result = if target != self.armed || native_ready {
            match target {
                // Bound the native period, even for an absolute deadline at i64::MAX. The
                // steady timer wakes in stages while ROS clock comparisons keep the exact target.
                Some(deadline) => self
                    .timer
                    .arm(deadline.saturating_sub(now).clamp(1, 86_400_000_000_000)),
                None => self.timer.cancel(),
            }
        } else {
            Ok(())
        };
        if timer_result.is_ok() {
            self.armed = target;
        }
        drop(clock);
        let progress = !completed.is_empty() || continuation;
        for (sender, result) in completed {
            let _ = sender.send(result);
        }
        timer_result?;
        Ok(progress)
    }

    pub(super) fn close(&mut self) -> VecDeque<heap::Entry> {
        if let Some(callback) = self.callback.take() {
            if let Err(error) = self.clock.lock().remove_jump_callback(callback) {
                log::error!("could not remove ROS sleep jump callback: {error}");
            }
        }
        self.shared.detach_all()
    }
}

impl Drop for RosSleeps {
    fn drop(&mut self) {
        let entries = self.close();
        for entry in entries {
            let _ = entry.sender.send(Err(Error::RCL_RET_NODE_INVALID));
        }
    }
}

impl Node {
    /// Wait until this node's ROS clock reaches `deadline_ns`, including signed simulated time.
    pub fn sleep_until_ros(&mut self, deadline_ns: i64) -> Result<RosSleep> {
        if !self.context.is_valid() {
            return Err(Error::RCL_RET_NOT_INIT);
        }
        if self.ros_clock.lock().get_now_ns()? >= deadline_ns {
            return Ok(RosSleep::ready(deadline_ns));
        }
        if self.ros_sleeps.is_none() {
            self.ros_sleeps = Some(Box::new(RosSleeps::create(self)?));
        }
        self.ros_sleeps.as_ref().unwrap().register(deadline_ns)
    }
    /// Wait a relative duration in ROS time. Paused simulated time does not consume the duration.
    pub fn sleep_for_ros(&mut self, duration: Duration) -> Result<RosSleep> {
        let nanos = duration_to_rcl_nanos(duration)?;
        let now = self.now_ns()?;
        let deadline = now
            .checked_add(nanos)
            .ok_or(Error::RCL_RET_INVALID_ARGUMENT)?;
        self.sleep_until_ros(deadline)
    }
}

#[cfg(test)]
mod tests;
