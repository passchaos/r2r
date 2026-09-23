use std::{fmt::Debug, mem::MaybeUninit, time::Duration};

use crate::{error::*, msg_types::generated_msgs::builtin_interfaces};
use r2r_rcl::*;

/// Different ROS clock types.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ClockType {
    RosTime,
    SystemTime,
    SteadyTime,
}

unsafe impl Send for Clock {}

/// A ROS clock.
pub struct Clock {
    pub(crate) clock_handle: Box<rcl_clock_t>,
    clock_type: ClockType,
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
        assert_eq!(clock.get_clock_type(), ClockType::SteadyTime);
        assert!(clock.get_now().unwrap() > Duration::ZERO);
        assert!(clock.get_now_ns().unwrap() > 0);
        assert!(matches!(clock.ros_time_override_enabled(), Err(Error::ClockTypeNotRosTime)));
    }

    #[cfg(r2r__rosgraph_msgs__msg__Clock)]
    #[test]
    fn rejects_negative_ros_clock_values_instead_of_wrapping() {
        let mut clock = Clock::create(ClockType::RosTime).unwrap();
        assert!(!clock.ros_time_override_enabled().unwrap());
        clock.enable_ros_time_override(-1).unwrap();
        assert!(clock.ros_time_override_enabled().unwrap());

        assert_eq!(clock.get_now_ns().unwrap(), -1);
        assert!(matches!(clock.get_now(), Err(Error::RCL_RET_INVALID_ARGUMENT)));
        clock.disable_ros_time_override().unwrap();
        assert!(!clock.ros_time_override_enabled().unwrap());
    }
}
