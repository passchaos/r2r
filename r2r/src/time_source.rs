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
    rmw_message_info_t, RCL_RET_OK, RCL_RET_SUBSCRIPTION_TAKE_FAILED,
};
use std::sync::{Arc, Weak};

/// Provides time from `/clock` topic to attached ROS clocks
///
/// By default only clock used by ROS timers is attached and time from `/clock` topic is disabled.
///
/// The time from `/clock` topic can be activated by either of these:
/// - calling [`TimeSource::enable_sim_time`]
/// - having registered parameter handler and launching the node with parameter `use_sim_time:=true`
/// - changing the registered `use_sim_time` parameter while the node is running
///
/// Similar to `rclcpp/time_source.hpp`
#[derive(Clone)]
pub struct TimeSource {
    inner: Arc<Mutex<TimeSource_>>,
    // Serializes state snapshots with their corresponding clock operations without nesting the
    // state mutex inside a managed clock mutex.
    operations: Arc<Mutex<()>>,
}

pub(crate) struct TimeSource_ {
    managed_clocks: Vec<Weak<Mutex<Clock>>>,
    subscriber_state: TimeSourceSubscriberState,
    simulated_time_enabled: bool,
    requested_simulated_time: Option<bool>,
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
            operations: Arc::new(Mutex::new(())),
        }
    }

    /// Attach clock of type [`RosTime`](ClockType::RosTime) to the [`TimeSource`]
    ///
    /// If the simulated time is enabled the [`TimeSource`] will distribute simulated time
    /// to all attached clocks.
    pub fn attach_ros_clock(&self, clock: Weak<Mutex<Clock>>) -> Result<()> {
        let _operation = self.operations.lock();
        let Some(clock_arc) = clock.upgrade() else {
            return Ok(());
        };
        let (simulated_time_enabled, last_time_msg) = {
            let inner = self.inner.lock();
            (inner.simulated_time_enabled, inner.last_time_msg)
        };
        {
            let mut clock_guard = clock_arc.lock();
            if !matches!(clock_guard.get_clock_type(), ClockType::RosTime) {
                return Err(Error::ClockTypeNotRosTime);
            }
            if simulated_time_enabled {
                clock_guard.enable_ros_time_override(last_time_msg)?;
            }
        }
        self.inner.lock().managed_clocks.push(clock);

        Ok(())
    }

    /// Enables usage of simulated time
    ///
    /// Simulated time is provided on topic `"/clock"` in the message [rosgraph_msgs::msg::Clock].
    ///
    /// See example: sim_time_publisher.rs
    pub fn enable_sim_time(&self, node: &mut Node) -> Result<()> {
        let _operation = self.operations.lock();
        self.enable_sim_time_locked(node)
    }

    fn enable_sim_time_locked(&self, node: &mut Node) -> Result<()> {
        let (subscriber, initial_time, clocks) = {
            let mut inner = self.inner.lock();
            if inner.simulated_time_enabled {
                return Ok(());
            }
            let subscriber = inner.activate_subscriber(|| {
                TimeSourceSubscriber::new(&mut node.node_handle, self.clone())
            })?;
            let initial_time = inner.last_time_msg;
            let clocks = inner.live_managed_clocks();
            (subscriber, initial_time, clocks)
        };
        if let Some(subscriber) = subscriber {
            node.add_subscriber(Box::new(subscriber));
        }
        if let Err(err) =
            try_for_each_clock(&clocks, |clock| clock.enable_ros_time_override(initial_time))
        {
            // Some clocks may already have been enabled. Restore every live clock and schedule
            // the subscription for removal so callers can retry from a consistent state.
            if let Err(rollback_err) = try_for_each_clock(&clocks, Clock::disable_ros_time_override)
            {
                log::error!("failed to roll back simulated time activation: {rollback_err}");
            }
            self.inner.lock().subscriber_state = TimeSourceSubscriberState::ToBeDestroyed;
            return Err(err);
        }

        // Do not publish the enabled state until subscription and clock setup have succeeded.
        let mut inner = self.inner.lock();
        inner.simulated_time_enabled = true;
        inner.requested_simulated_time = None;

        Ok(())
    }

    /// Disables usage of simulated time
    ///
    /// This schedules removal of the internal subscriber to the `"/clock"` topic before the
    /// node enters its next wait cycle.
    pub fn disable_sim_time(&self) {
        let _operation = self.operations.lock();
        self.disable_sim_time_locked();
    }

    fn disable_sim_time_locked(&self) {
        let (was_enabled, clocks) = {
            let mut inner = self.inner.lock();
            inner.requested_simulated_time = None;
            (inner.simulated_time_enabled, inner.live_managed_clocks())
        };
        if was_enabled {
            if let Err(err) = try_for_each_clock(&clocks, Clock::disable_ros_time_override) {
                log::error!("failed to disable simulated time on an attached clock: {err}");
            }
        }
        let mut inner = self.inner.lock();
        inner.simulated_time_enabled = false;
        if matches!(inner.subscriber_state, TimeSourceSubscriberState::Active) {
            inner.subscriber_state = TimeSourceSubscriberState::ToBeDestroyed;
        }
    }

    /// Returns whether attached clocks currently use time from `/clock`.
    pub fn is_sim_time_enabled(&self) -> bool {
        self.inner.lock().simulated_time_enabled
    }

    pub(crate) fn request_sim_time(&self, enabled: bool) {
        let _operation = self.operations.lock();
        let mut inner = self.inner.lock();
        if inner.simulated_time_enabled == enabled {
            inner.requested_simulated_time = None;
        } else {
            inner.requested_simulated_time = Some(enabled);
        }
    }

    pub(crate) fn apply_requested_sim_time(&self, node: &mut Node) -> Result<()> {
        let _operation = self.operations.lock();
        let requested = self.inner.lock().requested_simulated_time.take();
        match requested {
            Some(true) => {
                if let Err(err) = self.enable_sim_time_locked(node) {
                    self.inner.lock().requested_simulated_time = Some(true);
                    return Err(err);
                }
            }
            Some(false) => self.disable_sim_time_locked(),
            None => {}
        }
        Ok(())
    }
}

impl TimeSource_ {
    fn new() -> Self {
        Self {
            managed_clocks: vec![],
            subscriber_state: TimeSourceSubscriberState::None,
            simulated_time_enabled: false,
            requested_simulated_time: None,
            last_time_msg: 0,
        }
    }

    fn live_managed_clocks(&mut self) -> Vec<Arc<Mutex<Clock>>> {
        let mut clocks = Vec::new();
        self.managed_clocks.retain(|weak_clock| {
            let Some(clock_arc) = weak_clock.upgrade() else {
                return false;
            };
            clocks.push(clock_arc);
            true
        });
        clocks
    }
}

fn try_for_each_clock(
    clocks: &[Arc<Mutex<Clock>>], mut f: impl FnMut(&mut Clock) -> Result<()>,
) -> Result<()> {
    let mut first_error = None;
    for clock in clocks {
        if let Err(err) = f(&mut clock.lock()) {
            first_error.get_or_insert(err);
        }
    }
    first_error.map_or(Ok(()), Err)
}

impl TimeSource_ {
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

    fn subscriber_should_remove_after_take(&mut self) -> bool {
        match self.subscriber_state {
            TimeSourceSubscriberState::Active => false,
            TimeSourceSubscriberState::ToBeDestroyed => {
                self.subscriber_state = TimeSourceSubscriberState::None;
                true
            }
            // A subscriber whose state was already consumed is stale. Remove it idempotently
            // instead of turning an internal lifecycle mismatch into a process panic.
            TimeSourceSubscriberState::None => true,
        }
    }
}

impl TimeSource {
    // This is similar to the internal rclcpp `set_all_clocks` helper.
    fn set_clock_time(&self, time_msg: Time) {
        let _operation = self.operations.lock();
        let time = time_msg.into();
        let clocks = {
            let mut inner = self.inner.lock();
            inner.last_time_msg = time;
            inner.live_managed_clocks()
        };
        if let Err(err) = try_for_each_clock(&clocks, |clock| clock.set_ros_time_override(time)) {
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
            false,
            crate::UniqueNetworkFlowRequirement::NotRequired,
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

    fn handle_incoming(&mut self) -> Result<crate::subscribers::SubscriptionHandleResult> {
        // update clock
        let mut msg_info = rmw_message_info_t::default(); // we dont care for now
        let mut clock_msg = WrappedNativeMsg::<rosgraph_msgs::msg::Clock>::try_new()?;
        let ret = unsafe {
            rcl_take(
                &self.subscriber_handle,
                clock_msg.void_ptr_mut(),
                &mut msg_info,
                std::ptr::null_mut(),
            )
        };

        if ret != RCL_RET_OK as i32 && ret != RCL_RET_SUBSCRIPTION_TAKE_FAILED as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let result = if ret == RCL_RET_OK as i32 {
            self.time_source
                .set_clock_time(rosgraph_msgs::msg::Clock::from_native(&clock_msg).clock);
            crate::subscribers::SubscriptionHandleResult::Taken
        } else {
            crate::subscribers::SubscriptionHandleResult::Idle
        };

        if self
            .time_source
            .inner
            .lock()
            .subscriber_should_remove_after_take()
        {
            Ok(crate::subscribers::SubscriptionHandleResult::Remove)
        } else {
            Ok(result)
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
    fn subscriber_post_take_state_is_panic_free_and_idempotent() {
        let mut inner = TimeSource_::new();
        assert!(inner.subscriber_should_remove_after_take());
        assert!(matches!(inner.subscriber_state, TimeSourceSubscriberState::None));

        inner.subscriber_state = TimeSourceSubscriberState::Active;
        assert!(!inner.subscriber_should_remove_after_take());
        assert!(matches!(inner.subscriber_state, TimeSourceSubscriberState::Active));

        inner.subscriber_state = TimeSourceSubscriberState::ToBeDestroyed;
        assert!(inner.subscriber_should_remove_after_take());
        assert!(matches!(inner.subscriber_state, TimeSourceSubscriberState::None));
        assert!(inner.subscriber_should_remove_after_take());
    }

    #[test]
    fn failed_subscription_setup_does_not_activate_time_source() {
        let mut inner = TimeSource_::new();

        let result = inner.activate_subscriber::<()>(|| Err(Error::RCL_RET_ERROR));

        assert!(result.is_err());
        assert!(!inner.simulated_time_enabled);
        assert!(matches!(inner.subscriber_state, TimeSourceSubscriberState::None));
    }

    #[test]
    fn latest_sim_time_request_wins_before_spin() {
        let time_source = TimeSource::new();

        time_source.request_sim_time(true);
        time_source.request_sim_time(false);

        assert!(time_source.inner.lock().requested_simulated_time.is_none());
    }

    #[test]
    fn managed_clock_operations_do_not_hold_the_time_source_state_lock() {
        let time_source = TimeSource::new();
        let clock = Arc::new(Mutex::new(Clock::create(ClockType::RosTime).unwrap()));
        time_source
            .inner
            .lock()
            .managed_clocks
            .push(Arc::downgrade(&clock));
        let clocks = time_source.inner.lock().live_managed_clocks();
        let mut visited = false;

        try_for_each_clock(&clocks, |_| {
            assert!(time_source.inner.try_lock().is_some());
            visited = true;
            Ok(())
        })
        .unwrap();

        assert!(visited);
    }
}
