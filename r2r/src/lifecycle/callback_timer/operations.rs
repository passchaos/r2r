use super::LifecycleCallbackTimer;
use crate::{CallbackTimer, Error, Node, Result};
use std::time::Duration;

impl LifecycleCallbackTimer {
    fn running<T>(&self, node: &Node, read: impl FnOnce(&CallbackTimer) -> Result<T>) -> Result<T> {
        self.state.with_timer(|timer| {
            timer.period(node)?;
            if self.state.enabled() {
                read(timer)
            } else {
                Err(Error::RCL_RET_TIMER_CANCELED)
            }
        })
    }
    pub fn cancel(&self, node: &mut Node) -> Result<()> {
        self.state.control(node, false)
    }
    pub fn reset(&self, node: &mut Node) -> Result<()> {
        self.state.control(node, true)
    }
    /// Observes both explicit resets and activation-driven native restarts.
    pub fn observe_resets(&self, node: &mut Node) -> Result<crate::TimerResetEvents> {
        self.state.with_timer(|timer| timer.observe_resets(node))
    }
    pub fn is_canceled(&self, node: &Node) -> Result<bool> {
        self.state
            .with_timer(|timer| Ok(timer.is_canceled(node)? || !self.state.enabled()))
    }
    pub fn is_ready(&self, node: &Node) -> Result<bool> {
        self.state
            .with_timer(|timer| Ok(timer.is_ready(node)? && self.state.enabled()))
    }
    pub fn period(&self, node: &Node) -> Result<Duration> {
        self.state.with_timer(|timer| timer.period(node))
    }
    pub fn set_period(&self, node: &mut Node, period: Duration) -> Result<Duration> {
        self.state
            .with_timer(|timer| timer.set_period(node, period))
    }
    pub fn next_call_time_ns(&self, node: &Node) -> Result<i64> {
        self.running(node, |timer| timer.next_call_time_ns(node))
    }
    pub fn time_until_next_call_ns(&self, node: &Node) -> Result<i64> {
        self.running(node, |timer| timer.time_until_next_call_ns(node))
    }
    pub fn time_since_last_call_ns(&self, node: &Node) -> Result<i64> {
        self.state
            .with_timer(|timer| timer.time_since_last_call_ns(node))
    }
}
