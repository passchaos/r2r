//! Optional native reconciliation for lifecycle timer controls.

use std::sync::Weak;

use crate::lifecycle::callback_timer::state::CallbackTimerState;
use crate::lifecycle::timer::state::LifecycleTimerState;
use crate::{Node, Result};

#[derive(Default)]
pub(super) struct ManagedTimers {
    timers: Vec<Weak<LifecycleTimerState>>,
    callbacks: Vec<Weak<CallbackTimerState>>,
}

impl Drop for ManagedTimers {
    fn drop(&mut self) {
        for timer in self.timers.iter().filter_map(Weak::upgrade) {
            timer.close();
        }
        for timer in self.callbacks.iter().filter_map(Weak::upgrade) {
            timer.close();
        }
    }
}

impl Node {
    pub(crate) fn register_managed_callback_timer(&mut self, timer: Weak<CallbackTimerState>) {
        self.managed_timers
            .get_or_insert_with(Default::default)
            .callbacks
            .push(timer);
    }
    pub(crate) fn register_managed_timer(&mut self, timer: Weak<LifecycleTimerState>) {
        self.managed_timers
            .get_or_insert_with(Default::default)
            .timers
            .push(timer);
    }

    pub(in crate::nodes) fn sync_managed_timers(&mut self) -> Result<()> {
        let Some(mut registry) = self.managed_timers.take() else {
            return Ok(());
        };
        let mut error = None;
        registry.timers.retain(|weak| match weak.upgrade() {
            Some(timer) if !timer.is_closed() => {
                if let Err(failure) = timer.sync(self) {
                    error.get_or_insert(failure);
                }
                true
            }
            _ => false,
        });
        registry.callbacks.retain(|weak| match weak.upgrade() {
            Some(timer) if !timer.is_closed() => {
                if let Err(failure) = timer.sync(self) {
                    error.get_or_insert(failure);
                }
                true
            }
            _ => false,
        });
        if !registry.timers.is_empty() || !registry.callbacks.is_empty() {
            self.managed_timers = Some(registry);
        }
        match error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

#[cfg(all(test, not(feature = "doc-only")))]
mod tests;
