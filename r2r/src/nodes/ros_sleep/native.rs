//! One native timer per node, canceled while time is simulated or no deadline is pending.

use super::*;

pub(super) struct SleepTimer {
    handle: Box<rcl_timer_t>,
    clock: Arc<Mutex<Clock>>,
}

impl SleepTimer {
    pub(super) fn create(node: &Node) -> Result<Self> {
        // ROS deadlines are compared by the scheduler. A steady native wake timer avoids
        // RCL's internal now+period overflow when signed simulated time jumps near i64::MAX.
        let clock = Arc::new(Mutex::new(Clock::create(ClockType::SteadyTime)?));
        let mut handle = Box::new(unsafe { rcl_get_zero_initialized_timer() });
        let mut clock_handle = clock.lock();
        let mut context = node.context.context_handle.lock();
        let ret = unsafe {
            super::super::timers::init_timer(
                &mut *handle,
                clock_handle.clock_handle.as_mut(),
                context.as_mut(),
                1,
                None,
                rcutils_get_default_allocator(),
                false,
            )
        };
        check_rcl_ret(ret)?;
        drop(clock_handle);
        Ok(Self { handle, clock })
    }
    pub(super) fn handle(&self) -> *const rcl_timer_t {
        self.handle.as_ref()
    }
    pub(super) fn arm(&mut self, period: i64) -> Result<()> {
        let mut previous = 0;
        check_rcl_ret(unsafe {
            rcl_timer_exchange_period(self.handle.as_ref(), period, &mut previous)
        })?;
        check_rcl_ret(unsafe { rcl_timer_reset(self.handle.as_mut()) })
    }
    pub(super) fn cancel(&mut self) -> Result<()> {
        check_rcl_ret(unsafe { rcl_timer_cancel(self.handle.as_mut()) })
    }
}

impl Drop for SleepTimer {
    fn drop(&mut self) {
        // rcl_timer_fini removes its internal clock-jump registration. Serialize that mutation
        // with user clock callbacks, including when a clock Arc outlives the owning node.
        let _clock = self.clock.lock();
        let ret = unsafe { rcl_timer_fini(self.handle.as_mut()) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize ROS sleep timer: {ret}");
        }
    }
}
