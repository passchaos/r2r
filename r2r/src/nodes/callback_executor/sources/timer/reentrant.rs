use super::*;

impl Node {
    fn register_reentrant_timer<H>(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration,
        options: CallbackTimerOptions, clock_type: ClockType, handlers: Vec<H>,
    ) -> Result<GroupedTimer>
    where
        H: Handler + Send + 'static,
        H::Tick: Send + 'static,
        H::Permit: Send + 'static,
    {
        self.register_reentrant_timer_with_generation(
            group, period, options, clock_type, handlers, None,
        )
    }
    pub(crate) fn register_reentrant_timer_with_generation<H>(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration,
        options: CallbackTimerOptions, clock_type: ClockType, handlers: Vec<H>,
        generation: Option<Generation>,
    ) -> Result<GroupedTimer>
    where
        H: Handler + Send + 'static,
        H::Tick: Send + 'static,
        H::Permit: Send + 'static,
    {
        if handlers.len() != group.concurrency() {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let native = self.allocate_grouped_timer(period, options, clock_type, generation)?;
        let callbacks = handlers
            .into_iter()
            .map(|mut handler| {
                let selected = native.clone();
                move |generation| {
                    let invocation = Invocation(&selected);
                    let Some(permit) = handler.permit() else {
                        return Ok(());
                    };
                    let tick = selected.advance::<H>(generation)?;
                    // One selected advancement remains exclusive. After advancing,
                    // the next deadline can be selected while this user body runs.
                    // Drop before invoking; panic/error cannot strand selection.
                    drop(invocation);
                    if let Some(tick) = tick {
                        handler.invoke(tick, permit)?;
                    }
                    Ok(())
                }
            })
            .collect();
        let (producer, consumers) = group.prepare_inputs(1, callbacks)?;
        let timer = self.install_grouped_timer(group.id(), native, producer);
        let registry = self.grouped_callbacks.as_ref().unwrap();
        group
            .sources
            .push((registry.node_id, Arc::downgrade(&timer.control)));
        group.commit_inputs(consumers);
        Ok(timer)
    }
    /// Registers a shared steady-clock callback with bounded overlapping invocations.
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
    /// Registers a wall callback with explicit startup and one-shot policy.
    pub fn create_wall_timer_reentrant_callback_with_options<
        F: Fn() -> Result<()> + Send + Sync + 'static,
    >(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration,
        options: CallbackTimerOptions, callback: F,
    ) -> Result<GroupedTimer> {
        self.register_reentrant_timer(group, period, options, ClockType::SteadyTime, {
            let callback = Arc::new(callback);
            (0..group.concurrency())
                .map(|_| {
                    let callback = callback.clone();
                    crate::nodes::timer_callbacks::native::Plain(move || callback())
                })
                .collect()
        })
    }
    /// Registers a callback driven by this Node's managed ROS clock.
    pub fn create_timer_reentrant_callback<F: Fn() -> Result<()> + Send + Sync + 'static>(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration, callback: F,
    ) -> Result<GroupedTimer> {
        self.register_reentrant_timer(
            group,
            period,
            CallbackTimerOptions::default(),
            ClockType::RosTime,
            {
                let callback = Arc::new(callback);
                (0..group.concurrency())
                    .map(|_| {
                        let callback = callback.clone();
                        crate::nodes::timer_callbacks::native::Plain(move || callback())
                    })
                    .collect()
            },
        )
    }
    /// Registers a ROS-clock callback with startup and one-shot policy.
    pub fn create_timer_reentrant_callback_with_options<
        F: Fn() -> Result<()> + Send + Sync + 'static,
    >(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration,
        options: CallbackTimerOptions, callback: F,
    ) -> Result<GroupedTimer> {
        self.register_reentrant_timer(group, period, options, ClockType::RosTime, {
            let callback = Arc::new(callback);
            (0..group.concurrency())
                .map(|_| {
                    let callback = callback.clone();
                    crate::nodes::timer_callbacks::native::Plain(move || callback())
                })
                .collect()
        })
    }
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    /// Captures native expected/actual call timestamps at worker advancement.
    pub fn create_timer_reentrant_callback_with_info<
        F: Fn(TimerCallInfo) -> Result<()> + Send + Sync + 'static,
    >(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration, callback: F,
    ) -> Result<GroupedTimer> {
        self.register_reentrant_timer(
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
    /// Registers timestamped ROS-clock invocation with startup/one-shot policy.
    pub fn create_timer_reentrant_callback_with_info_and_options<
        F: Fn(TimerCallInfo) -> Result<()> + Send + Sync + 'static,
    >(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration,
        options: CallbackTimerOptions, callback: F,
    ) -> Result<GroupedTimer> {
        self.register_reentrant_timer(group, period, options, ClockType::RosTime, {
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
    /// Registers timestamped steady-clock invocation in the group.
    pub fn create_wall_timer_reentrant_callback_with_info<
        F: Fn(TimerCallInfo) -> Result<()> + Send + Sync + 'static,
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
    /// Registers timestamped wall invocation with startup/one-shot policy.
    pub fn create_wall_timer_reentrant_callback_with_info_and_options<
        F: Fn(TimerCallInfo) -> Result<()> + Send + Sync + 'static,
    >(
        &mut self, group: &mut ReentrantCallbackGroup, period: Duration,
        options: CallbackTimerOptions, callback: F,
    ) -> Result<GroupedTimer> {
        self.register_reentrant_timer(group, period, options, ClockType::SteadyTime, {
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
