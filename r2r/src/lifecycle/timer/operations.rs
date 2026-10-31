//! Timer controls and native clock queries with ownership validation.

use std::time::Duration;

use super::LifecycleTimer;
use crate::{Error, Node, Result, Timer};

impl LifecycleTimer {
    fn with_running_timer<T>(
        &self, node: &Node, read: impl FnOnce(&Timer) -> Result<T>,
    ) -> Result<T> {
        self.state.with_timer(|timer| {
            timer.period(node)?;
            if self.state.enabled() {
                read(timer)
            } else {
                Err(Error::RCL_RET_TIMER_CANCELED)
            }
        })
    }

    /// Pause this timer immediately. The pause survives later node activation.
    pub fn cancel(&self, node: &mut Node) -> Result<()> {
        self.state.control(node, false)
    }

    /// Enable the timer and start a fresh period if its node is active.
    /// On an inactive node it stays natively canceled until activation.
    pub fn reset(&self, node: &mut Node) -> Result<()> {
        self.state.control(node, true)
    }

    /// Observes both explicit resets and activation-driven native restarts.
    pub fn observe_resets(&self, node: &mut Node) -> Result<crate::TimerResetEvents> {
        self.state.with_timer(|timer| timer.observe_resets(node))
    }

    /// Whether lifecycle or manual controls currently pause the timer.
    pub fn is_canceled(&self, node: &Node) -> Result<bool> {
        self.state
            .with_timer(|timer| Ok(timer.is_canceled(node)? || !self.state.enabled()))
    }

    /// Whether an enabled timer is ready now.
    pub fn is_ready(&self, node: &Node) -> Result<bool> {
        self.state
            .with_timer(|timer| Ok(timer.is_ready(node)? && self.state.enabled()))
    }

    /// Current native timer period.
    pub fn period(&self, node: &Node) -> Result<Duration> {
        self.state.with_timer(|timer| timer.period(node))
    }

    /// Change the native period and return its prior value.
    pub fn set_period(&self, node: &mut Node, period: Duration) -> Result<Duration> {
        self.state
            .with_timer(|timer| timer.set_period(node, period))
    }

    /// Time remaining in the native clock domain, zero when overdue.
    pub fn time_until_next_call(&self, node: &Node) -> Result<Duration> {
        self.with_running_timer(node, |timer| timer.time_until_next_call(node))
    }

    /// Signed nanoseconds remaining in the native clock domain.
    pub fn time_until_next_call_ns(&self, node: &Node) -> Result<i64> {
        self.with_running_timer(node, |timer| timer.time_until_next_call_ns(node))
    }

    /// Signed clock time since the previous call, including backward ROS jumps.
    pub fn time_since_last_call_ns(&self, node: &Node) -> Result<i64> {
        self.state
            .with_timer(|timer| timer.time_since_last_call_ns(node))
    }

    /// Absolute native-clock deadline for the next call.
    pub fn next_call_time_ns(&self, node: &Node) -> Result<i64> {
        self.with_running_timer(node, |timer| timer.next_call_time_ns(node))
    }
}
