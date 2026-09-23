use std::{collections::HashMap, fmt::Debug, mem::MaybeUninit, time::Duration};

use crate::{error::*, msg_types::generated_msgs::builtin_interfaces};
use r2r_rcl::*;

/// Different ROS clock types.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ClockType {
    RosTime,
    SystemTime,
    SteadyTime,
}

/// The clock source state reported around a time jump.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ClockChange {
    RosTimeNoChange,
    RosTimeActivated,
    RosTimeDeactivated,
    SystemTimeNoChange,
}

/// Whether a jump notification runs before or after the clock value changes.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TimeJumpPhase {
    Before,
    After,
}

/// A clock jump delivered to a registered callback.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TimeJump {
    pub clock_change: ClockChange,
    /// New clock value minus the previous value, in nanoseconds.
    pub delta_ns: i64,
    pub phase: TimeJumpPhase,
}

/// Conditions that select time jumps for a callback.
///
/// Forward and backward thresholds are inclusive positive magnitudes. A missing or zero duration
/// disables that direction.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct TimeJumpThreshold {
    pub on_clock_change: bool,
    pub min_forward: Option<Duration>,
    pub min_backward: Option<Duration>,
}

impl TimeJumpThreshold {
    pub const fn on_clock_change(mut self, enabled: bool) -> Self {
        self.on_clock_change = enabled;
        self
    }

    pub const fn min_forward(mut self, threshold: Duration) -> Self {
        self.min_forward = Some(threshold);
        self
    }

    pub const fn min_backward(mut self, threshold: Duration) -> Self {
        self.min_backward = Some(threshold);
        self
    }

    fn into_native(self) -> Result<rcl_jump_threshold_t> {
        fn duration_ns(duration: Option<Duration>) -> Result<i64> {
            duration
                .map(|duration| {
                    i64::try_from(duration.as_nanos()).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)
                })
                .transpose()
                .map(Option::unwrap_or_default)
        }

        Ok(rcl_jump_threshold_t {
            on_clock_change: self.on_clock_change,
            min_forward: rcl_duration_t {
                nanoseconds: duration_ns(self.min_forward)?,
            },
            min_backward: rcl_duration_t {
                nanoseconds: -duration_ns(self.min_backward)?,
            },
        })
    }
}

/// Identity of a callback registered on a [`Clock`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct TimeJumpCallbackId(u64);

struct TimeJumpCallback {
    callback: Box<dyn FnMut(TimeJump) + Send + 'static>,
}

unsafe impl Send for Clock {}

/// A ROS clock.
pub struct Clock {
    pub(crate) clock_handle: Box<rcl_clock_t>,
    clock_type: ClockType,
    jump_callbacks: HashMap<TimeJumpCallbackId, Box<TimeJumpCallback>>,
    next_jump_callback_id: u64,
}

pub fn clock_type_to_rcl(ct: &ClockType) -> rcl_clock_type_t {
    match ct {
        ClockType::RosTime => rcl_clock_type_t::RCL_ROS_TIME,
        ClockType::SystemTime => rcl_clock_type_t::RCL_SYSTEM_TIME,
        ClockType::SteadyTime => rcl_clock_type_t::RCL_STEADY_TIME,
    }
}

impl Clock {
    fn handle_ptr(&self) -> *mut rcl_clock_t {
        self.clock_handle.as_ref() as *const rcl_clock_t as *mut rcl_clock_t
    }

    /// Create a new clock with the specified type.
    pub fn create(ct: ClockType) -> Result<Clock> {
        let mut clock_handle = MaybeUninit::<rcl_clock_t>::uninit();

        let rcl_ct = clock_type_to_rcl(&ct);
        let ret = unsafe {
            rcl_clock_init(rcl_ct, clock_handle.as_mut_ptr(), &mut rcutils_get_default_allocator())
        };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not create {:?} clock: {}", ct, ret);
            return Err(Error::from_rcl_error(ret));
        }

        let clock_handle = Box::new(unsafe { clock_handle.assume_init() });
        Ok(Clock {
            clock_handle,
            clock_type: ct,
            jump_callbacks: HashMap::new(),
            next_jump_callback_id: 0,
        })
    }

    /// Returns the current clock value.
    ///
    /// [`Duration`] cannot represent clock values before zero, so a negative ROS time is
    /// reported as [`Error::RCL_RET_INVALID_ARGUMENT`] instead of wrapping to a very large value.
    pub fn get_now(&self) -> Result<Duration> {
        let nanos =
            u64::try_from(self.get_now_ns()?).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        Ok(Duration::from_nanos(nanos))
    }

    /// Returns the signed current clock value in nanoseconds.
    ///
    /// Unlike [`Clock::get_now`], this preserves ROS time values before zero.
    pub fn get_now_ns(&self) -> Result<i64> {
        let valid = unsafe { rcl_clock_valid(self.handle_ptr()) };
        if !valid {
            return Err(Error::from_rcl_error(RCL_RET_INVALID_ARGUMENT as i32));
        }
        let mut tp: rcutils_time_point_value_t = 0;
        let ret = unsafe { rcl_clock_get_now(self.handle_ptr(), &mut tp) };

        if ret != RCL_RET_OK as i32 {
            log::error!("could not create steady clock: {}", ret);
            return Err(Error::from_rcl_error(ret));
        }
        Ok(tp)
    }

    pub fn get_clock_type(&self) -> ClockType {
        self.clock_type
    }

    /// Returns whether the native clock handle is valid.
    pub fn is_valid(&self) -> bool {
        // The RCL API predates const-correctness here but documents this as a read-only query.
        unsafe { rcl_clock_valid(self.handle_ptr()) }
    }

    /// Returns whether this clock currently reports a value strictly greater than zero.
    ///
    /// For a ROS clock with simulated time enabled, this remains `false` until a positive clock
    /// value is received. System and steady clocks are normally already started.
    pub fn started(&self) -> Result<bool> {
        if !self.is_valid() {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        Ok(unsafe { rcl_clock_time_started(self.handle_ptr()) })
    }

    /// Returns whether ROS time override is active for this ROS clock.
    pub fn ros_time_override_enabled(&self) -> Result<bool> {
        if self.clock_type != ClockType::RosTime {
            return Err(Error::ClockTypeNotRosTime);
        }
        let mut enabled = false;
        let ret = unsafe { rcl_is_enabled_ros_time_override(self.handle_ptr(), &mut enabled) };
        if ret == RCL_RET_OK as i32 {
            Ok(enabled)
        } else {
            Err(Error::from_rcl_error(ret))
        }
    }

    /// Registers a callback for ROS time jumps matching `threshold`.
    ///
    /// The callback is invoked synchronously both before and after a matching change. It must not
    /// attempt to lock or mutate this clock recursively. Panics are caught at the FFI boundary and
    /// logged instead of unwinding into RCL.
    pub fn add_jump_callback<F>(
        &mut self, threshold: TimeJumpThreshold, callback: F,
    ) -> Result<TimeJumpCallbackId>
    where
        F: FnMut(TimeJump) + Send + 'static,
    {
        if self.clock_type != ClockType::RosTime {
            return Err(Error::ClockTypeNotRosTime);
        }
        let threshold = threshold.into_native()?;
        let id = loop {
            let id = TimeJumpCallbackId(self.next_jump_callback_id);
            self.next_jump_callback_id = self.next_jump_callback_id.wrapping_add(1);
            if !self.jump_callbacks.contains_key(&id) {
                break id;
            }
        };
        let mut callback = Box::new(TimeJumpCallback {
            callback: Box::new(callback),
        });
        let user_data = (&mut *callback as *mut TimeJumpCallback).cast();
        let ret = unsafe {
            rcl_clock_add_jump_callback(
                self.clock_handle.as_mut(),
                threshold,
                Some(dispatch_time_jump),
                user_data,
            )
        };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        self.jump_callbacks.insert(id, callback);
        Ok(id)
    }

    /// Removes a previously registered jump callback.
    ///
    /// Returns `false` when the callback ID is not registered on this clock.
    pub fn remove_jump_callback(&mut self, id: TimeJumpCallbackId) -> Result<bool> {
        let Some(callback) = self.jump_callbacks.get_mut(&id) else {
            return Ok(false);
        };
        let user_data = (&mut **callback as *mut TimeJumpCallback).cast();
        let ret = unsafe {
            rcl_clock_remove_jump_callback(
                self.clock_handle.as_mut(),
                Some(dispatch_time_jump),
                user_data,
            )
        };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        self.jump_callbacks.remove(&id);
        Ok(true)
    }

    /// TODO: move to builtin helper methods module.
    pub fn to_builtin_time(d: &Duration) -> builtin_interfaces::msg::Time {
        Self::try_to_builtin_time(d).unwrap_or(builtin_interfaces::msg::Time {
            sec: i32::MAX,
            nanosec: 999_999_999,
        })
    }

    /// Convert a duration to a ROS time, returning an error if its seconds exceed `i32`.
    pub fn try_to_builtin_time(d: &Duration) -> Result<builtin_interfaces::msg::Time> {
        let sec = i32::try_from(d.as_secs()).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let nanosec = d.subsec_nanos();
        Ok(builtin_interfaces::msg::Time { sec, nanosec })
    }

    /// Enables alternative source of time for this clock
    ///
    /// The clock must be [`ClockType::RosTime`].
    ///
    /// Wrapper for `rcl_enable_ros_time_override`
    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    pub(crate) fn enable_ros_time_override(
        &mut self, initial_time: rcl_time_point_value_t,
    ) -> Result<()> {
        let valid = unsafe { rcl_clock_valid(&mut *self.clock_handle) };
        if !valid {
            return Err(Error::from_rcl_error(RCL_RET_INVALID_ARGUMENT as i32));
        }

        let ret = unsafe { rcl_enable_ros_time_override(&mut *self.clock_handle) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not enable ros time override: {}", ret);
            return Err(Error::from_rcl_error(ret));
        }

        self.set_ros_time_override(initial_time)?;

        Ok(())
    }

    /// Disables alternative source of time for this clock
    ///
    /// The clock must be [`ClockType::RosTime`].
    ///
    /// Wrapper for `rcl_disable_ros_time_override`
    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    pub(crate) fn disable_ros_time_override(&mut self) -> Result<()> {
        let valid = unsafe { rcl_clock_valid(&mut *self.clock_handle) };
        if !valid {
            return Err(Error::from_rcl_error(RCL_RET_INVALID_ARGUMENT as i32));
        }

        let ret = unsafe { rcl_disable_ros_time_override(&mut *self.clock_handle) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not disable ros time override: {}", ret);
            return Err(Error::from_rcl_error(ret));
        }

        Ok(())
    }

    /// Sets new time value if the clock has enabled alternative time source
    ///
    /// If the clock does not have alternative time source enabled this function will not change the time.
    ///
    /// The clock must be [`ClockType::RosTime`].
    ///
    /// Wrapper for `rcl_set_ros_time_override`
    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    pub(crate) fn set_ros_time_override(&mut self, time: rcl_time_point_value_t) -> Result<()> {
        let valid = unsafe { rcl_clock_valid(&mut *self.clock_handle) };
        if !valid {
            return Err(Error::from_rcl_error(RCL_RET_INVALID_ARGUMENT as i32));
        }

        let ret = unsafe { rcl_set_ros_time_override(&mut *self.clock_handle, time) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not set ros time override: {}", ret);
            return Err(Error::from_rcl_error(ret));
        }

        Ok(())
    }
}

unsafe extern "C" fn dispatch_time_jump(
    time_jump: *const rcl_time_jump_t, before_jump: bool, user_data: *mut std::os::raw::c_void,
) {
    let (Some(time_jump), Some(callback)) = (unsafe { time_jump.as_ref() }, unsafe {
        user_data.cast::<TimeJumpCallback>().as_mut()
    }) else {
        return;
    };
    let clock_change = match time_jump.clock_change {
        rcl_clock_change_t::RCL_ROS_TIME_NO_CHANGE => ClockChange::RosTimeNoChange,
        rcl_clock_change_t::RCL_ROS_TIME_ACTIVATED => ClockChange::RosTimeActivated,
        rcl_clock_change_t::RCL_ROS_TIME_DEACTIVATED => ClockChange::RosTimeDeactivated,
        rcl_clock_change_t::RCL_SYSTEM_TIME_NO_CHANGE => ClockChange::SystemTimeNoChange,
    };
    let event = TimeJump {
        clock_change,
        delta_ns: time_jump.delta.nanoseconds,
        phase: if before_jump {
            TimeJumpPhase::Before
        } else {
            TimeJumpPhase::After
        },
    };
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (callback.callback)(event)))
        .is_err()
    {
        log::error!("time jump callback panicked");
    }
}

impl From<builtin_interfaces::msg::Time> for rcutils_time_point_value_t {
    fn from(msg: builtin_interfaces::msg::Time) -> Self {
        (msg.sec as rcl_time_point_value_t) * 1_000_000_000
            + (msg.nanosec as rcl_time_point_value_t)
    }
}

impl Drop for Clock {
    fn drop(&mut self) {
        unsafe {
            rcl_clock_fini(&mut *self.clock_handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_builtin_times_outside_the_ros_range() {
        assert_eq!(
            Clock::try_to_builtin_time(&Duration::new(i32::MAX as u64, 42))
                .unwrap()
                .nanosec,
            42
        );
        assert!(Clock::try_to_builtin_time(&Duration::from_secs(i32::MAX as u64 + 1)).is_err());
        assert_eq!(
            Clock::to_builtin_time(&Duration::from_secs(i32::MAX as u64 + 1)),
            builtin_interfaces::msg::Time {
                sec: i32::MAX,
                nanosec: 999_999_999,
            }
        );
    }

    #[test]
    fn reports_clock_validity_and_rejects_override_queries_for_non_ros_clocks() {
        let clock = Clock::create(ClockType::SteadyTime).unwrap();

        assert!(clock.is_valid());
        assert!(clock.started().unwrap());
        assert_eq!(clock.get_clock_type(), ClockType::SteadyTime);
        assert!(clock.get_now().unwrap() > Duration::ZERO);
        assert!(clock.get_now_ns().unwrap() > 0);
        assert!(matches!(clock.ros_time_override_enabled(), Err(Error::ClockTypeNotRosTime)));
    }

    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    #[test]
    fn rejects_negative_ros_clock_values_instead_of_wrapping() {
        let mut clock = Clock::create(ClockType::RosTime).unwrap();
        assert!(clock.started().unwrap());
        assert!(!clock.ros_time_override_enabled().unwrap());
        clock.enable_ros_time_override(0).unwrap();
        assert!(clock.ros_time_override_enabled().unwrap());
        assert!(!clock.started().unwrap());
        clock.set_ros_time_override(-1).unwrap();
        assert!(!clock.started().unwrap());

        assert_eq!(clock.get_now_ns().unwrap(), -1);
        assert!(matches!(clock.get_now(), Err(Error::RCL_RET_INVALID_ARGUMENT)));
        clock.set_ros_time_override(1).unwrap();
        assert!(clock.started().unwrap());
        clock.disable_ros_time_override().unwrap();
        assert!(!clock.ros_time_override_enabled().unwrap());
        assert!(clock.started().unwrap());
    }

    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    #[test]
    fn jump_callbacks_observe_thresholds_phases_and_removal() {
        use parking_lot::Mutex;
        use std::sync::Arc;

        let mut clock = Clock::create(ClockType::RosTime).unwrap();
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&events);
        let id = clock
            .add_jump_callback(
                TimeJumpThreshold::default()
                    .on_clock_change(true)
                    .min_forward(Duration::from_nanos(10))
                    .min_backward(Duration::from_nanos(10)),
                move |event| captured.lock().push(event),
            )
            .unwrap();

        clock.enable_ros_time_override(100).unwrap();
        clock.set_ros_time_override(105).unwrap();
        clock.set_ros_time_override(115).unwrap();
        clock.set_ros_time_override(105).unwrap();
        clock.disable_ros_time_override().unwrap();

        let events_before_removal = events.lock().clone();
        assert_eq!(
            events_before_removal
                .iter()
                .map(|event| (event.clock_change, event.delta_ns, event.phase))
                .collect::<Vec<_>>(),
            [
                (ClockChange::RosTimeActivated, 0, TimeJumpPhase::Before),
                (ClockChange::RosTimeActivated, 0, TimeJumpPhase::After),
                (ClockChange::RosTimeNoChange, 100, TimeJumpPhase::Before),
                (ClockChange::RosTimeNoChange, 100, TimeJumpPhase::After),
                (ClockChange::RosTimeNoChange, 10, TimeJumpPhase::Before),
                (ClockChange::RosTimeNoChange, 10, TimeJumpPhase::After),
                (ClockChange::RosTimeNoChange, -10, TimeJumpPhase::Before),
                (ClockChange::RosTimeNoChange, -10, TimeJumpPhase::After),
                (ClockChange::RosTimeDeactivated, 0, TimeJumpPhase::Before),
                (ClockChange::RosTimeDeactivated, 0, TimeJumpPhase::After),
            ]
        );

        assert!(clock.remove_jump_callback(id).unwrap());
        assert!(!clock.remove_jump_callback(id).unwrap());
        clock.enable_ros_time_override(200).unwrap();
        assert_eq!(*events.lock(), events_before_removal);
    }

    #[test]
    fn jump_callbacks_reject_wrong_clock_types_and_oversized_thresholds() {
        let mut steady = Clock::create(ClockType::SteadyTime).unwrap();
        assert!(matches!(
            steady.add_jump_callback(TimeJumpThreshold::default(), |_| {}),
            Err(Error::ClockTypeNotRosTime)
        ));

        let mut ros = Clock::create(ClockType::RosTime).unwrap();
        assert!(matches!(
            ros.add_jump_callback(
                TimeJumpThreshold::default().min_forward(Duration::from_nanos(i64::MAX as u64 + 1)),
                |_| {},
            ),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
    }

    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    #[test]
    fn panicking_jump_callback_does_not_cross_ffi_or_stop_other_callbacks() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        let mut clock = Clock::create(ClockType::RosTime).unwrap();
        clock
            .add_jump_callback(TimeJumpThreshold::default().on_clock_change(true), |_| {
                panic!("callback panic")
            })
            .unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let captured = Arc::clone(&calls);
        clock
            .add_jump_callback(TimeJumpThreshold::default().on_clock_change(true), move |_| {
                captured.fetch_add(1, Ordering::Relaxed);
            })
            .unwrap();

        clock.enable_ros_time_override(0).unwrap();

        assert_eq!(calls.load(Ordering::Relaxed), 2);
    }
}
