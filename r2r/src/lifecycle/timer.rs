//! Native timers that pause and resume with the managed node.

use std::sync::Arc;
use std::time::Duration;

use super::{LifecycleNode, LifecycleState};
use crate::{ClockType, Node, Result, Timer, TimerOptions};

pub(crate) mod generation;
mod operations;
pub(crate) mod state;
#[cfg(test)]
mod tests;

use state::LifecycleTimerState;

/// A timer enabled only when its lifecycle node is active and it is not manually paused.
///
/// Activation starts a fresh full period. Deactivation, cleanup, shutdown and
/// error recovery gate ticks immediately and pause the native timer before the
/// next wait. Buffered ticks from an earlier activation are discarded before
/// restart, including deactivate/reactivate commands issued between spins.
/// The owning node must be spun, either directly or through [`crate::Executor`].
///
/// `TimerOptions::autostart(false)` keeps the timer manually paused until
/// [`Self::reset`]. Cancellation stays in effect across lifecycle transitions.
/// A tick that observes the active state may complete during a concurrent
/// deactivation; deactivation does not retract already-delivered ticks.
pub struct LifecycleTimer {
    state: Arc<LifecycleTimerState>,
    clock_type: ClockType,
}

impl LifecycleTimer {
    fn new(timer: Timer, active: bool, autostart: bool) -> Self {
        let clock_type = timer.clock_type();
        Self {
            state: Arc::new(LifecycleTimerState::new(timer, active, autostart)),
            clock_type,
        }
    }

    /// Clock domain used by this timer.
    pub const fn clock_type(&self) -> ClockType {
        self.clock_type
    }

    /// Whether the timer uses a monotonic steady clock.
    pub const fn is_steady(&self) -> bool {
        matches!(self.clock_type, ClockType::SteadyTime)
    }

    /// Wait for a tick in the current activation, returning elapsed clock time.
    /// Remains pending while paused; destroyed native timers report invalidity.
    /// Elapsed time follows RCL: it is measured since the last actual call and
    /// can include paused time, even though reset restarts the deadline.
    pub async fn tick(&mut self) -> Result<Duration> {
        futures::future::poll_fn(|context| self.state.poll(context, Timer::poll_tick)).await
    }

    /// Wait for native timing metadata in the current activation.
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    pub async fn tick_with_info(&mut self) -> Result<crate::TimerCallInfo> {
        futures::future::poll_fn(|context| self.state.poll(context, Timer::poll_tick_with_info))
            .await
    }

    /// Immediately destroy the owning node's native timer.
    /// Dropping the handle instead schedules ordinary cleanup on the next spin.
    pub fn destroy(self, node: &mut Node) {
        self.state.destroy(node);
    }
}

impl LifecycleNode {
    fn manage_timer(&mut self, timer: Timer, autostart: bool) -> Result<LifecycleTimer> {
        let managed =
            LifecycleTimer::new(timer, self.lifecycle_state() == LifecycleState::Active, autostart);
        if let Err(error) = managed.state.sync(self) {
            managed.state.destroy(self);
            return Err(error);
        }
        let weak = Arc::downgrade(&managed.state);
        Node::register_managed_timer(self, weak.clone());
        LifecycleNode::register_managed_timer(self, weak);
        Ok(managed)
    }

    /// Create a lifecycle-managed monotonic wall timer.
    pub fn create_wall_timer(&mut self, period: Duration) -> Result<LifecycleTimer> {
        self.create_wall_timer_with_options(period, TimerOptions::default())
    }

    /// Create a wall timer with explicit buffering and manual startup policy.
    /// `autostart(false)` requires `reset` before any active-phase ticks.
    pub fn create_wall_timer_with_options(
        &mut self, period: Duration, options: TimerOptions,
    ) -> Result<LifecycleTimer> {
        let timer = Node::create_wall_timer_with_options(self, period, options.autostart(false))?;
        self.manage_timer(timer, options.autostart)
    }

    /// Create a lifecycle-managed timer driven by the node's ROS clock.
    pub fn create_timer(&mut self, period: Duration) -> Result<LifecycleTimer> {
        self.create_timer_with_options(period, TimerOptions::default())
    }

    /// Create a ROS-clock timer with explicit buffering and startup policy.
    pub fn create_timer_with_options(
        &mut self, period: Duration, options: TimerOptions,
    ) -> Result<LifecycleTimer> {
        let timer = Node::create_timer_with_options(self, period, options.autostart(false))?;
        self.manage_timer(timer, options.autostart)
    }
}
