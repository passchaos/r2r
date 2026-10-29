use super::*;
use native::{Handler, Plain, Timer};

impl Node {
    pub(crate) fn register_callback_timer<H: Handler + Send + 'static>(
        &mut self, period: Duration, options: CallbackTimerOptions, clock_type: ClockType,
        handler: H,
    ) -> Result<CallbackTimer> {
        let period = duration_to_rcl_nanos(period)?;
        if !self.context.is_valid() {
            return Err(Error::RCL_RET_NOT_INIT);
        }
        // Captured state is allocated/retired outside context and managed-clock locks.
        let mut timer = Timer::allocate(clock_type, options, handler);
        if clock_type == ClockType::SteadyTime {
            let mut clock = Clock::create(ClockType::SteadyTime)?;
            timer
                .as_mut()
                .initialize(&self.context, &mut clock, period)?;
            timer.as_mut().own_clock(clock);
        } else {
            let result = {
                let mut clock = self.ros_clock.lock();
                timer.as_mut().initialize(&self.context, &mut clock, period)
            };
            result?;
        }
        let timer: Pin<Box<dyn NativeTimer>> = timer;
        let wake = self.canceled_timers.lock().wake();
        let registry = self.timer_callbacks.get_or_insert_with(Default::default);
        registry.set_wake(wake);
        Ok(registry.insert(timer))
    }
    /// Register a bounded synchronous callback driven by a steady-clock native timer.
    ///
    /// The handler must be Send because the node can move into a background executor.
    ///
    /// ```compile_fail
    /// use std::{rc::Rc, time::Duration};
    /// fn install(node: &mut r2r::Node) -> r2r::Result<r2r::CallbackTimer> {
    ///     let local = Rc::new(1);
    ///     node.create_wall_timer_callback(Duration::from_secs(1), move || {
    ///         let _ = Rc::clone(&local);
    ///         Ok(())
    ///     })
    /// }
    /// ```
    pub fn create_wall_timer_callback<F: FnMut() -> Result<()> + Send + 'static>(
        &mut self, period: Duration, callback: F,
    ) -> Result<CallbackTimer> {
        self.create_wall_timer_callback_with_options(
            period,
            CallbackTimerOptions::default(),
            callback,
        )
    }
    pub fn create_wall_timer_callback_with_options<F: FnMut() -> Result<()> + Send + 'static>(
        &mut self, period: Duration, options: CallbackTimerOptions, callback: F,
    ) -> Result<CallbackTimer> {
        self.register_callback_timer(period, options, ClockType::SteadyTime, Plain(callback))
    }
    /// Register a synchronous callback driven by this node's managed ROS clock.
    pub fn create_timer_callback<F: FnMut() -> Result<()> + Send + 'static>(
        &mut self, period: Duration, callback: F,
    ) -> Result<CallbackTimer> {
        self.create_timer_callback_with_options(period, CallbackTimerOptions::default(), callback)
    }
    pub fn create_timer_callback_with_options<F: FnMut() -> Result<()> + Send + 'static>(
        &mut self, period: Duration, options: CallbackTimerOptions, callback: F,
    ) -> Result<CallbackTimer> {
        self.register_callback_timer(period, options, ClockType::RosTime, Plain(callback))
    }

    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    pub fn create_wall_timer_callback_with_info<
        F: FnMut(TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, period: Duration, callback: F,
    ) -> Result<CallbackTimer> {
        self.create_wall_timer_callback_with_info_and_options(
            period,
            CallbackTimerOptions::default(),
            callback,
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    pub fn create_wall_timer_callback_with_info_and_options<
        F: FnMut(TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, period: Duration, options: CallbackTimerOptions, callback: F,
    ) -> Result<CallbackTimer> {
        self.register_callback_timer(
            period,
            options,
            ClockType::SteadyTime,
            native::WithInfo(callback),
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    pub fn create_timer_callback_with_info<
        F: FnMut(TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, period: Duration, callback: F,
    ) -> Result<CallbackTimer> {
        self.create_timer_callback_with_info_and_options(
            period,
            CallbackTimerOptions::default(),
            callback,
        )
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    pub fn create_timer_callback_with_info_and_options<
        F: FnMut(TimerCallInfo) -> Result<()> + Send + 'static,
    >(
        &mut self, period: Duration, options: CallbackTimerOptions, callback: F,
    ) -> Result<CallbackTimer> {
        self.register_callback_timer(
            period,
            options,
            ClockType::RosTime,
            native::WithInfo(callback),
        )
    }

    pub fn destroy_callback_timer(&mut self, timer: CallbackTimer) {
        let id = timer.identity();
        drop(timer);
        if let Some(registry) = &mut self.timer_callbacks {
            if registry.remove(id) {
                registry.canceled.lock().retain(|queued| *queued != id);
            }
        }
    }
    pub(super) fn callback_timer(&self, id: uuid::Uuid) -> Result<&dyn NativeTimer> {
        self.timer_callbacks
            .as_ref()
            .ok_or(Error::RCL_RET_TIMER_INVALID)?
            .get(id)
    }
    pub(super) fn callback_timer_mut(
        &mut self, id: uuid::Uuid,
    ) -> Result<Pin<&mut (dyn NativeTimer + 'static)>> {
        self.timer_callbacks
            .as_mut()
            .ok_or(Error::RCL_RET_TIMER_INVALID)?
            .get_mut(id)
    }
}
