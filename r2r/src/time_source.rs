use crate::{
    builtin_interfaces::msg::Time,
    error::*,
    msg_types::{VoidPtr, WrappedNativeMsg},
    rosgraph_msgs,
    subscribers::{create_subscription_helper, Subscriber_},
    Clock, ClockType, Node, QosProfile, WrappedTypesupport,
};
use parking_lot::Mutex;
use r2r_rcl::{
    rcl_node_t, rcl_subscription_fini, rcl_subscription_t, rcl_take, rcl_time_point_value_t,
    rmw_message_info_t, RCL_RET_OK,
};
use std::sync::{Arc, Weak};

/// Provides time from `/clock` topic to attached ROS clocks
///
/// By default only clock used by ROS timers is attached and time from `/clock` topic is disabled.
///
/// The time from `/clock` topic can be activated by either of these:
/// - calling [`TimeSource::enable_sim_time`]
/// - having registered parameter handler and launching the node with parameter `use_sim_time:=true`
///
/// Similar to `rclcpp/time_source.hpp`
#[derive(Clone)]
pub struct TimeSource {
    inner: Arc<Mutex<TimeSource_>>,
}

pub(crate) struct TimeSource_ {
    managed_clocks: Vec<Weak<Mutex<Clock>>>,
    subscriber_state: TimeSourceSubscriberState,
    simulated_time_enabled: bool,
    last_time_msg: rcl_time_point_value_t,
}

#[derive(Copy, Clone)]
enum TimeSourceSubscriberState {
    None, // subscriber does not exist
    Active,
    ToBeDestroyed,
}

struct TimeSourceSubscriber {
    subscriber_handle: rcl_subscription_t,
    time_source: TimeSource,
}

impl TimeSource {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TimeSource_::new())),
        }
    }

    /// Attach clock of type [`RosTime`](ClockType::RosTime) to the [`TimeSource`]
    ///
    /// If the simulated time is enabled the [`TimeSource`] will distribute simulated time
    /// to all attached clocks.
    pub fn attach_ros_clock(&self, clock: Weak<Mutex<Clock>>) -> Result<()> {
        let mut time_source = self.inner.lock();
        let clock_valid = clock
            .upgrade()
            .map(|clock_arc| {
                let mut clock = clock_arc.lock();

                if !matches!(clock.get_clock_type(), ClockType::RosTime) {
                    return Err(Error::ClockTypeNotRosTime);
                }

                if time_source.simulated_time_enabled {
                    clock.enable_ros_time_override(time_source.last_time_msg)?;
                }

                Ok(())
            })
            .transpose()?
            .is_some();
        if clock_valid {
            time_source.managed_clocks.push(clock);
        }
        // if upgrade is none no need to attach the clock since it is already dropped

        Ok(())
    }

    /// Enables usage of simulated time
    ///
    /// Simulated time is provided on topic `"/clock"` in the message [rosgraph_msgs::msg::Clock].
    ///
    /// See example: sim_time_publisher.rs
    pub fn enable_sim_time(&self, node: &mut Node) -> Result<()> {
        let mut inner = self.inner.lock();
        if inner.simulated_time_enabled {
            // already enabled nothing to do
            return Ok(());
        }

        if let Some(subscriber) = inner.activate_subscriber(|| {
            TimeSourceSubscriber::new(&mut node.node_handle, self.clone())
        })? {
            node.subscribers.push(Box::new(subscriber));
        }
        let initial_time = inner.last_time_msg;
        if let Err(err) =
            inner.try_for_each_managed_clock(|clock| clock.enable_ros_time_override(initial_time))
        {
            // Some clocks may already have been enabled. Restore every live clock and schedule
            // the subscription for removal so callers can retry from a consistent state.
            if let Err(rollback_err) =
                inner.try_for_each_managed_clock(Clock::disable_ros_time_override)
            {
                log::error!("failed to roll back simulated time activation: {rollback_err}");
            }
            inner.subscriber_state = TimeSourceSubscriberState::ToBeDestroyed;
            return Err(err);
        }

        // Do not publish the enabled state until subscription and clock setup have succeeded.
        inner.simulated_time_enabled = true;

        Ok(())
    }

    /// Disables usage of simulated time
    ///
    /// This schedules removal of the internal subscriber to the `"/clock"` topic before the
    /// node enters its next wait cycle.
    pub fn disable_sim_time(&self) {
        let mut inner = self.inner.lock();
        if inner.simulated_time_enabled {
            inner.simulated_time_enabled = false;

            // disable ros time override on all attached clocks
            if let Err(err) = inner.try_for_each_managed_clock(Clock::disable_ros_time_override) {
                log::error!("failed to disable simulated time on an attached clock: {err}");
            }
        }

        if matches!(inner.subscriber_state, TimeSourceSubscriberState::Active) {
            inner.subscriber_state = TimeSourceSubscriberState::ToBeDestroyed;
        }
    }
}

impl TimeSource_ {
    fn new() -> Self {
        Self {
            managed_clocks: vec![],
            subscriber_state: TimeSourceSubscriberState::None,
            simulated_time_enabled: false,
            last_time_msg: 0,
        }
    }

    fn for_each_managed_clock<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut Clock),
    {
        self.managed_clocks.retain(|weak_clock| {
            let Some(clock_arc) = weak_clock.upgrade() else {
                // clock can be deleted
                return false;
            };

            let mut clock = clock_arc.lock();
            f(&mut clock);

            // retain clock
            true
        });
    }

    fn try_for_each_managed_clock<F>(&mut self, mut f: F) -> Result<()>
    where
        F: FnMut(&mut Clock) -> Result<()>,
    {
        let mut first_error = None;
        self.for_each_managed_clock(|clock| {
            if let Err(err) = f(clock) {
                first_error.get_or_insert(err);
            }
        });
        first_error.map_or(Ok(()), Err)
    }

    fn activate_subscriber<T>(&mut self, create: impl FnOnce() -> Result<T>) -> Result<Option<T>> {
        match self.subscriber_state {
            TimeSourceSubscriberState::None => {
                let subscriber = create()?;
                self.subscriber_state = TimeSourceSubscriberState::Active;
                Ok(Some(subscriber))
            }
            TimeSourceSubscriberState::ToBeDestroyed => {
                self.subscriber_state = TimeSourceSubscriberState::Active;
                Ok(None)
            }
            TimeSourceSubscriberState::Active => Ok(None),
        }
    }

    fn take_subscriber_destruction_request(&mut self) -> bool {
        if matches!(self.subscriber_state, TimeSourceSubscriberState::ToBeDestroyed) {
            self.subscriber_state = TimeSourceSubscriberState::None;
            true
        } else {
            false
        }
    }

    // this is similar to internal rclcpp function `set_all_clocks` in `time_source.cpp`
    fn set_clock_time(&mut self, time_msg: Time) {
        let time = time_msg.into();
        self.last_time_msg = time;
        if let Err(err) = self.try_for_each_managed_clock(|clock| clock.set_ros_time_override(time))
        {
            log::error!("failed to update simulated time on an attached clock: {err}");
        }
    }
}

impl TimeSourceSubscriber {
    fn new(node_handle: &mut rcl_node_t, time_source: TimeSource) -> Result<TimeSourceSubscriber> {
        // The values are set based on default values in rclcpp
        let qos = QosProfile::default().keep_last(1).best_effort();

        let subscriber = create_subscription_helper(
            node_handle,
            "/clock",
            crate::rosgraph_msgs::msg::Clock::get_ts(),
            qos,
        )?;
        Ok(Self {
            subscriber_handle: subscriber,
            time_source,
        })
    }
}

impl Subscriber_ for TimeSourceSubscriber {
    fn handle(&self) -> &rcl_subscription_t {
        &self.subscriber_handle
    }

    fn should_remove(&mut self) -> bool {
        self.time_source
            .inner
            .lock()
            .take_subscriber_destruction_request()
    }

    fn handle_incoming(&mut self) -> bool {
        // update clock
        let mut msg_info = rmw_message_info_t::default(); // we dont care for now
        let Ok(mut clock_msg) = WrappedNativeMsg::<rosgraph_msgs::msg::Clock>::try_new() else {
            log::error!("failed to allocate simulated clock message buffer");
            return false;
        };
        let ret = unsafe {
            rcl_take(
                &self.subscriber_handle,
                clock_msg.void_ptr_mut(),
                &mut msg_info,
                std::ptr::null_mut(),
            )
        };

        let mut inner_time_source = self.time_source.inner.lock();
        if ret == RCL_RET_OK as i32 {
            let msg = rosgraph_msgs::msg::Clock::from_native(&clock_msg);

            inner_time_source.set_clock_time(msg.clock);
        }

        match inner_time_source.subscriber_state {
            TimeSourceSubscriberState::Active => {
                // keep the subscriber
                false
            }
            TimeSourceSubscriberState::ToBeDestroyed => {
                inner_time_source.subscriber_state = TimeSourceSubscriberState::None;
                // destroy the subscriber
                true
            }
            TimeSourceSubscriberState::None => unreachable!(),
        }
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_subscription_fini(&mut self.subscriber_handle, node);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscriber_destruction_request_is_consumed_without_a_clock_message() {
        let mut inner = TimeSource_::new();
        inner.subscriber_state = TimeSourceSubscriberState::ToBeDestroyed;

        assert!(inner.take_subscriber_destruction_request());
        assert!(matches!(inner.subscriber_state, TimeSourceSubscriberState::None));
        assert!(!inner.take_subscriber_destruction_request());
    }

    #[test]
    fn failed_subscription_setup_does_not_activate_time_source() {
        let mut inner = TimeSource_::new();

        let result = inner.activate_subscriber::<()>(|| Err(Error::RCL_RET_ERROR));

        assert!(result.is_err());
        assert!(!inner.simulated_time_enabled);
        assert!(matches!(inner.subscriber_state, TimeSourceSubscriberState::None));
    }
}
