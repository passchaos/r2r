use crate::lifecycle::timer::generation::Generation;
use crate::{CallbackTimer, Error, Node, Result};
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct CallbackTimerState {
    generation: Generation,
    closed: AtomicBool,
    timer: Mutex<Option<CallbackTimer>>,
}
impl CallbackTimerState {
    pub(super) fn new(active: bool, autostart: bool) -> Self {
        Self {
            generation: Generation::new(active, autostart),
            closed: AtomicBool::new(false),
            timer: Mutex::new(None),
        }
    }
    pub(super) fn attach(&self, timer: CallbackTimer) {
        *self.timer.lock() = Some(timer);
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
    }
    pub(super) fn enabled(&self) -> bool {
        !self.is_closed() && self.generation.enabled()
    }
    pub(crate) fn set_node_active(&self, active: bool) {
        if self.is_closed() || !self.generation.set_active(active) {
            return;
        }
        let wake = {
            self.timer
                .lock()
                .as_ref()
                .and_then(CallbackTimer::owner_wake)
        };
        if let Some(wake) = wake {
            wake.wake();
        }
    }
    pub(super) fn consume_oneshot(&self) {
        self.generation.set_manual(false, false);
    }
    pub(super) fn with_timer<T>(
        &self, read: impl FnOnce(&CallbackTimer) -> Result<T>,
    ) -> Result<T> {
        if self.is_closed() {
            return Err(Error::RCL_RET_TIMER_INVALID);
        }
        let timer = self.timer.lock();
        if self.is_closed() {
            return Err(Error::RCL_RET_TIMER_INVALID);
        }
        read(timer.as_ref().ok_or(Error::RCL_RET_TIMER_INVALID)?)
    }
    fn apply(&self, node: &mut Node, word: u64, timer: &CallbackTimer) -> Result<()> {
        if self.generation.matches(word) {
            return Ok(());
        }
        timer.cancel(node)?;
        if Generation::requested_enabled(word) {
            timer.reset(node)?;
        }
        self.generation.mark_applied(word);
        Ok(())
    }
    pub(crate) fn sync(&self, node: &mut Node) -> Result<()> {
        let word = self.generation.current();
        if self.generation.matches(word) {
            return Ok(());
        }
        self.with_timer(|timer| self.apply(node, word, timer))
    }
    pub(super) fn control(&self, node: &mut Node, enable: bool) -> Result<()> {
        self.with_timer(|timer| {
            timer.period(node)?;
            self.generation.set_manual(enable, enable);
            self.apply(node, self.generation.current(), timer)
        })
    }
    pub(super) fn destroy(&self, node: &mut Node) {
        let timer = self.timer.lock().take();
        self.close();
        if let Some(timer) = timer {
            node.destroy_callback_timer(timer);
        }
    }
}
