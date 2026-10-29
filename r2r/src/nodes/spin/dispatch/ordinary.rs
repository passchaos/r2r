use super::*;

impl WaitEntities<'_> {
    pub(super) fn dispatch_ordinary(
        &mut self, ws: &rcl_wait_set_t, offsets: EntityOffsets, errors: &mut DispatchErrors,
    ) -> Result<()> {
        if let Some(index) = offsets.graph_guard {
            let graph_changed = !ws.guard_conditions.is_null()
                && unsafe { !(*ws.guard_conditions.add(index)).is_null() };
            if graph_changed {
                for waiter in self.graph_change_waiters.take_all() {
                    let _ = waiter.send(());
                }
            }
        }

        if let Some(events) = self.qos_events.as_ref() {
            errors.record(events.dispatch(ws, offsets.events));
        }

        let mut subs_to_remove = HashSet::new();
        if !ws.subscriptions.is_null() {
            let ws_subs = unsafe {
                std::slice::from_raw_parts(
                    ws.subscriptions.add(offsets.subscribers),
                    self.subscribers.len(),
                )
            };
            for (s, ws_s) in self.subscribers.iter_mut().zip(ws_subs) {
                if ws_s != &std::ptr::null() {
                    match s.handle_ready() {
                        Ok(true) => {
                            subs_to_remove.insert(s.handle() as *const rcl_subscription_t);
                        }
                        Ok(false) => {}
                        Err(err) => {
                            errors.error(err);
                        }
                    }
                }
            }
        }
        if !subs_to_remove.is_empty() {
            let node_handle = &mut *self.node_handle;
            self.subscribers.retain_mut(|subscriber| {
                if subs_to_remove.contains(&(subscriber.handle() as *const rcl_subscription_t)) {
                    if let (Some(events), Some(state)) =
                        (self.qos_events.as_mut(), subscriber.match_state())
                    {
                        events.remove_endpoint(qos_events::EventEndpoint::Subscription(state.id));
                    }
                    subscriber.destroy(node_handle);
                    false
                } else {
                    true
                }
            });
            Node::rebuild_subscription_indexes(self.subscribers, self.subscription_indexes);
        }
        // Clock samples are applied by subscribers above. Complete due ROS sleeps in the same
        // dispatch, while the shared native timer never enters an ordinary timer's tick channel.
        if let Some(index) = offsets.ros_sleep_timer {
            if ws.timers.is_null() || index >= ws.size_of_timers {
                return Err(Error::RCL_RET_WAIT_SET_INVALID);
            }
            if let Some(sleeps) = self.ros_sleeps.as_mut() {
                let ready = unsafe { *ws.timers.add(index) };
                if !ready.is_null() && ready != sleeps.handle() {
                    return Err(Error::RCL_RET_WAIT_SET_INVALID);
                }
                errors.record(sleeps.sync(!ready.is_null()).map(|_| ()));
            }
        }

        let mut timers_to_remove = Vec::new();
        if !ws.timers.is_null() {
            let ws_timers = unsafe {
                std::slice::from_raw_parts(ws.timers.add(offsets.timers), self.timers.len())
            };
            for (s, ws_s) in self.timers.iter_mut().zip(ws_timers) {
                if ws_s != &std::ptr::null() {
                    // TODO: move this to impl Timer
                    match s.handle_incoming() {
                        Ok(true) => {
                            timers_to_remove.push(s.id);
                        }
                        Ok(false) => {}
                        Err(err) => {
                            errors.error(err);
                        }
                    }
                }
            }
        }
        for id in timers_to_remove {
            Node::remove_timer(self.timers, self.timer_indexes, id);
        }
        if let Some(callbacks) = self.timer_callbacks.as_mut() {
            errors.record(callbacks.dispatch(ws, offsets.timer_callbacks));
        }

        if !ws.clients.is_null() {
            let ws_clients = unsafe {
                std::slice::from_raw_parts(ws.clients.add(offsets.clients), self.clients.len())
            };
            for (s, ws_s) in self.clients.iter().zip(ws_clients) {
                if ws_s != &std::ptr::null() {
                    errors.record(complete_client_responses(s, Client_::handle_response));
                }
            }
        }

        let mut services_to_remove = Vec::new();
        if !ws.services.is_null() {
            let ws_services = unsafe {
                std::slice::from_raw_parts(ws.services.add(offsets.services), self.services.len())
            };
            for (s, ws_s) in self.services.iter_mut().zip(ws_services) {
                if ws_s != &std::ptr::null() {
                    match complete_ready_service(s) {
                        Ok(ServiceHandleResult::Remove) => {
                            if let Some(id) = s.lock().id() {
                                services_to_remove.push(id);
                            }
                        }
                        Ok(ServiceHandleResult::Taken | ServiceHandleResult::Idle) => {}
                        Err(err) => {
                            errors.error(err);
                        }
                    }
                }
            }
        }
        let node_handle = &mut *self.node_handle;
        for id in services_to_remove {
            Node::remove_service(self.services, self.service_indexes, id, node_handle);
        }
        if let Some(callbacks) = self.service_callbacks.as_mut() {
            if let Err(failure) = callbacks.dispatch(ws, offsets.service_callbacks) {
                errors.error(failure);
            }
        }

        Ok(())
    }
}
