use super::Node;
use crate::{Context, Error, Result};
use r2r_rcl::*;
use std::{
    cell::UnsafeCell,
    future::Future,
    sync::{
        atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
        Arc, Weak,
    },
    task::{Context as TaskContext, Poll, Wake, Waker},
    time::{Duration, Instant},
};

/// Controls how a node waits while driving a Rust future.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FutureExecutionOptions {
    /// Periodic fallback for futures that do not notify their waker.
    /// `None` waits for ROS events or future notifications until the overall timeout.
    pub poll_interval: Option<Duration>,
}
impl Default for FutureExecutionOptions {
    fn default() -> Self {
        Self {
            poll_interval: Some(Duration::from_millis(10)),
        }
    }
}
impl FutureExecutionOptions {
    /// Selects notification-driven execution without periodic polling.
    /// External futures must wake when progress is possible. ROS availability, services,
    /// actions, subscriptions, timers and graph waiters are driven by this node's wait set.
    pub fn wake_driven() -> Self {
        Self {
            poll_interval: None,
        }
    }
    /// Sets a nonzero fallback interval for futures that need periodic polling.
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = Some(interval);
        self
    }
}

pub(super) struct FutureGuard {
    handle: Box<UnsafeCell<rcl_guard_condition_t>>,
    pub(super) active: AtomicBool,
    _context: Context,
    #[cfg(test)]
    waiting: AtomicBool,
    #[cfg(test)]
    wait_return: AtomicI32,
    #[cfg(test)]
    triggers: std::sync::atomic::AtomicUsize,
}
// RCL permits concurrent trigger calls and triggering while rcl_wait is blocked. The stable
// handle is initialized before sharing; Arc ownership excludes fini until every trigger ends.
// Only the owning Node adds it to a wait set, excluding concurrent waits with shared contents.
unsafe impl Send for FutureGuard {}
unsafe impl Sync for FutureGuard {}
impl FutureGuard {
    fn new(context: &Context) -> Result<Arc<Self>> {
        let handle =
            Box::new(UnsafeCell::new(unsafe { rcl_get_zero_initialized_guard_condition() }));
        let ret = {
            let mut ctx = context.context_handle.lock();
            unsafe {
                rcl_guard_condition_init(
                    handle.get(),
                    ctx.as_mut(),
                    rcl_guard_condition_get_default_options(),
                )
            }
        };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        let guard = Arc::new(Self {
            handle,
            active: AtomicBool::new(false),
            _context: context.clone(),
            #[cfg(test)]
            waiting: AtomicBool::new(false),
            #[cfg(test)]
            wait_return: AtomicI32::new(i32::MIN),
            #[cfg(test)]
            triggers: std::sync::atomic::AtomicUsize::new(0),
        });
        {
            let wake: Arc<dyn crate::context::ShutdownWake> = guard.clone();
            context
                .context_handle
                .lock()
                .register_shutdown_wake(Arc::downgrade(&wake));
        }
        Ok(guard)
    }
    pub(super) fn handle(&self) -> *mut rcl_guard_condition_t {
        self.handle.get()
    }
    #[cfg(test)]
    pub(super) fn before_wait(&self) {
        self.waiting.store(true, Ordering::Release);
    }
    #[cfg(test)]
    pub(super) fn after_wait(&self, ret: i32) {
        self.wait_return.store(ret, Ordering::Release);
        self.waiting.store(false, Ordering::Release);
    }
}
impl crate::context::ShutdownWake for FutureGuard {
    fn wake_shutdown(&self) {
        if self.active.load(Ordering::Acquire) {
            let ret = unsafe { rcl_trigger_guard_condition(self.handle()) };
            if ret != RCL_RET_OK as i32 {
                unsafe {
                    rcutils_reset_error();
                }
            }
        }
    }
}
impl Drop for FutureGuard {
    fn drop(&mut self) {
        let ret = unsafe { rcl_guard_condition_fini(self.handle.get()) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize future wake guard: {ret}");
        }
    }
}

const ARMED: u8 = 1;
const NOTIFIED: u8 = 2;
const WAITING: u8 = 4;
pub(super) struct FutureWake {
    guard: Weak<FutureGuard>,
    flags: AtomicU8,
    error: AtomicI32,
}
impl FutureWake {
    pub(super) fn finish_native_wait(&self) {
        self.flags.fetch_and(!WAITING, Ordering::AcqRel);
    }
}
impl Wake for FutureWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        let previous = self.flags.fetch_or(NOTIFIED, Ordering::AcqRel);
        if previous & ARMED == 0 || previous & NOTIFIED != 0 || previous & WAITING == 0 {
            return;
        }
        let Some(guard) = self.guard.upgrade() else {
            return;
        };
        #[cfg(test)]
        guard.triggers.fetch_add(1, Ordering::Relaxed);
        let ret = unsafe { rcl_trigger_guard_condition(guard.handle()) };
        if ret != RCL_RET_OK as i32 {
            let _ = self
                .error
                .compare_exchange(0, ret, Ordering::AcqRel, Ordering::Acquire);
            unsafe {
                rcutils_reset_error();
            }
        }
    }
}
struct FutureRun {
    guard: Arc<FutureGuard>,
    wake: Arc<FutureWake>,
}
impl Drop for FutureRun {
    fn drop(&mut self) {
        self.wake.flags.fetch_and(!ARMED, Ordering::AcqRel);
        self.guard.active.store(false, Ordering::Release);
    }
}

impl Node {
    /// Drives a Rust future and ROS callbacks until completion or a steady-clock timeout.
    /// Already-ready futures complete even with a zero timeout. Rust wake notifications can
    /// interrupt the native wait; a 10 ms fallback also supports futures that do not wake.
    pub fn spin_until_future_complete<T>(
        &mut self, future: impl Future<Output = Result<T>>, timeout: Duration,
    ) -> Result<T> {
        self.spin_until_future_complete_with_options(
            future,
            timeout,
            FutureExecutionOptions::default(),
        )
    }

    /// Drives a future with explicit periodic-polling or notification-driven waiting options.
    ///
    /// This accepts non-Send futures and works without an async runtime. Notifications are
    /// coalesced, native wake storage is reused, and stale wakers do not retain a node/context
    /// or complete a later future. ROS failures and the future's error are propagated.
    pub fn spin_until_future_complete_with_options<T>(
        &mut self, future: impl Future<Output = Result<T>>, timeout: Duration,
        options: FutureExecutionOptions,
    ) -> Result<T> {
        if options
            .poll_interval
            .is_some_and(|interval| interval.is_zero())
        {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        futures::pin_mut!(future);
        let started = Instant::now();
        // Preserve the allocation-free already-ready path and zero-timeout behavior.
        let mut initial = TaskContext::from_waker(futures::task::noop_waker_ref());
        if let Poll::Ready(result) = future.as_mut().poll(&mut initial) {
            return result;
        }
        if timeout.saturating_sub(started.elapsed()).is_zero() {
            return Err(Error::RCL_RET_TIMEOUT);
        }
        let guard = match &self.future_wake_guard {
            Some(guard) => Arc::clone(guard),
            None => {
                let guard = FutureGuard::new(&self.context)?;
                self.future_wake_guard = Some(Arc::clone(&guard));
                guard
            }
        };
        let wake = Arc::new(FutureWake {
            guard: Arc::downgrade(&guard),
            flags: AtomicU8::new(ARMED),
            error: AtomicI32::new(0),
        });
        self.future_wake_state = Some(Arc::clone(&wake));
        guard.active.store(true, Ordering::Release);
        let run = FutureRun { guard, wake };
        let waker = Waker::from(Arc::clone(&run.wake));
        let mut context = TaskContext::from_waker(&waker);
        loop {
            run.wake
                .flags
                .fetch_and(!(NOTIFIED | WAITING), Ordering::AcqRel);
            if let Poll::Ready(result) = future.as_mut().poll(&mut context) {
                return result;
            }
            let error = run.wake.error.load(Ordering::Acquire);
            if error != 0 {
                return Err(Error::from_rcl_error(error));
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                return Err(Error::RCL_RET_TIMEOUT);
            }
            // One atomic state prevents lost-wake store/load races. Self-wakes still drain
            // ready ROS work with a zero wait, rather than starving the node.
            let previous = run.wake.flags.fetch_or(WAITING, Ordering::AcqRel);
            let wait = if previous & NOTIFIED != 0 {
                run.wake.flags.fetch_and(!WAITING, Ordering::AcqRel);
                Duration::ZERO
            } else {
                options
                    .poll_interval
                    .map_or(remaining, |interval| remaining.min(interval))
                    .min(Duration::from_nanos(i64::MAX as u64))
            };
            let result = self.try_spin_once(wait);
            run.wake.flags.fetch_and(!WAITING, Ordering::AcqRel);
            result?;
        }
    }
}

#[cfg(all(test, not(feature = "doc-only")))]
mod tests;
