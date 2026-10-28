//! Timers: native rcl timers, per-timer tick channels and the public
//! [`Timer`] handle, plus the [`Node`] factory methods.

use super::*;

/// Options controlling native timer startup and Rust-side tick buffering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerOptions {
    /// Whether the timer starts running immediately after creation.
    pub autostart: bool,
    /// Number of pending ticks retained for the returned [`Timer`].
    pub channel_capacity: usize,
}

/// Timing metadata captured atomically when a timer is called.
///
/// The absolute timestamps use the clock domain reported by [`Self::clock_type`].
#[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimerCallInfo {
    /// Time elapsed since the timer's previous call.
    pub elapsed: Duration,
    /// Scheduled call time in nanoseconds.
    pub expected_call_time_ns: i64,
    /// Actual call time in nanoseconds.
    pub actual_call_time_ns: i64,
    /// Clock domain used by the timer.
    pub clock_type: ClockType,
}

impl Default for TimerOptions {
    fn default() -> Self {
        Self {
            autostart: true,
            channel_capacity: 1,
        }
    }
}

impl TimerOptions {
    /// Selects whether the native timer starts immediately.
    pub const fn autostart(mut self, autostart: bool) -> Self {
        self.autostart = autostart;
        self
    }

    /// Sets the Rust-side tick stream capacity.
    pub const fn channel_capacity(mut self, channel_capacity: usize) -> Self {
        self.channel_capacity = channel_capacity;
        self
    }

    fn validate(self) -> Result<Self> {
        if self.channel_capacity >= usize::MAX >> 2 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        Ok(self)
    }
}

#[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
#[inline]
pub unsafe fn init_timer(
    timer: *mut rcl_timer_t, clock: *mut rcl_clock_t, context: *mut rcl_context_t, period: i64,
    callback: rcl_timer_callback_t, allocator: rcl_allocator_t, autostart: bool,
) -> rcl_ret_t {
    let ret = rcl_timer_init(timer, clock, context, period, callback, allocator);
    if ret != RCL_RET_OK as i32 || autostart {
        return ret;
    }
    let cancel_ret = rcl_timer_cancel(timer);
    if cancel_ret != RCL_RET_OK as i32 {
        rcl_timer_fini(timer);
    }
    cancel_ret
}

// For Jazzy, Rolling, and newer
#[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
#[inline]
pub unsafe fn init_timer(
    timer: *mut rcl_timer_t, clock: *mut rcl_clock_t, context: *mut rcl_context_t, period: i64,
    callback: rcl_timer_callback_t, allocator: rcl_allocator_t, autostart: bool,
) -> rcl_ret_t {
    rcl_timer_init2(timer, clock, context, period, callback, allocator, autostart)
}

#[derive(Debug)]
pub(crate) struct RclTimer {
    handle: rcl_timer_t,
    _pin: PhantomPinned, // To prevent Unpin implementation
}

impl RclTimer {
    unsafe fn new() -> Self {
        Self {
            handle: rcl_get_zero_initialized_timer(),
            _pin: PhantomPinned,
        }
    }
}

pub(crate) struct Timer_ {
    timer_handle: Pin<Box<RclTimer>>,
    _clock: Option<Clock>, // Some(clock) if the timer owns the clock, just here to be dropped properly later.
    sender: mpsc::Sender<TimerTick>,
    pub(in crate::nodes) id: uuid::Uuid,
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    clock_type: ClockType,
}

pub(crate) struct TimerTick {
    elapsed: Duration,
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    expected_call_time_ns: i64,
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    actual_call_time_ns: i64,
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    clock_type: ClockType,
}

impl Timer_ {
    pub(in crate::nodes) fn get_handle(&self) -> *const rcl_timer_t {
        &self.timer_handle.handle
    }

    /// Get mutable pointer to handle
    ///
    /// SAFETY:
    ///     Pointer valid only for modification.
    ///     Must not invalidate or replace the timer unless in Drop.
    unsafe fn get_handle_mut(&mut self) -> *mut rcl_timer_t {
        self.timer_handle
            .as_mut()
            .map_unchecked_mut(|s| &mut s.handle)
            .get_unchecked_mut()
    }

    pub(in crate::nodes) fn handle_incoming(&mut self) -> Result<bool> {
        let mut is_ready = false;
        let ret = unsafe { rcl_timer_is_ready(self.get_handle(), &mut is_ready) };
        check_rcl_ret(ret)?;
        if !is_ready {
            return Ok(false);
        }

        let mut nanos = 0i64;
        let ret = unsafe { rcl_timer_get_time_since_last_call(self.get_handle(), &mut nanos) };
        check_rcl_ret(ret)?;
        let elapsed = duration_from_nonnegative_nanos(nanos)?;

        #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
        let tick = {
            let ret = unsafe { rcl_timer_call(self.get_handle_mut()) };
            check_rcl_ret(ret)?;
            TimerTick { elapsed }
        };
        #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
        let tick = {
            let mut call_info = std::mem::MaybeUninit::<rcl_timer_call_info_t>::uninit();
            let ret =
                unsafe { rcl_timer_call_with_info(self.get_handle_mut(), call_info.as_mut_ptr()) };
            check_rcl_ret(ret)?;
            let call_info = unsafe { call_info.assume_init() };
            TimerTick {
                elapsed,
                expected_call_time_ns: call_info.expected_call_time,
                actual_call_time_ns: call_info.actual_call_time,
                clock_type: self.clock_type,
            }
        };

        if let Err(e) = self.sender.try_send(tick) {
            if e.is_disconnected() {
                // client dropped the timer handle, let's drop our timer as well.
                return Ok(true);
            }
            if e.is_full() {
                log::debug!("Warning: timer tick not handled in time - no wakeup will occur");
            }
        }
        Ok(false)
    }
}

impl Drop for Timer_ {
    fn drop(&mut self) {
        let _ret = unsafe { rcl_timer_fini(self.get_handle_mut()) };
    }
}

/// A ROS timer.
pub struct Timer {
    receiver: mpsc::Receiver<TimerTick>,
    pub(crate) id: uuid::Uuid,
    clock_type: ClockType,
    canceled_timers: Arc<Mutex<VecDeque<uuid::Uuid>>>,
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.receiver.close();
        self.canceled_timers.lock().push_back(self.id);
    }
}

impl Timer {
    /// Returns the type of clock driving this timer.
    pub const fn clock_type(&self) -> ClockType {
        self.clock_type
    }

    /// Returns whether this timer uses a monotonic steady clock.
    pub const fn is_steady(&self) -> bool {
        matches!(self.clock_type, ClockType::SteadyTime)
    }

    /// Completes when the next instant in the interval has been reached.
    ///
    /// Returns the time passed since the timer was last woken up.
    pub async fn tick(&mut self) -> Result<Duration> {
        self.receiver
            .next()
            .await
            .map(|tick| tick.elapsed)
            .ok_or(Error::RCL_RET_TIMER_INVALID)
    }

    /// Completes with timing metadata for the next timer call.
    ///
    /// The expected and actual timestamps are captured atomically by RCL. The owning node must be
    /// spun while awaiting.
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    pub async fn tick_with_info(&mut self) -> Result<TimerCallInfo> {
        self.receiver
            .next()
            .await
            .map(|tick| TimerCallInfo {
                elapsed: tick.elapsed,
                expected_call_time_ns: tick.expected_call_time_ns,
                actual_call_time_ns: tick.actual_call_time_ns,
                clock_type: tick.clock_type,
            })
            .ok_or(Error::RCL_RET_TIMER_INVALID)
    }

    /// Pauses the timer until [`Timer::reset`] is called.
    pub fn cancel(&self, node: &mut Node) -> Result<()> {
        let timer = node.timer_mut(self.id)?;
        let ret = unsafe { rcl_timer_cancel(timer.get_handle_mut()) };
        check_rcl_ret(ret)
    }

    /// Restarts the timer from the current clock time, also resuming a canceled timer.
    pub fn reset(&self, node: &mut Node) -> Result<()> {
        let timer = node.timer_mut(self.id)?;
        let ret = unsafe { rcl_timer_reset(timer.get_handle_mut()) };
        check_rcl_ret(ret)
    }

    /// Returns whether the timer is currently canceled.
    pub fn is_canceled(&self, node: &Node) -> Result<bool> {
        let timer = node.timer(self.id)?;
        let mut canceled = false;
        let ret = unsafe { rcl_timer_is_canceled(timer.get_handle(), &mut canceled) };
        check_rcl_ret(ret)?;
        Ok(canceled)
    }

    /// Returns whether the timer is ready to trigger now.
    pub fn is_ready(&self, node: &Node) -> Result<bool> {
        let timer = node.timer(self.id)?;
        let mut ready = false;
        let ret = unsafe { rcl_timer_is_ready(timer.get_handle(), &mut ready) };
        check_rcl_ret(ret)?;
        Ok(ready)
    }

    /// Returns the remaining time until the next trigger.
    ///
    /// A timer that is ready or overdue returns [`Duration::ZERO`]. A canceled timer returns
    /// [`Error::RCL_RET_TIMER_CANCELED`].
    pub fn time_until_next_call(&self, node: &Node) -> Result<Duration> {
        Ok(Duration::from_nanos(
            self.time_until_next_call_ns(node)?.try_into().unwrap_or(0),
        ))
    }

    /// Returns the signed number of nanoseconds until the next trigger.
    ///
    /// Unlike [`Timer::time_until_next_call`], this preserves negative values, which indicate
    /// how far an active timer is overdue. A canceled timer returns
    /// [`Error::RCL_RET_TIMER_CANCELED`].
    pub fn time_until_next_call_ns(&self, node: &Node) -> Result<i64> {
        let timer = node.timer(self.id)?;
        let mut nanos = 0;
        let ret = unsafe { rcl_timer_get_time_until_next_call(timer.get_handle(), &mut nanos) };
        check_rcl_ret(ret)?;
        Ok(nanos)
    }

    /// Returns the signed number of nanoseconds since the previous timer call.
    ///
    /// The value may be negative when a ROS clock moves backwards.
    pub fn time_since_last_call_ns(&self, node: &Node) -> Result<i64> {
        let timer = node.timer(self.id)?;
        let mut nanos = 0;
        let ret = unsafe { rcl_timer_get_time_since_last_call(timer.get_handle(), &mut nanos) };
        check_rcl_ret(ret)?;
        Ok(nanos)
    }

    /// Returns the absolute next call time in nanoseconds in the timer's clock domain.
    ///
    /// A canceled timer returns [`Error::RCL_RET_TIMER_CANCELED`].
    pub fn next_call_time_ns(&self, node: &Node) -> Result<i64> {
        let timer = node.timer(self.id)?;
        let mut nanos = 0;
        let ret = unsafe { rcl_timer_get_next_call_time(timer.get_handle(), &mut nanos) };
        check_rcl_ret(ret)?;
        Ok(nanos)
    }

    /// Returns the timer period.
    pub fn period(&self, node: &Node) -> Result<Duration> {
        let timer = node.timer(self.id)?;
        let mut nanos = 0;
        let ret = unsafe { rcl_timer_get_period(timer.get_handle(), &mut nanos) };
        check_rcl_ret(ret)?;
        duration_from_nonnegative_nanos(nanos)
    }

    /// Changes the timer period and returns its previous value.
    pub fn set_period(&self, node: &mut Node, period: Duration) -> Result<Duration> {
        let period = duration_to_rcl_nanos(period)?;
        let timer = node.timer_mut(self.id)?;
        let mut previous = 0;
        let ret = unsafe { rcl_timer_exchange_period(timer.get_handle(), period, &mut previous) };
        check_rcl_ret(ret)?;
        duration_from_nonnegative_nanos(previous)
    }
}

pub(crate) async fn future_with_timeout<T>(
    future: impl Future<Output = Result<T>>, mut timer: Timer,
) -> Result<T> {
    futures::pin_mut!(future);
    let timeout = timer.tick();
    futures::pin_mut!(timeout);
    match future::select(future, timeout).await {
        future::Either::Left((result, _)) => result,
        future::Either::Right((timer_result, _)) => {
            timer_result?;
            Err(Error::RCL_RET_TIMEOUT)
        }
    }
}

impl Node {
    fn add_timer(&mut self, timer: Timer_) {
        let index = self.timers.len();
        let previous = self.timer_indexes.insert(timer.id, index);
        debug_assert!(previous.is_none());
        self.timers.push(timer);
    }

    pub(in crate::nodes) fn remove_timer(
        timers: &mut Vec<Timer_>, timer_indexes: &mut HashMap<uuid::Uuid, usize>, id: uuid::Uuid,
    ) -> Option<Timer_> {
        let index = timer_indexes.remove(&id)?;
        let removed = timers.swap_remove(index);
        debug_assert_eq!(removed.id, id);
        if let Some(moved) = timers.get(index) {
            timer_indexes.insert(moved.id, index);
        }
        Some(removed)
    }

    /// Create a ROS wall timer.
    ///
    /// Create a ROS timer that is woken up by spin every `period`.
    ///
    /// This timer uses [`ClockType::SteadyTime`] clock.
    pub fn create_wall_timer(&mut self, period: Duration) -> Result<Timer> {
        self.create_wall_timer_with_options(period, TimerOptions::default())
    }

    /// Creates a steady-clock timer with explicit startup and buffering options.
    pub fn create_wall_timer_with_options(
        &mut self, period: Duration, options: TimerOptions,
    ) -> Result<Timer> {
        let options = options.validate()?;
        let mut clock = Clock::create(ClockType::SteadyTime)?;
        let timer_handle = self.create_timer_helper(&mut clock, period, options.autostart)?;

        let (tx, rx) = mpsc::channel(options.channel_capacity);

        let timer = Timer_ {
            timer_handle,
            _clock: Some(clock), // The timer owns the clock.
            sender: tx,
            id: uuid::Uuid::new_v4(),
            #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
            clock_type: ClockType::SteadyTime,
        };
        let id = timer.id;
        self.add_timer(timer);

        let out_timer = Timer {
            receiver: rx,
            id,
            clock_type: ClockType::SteadyTime,
            canceled_timers: Arc::clone(&self.canceled_timers),
        };

        Ok(out_timer)
    }

    /// Create a ROS timer
    ///
    /// Create a ROS timer that is woken up by spin every `period`.
    ///
    /// This timer uses node's [`ClockType::RosTime`] clock.
    pub fn create_timer(&mut self, period: Duration) -> Result<Timer> {
        self.create_timer_with_options(period, TimerOptions::default())
    }

    /// Creates a ROS-clock timer with explicit startup and buffering options.
    pub fn create_timer_with_options(
        &mut self, period: Duration, options: TimerOptions,
    ) -> Result<Timer> {
        let options = options.validate()?;
        let timer_handle = {
            let mut clock = self.ros_clock.lock();
            self.create_timer_helper(&mut clock, period, options.autostart)?
        };

        let (tx, rx) = mpsc::channel(options.channel_capacity);

        let timer = Timer_ {
            timer_handle,
            _clock: None, // The timer does not own the clock (the node owns it).
            sender: tx,
            id: uuid::Uuid::new_v4(),
            #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
            clock_type: ClockType::RosTime,
        };
        let id = timer.id;
        self.add_timer(timer);

        let out_timer = Timer {
            receiver: rx,
            id,
            clock_type: ClockType::RosTime,
            canceled_timers: Arc::clone(&self.canceled_timers),
        };

        Ok(out_timer)
    }

    /// Immediately destroys a timer owned by this node.
    ///
    /// Dropping a timer schedules it for removal during the next spin. This method removes and
    /// finalizes it before returning. Passing a timer created by another node leaves this node
    /// unchanged and schedules normal cleanup by the owning node.
    pub fn destroy_timer(&mut self, timer: Timer) {
        let id = timer.id;
        drop(timer);
        if Self::remove_timer(&mut self.timers, &mut self.timer_indexes, id).is_some() {
            self.canceled_timers.lock().retain(|queued| *queued != id);
        }
    }

    fn create_timer_helper(
        &self, clock: &mut Clock, period: Duration, autostart: bool,
    ) -> Result<Pin<Box<RclTimer>>> {
        // rcl expects that the address of the rcl_timer_t does not change.
        let mut timer_handle = unsafe { Box::pin(RclTimer::new()) };

        let mut ctx = self.context.context_handle.lock();
        let ret = unsafe {
            init_timer(
                &mut timer_handle.as_mut().get_unchecked_mut().handle,
                clock.clock_handle.as_mut(),
                ctx.as_mut(),
                duration_to_rcl_nanos(period)?,
                None,
                rcutils_get_default_allocator(),
                autostart,
            )
        };

        if ret != RCL_RET_OK as i32 {
            log::error!("could not create timer: {}", ret);
            return Err(Error::from_rcl_error(ret));
        }

        Ok(timer_handle)
    }

    fn timer(&self, id: uuid::Uuid) -> Result<&Timer_> {
        self.timer_indexes
            .get(&id)
            .and_then(|index| self.timers.get(*index))
            .ok_or(Error::RCL_RET_TIMER_INVALID)
    }

    fn timer_mut(&mut self, id: uuid::Uuid) -> Result<&mut Timer_> {
        self.timer_indexes
            .get(&id)
            .and_then(|index| self.timers.get_mut(*index))
            .ok_or(Error::RCL_RET_TIMER_INVALID)
    }

    #[cfg(test)]
    pub(crate) fn timer_count(&self) -> usize {
        self.timers.len()
    }
}
