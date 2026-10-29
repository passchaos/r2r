//! Lifecycle generations and exclusive access to the ordinary timer inbox.

use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use futures::task::AtomicWaker;
use parking_lot::{Mutex, MutexGuard};

use crate::{Error, Node, Result, Timer};

use super::generation::Generation;

pub(crate) struct LifecycleTimerState {
    generation: Generation,
    closed: AtomicBool,
    timer: Mutex<Option<Timer>>,
    waker: AtomicWaker,
}

impl LifecycleTimerState {
    pub(super) fn new(timer: Timer, node_active: bool, autostart: bool) -> Self {
        Self {
            generation: Generation::new(node_active, autostart),
            closed: AtomicBool::new(false),
            timer: Mutex::new(Some(timer)),
            waker: AtomicWaker::new(),
        }
    }

    pub(crate) fn set_node_active(&self, active: bool) {
        self.generation.set_active(active);
        self.waker.wake();
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.waker.wake();
    }

    pub(super) fn enabled(&self) -> bool {
        self.generation.enabled()
    }

    pub(super) fn with_timer<T>(&self, read: impl FnOnce(&Timer) -> Result<T>) -> Result<T> {
        if self.is_closed() {
            return Err(Error::RCL_RET_TIMER_INVALID);
        }
        let timer = self.timer.lock();
        read(timer.as_ref().ok_or(Error::RCL_RET_TIMER_INVALID)?)
    }

    fn apply(
        &self, node: &mut Node, word: u64, inbox: &mut MutexGuard<'_, Option<Timer>>,
    ) -> Result<()> {
        if self.generation.matches(word) {
            return Ok(());
        }
        let timer = inbox.as_mut().ok_or(Error::RCL_RET_TIMER_INVALID)?;
        // Stop the sole producer before draining; restart from this clock
        // instant rather than carrying an old deadline across inactivity.
        timer.cancel(node)?;
        timer.discard_pending_ticks();
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
        let mut inbox = self.timer.lock();
        let result = self.apply(node, word, &mut inbox);
        drop(inbox);
        self.waker.wake();
        result
    }

    pub(super) fn control(&self, node: &mut Node, enable: bool) -> Result<()> {
        if self.is_closed() {
            return Err(Error::RCL_RET_TIMER_INVALID);
        }
        let mut inbox = self.timer.lock();
        // Validate ownership before changing either native or logical state.
        inbox
            .as_ref()
            .ok_or(Error::RCL_RET_TIMER_INVALID)?
            .period(node)?;
        self.generation.set_manual(enable, enable);
        let word = self.generation.current();
        let result = self.apply(node, word, &mut inbox);
        drop(inbox);
        self.waker.wake();
        result
    }

    pub(super) fn poll<T>(
        &self, context: &mut Context<'_>,
        poll: impl FnOnce(&mut Timer, &mut Context<'_>) -> Poll<Result<T>>,
    ) -> Poll<Result<T>> {
        self.waker.register(context.waker());
        if self.is_closed() {
            return Poll::Ready(Err(Error::RCL_RET_TIMER_INVALID));
        }
        let mut inbox = self.timer.lock();
        let word = self.generation.current();
        if self.is_closed() {
            return Poll::Ready(Err(Error::RCL_RET_TIMER_INVALID));
        }
        if !Generation::requested_enabled(word) || !self.generation.matches(word) {
            return Poll::Pending;
        }
        match inbox.as_mut() {
            Some(timer) => poll(timer, context),
            None => Poll::Ready(Err(Error::RCL_RET_TIMER_INVALID)),
        }
    }

    pub(super) fn destroy(&self, node: &mut Node) {
        let timer = self.timer.lock().take();
        if let Some(timer) = timer {
            node.destroy_timer(timer);
        }
        self.close();
    }
}
