use super::*;

impl Node {
    /// Registers a steady-clock callback in a mutually exclusive group.
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
    /// Registers a wall callback with explicit startup and one-shot policy.
    pub fn create_wall_timer_callback_in_group_with_options<
        F: FnMut() -> Result<()> + Send + 'static,
    >(
        &mut self, group: &mut CallbackGroup, period: Duration, options: CallbackTimerOptions,
        callback: F,
    ) -> Result<GroupedTimer> {
        self.register_grouped_timer(
            group,
            period,
            options,
            ClockType::SteadyTime,
            crate::nodes::timer_callbacks::native::Plain(callback),
        )
    }
    /// Registers a callback driven by this Node's managed ROS clock.
    pub fn create_timer_callback_in_group<F: FnMut() -> Result<()> + Send + 'static>(
        &mut self, group: &mut CallbackGroup, period: Duration, callback: F,
    ) -> Result<GroupedTimer> {
        self.register_grouped_timer(
            group,
            period,
            CallbackTimerOptions::default(),
            ClockType::RosTime,
            crate::nodes::timer_callbacks::native::Plain(callback),
        )
    }
    /// Registers a ROS-clock callback with startup and one-shot policy.
    pub fn create_timer_callback_in_group_with_options<
        F: FnMut() -> Result<()> + Send + 'static,
    >(
        &mut self, group: &mut CallbackGroup, period: Duration, options: CallbackTimerOptions,
        callback: F,
    ) -> Result<GroupedTimer> {
        self.register_grouped_timer(
            group,
            period,
            options,
            ClockType::RosTime,
            crate::nodes::timer_callbacks::native::Plain(callback),
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    /// Captures native expected/actual call timestamps at worker advancement.
    pub fn create_timer_callback_in_group_with_info<
        F: FnMut(TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, group: &mut CallbackGroup, period: Duration, callback: F,
    ) -> Result<GroupedTimer> {
        self.register_grouped_timer(
            group,
            period,
            CallbackTimerOptions::default(),
            ClockType::RosTime,
            crate::nodes::timer_callbacks::native::WithInfo(callback),
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    /// Registers timestamped ROS-clock invocation with startup/one-shot policy.
    pub fn create_timer_callback_in_group_with_info_and_options<
        F: FnMut(TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, group: &mut CallbackGroup, period: Duration, options: CallbackTimerOptions,
        callback: F,
    ) -> Result<GroupedTimer> {
        self.register_grouped_timer(
            group,
            period,
            options,
            ClockType::RosTime,
            crate::nodes::timer_callbacks::native::WithInfo(callback),
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    /// Registers timestamped steady-clock invocation in the group.
    pub fn create_wall_timer_callback_in_group_with_info<
        F: FnMut(TimerCallInfo) -> Result<()> + Send + 'static,
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
    /// Registers timestamped wall invocation with startup/one-shot policy.
    pub fn create_wall_timer_callback_in_group_with_info_and_options<
        F: FnMut(TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, group: &mut CallbackGroup, period: Duration, options: CallbackTimerOptions,
        callback: F,
    ) -> Result<GroupedTimer> {
        self.register_grouped_timer(
            group,
            period,
            options,
            ClockType::SteadyTime,
            crate::nodes::timer_callbacks::native::WithInfo(callback),
        )
    }
}
