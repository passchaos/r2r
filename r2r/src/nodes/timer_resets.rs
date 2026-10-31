//! Lossless, asynchronous observation of native timer resets.

use super::*;
use futures::{
    stream::{FusedStream, Stream},
    task::AtomicWaker,
};
#[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
use std::ffi::c_void;
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    sync::OnceLock,
    task::{Context as TaskContext, Poll},
};

/// One or more native timer resets accumulated since the preceding stream item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerReset {
    /// Number of resets coalesced into this item.
    pub count: usize,
    /// Total resets observed since native timer creation.
    pub total: usize,
}

/// A lossless stream of reset notifications for one native timer.
///
/// The first item includes resets that happened before observation began. If
/// resets occur faster than the stream is polled, they are coalesced in
/// [`TimerReset::count`] rather than dropped. Only one stream may observe a
/// timer at a time; dropping it permits a replacement stream, which resumes at
/// the previous stream's cursor without losing intervening resets.
///
/// Timer destruction delivers any pending item and then closes the stream.
pub struct TimerResetEvents {
    state: Arc<TimerResetState>,
    observed: usize,
}

impl TimerResetEvents {
    /// Total resets recorded for the timer so far.
    pub fn total(&self) -> usize {
        self.state.total.load(Ordering::Acquire)
    }

    /// Resets not yet delivered by this stream.
    pub fn pending(&self) -> usize {
        self.total().wrapping_sub(self.observed)
    }
}

impl Stream for TimerResetEvents {
    type Item = TimerReset;

    fn poll_next(
        mut self: Pin<&mut Self>, context: &mut TaskContext<'_>,
    ) -> Poll<Option<Self::Item>> {
        loop {
            let total = self.state.total.load(Ordering::Acquire);
            if total != self.observed {
                let event = TimerReset {
                    count: total.wrapping_sub(self.observed),
                    total,
                };
                self.observed = total;
                self.state.cursor.store(total, Ordering::Release);
                self.state.waker.take();
                return Poll::Ready(Some(event));
            }
            if self.state.closed.load(Ordering::Acquire) {
                self.state.waker.take();
                return Poll::Ready(None);
            }

            self.state.waker.register(context.waker());
            let updated = self.state.total.load(Ordering::Acquire);
            if updated != self.observed {
                continue;
            }
            if self.state.closed.load(Ordering::Acquire) {
                self.state.waker.take();
                return Poll::Ready(None);
            }
            return Poll::Pending;
        }
    }
}

impl FusedStream for TimerResetEvents {
    fn is_terminated(&self) -> bool {
        self.state.closed.load(Ordering::Acquire) && self.pending() == 0
    }
}

impl Drop for TimerResetEvents {
    fn drop(&mut self) {
        self.state.cursor.store(self.observed, Ordering::Release);
        // Take the old observer's waker before another observer can claim the
        // stream slot and register its own.
        self.state.waker.take();
        self.state.observer_active.store(false, Ordering::Release);
    }
}

pub(crate) struct TimerResetSlot {
    state: OnceLock<Arc<TimerResetState>>,
    #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
    resets_before_observation: AtomicUsize,
}

impl TimerResetSlot {
    pub(crate) const fn new() -> Self {
        Self {
            state: OnceLock::new(),
            #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
            resets_before_observation: AtomicUsize::new(0),
        }
    }

    fn state(&self) -> &Arc<TimerResetState> {
        self.state.get_or_init(|| {
            let state = TimerResetState::new();
            #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
            state.notify(self.resets_before_observation.swap(0, Ordering::AcqRel));
            state
        })
    }

    pub(crate) fn observe(&self, handle: *const rcl_timer_t) -> Result<TimerResetEvents> {
        self.state().observe(handle)
    }

    pub(crate) fn reset(&self, handle: *mut rcl_timer_t) -> Result<()> {
        check_rcl_ret(unsafe { rcl_timer_reset(handle) })?;
        #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
        match self.state.get() {
            Some(state) => state.notify(1),
            None => {
                self.resets_before_observation
                    .fetch_add(1, Ordering::AcqRel);
            }
        }
        Ok(())
    }

    pub(crate) fn close(&self, handle: *const rcl_timer_t) {
        if let Some(state) = self.state.get() {
            state.close(handle);
        }
    }
}

struct TimerResetState {
    total: AtomicUsize,
    cursor: AtomicUsize,
    closed: AtomicBool,
    observer_active: AtomicBool,
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    native_registered: AtomicBool,
    waker: AtomicWaker,
}

impl TimerResetState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            total: AtomicUsize::new(0),
            cursor: AtomicUsize::new(0),
            closed: AtomicBool::new(false),
            observer_active: AtomicBool::new(false),
            #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
            native_registered: AtomicBool::new(false),
            waker: AtomicWaker::new(),
        })
    }

    fn observe(self: &Arc<Self>, handle: *const rcl_timer_t) -> Result<TimerResetEvents> {
        if self
            .observer_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(Error::TimerResetAlreadyObserved);
        }

        if let Err(error) = self.register_native(handle) {
            self.observer_active.store(false, Ordering::Release);
            return Err(error);
        }

        Ok(TimerResetEvents {
            state: Arc::clone(self),
            observed: self.cursor.load(Ordering::Acquire),
        })
    }

    #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
    fn register_native(&self, _handle: *const rcl_timer_t) -> Result<()> {
        // These distributions predate rcl_timer_set_on_reset_callback. Every
        // r2r reset passes through `reset`, which provides identical counting.
        Ok(())
    }

    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    fn register_native(&self, handle: *const rcl_timer_t) -> Result<()> {
        if self.native_registered.load(Ordering::Acquire) {
            return Ok(());
        }
        check_rcl_ret(unsafe {
            rcl_timer_set_on_reset_callback(
                handle,
                Some(timer_reset_trampoline),
                (self as *const Self).cast::<c_void>(),
            )
        })?;
        self.native_registered.store(true, Ordering::Release);
        Ok(())
    }

    fn close(&self, handle: *const rcl_timer_t) {
        #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
        let _ = handle;
        #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
        if self.native_registered.swap(false, Ordering::AcqRel) {
            // Native ownership is serialized by the caller. Even if RCL
            // rejects clearing an already-invalid handle, finalization follows
            // before the owning Arc can be released.
            let _ = unsafe { rcl_timer_set_on_reset_callback(handle, None, std::ptr::null()) };
        }
        self.closed.store(true, Ordering::Release);
        self.wake_without_unwinding();
    }

    fn notify(&self, count: usize) {
        if count == 0 || self.closed.load(Ordering::Acquire) {
            return;
        }
        self.total.fetch_add(count, Ordering::AcqRel);
        self.wake_without_unwinding();
    }

    fn wake_without_unwinding(&self) {
        // A custom executor waker is application code. Never allow a panic in
        // it to cross the native RCL callback boundary.
        let _ = catch_unwind(AssertUnwindSafe(|| self.waker.wake()));
    }
}

#[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
unsafe extern "C" fn timer_reset_trampoline(user_data: *const c_void, count: usize) {
    if user_data.is_null() {
        return;
    }
    // SAFETY: every native owner clears the callback before finalizing the RCL
    // timer and releasing its Arc<TimerResetState>.
    let state = unsafe { &*user_data.cast::<TimerResetState>() };
    let _ = catch_unwind(AssertUnwindSafe(|| state.notify(count)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_events_state_allocation_is_lazy() {
        let slot = TimerResetSlot::new();
        assert!(slot.state.get().is_none());
        let state = slot.state();
        assert!(Arc::ptr_eq(state, slot.state.get().unwrap()));
    }
}
