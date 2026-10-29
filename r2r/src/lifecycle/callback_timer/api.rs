use super::handler::Managed;
use super::*;
use crate::nodes::timer_callbacks::native::{Handler, Plain};
use crate::{CallbackTimerOptions, LifecycleNode, LifecycleState, Result};
use std::time::Duration;

impl LifecycleNode {
    fn managed_callback<H: Handler + Send + 'static>(
        &mut self, period: Duration, options: CallbackTimerOptions, clock_type: ClockType,
        handler: H,
    ) -> Result<LifecycleCallbackTimer> {
        let state = Arc::new(CallbackTimerState::new(
            self.lifecycle_state() == LifecycleState::Active,
            options.autostart,
        ));
        let timer = Node::register_callback_timer(
            self,
            period,
            options.autostart(false),
            clock_type,
            Managed {
                handler,
                state: Arc::downgrade(&state),
                oneshot: options.oneshot,
            },
        )?;
        state.attach(timer);
        if let Err(error) = state.sync(self) {
            state.destroy(self);
            return Err(error);
        }
        let weak = Arc::downgrade(&state);
        Node::register_managed_callback_timer(self, weak.clone());
        self.runtime_register_callback_timer(weak);
        Ok(LifecycleCallbackTimer { state, clock_type })
    }
    /// Native wall timer callback enabled only in the active lifecycle generation.
    pub fn create_wall_timer_callback<F: FnMut() -> Result<()> + Send + 'static>(
        &mut self, period: Duration, callback: F,
    ) -> Result<LifecycleCallbackTimer> {
        self.create_wall_timer_callback_with_options(
            period,
            CallbackTimerOptions::default(),
            callback,
        )
    }
    pub fn create_wall_timer_callback_with_options<F: FnMut() -> Result<()> + Send + 'static>(
        &mut self, period: Duration, options: CallbackTimerOptions, callback: F,
    ) -> Result<LifecycleCallbackTimer> {
        self.managed_callback(period, options, ClockType::SteadyTime, Plain(callback))
    }
    /// Native ROS timer callback enabled only in the active lifecycle generation.
    pub fn create_timer_callback<F: FnMut() -> Result<()> + Send + 'static>(
        &mut self, period: Duration, callback: F,
    ) -> Result<LifecycleCallbackTimer> {
        self.create_timer_callback_with_options(period, CallbackTimerOptions::default(), callback)
    }
    pub fn create_timer_callback_with_options<F: FnMut() -> Result<()> + Send + 'static>(
        &mut self, period: Duration, options: CallbackTimerOptions, callback: F,
    ) -> Result<LifecycleCallbackTimer> {
        self.managed_callback(period, options, ClockType::RosTime, Plain(callback))
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    pub fn create_wall_timer_callback_with_info<
        F: FnMut(crate::TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, period: Duration, callback: F,
    ) -> Result<LifecycleCallbackTimer> {
        self.create_wall_timer_callback_with_info_and_options(
            period,
            CallbackTimerOptions::default(),
            callback,
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    pub fn create_wall_timer_callback_with_info_and_options<
        F: FnMut(crate::TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, period: Duration, options: CallbackTimerOptions, callback: F,
    ) -> Result<LifecycleCallbackTimer> {
        self.managed_callback(
            period,
            options,
            ClockType::SteadyTime,
            crate::nodes::timer_callbacks::native::WithInfo(callback),
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    pub fn create_timer_callback_with_info<
        F: FnMut(crate::TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, period: Duration, callback: F,
    ) -> Result<LifecycleCallbackTimer> {
        self.create_timer_callback_with_info_and_options(
            period,
            CallbackTimerOptions::default(),
            callback,
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    pub fn create_timer_callback_with_info_and_options<
        F: FnMut(crate::TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, period: Duration, options: CallbackTimerOptions, callback: F,
    ) -> Result<LifecycleCallbackTimer> {
        self.managed_callback(
            period,
            options,
            ClockType::RosTime,
            crate::nodes::timer_callbacks::native::WithInfo(callback),
        )
    }
    pub fn destroy_callback_timer(&mut self, timer: LifecycleCallbackTimer) {
        timer.destroy(self);
    }
}
