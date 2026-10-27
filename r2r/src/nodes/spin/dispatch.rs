use super::*;

impl WaitEntities<'_> {
    pub(in crate::nodes) fn dispatch(
        &mut self, ws: &rcl_wait_set_t, offsets: EntityOffsets,
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

        let mut subs_to_remove = HashSet::new();
        let mut subscriber_error = None;
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
                            subscriber_error.get_or_insert(err);
                        }
                    }
                }
            }
        }
        if !subs_to_remove.is_empty() {
            let node_handle = &mut *self.node_handle;
            self.subscribers.retain_mut(|subscriber| {
                if subs_to_remove.contains(&(subscriber.handle() as *const rcl_subscription_t)) {
                    subscriber.destroy(node_handle);
                    false
                } else {
                    true
                }
            });
            Node::rebuild_subscription_indexes(self.subscribers, self.subscription_indexes);
        }
        if let Some(err) = subscriber_error {
            return Err(err);
        }

        let mut timers_to_remove = Vec::new();
        let mut timer_error = None;
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
                            timer_error.get_or_insert(err);
                        }
                    }
                }
            }
        }
        for id in timers_to_remove {
            Node::remove_timer(self.timers, self.timer_indexes, id);
        }
        if let Some(err) = timer_error {
            return Err(err);
        }

        if !ws.clients.is_null() {
            let ws_clients = unsafe {
                std::slice::from_raw_parts(ws.clients.add(offsets.clients), self.clients.len())
            };
            for (s, ws_s) in self.clients.iter().zip(ws_clients) {
                if ws_s != &std::ptr::null() {
                    complete_client_responses(s, Client_::handle_response)?;
                }
            }
        }

        let mut services_to_remove = Vec::new();
        let mut service_error = None;
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
                            service_error.get_or_insert(err);
                        }
                    }
                }
            }
        }
        let node_handle = &mut *self.node_handle;
        for id in services_to_remove {
            Node::remove_service(self.services, self.service_indexes, id, node_handle);
        }
        if let Some(err) = service_error {
            return Err(err);
        }

        for ac in self.action_clients.iter() {
            let (
                is_feedback_ready,
                is_status_ready,
                is_goal_response_ready,
                is_cancel_response_ready,
                is_result_response_ready,
            ) = {
                let ac = ac.lock();
                let mut is_feedback_ready = false;
                let mut is_status_ready = false;
                let mut is_goal_response_ready = false;
                let mut is_cancel_response_ready = false;
                let mut is_result_response_ready = false;
                let ret = unsafe {
                    rcl_action_client_wait_set_get_entities_ready(
                        ws,
                        ac.handle(),
                        &mut is_feedback_ready,
                        &mut is_status_ready,
                        &mut is_goal_response_ready,
                        &mut is_cancel_response_ready,
                        &mut is_result_response_ready,
                    )
                };
                if ret != RCL_RET_OK as i32 {
                    return Err(Error::from_rcl_error(ret));
                }
                (
                    is_feedback_ready,
                    is_status_ready,
                    is_goal_response_ready,
                    is_cancel_response_ready,
                    is_result_response_ready,
                )
            };

            let ac_ref = Arc::clone(ac);
            if is_feedback_ready {
                complete_action_feedback_batch(
                    ac,
                    |client, completions| client.handle_feedback_msg(ac_ref.clone(), completions),
                    |client, uuid| client.forget_goal(uuid),
                )?;
            }
            complete_action_client_operation(
                ac,
                |ac, completions| {
                    if is_status_ready {
                        completions.extend(ac.handle_status_msg(ac_ref.clone())?);
                    }
                    if is_goal_response_ready {
                        collect_action_client_responses(ac, completions, |ac, completions| {
                            ac.handle_goal_response(ac_ref.clone(), completions)
                        })?;
                    }
                    if is_cancel_response_ready {
                        collect_action_client_responses(ac, completions, |ac, completions| {
                            ac.handle_cancel_response(completions)
                        })?;
                    }
                    if is_result_response_ready {
                        collect_action_client_responses(ac, completions, |ac, completions| {
                            ac.handle_result_response(ac_ref.clone(), completions)
                        })?;
                    }
                    Ok(())
                },
                |client, uuid| client.forget_goal(uuid),
            )?;
        }

        for s in self.action_servers.iter() {
            let (
                server_id,
                is_goal_request_ready,
                is_cancel_request_ready,
                is_result_request_ready,
                is_goal_expired,
            ) = {
                let server = s.lock();
                let mut is_goal_request_ready = false;
                let mut is_cancel_request_ready = false;
                let mut is_result_request_ready = false;
                let mut is_goal_expired = false;

                let ret = unsafe {
                    rcl_action_server_wait_set_get_entities_ready(
                        ws,
                        server.handle(),
                        &mut is_goal_request_ready,
                        &mut is_cancel_request_ready,
                        &mut is_result_request_ready,
                        &mut is_goal_expired,
                    )
                };

                if ret != RCL_RET_OK as i32 {
                    return Err(Error::from_rcl_error(ret));
                }
                (
                    server.id(),
                    is_goal_request_ready,
                    is_cancel_request_ready,
                    is_result_request_ready,
                    is_goal_expired,
                )
            };

            if is_goal_request_ready {
                complete_action_server_request_batch(s, |server| {
                    server.handle_goal_request(s.clone())
                })?;
            }
            if is_cancel_request_ready {
                let cancel_result = complete_action_server_delivery_batch(
                    s,
                    |server| server.handle_cancel_request(s.clone()),
                    ActionServer_::has_pending_cancel_delivery,
                );
                if s.lock().has_pending_cancel_work() {
                    self.action_servers_with_pending_cancels.insert(server_id);
                } else {
                    self.action_servers_with_pending_cancels.remove(&server_id);
                }
                cancel_result?;
            }
            if is_result_request_ready {
                complete_action_server_take_batch(s, |server| {
                    server.handle_result_request(s.clone())
                })?;
            }
            complete_action_server_operation(s, |server, completions| {
                if is_goal_expired {
                    server.handle_goal_expired(completions)?;
                }
                Ok(())
            })?;
        }

        Ok(())
    }
}
