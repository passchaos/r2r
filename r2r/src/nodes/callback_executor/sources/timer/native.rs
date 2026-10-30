use super::*;

pub(super) struct NativeTimer {
    pub(super) handle: Box<UnsafeCell<rcl_timer_t>>,
    pub(super) operations: Mutex<()>,
    pub(super) initialized: AtomicBool,
    pub(super) in_flight: AtomicBool,
    pub(super) clock: Arc<Mutex<Clock>>,
    pub(super) clock_type: ClockType,
    pub(super) oneshot: bool,
    pub(super) generation: Option<Generation>,
    pub(super) wake: Mutex<Option<Waker>>,
}
// SAFETY: the handle has stable interior-mutable storage. One Node owns native
// registration/waiting. Selected advancement and finalization use operations;
// the clock outlives the timer and no user callback holds either mutex.
unsafe impl Send for NativeTimer {}
unsafe impl Sync for NativeTimer {}
impl NativeTimer {
    pub(super) fn synchronize_locked(&self) -> Result<()> {
        if let Some(generation) = &self.generation {
            let word = generation.current();
            if !generation.matches(word) {
                check_rcl_ret(unsafe { rcl_timer_cancel(self.handle.get()) })?;
                if Generation::requested_enabled(word) {
                    check_rcl_ret(unsafe { rcl_timer_reset(self.handle.get()) })?;
                }
                generation.mark_applied(word);
            }
        }
        Ok(())
    }
    pub(super) fn synchronize(&self) -> Result<()> {
        self.with_handle(|_| {
            let _clock = self.clock.lock();
            self.synchronize_locked()
        })
    }
    pub(super) fn control(&self, enabled: bool) -> Result<()> {
        self.with_handle(|handle| {
            let _clock = self.clock.lock();
            if let Some(generation) = &self.generation {
                generation.set_manual(enabled, enabled);
                self.synchronize_locked()
            } else if enabled {
                check_rcl_ret(unsafe { rcl_timer_reset(handle) })
            } else {
                check_rcl_ret(unsafe { rcl_timer_cancel(handle) })
            }
        })?;
        self.wake_native();
        Ok(())
    }
    pub(super) fn with_handle<T>(
        &self, operation: impl FnOnce(*mut rcl_timer_t) -> Result<T>,
    ) -> Result<T> {
        let _operation = self.operations.lock();
        if !self.initialized.load(Ordering::Acquire) {
            return Err(Error::RCL_RET_TIMER_INVALID);
        }
        operation(self.handle.get())
    }
    pub(super) fn wake_native(&self) {
        let wake = self.wake.lock().clone();
        if let Some(wake) = wake {
            wake.wake();
        }
    }
    pub(super) fn destroy(&self) {
        let _operation = self.operations.lock();
        let _clock = self.clock.lock();
        if self.initialized.swap(false, Ordering::AcqRel) {
            let ret = unsafe { rcl_timer_fini(self.handle.get()) };
            if ret != RCL_RET_OK as i32 {
                log::error!("could not finalize grouped timer: {ret}");
            }
        }
    }
    pub(super) fn advance<H: Handler>(&self, selected: Option<u64>) -> Result<Option<H::Tick>> {
        let _operation = self.operations.lock();
        if !self.initialized.load(Ordering::Acquire) {
            return Ok(None);
        }
        let _clock = self.clock.lock();
        self.synchronize_locked()?;
        if let Some(generation) = &self.generation {
            if selected != Some(generation.current()) || !generation.enabled() {
                return Ok(None);
            }
        }
        let mut ready = false;
        check_rcl_ret(unsafe { rcl_timer_is_ready(self.handle.get(), &mut ready) })?;
        if !ready {
            return Ok(None);
        }
        let tick = H::advance(self.handle.get(), self.clock_type)?;
        if self.oneshot {
            check_rcl_ret(unsafe { rcl_timer_cancel(self.handle.get()) })?;
            if let Some(generation) = &self.generation {
                generation.set_manual(false, false);
                generation.mark_applied(generation.current());
            }
        }
        Ok(Some(tick))
    }
}
impl GroupedTimerActivation for NativeTimer {
    fn set_active(&self, active: bool) {
        if !self.initialized.load(Ordering::Acquire) {
            return;
        }
        if let Some(generation) = &self.generation {
            if generation.set_active(active) {
                self.wake_native();
            }
        }
    }
}
impl Drop for NativeTimer {
    fn drop(&mut self) {
        self.destroy();
    }
}
pub(super) struct Invocation<'a>(pub(super) &'a NativeTimer);
impl Drop for Invocation<'_> {
    fn drop(&mut self) {
        self.0.in_flight.store(false, Ordering::Release);
        self.0.wake_native();
    }
}
