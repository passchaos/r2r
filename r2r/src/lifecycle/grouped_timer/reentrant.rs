use super::*;
use crate::ReentrantCallbackGroup;
use std::sync::Arc;

impl LifecycleNode {
    fn grouped_reentrant_callback<H>(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration,
        options: CallbackTimerOptions, clock: ClockType, handlers: Vec<H>,
    ) -> Result<GroupedTimer>
    where
        H: Handler + Send + 'static,
        H::Tick: Send + 'static,
        H::Permit: Send + 'static,
    {
        let active = self.lifecycle_state() == LifecycleState::Active;
        let timer = Node::register_reentrant_timer_with_generation(
            self,
            group,
            period,
            options,
            clock,
            handlers,
            Some(Generation::new(active, options.autostart)),
        )?;
        self.runtime_register_grouped_timer(timer.activation()?);
        Ok(timer)
    }
    /// Registers a group wall callback enabled only in the active lifecycle generation.
    pub fn create_wall_timer_reentrant_callback<F: Fn() -> Result<()> + Send + Sync + 'static>(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration, callback: F,
    ) -> Result<GroupedTimer> {
        self.create_wall_timer_reentrant_callback_with_options(
            group,
            period,
            CallbackTimerOptions::default(),
            callback,
        )
    }
    /// Preserves manual startup/one-shot policy across lifecycle transitions.
    pub fn create_wall_timer_reentrant_callback_with_options<
        F: Fn() -> Result<()> + Send + Sync + 'static,
    >(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration,
        options: CallbackTimerOptions, callback: F,
    ) -> Result<GroupedTimer> {
        self.grouped_reentrant_callback(group, period, options, ClockType::SteadyTime, {
            let callback = Arc::new(callback);
            (0..group.concurrency())
                .map(|_| {
                    let callback = callback.clone();
                    Plain(move || callback())
                })
                .collect()
        })
    }
    /// Registers a lifecycle-gated callback driven by the managed ROS clock.
    pub fn create_timer_reentrant_callback<F: Fn() -> Result<()> + Send + Sync + 'static>(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration, callback: F,
    ) -> Result<GroupedTimer> {
        self.create_timer_reentrant_callback_with_options(
            group,
            period,
            CallbackTimerOptions::default(),
            callback,
        )
    }
    /// Registers ROS-clock group work with lifecycle and manual generation policy.
    pub fn create_timer_reentrant_callback_with_options<
        F: Fn() -> Result<()> + Send + Sync + 'static,
    >(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration,
        options: CallbackTimerOptions, callback: F,
    ) -> Result<GroupedTimer> {
        self.grouped_reentrant_callback(group, period, options, ClockType::RosTime, {
            let callback = Arc::new(callback);
            (0..group.concurrency())
                .map(|_| {
                    let callback = callback.clone();
                    Plain(move || callback())
                })
                .collect()
        })
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    /// Captures native timestamps only after an active generation permit is acquired.
    pub fn create_timer_reentrant_callback_with_info<
        F: Fn(crate::TimerCallInfo) -> Result<()> + Send + Sync + 'static,
    >(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration, callback: F,
    ) -> Result<GroupedTimer> {
        self.grouped_reentrant_callback(
            group,
            period,
            CallbackTimerOptions::default(),
            ClockType::RosTime,
            {
                let callback = Arc::new(callback);
                (0..group.concurrency())
                    .map(|_| {
                        let callback = callback.clone();
                        crate::nodes::timer_callbacks::native::WithInfo(move |tick| callback(tick))
                    })
                    .collect()
            },
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    /// Registers timestamped group ROS work with explicit native policy.
    pub fn create_timer_reentrant_callback_with_info_and_options<
        F: Fn(crate::TimerCallInfo) -> Result<()> + Send + Sync + 'static,
    >(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration,
        options: CallbackTimerOptions, callback: F,
    ) -> Result<GroupedTimer> {
        self.grouped_reentrant_callback(group, period, options, ClockType::RosTime, {
            let callback = Arc::new(callback);
            (0..group.concurrency())
                .map(|_| {
                    let callback = callback.clone();
                    crate::nodes::timer_callbacks::native::WithInfo(move |tick| callback(tick))
                })
                .collect()
        })
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    /// Registers timestamped group wall work in the active lifecycle generation.
    pub fn create_wall_timer_reentrant_callback_with_info<
        F: Fn(crate::TimerCallInfo) -> Result<()> + Send + Sync + 'static,
    >(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration, callback: F,
    ) -> Result<GroupedTimer> {
        self.create_wall_timer_reentrant_callback_with_info_and_options(
            group,
            period,
            CallbackTimerOptions::default(),
            callback,
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    /// Registers timestamped group wall work with lifecycle/startup/one-shot policy.
    pub fn create_wall_timer_reentrant_callback_with_info_and_options<
        F: Fn(crate::TimerCallInfo) -> Result<()> + Send + Sync + 'static,
    >(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration,
        options: CallbackTimerOptions, callback: F,
    ) -> Result<GroupedTimer> {
        self.grouped_reentrant_callback(group, period, options, ClockType::SteadyTime, {
            let callback = Arc::new(callback);
            (0..group.concurrency())
                .map(|_| {
                    let callback = callback.clone();
                    crate::nodes::timer_callbacks::native::WithInfo(move |tick| callback(tick))
                })
                .collect()
        })
    }
}
