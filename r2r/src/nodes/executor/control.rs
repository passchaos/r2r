use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Weak,
};

#[derive(Debug)]
pub(super) struct ExecutorControl {
    cancelled: AtomicBool,
    guard: Mutex<Weak<FutureGuard>>,
}

impl ExecutorControl {
    pub(super) fn new() -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            guard: Mutex::new(Weak::new()),
        }
    }

    pub(super) fn attach_guard(&self, guard: &Arc<FutureGuard>) {
        *self.guard.lock() = Arc::downgrade(guard);
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub(super) fn reset(&self) {
        self.cancelled.store(false, Ordering::Release);
    }

    pub(super) fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::ExecutorCancelled)
        } else {
            Ok(())
        }
    }

    fn cancel(&self) -> Result<()> {
        // A persistent request cannot be lost before guard creation/registration. Subsequent
        // requests need no lock or native trigger until the owner explicitly resets it.
        if self.cancelled.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let guard = self.guard.lock().upgrade();
        if let Some(guard) = guard {
            if guard.active.load(Ordering::Acquire) {
                #[cfg(test)]
                guard.triggers.fetch_add(1, Ordering::Relaxed);
                let ret = unsafe { rcl_trigger_guard_condition(guard.handle()) };
                if ret != RCL_RET_OK as i32 {
                    unsafe {
                        rcutils_reset_error();
                    }
                    return Err(Error::from_rcl_error(ret));
                }
            }
        }
        Ok(())
    }
}

/// A clonable, thread-safe cancellation handle that does not keep its executor/context alive.
///
/// Cancellation stops an active or subsequent spin/future operation until the owning executor
/// calls `reset_cancel`. Repeated requests coalesce to one native wake. The ROS context and
/// owned nodes remain available for reuse.
#[derive(Debug, Clone)]
pub struct ExecutorHandle {
    control: Weak<ExecutorControl>,
}

impl ExecutorHandle {
    pub(super) fn new(control: &Arc<ExecutorControl>) -> Self {
        Self {
            control: Arc::downgrade(control),
        }
    }

    /// Requests cancellation. Returns `false` if the executor has already been dropped.
    pub fn cancel(&self) -> Result<bool> {
        let Some(control) = self.control.upgrade() else {
            return Ok(false);
        };
        control.cancel()?;
        Ok(true)
    }

    /// Observes the current cancellation state; an expired handle reports `false`.
    pub fn is_cancelled(&self) -> bool {
        self.control
            .upgrade()
            .is_some_and(|control| control.is_cancelled())
    }
}
