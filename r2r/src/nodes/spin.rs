use super::*;
mod dispatch;
mod prepare;

pub(super) struct SpinPreparation {
    pub(super) timeout: Duration,
    pub(super) timeout_nanos: i64,
    pub(super) capacities: WaitSetCapacities,
    pub(super) future_guard: Option<Arc<future_executor::FutureGuard>>,
    graph_interested: bool,
    ros_sleep_interested: bool,
}

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct EntityOffsets {
    subscribers: usize,
    timers: usize,
    clients: usize,
    services: usize,
    graph_guard: Option<usize>,
    events: usize,
    ros_sleep_timer: Option<usize>,
    service_callbacks: usize,
    timer_callbacks: usize,
}

pub(super) struct WaitEntities<'a> {
    context: &'a Context,
    qos_events: &'a mut Option<Box<qos_events::QosEvents>>,
    ros_sleeps: &'a mut Option<Box<ros_sleep::RosSleeps>>,
    service_callbacks: &'a mut Option<Box<service_callbacks::ServiceCallbacks>>,
    timer_callbacks: &'a mut Option<Box<timer_callbacks::TimerCallbacks>>,
    grouped_callbacks: &'a mut Option<Box<callback_executor::sources::Sources>>,
    node_handle: &'a mut rcl_node_t,
    subscribers: &'a mut Vec<Box<dyn Subscriber_>>,
    subscription_indexes: &'a mut HashMap<uuid::Uuid, usize>,
    timers: &'a mut Vec<Timer_>,
    timer_indexes: &'a mut HashMap<uuid::Uuid, usize>,
    clients: &'a Vec<Arc<Mutex<dyn Client_>>>,
    services: &'a mut Vec<Arc<Mutex<dyn Service_>>>,
    service_indexes: &'a mut HashMap<uuid::Uuid, usize>,
    action_clients: &'a Vec<Arc<Mutex<dyn ActionClient_>>>,
    action_servers: &'a Vec<Arc<Mutex<dyn ActionServer_>>>,
    action_server_notifications: &'a Arc<Mutex<crate::action_servers::ActionServerNotifications>>,
    graph_change_waiters: &'a mut AvailabilityWaiters,
    future_wake_state: &'a Option<Arc<future_executor::FutureWake>>,
}

impl Node {
    pub(super) fn wait_entities(&mut self) -> (&mut WaitSet, WaitEntities<'_>) {
        (
            &mut self.wait_set,
            WaitEntities {
                context: &self.context,
                qos_events: &mut self.qos_events,
                ros_sleeps: &mut self.ros_sleeps,
                service_callbacks: &mut self.service_callbacks,
                timer_callbacks: &mut self.timer_callbacks,
                grouped_callbacks: &mut self.grouped_callbacks,
                node_handle: &mut self.node_handle,
                subscribers: &mut self.subscribers,
                subscription_indexes: &mut self.subscription_indexes,
                timers: &mut self.timers,
                timer_indexes: &mut self.timer_indexes,
                clients: &self.clients,
                services: &mut self.services,
                service_indexes: &mut self.service_indexes,
                action_clients: &self.action_clients,
                action_servers: &self.action_servers,
                action_server_notifications: &self.dropped_action_servers,
                graph_change_waiters: &mut self.graph_change_waiters,
                future_wake_state: &self.future_wake_state,
            },
        )
    }
}

impl WaitEntities<'_> {
    pub(super) fn add_ordinary(
        &mut self, ws: &mut rcl_wait_set_t, preparation: &SpinPreparation,
    ) -> Result<EntityOffsets> {
        let mut offsets = EntityOffsets::default();
        for (position, s) in self.subscribers.iter().enumerate() {
            let mut index = 0;
            check_rcl_ret(unsafe { rcl_wait_set_add_subscription(ws, s.handle(), &mut index) })?;
            if position == 0 {
                offsets.subscribers = index;
            }
        }

        for (position, s) in self.timers.iter().enumerate() {
            let mut index = 0;
            check_rcl_ret(unsafe { rcl_wait_set_add_timer(ws, s.get_handle(), &mut index) })?;
            if position == 0 {
                offsets.timers = index;
            }
        }

        for (position, s) in self.clients.iter().enumerate() {
            let mut index = 0;
            check_rcl_ret(unsafe { rcl_wait_set_add_client(ws, s.lock().handle(), &mut index) })?;
            if position == 0 {
                offsets.clients = index;
            }
        }
        if preparation.ros_sleep_interested {
            let sleeps = self
                .ros_sleeps
                .as_ref()
                .ok_or(Error::RCL_RET_NODE_INVALID)?;
            let mut index = 0;
            check_rcl_ret(unsafe { rcl_wait_set_add_timer(ws, sleeps.handle(), &mut index) })?;
            offsets.ros_sleep_timer = Some(index);
        }
        if let Some(callbacks) = self.timer_callbacks.as_ref() {
            offsets.timer_callbacks = callbacks.add_to_wait_set(ws)?;
        }

        for (position, s) in self.services.iter().enumerate() {
            let mut index = 0;
            check_rcl_ret(unsafe { rcl_wait_set_add_service(ws, s.lock().handle(), &mut index) })?;
            if position == 0 {
                offsets.services = index;
            }
        }

        if let Some(guard) = &preparation.future_guard {
            check_rcl_ret(unsafe {
                rcl_wait_set_add_guard_condition(ws, guard.handle(), std::ptr::null_mut())
            })?;
        }
        if let Some(callbacks) = self.service_callbacks.as_ref() {
            offsets.service_callbacks = callbacks.add_to_wait_set(ws)?;
        }
        let graph_guard_index = if !preparation.graph_interested {
            None
        } else {
            let graph_guard = unsafe { rcl_node_get_graph_guard_condition(&*self.node_handle) };
            if graph_guard.is_null() {
                return Err(Error::RCL_RET_NODE_INVALID);
            }
            let mut index = 0;
            check_rcl_ret(unsafe {
                rcl_wait_set_add_guard_condition(ws, graph_guard, &mut index)
            })?;
            Some(index)
        };

        if let Some(events) = self.qos_events.as_ref() {
            offsets.events = events.add_to_wait_set(ws)?;
        }
        offsets.graph_guard = graph_guard_index;
        if let Some(callbacks) = self.grouped_callbacks.as_mut() {
            callbacks.add_to_wait_set(ws)?;
        }
        Ok(offsets)
    }
    pub(super) fn add_actions(&self, ws: &mut rcl_wait_set_t) -> Result<()> {
        for ac in self.action_clients.iter() {
            check_rcl_ret(unsafe {
                rcl_action_wait_set_add_action_client(
                    ws,
                    ac.lock().handle(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            })?;
        }
        for acs in self.action_servers.iter() {
            check_rcl_ret(unsafe {
                rcl_action_wait_set_add_action_server(ws, acs.lock().handle(), std::ptr::null_mut())
            })?;
        }

        Ok(())
    }
}

pub(super) fn spin_once(node: &mut Node, timeout: Duration) -> Result<()> {
    let _event_wait = node.activate_observer_guard()?;
    let preparation = node.prepare_spin(timeout)?;
    let (wait_set, mut entities) = node.wait_entities();
    if preparation.capacities == WaitSetCapacities::default() {
        return entities
            .context
            .wait_for_shutdown_or_timeout(preparation.timeout);
    }
    {
        let mut context = entities.context.context_handle.lock();
        wait_set.prepare(preparation.capacities, context.as_mut())?;
    }
    let offsets = entities.add_ordinary(&mut wait_set.handle, &preparation)?;
    entities.add_actions(&mut wait_set.handle)?;
    wait_set.refresh_event_layout(entities.context)?;
    #[cfg(test)]
    if let Some(guard) = &preparation.future_guard {
        guard.before_wait(preparation.timeout_nanos);
    }
    let ret = unsafe { rcl_wait(&mut wait_set.handle, preparation.timeout_nanos) };
    if preparation.future_guard.is_some() {
        if let Some(wake) = entities.future_wake_state {
            wake.finish_native_wait();
        }
    }
    #[cfg(test)]
    if let Some(guard) = &preparation.future_guard {
        guard.after_wait(ret);
    }
    if ret == RCL_RET_TIMEOUT as i32 {
        return Ok(());
    }
    check_rcl_ret(ret)?;
    entities.dispatch(&wait_set.handle, offsets)
}
