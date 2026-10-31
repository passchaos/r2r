use super::*;
use crate::lifecycle::timer::generation::Generation;
use crate::nodes::timer_callbacks::native::Handler;

pub(crate) trait GroupedTimerActivation: Send + Sync {
    fn set_active(&self, active: bool);
}
use std::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

mod exclusive;
mod native;
mod reentrant;
use native::{Invocation, NativeTimer};

/// Owns a native timer whose selected invocation runs in a callback group.
///
/// Native advancement occurs on the callback worker immediately before user
/// invocation. Only one selection awaits advancement. Exclusive groups retain
/// selection through the whole handler; reentrant groups release it after native
/// advancement so subsequent deadlines can overlap user work. Controls synchronize with advancement and
/// finalization and wake the current native owner. Lifecycle-created timers
/// retain their activation generation and manual pause in the native resource.
/// Dropping the token closes pending input and schedules native finalization.
pub struct GroupedTimer {
    native: std::sync::Weak<NativeTimer>,
    control: Arc<dyn input::Control>,
    clock_type: ClockType,
}
impl GroupedTimer {
    pub(crate) fn activation(&self) -> Result<std::sync::Weak<dyn GroupedTimerActivation>> {
        let native: Arc<dyn GroupedTimerActivation> = self.native()?;
        Ok(Arc::downgrade(&native))
    }
    fn native(&self) -> Result<Arc<NativeTimer>> {
        if self.control.is_closed() {
            return Err(Error::RCL_RET_TIMER_INVALID);
        }
        self.native.upgrade().ok_or(Error::RCL_RET_TIMER_INVALID)
    }
    /// Whether this timer's input and native ownership remain available.
    pub fn is_valid(&self) -> bool {
        !self.control.is_closed()
    }
    /// Native clock domain, retained after endpoint destruction.
    pub fn clock_type(&self) -> ClockType {
        self.clock_type
    }
    /// Whether this is a steady wall-clock timer.
    pub fn is_steady(&self) -> bool {
        matches!(self.clock_type, ClockType::SteadyTime)
    }
    /// Pauses native production. Lifecycle timers retain manual pause across
    /// transitions; a handler already permitted may finish.
    pub fn cancel(&self) -> Result<()> {
        self.native()?.control(false)
    }
    /// Resets from the current clock and wakes native waiting. Managed timers
    /// restart only when active and reject older queued generation selections.
    pub fn reset(&self) -> Result<()> {
        self.native()?.control(true)
    }
    /// Observes manual and lifecycle-driven native resets as a lossless stream.
    /// Only one observer may be alive at a time.
    pub fn observe_resets(&self) -> Result<TimerResetEvents> {
        let native = self.native()?;
        native.with_handle(|handle| native.reset_state.observe(handle))
    }
    /// Current native cancellation state.
    pub fn is_canceled(&self) -> Result<bool> {
        self.native()?.with_handle(|handle| {
            let mut value = false;
            check_rcl_ret(unsafe { rcl_timer_is_canceled(handle, &mut value) })?;
            Ok(value)
        })
    }
    /// Current nonnegative native period.
    pub fn period(&self) -> Result<Duration> {
        self.native()?.with_handle(|handle| {
            let mut value = 0;
            check_rcl_ret(unsafe { rcl_timer_get_period(handle, &mut value) })?;
            duration_from_nonnegative_nanos(value)
        })
    }
    /// Signed next deadline in this timer's native clock domain.
    pub fn next_call_time_ns(&self) -> Result<i64> {
        self.native()?.with_handle(|handle| {
            let mut value = 0;
            check_rcl_ret(unsafe { rcl_timer_get_next_call_time(handle, &mut value) })?;
            Ok(value)
        })
    }
    /// Exchanges the native period and returns its old value, waking the owner.
    pub fn set_period(&self, period: Duration) -> Result<Duration> {
        let period = duration_to_rcl_nanos(period)?;
        let native = self.native()?;
        let old = native.with_handle(|handle| {
            let mut old = 0;
            check_rcl_ret(unsafe { rcl_timer_exchange_period(handle, period, &mut old) })?;
            duration_from_nonnegative_nanos(old)
        })?;
        native.wake_native();
        Ok(old)
    }
}
impl Drop for GroupedTimer {
    fn drop(&mut self) {
        self.control.close();
    }
}

struct TimerSource {
    id: uuid::Uuid,
    group: CallbackGroupId,
    native: Arc<NativeTimer>,
    input: Producer<Option<u64>>,
}
impl Source for TimerSource {
    fn prepare(&mut self) -> Result<()> {
        self.native.synchronize()
    }
    fn id(&self) -> uuid::Uuid {
        self.id
    }
    fn group_id(&self) -> CallbackGroupId {
        self.group
    }
    fn is_closed(&self) -> bool {
        self.input.is_closed()
    }
    fn is_waitable(&self) -> bool {
        self.native
            .generation
            .as_ref()
            .is_none_or(Generation::enabled)
            && !self.native.in_flight.load(Ordering::Acquire)
            && self.input.is_ready()
    }
    fn set_wake(&mut self, wake: Option<Waker>) {
        self.input.control().set_native_wake(wake.clone());
        *self.native.wake.lock() = wake;
    }
    fn capacities(&self) -> WaitSetCapacities {
        WaitSetCapacities {
            timers: 1,
            ..Default::default()
        }
    }
    fn register(&self, ws: &mut rcl_wait_set_t, index: &mut usize) -> Result<()> {
        check_rcl_ret(unsafe { rcl_wait_set_add_timer(ws, self.native.handle.get(), index) })
    }
    fn ready(&self, ws: &rcl_wait_set_t, index: usize) -> Result<bool> {
        ready(ws.timers, ws.size_of_timers, index, self.native.handle.get())
    }
    fn maximum(&self) -> usize {
        1
    }
    fn take(&mut self) -> Result<bool> {
        if self.input.is_closed()
            || self
                .native
                .in_flight
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return Ok(false);
        }
        let selected = self.native.generation.as_ref().map(Generation::current);
        if self.input.push(selected).is_err() {
            self.native.in_flight.store(false, Ordering::Release);
            if self.input.is_closed() {
                return Ok(false);
            }
            return Err(Error::RCL_RET_ERROR);
        }
        Ok(true)
    }
    fn destroy(&mut self, _node: &mut rcl_node_t) {
        self.input.close();
        self.native.destroy();
    }
}

impl Node {
    fn register_grouped_timer<H>(
        &mut self, group: &mut CallbackGroup, period: Duration, options: CallbackTimerOptions,
        clock_type: ClockType, handler: H,
    ) -> Result<GroupedTimer>
    where
        H: Handler + Send + 'static,
        H::Tick: Send + 'static,
        H::Permit: Send + 'static,
    {
        self.register_grouped_timer_with_generation(
            group, period, options, clock_type, handler, None,
        )
    }
    pub(crate) fn register_grouped_timer_with_generation<H>(
        &mut self, group: &mut CallbackGroup, period: Duration, options: CallbackTimerOptions,
        clock_type: ClockType, mut handler: H, generation: Option<Generation>,
    ) -> Result<GroupedTimer>
    where
        H: Handler + Send + 'static,
        H::Tick: Send + 'static,
        H::Permit: Send + 'static,
    {
        let native = self.allocate_grouped_timer(period, options, clock_type, generation)?;
        let selected = native.clone();
        let (producer, consumer) = group.prepare_input(1, move |generation| {
            let _invocation = Invocation(&selected);
            let Some(permit) = handler.permit() else {
                return Ok(());
            };
            if let Some(tick) = selected.advance::<H>(generation)? {
                handler.invoke(tick, permit)?;
            }
            Ok(())
        })?;
        let timer = self.install_grouped_timer(group.id(), native, producer);
        let registry = self.grouped_callbacks.as_ref().unwrap();
        group
            .sources
            .push((registry.node_id, Arc::downgrade(&timer.control)));
        group.commit_input(consumer);
        Ok(timer)
    }
    fn allocate_grouped_timer(
        &mut self, period: Duration, options: CallbackTimerOptions, clock_type: ClockType,
        generation: Option<Generation>,
    ) -> Result<Arc<NativeTimer>> {
        let period = duration_to_rcl_nanos(period)?;
        let clock = match clock_type {
            ClockType::RosTime => self.ros_clock.clone(),
            _ => Arc::new(Mutex::new(Clock::create(clock_type)?)),
        };
        let native = Arc::new(NativeTimer {
            handle: Box::new(UnsafeCell::new(unsafe { rcl_get_zero_initialized_timer() })),
            operations: Mutex::new(()),
            initialized: AtomicBool::new(false),
            in_flight: AtomicBool::new(false),
            clock,
            clock_type,
            oneshot: options.oneshot,
            generation,
            reset_state: timer_resets::TimerResetSlot::new(),
            wake: Mutex::new(None),
        });
        {
            let mut clock = native.clock.lock();
            let mut context = self.context.context_handle.lock();
            check_rcl_ret(unsafe {
                timers::init_timer(
                    native.handle.get(),
                    clock.clock_handle.as_mut(),
                    context.as_mut(),
                    period,
                    None,
                    rcutils_get_default_allocator(),
                    options.autostart && native.generation.is_none(),
                )
            })?;
            native.initialized.store(true, Ordering::Release);
        }
        native.synchronize()?;
        Ok(native)
    }
    fn install_grouped_timer(
        &mut self, group: CallbackGroupId, native: Arc<NativeTimer>,
        producer: Producer<Option<u64>>,
    ) -> GroupedTimer {
        let control = producer.control();
        let mut source = Box::new(TimerSource {
            id: uuid::Uuid::new_v4(),
            group,
            native: native.clone(),
            input: producer,
        });
        source.set_wake(self.canceled_subscriptions.lock().wake());
        let registry = self.grouped_callbacks.get_or_insert_with(Default::default);
        registry.indexes.insert(source.id, registry.rows.len());
        registry.rows.push(source);
        GroupedTimer {
            native: Arc::downgrade(&native),
            control,
            clock_type: native.clock_type,
        }
    }
}
