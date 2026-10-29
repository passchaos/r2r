//! Lifecycle-gated native callbacks with no tick inbox or user-code control lock.
use crate::{ClockType, Node};
use std::sync::Arc;
mod api;
mod handler;
mod operations;
pub(crate) mod state;
use state::CallbackTimerState;

/// A native callback timer active only in its active lifecycle generation and manual state.
/// Activation starts a fresh period. One-shot completion remains paused until reset.
/// A callback that acquires an active permit may finish during concurrent deactivation.
pub struct LifecycleCallbackTimer {
    state: Arc<CallbackTimerState>,
    clock_type: ClockType,
}
impl LifecycleCallbackTimer {
    pub const fn clock_type(&self) -> ClockType {
        self.clock_type
    }
    pub const fn is_steady(&self) -> bool {
        matches!(self.clock_type, ClockType::SteadyTime)
    }
    pub fn is_valid(&self) -> bool {
        self.state
            .with_timer(|timer| Ok(timer.is_valid()))
            .unwrap_or(false)
    }
    pub fn destroy(self, node: &mut Node) {
        self.state.destroy(node);
    }
}

#[cfg(all(test, not(feature = "doc-only")))]
mod tests;
