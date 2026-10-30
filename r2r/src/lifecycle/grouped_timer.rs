//! Generation policy stays attached to grouped native timer storage after Node transfer.
use super::timer::generation::Generation;
mod reentrant;
use super::*;
use crate::nodes::timer_callbacks::native::{Handler, Plain};
use crate::{CallbackGroup, CallbackTimerOptions, ClockType, GroupedTimer, Node, Result};
use std::time::Duration;

impl LifecycleNode {
    fn grouped_callback<H>(
        &mut self, group: &mut CallbackGroup, period: Duration, options: CallbackTimerOptions,
        clock: ClockType, handler: H,
    ) -> Result<GroupedTimer>
    where
        H: Handler + Send + 'static,
        H::Tick: Send + 'static,
        H::Permit: Send + 'static,
    {
        let active = self.lifecycle_state() == LifecycleState::Active;
        let timer = Node::register_grouped_timer_with_generation(
            self,
            group,
            period,
            options,
            clock,
            handler,
            Some(Generation::new(active, options.autostart)),
        )?;
        self.runtime_register_grouped_timer(timer.activation()?);
        Ok(timer)
    }
    /// Registers a group wall callback enabled only in the active lifecycle generation.
    pub fn create_wall_timer_callback_in_group<F: FnMut() -> Result<()> + Send + 'static>(
        &mut self, group: &mut CallbackGroup, period: Duration, callback: F,
    ) -> Result<GroupedTimer> {
        self.create_wall_timer_callback_in_group_with_options(
            group,
            period,
            CallbackTimerOptions::default(),
            callback,
        )
    }
    /// Preserves manual startup/one-shot policy across lifecycle transitions.
    pub fn create_wall_timer_callback_in_group_with_options<
        F: FnMut() -> Result<()> + Send + 'static,
    >(
        &mut self, group: &mut CallbackGroup, period: Duration, options: CallbackTimerOptions,
        callback: F,
    ) -> Result<GroupedTimer> {
        self.grouped_callback(group, period, options, ClockType::SteadyTime, Plain(callback))
    }
    /// Registers a lifecycle-gated callback driven by the managed ROS clock.
    pub fn create_timer_callback_in_group<F: FnMut() -> Result<()> + Send + 'static>(
        &mut self, group: &mut CallbackGroup, period: Duration, callback: F,
    ) -> Result<GroupedTimer> {
        self.create_timer_callback_in_group_with_options(
            group,
            period,
            CallbackTimerOptions::default(),
            callback,
        )
    }
    /// Registers ROS-clock group work with lifecycle and manual generation policy.
    pub fn create_timer_callback_in_group_with_options<
        F: FnMut() -> Result<()> + Send + 'static,
    >(
        &mut self, group: &mut CallbackGroup, period: Duration, options: CallbackTimerOptions,
        callback: F,
    ) -> Result<GroupedTimer> {
        self.grouped_callback(group, period, options, ClockType::RosTime, Plain(callback))
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    /// Captures native timestamps only after an active generation permit is acquired.
    pub fn create_timer_callback_in_group_with_info<
        F: FnMut(crate::TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, group: &mut CallbackGroup, period: Duration, callback: F,
    ) -> Result<GroupedTimer> {
        self.grouped_callback(
            group,
            period,
            CallbackTimerOptions::default(),
            ClockType::RosTime,
            crate::nodes::timer_callbacks::native::WithInfo(callback),
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    /// Registers timestamped group ROS work with explicit native policy.
    pub fn create_timer_callback_in_group_with_info_and_options<
        F: FnMut(crate::TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, group: &mut CallbackGroup, period: Duration, options: CallbackTimerOptions,
        callback: F,
    ) -> Result<GroupedTimer> {
        self.grouped_callback(
            group,
            period,
            options,
            ClockType::RosTime,
            crate::nodes::timer_callbacks::native::WithInfo(callback),
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    /// Registers timestamped group wall work in the active lifecycle generation.
    pub fn create_wall_timer_callback_in_group_with_info<
        F: FnMut(crate::TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, group: &mut CallbackGroup, period: Duration, callback: F,
    ) -> Result<GroupedTimer> {
        self.create_wall_timer_callback_in_group_with_info_and_options(
            group,
            period,
            CallbackTimerOptions::default(),
            callback,
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    /// Registers timestamped group wall work with lifecycle/startup/one-shot policy.
    pub fn create_wall_timer_callback_in_group_with_info_and_options<
        F: FnMut(crate::TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, group: &mut CallbackGroup, period: Duration, options: CallbackTimerOptions,
        callback: F,
    ) -> Result<GroupedTimer> {
        self.grouped_callback(
            group,
            period,
            options,
            ClockType::SteadyTime,
            crate::nodes::timer_callbacks::native::WithInfo(callback),
        )
    }
}
