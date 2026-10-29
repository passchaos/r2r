use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// Startup and repetition policy for a synchronous native timer callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackTimerOptions {
    pub autostart: bool,
    pub oneshot: bool,
}
impl Default for CallbackTimerOptions {
    fn default() -> Self {
        Self {
            autostart: true,
            oneshot: false,
        }
    }
}
impl CallbackTimerOptions {
    pub const fn autostart(mut self, autostart: bool) -> Self {
        self.autostart = autostart;
        self
    }
    pub const fn oneshot(mut self, oneshot: bool) -> Self {
        self.oneshot = oneshot;
        self
    }
}

/// Owns a synchronous timer registration. Drop stops future calls and schedules native cleanup.
pub struct CallbackTimer {
    id: uuid::Uuid,
    alive: Arc<AtomicBool>,
    clock_type: ClockType,
    canceled: Arc<Mutex<crate::notifications::NotificationQueue<uuid::Uuid>>>,
}
impl CallbackTimer {
    pub(super) fn new(
        id: uuid::Uuid, alive: Arc<AtomicBool>, clock_type: ClockType,
        canceled: Arc<Mutex<crate::notifications::NotificationQueue<uuid::Uuid>>>,
    ) -> Self {
        Self {
            id,
            alive,
            clock_type,
            canceled,
        }
    }
    pub const fn clock_type(&self) -> ClockType {
        self.clock_type
    }
    pub const fn is_steady(&self) -> bool {
        matches!(self.clock_type, ClockType::SteadyTime)
    }
    pub fn is_valid(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
    pub fn cancel(&self, node: &mut Node) -> Result<()> {
        check_rcl_ret(unsafe { rcl_timer_cancel(node.callback_timer_mut(self.id)?.handle_mut()) })
    }
    pub fn reset(&self, node: &mut Node) -> Result<()> {
        check_rcl_ret(unsafe { rcl_timer_reset(node.callback_timer_mut(self.id)?.handle_mut()) })
    }
    pub fn is_canceled(&self, node: &Node) -> Result<bool> {
        let mut value = false;
        check_rcl_ret(unsafe {
            rcl_timer_is_canceled(node.callback_timer(self.id)?.handle(), &mut value)
        })?;
        Ok(value)
    }
    pub fn is_ready(&self, node: &Node) -> Result<bool> {
        let mut value = false;
        check_rcl_ret(unsafe {
            rcl_timer_is_ready(node.callback_timer(self.id)?.handle(), &mut value)
        })?;
        Ok(value)
    }
    pub fn period(&self, node: &Node) -> Result<Duration> {
        let mut value = 0;
        check_rcl_ret(unsafe {
            rcl_timer_get_period(node.callback_timer(self.id)?.handle(), &mut value)
        })?;
        duration_from_nonnegative_nanos(value)
    }
    pub fn set_period(&self, node: &mut Node, period: Duration) -> Result<Duration> {
        let period = duration_to_rcl_nanos(period)?;
        let mut previous = 0;
        check_rcl_ret(unsafe {
            rcl_timer_exchange_period(
                node.callback_timer_mut(self.id)?.handle_mut(),
                period,
                &mut previous,
            )
        })?;
        duration_from_nonnegative_nanos(previous)
    }
    pub fn next_call_time_ns(&self, node: &Node) -> Result<i64> {
        let mut value = 0;
        check_rcl_ret(unsafe {
            rcl_timer_get_next_call_time(node.callback_timer(self.id)?.handle(), &mut value)
        })?;
        Ok(value)
    }
    pub fn time_until_next_call_ns(&self, node: &Node) -> Result<i64> {
        let mut value = 0;
        check_rcl_ret(unsafe {
            rcl_timer_get_time_until_next_call(node.callback_timer(self.id)?.handle(), &mut value)
        })?;
        Ok(value)
    }
    pub fn time_since_last_call_ns(&self, node: &Node) -> Result<i64> {
        let mut value = 0;
        check_rcl_ret(unsafe {
            rcl_timer_get_time_since_last_call(node.callback_timer(self.id)?.handle(), &mut value)
        })?;
        Ok(value)
    }
    pub(crate) fn identity(&self) -> uuid::Uuid {
        self.id
    }
    pub(crate) fn owner_wake(&self) -> Option<std::task::Waker> {
        self.canceled.lock().wake()
    }
}
impl Drop for CallbackTimer {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
        crate::notifications::enqueue(&self.canceled, self.id);
    }
}
