use super::*;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) trait NativeTimer {
    fn id(&self) -> uuid::Uuid;
    fn alive(&self) -> Arc<AtomicBool>;
    fn clock_type(&self) -> ClockType;
    fn handle(&self) -> *const rcl_timer_t;
    fn handle_mut(self: Pin<&mut Self>) -> *mut rcl_timer_t;
    fn call(self: Pin<&mut Self>) -> Result<()>;
    fn destroy(self: Pin<&mut Self>);
}

pub(super) trait Handler {
    type Tick;
    fn advance(timer: *mut rcl_timer_t, clock: ClockType) -> Result<Self::Tick>;
    fn invoke(&mut self, tick: Self::Tick) -> Result<()>;
}

pub(super) struct Plain<F>(pub(super) F);
impl<F: FnMut() -> Result<()>> Handler for Plain<F> {
    type Tick = ();
    fn advance(timer: *mut rcl_timer_t, _clock: ClockType) -> Result<()> {
        check_rcl_ret(unsafe { rcl_timer_call(timer) })
    }
    fn invoke(&mut self, (): ()) -> Result<()> {
        (self.0)()
    }
}

#[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
pub(super) struct WithInfo<F>(pub(super) F);
#[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
impl<F: FnMut(TimerCallInfo) -> Result<()>> Handler for WithInfo<F> {
    type Tick = TimerCallInfo;
    fn advance(timer: *mut rcl_timer_t, clock: ClockType) -> Result<TimerCallInfo> {
        let mut elapsed = 0;
        check_rcl_ret(unsafe { rcl_timer_get_time_since_last_call(timer, &mut elapsed) })?;
        let elapsed = duration_from_nonnegative_nanos(elapsed)?;
        let mut call = std::mem::MaybeUninit::<rcl_timer_call_info_t>::uninit();
        check_rcl_ret(unsafe { rcl_timer_call_with_info(timer, call.as_mut_ptr()) })?;
        let call = unsafe { call.assume_init() };
        Ok(TimerCallInfo {
            elapsed,
            clock_type: clock,
            expected_call_time_ns: call.expected_call_time,
            actual_call_time_ns: call.actual_call_time,
        })
    }
    fn invoke(&mut self, tick: TimerCallInfo) -> Result<()> {
        (self.0)(tick)
    }
}

pub(super) struct Timer<H: Handler> {
    native: timers::RclTimer,
    initialized: bool,
    id: uuid::Uuid,
    alive: Arc<AtomicBool>,
    clock_type: ClockType,
    _owned_clock: Option<Clock>,
    options: CallbackTimerOptions,
    handler: H,
}

impl<H: Handler + Send + 'static> Timer<H> {
    pub(super) fn allocate(
        clock_type: ClockType, options: CallbackTimerOptions, handler: H,
    ) -> Pin<Box<Self>> {
        Box::pin(Self {
            native: unsafe { timers::RclTimer::new() },
            initialized: false,
            id: uuid::Uuid::new_v4(),
            alive: Arc::new(AtomicBool::new(true)),
            clock_type,
            _owned_clock: None,
            options,
            handler,
        })
    }
    pub(super) fn initialize(
        self: Pin<&mut Self>, context: &Context, clock: &mut Clock, period: i64,
    ) -> Result<()> {
        let timer = unsafe { self.get_unchecked_mut() };
        let ret = {
            let mut context = context.context_handle.lock();
            unsafe {
                timers::init_timer(
                    &mut timer.native.handle,
                    clock.clock_handle.as_mut(),
                    context.as_mut(),
                    period,
                    None,
                    rcutils_get_default_allocator(),
                    timer.options.autostart,
                )
            }
        };
        check_rcl_ret(ret)?;
        timer.initialized = true;
        Ok(())
    }
    pub(super) fn own_clock(self: Pin<&mut Self>, clock: Clock) {
        unsafe { self.get_unchecked_mut() }._owned_clock = Some(clock);
    }
}

impl<H: Handler> Timer<H> {
    fn destroy_native(&mut self) {
        self.alive.store(false, Ordering::Release);
        if self.initialized {
            self.initialized = false;
            let ret = unsafe { rcl_timer_fini(&mut self.native.handle) };
            if ret != RCL_RET_OK as i32 {
                log::error!("could not finalize callback timer: {ret}");
            }
        }
    }
}

impl<H: Handler + Send + 'static> NativeTimer for Timer<H> {
    fn id(&self) -> uuid::Uuid {
        self.id
    }
    fn alive(&self) -> Arc<AtomicBool> {
        self.alive.clone()
    }
    fn clock_type(&self) -> ClockType {
        self.clock_type
    }
    fn handle(&self) -> *const rcl_timer_t {
        &self.native.handle
    }
    fn handle_mut(self: Pin<&mut Self>) -> *mut rcl_timer_t {
        &mut unsafe { self.get_unchecked_mut() }.native.handle
    }
    fn call(self: Pin<&mut Self>) -> Result<()> {
        let timer = unsafe { self.get_unchecked_mut() };
        if !timer.alive.load(Ordering::Acquire) {
            return Ok(());
        }
        let mut ready = false;
        check_rcl_ret(unsafe { rcl_timer_is_ready(&timer.native.handle, &mut ready) })?;
        if !ready {
            return Ok(());
        }
        let tick = H::advance(&mut timer.native.handle, timer.clock_type)?;
        if timer.options.oneshot {
            check_rcl_ret(unsafe { rcl_timer_cancel(&mut timer.native.handle) })?;
        }
        // All user work, including error/panic paths, runs outside RCL and internal locks.
        timer.handler.invoke(tick)
    }
    fn destroy(self: Pin<&mut Self>) {
        unsafe { self.get_unchecked_mut() }.destroy_native();
    }
}

impl<H: Handler> Drop for Timer<H> {
    fn drop(&mut self) {
        self.destroy_native();
    }
}
