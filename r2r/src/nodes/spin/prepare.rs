use super::*;

impl Node {
    pub(in crate::nodes) fn prepare_spin(&mut self, timeout: Duration) -> Result<SpinPreparation> {
        if !self.context.is_valid() {
            return Err(Error::RCL_RET_NOT_INIT);
        }

        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        {
            let time_source = self.time_source.clone();
            time_source.apply_requested_sim_time(self)?;
        }

        self.graph_change_waiters.prune_canceled();

        let canceled_subscriptions = {
            let mut canceled_subscriptions = self.canceled_subscriptions.lock();
            canceled_subscriptions.drain(..).collect::<Vec<_>>()
        };
        let node_handle = self.node_handle.as_mut();
        for id in canceled_subscriptions {
            Self::remove_subscription(
                &mut self.subscribers,
                &mut self.subscription_indexes,
                id,
                node_handle,
            );
        }

        // Internal subscriptions, such as `/clock`, have no public stream whose Drop can report
        // an ID. Only inspect them when their owning subsystem has requested a state transition.
        #[cfg(r2r__rosgraph_msgs__msg__Clock)]
        if self.time_source.has_pending_subscriber_removal() {
            let previous_subscriber_count = self.subscribers.len();
            self.subscribers.retain_mut(|subscriber| {
                if subscriber.match_state().is_none() && subscriber.should_remove() {
                    subscriber.destroy(node_handle);
                    false
                } else {
                    true
                }
            });
            if self.subscribers.len() != previous_subscriber_count {
                Self::rebuild_subscription_indexes(
                    &self.subscribers,
                    &mut self.subscription_indexes,
                );
            }
        }

        let canceled_services = {
            let mut canceled_services = self.canceled_services.lock();
            canceled_services.drain(..).collect::<Vec<_>>()
        };
        let node_handle = self.node_handle.as_mut();
        for id in canceled_services {
            Self::remove_service(&mut self.services, &mut self.service_indexes, id, node_handle);
        }

        {
            let mut dropped = self.dropped_clients.lock();
            self.pending_client_removals.extend(dropped.drain(..));
        }
        let pending_client_removals = std::mem::take(&mut self.pending_client_removals);
        for identity in pending_client_removals {
            if !self.remove_client(identity) {
                self.pending_client_removals.insert(identity);
            }
        }

        {
            let mut dropped = self.dropped_publishers.lock();
            self.pending_publisher_removals.extend(dropped.drain(..));
        }
        let pending_publisher_removals = std::mem::take(&mut self.pending_publisher_removals);
        for identity in pending_publisher_removals {
            if !self.remove_dropped_publisher(identity) {
                self.pending_publisher_removals.insert(identity);
            }
        }

        // Do not wait for a long-period timer to become ready before noticing that its public
        // handle has gone away. Timer::drop reports the exact UUID, avoiding a full timer scan.
        let canceled_timers = {
            let mut canceled_timers = self.canceled_timers.lock();
            canceled_timers.drain(..).collect::<Vec<_>>()
        };
        for id in canceled_timers {
            Self::remove_timer(&mut self.timers, &mut self.timer_indexes, id);
        }

        {
            let mut dropped = self.dropped_action_clients.lock();
            self.pending_action_client_removals
                .extend(dropped.drain(..));
        }
        let pending_action_client_removals =
            std::mem::take(&mut self.pending_action_client_removals);
        for identity in pending_action_client_removals {
            match self.remove_action_client(identity) {
                Ok(true) => {}
                Ok(false) => {
                    self.pending_action_client_removals.insert(identity);
                }
                Err(err) => {
                    log::warn!("could not inspect dropped action client: {err}");
                    self.pending_action_client_removals.insert(identity);
                }
            }
        }

        // Only action servers with an in-flight cancel delivery or decision need maintenance
        // before waiting. Keep them registered until every response has been sent.
        let action_servers_with_pending_cancels = self
            .action_servers_with_pending_cancels
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let mut local_action_progress = false;
        for id in action_servers_with_pending_cancels {
            let Some(index) = self.action_server_indexes.get(&id).copied() else {
                self.action_servers_with_pending_cancels.remove(&id);
                continue;
            };
            let action_server = &self.action_servers[index];
            record_action_server_maintenance_poll();
            let maintenance_result =
                complete_action_server_operation(action_server, |server, completions| {
                    completions
                        .extend(server.take_pending_cancel_delivery(Arc::clone(action_server)));
                    let delivery_completions = completions.len();
                    server
                        .send_completed_cancel_requests(Arc::clone(action_server), completions)?;
                    local_action_progress |= completions.len() > delivery_completions;
                    Ok(())
                });
            if !action_server.lock().has_pending_cancel_work() {
                self.action_servers_with_pending_cancels.remove(&id);
            }
            maintenance_result?;
        }

        {
            let mut dropped = self.dropped_action_servers.lock();
            self.pending_action_server_removals
                .extend(dropped.drain(..));
        }
        let pending_action_server_removals =
            std::mem::take(&mut self.pending_action_server_removals);
        for id in pending_action_server_removals {
            match self.remove_action_server(id) {
                Ok(true) => {}
                Ok(false) => {
                    self.pending_action_server_removals.insert(id);
                }
                Err(err) => {
                    log::warn!("could not inspect dropped action server: {err}");
                    self.pending_action_server_removals.insert(id);
                }
            }
        }

        {
            let mut active = self.active_availability_clients.lock();
            self.clients_with_active_availability
                .extend(active.drain(..));
        }
        let clients_with_active_availability =
            std::mem::take(&mut self.clients_with_active_availability);
        for identity in clients_with_active_availability {
            let Some(index) = self.client_identities.get(&identity).copied() else {
                continue;
            };
            let client = &self.clients[index];
            complete_client_operation(client, |client| {
                Ok(client.poll_available(self.node_handle.as_mut()))
            })?;
            if client.lock().has_pending_availability() {
                self.clients_with_active_availability.insert(identity);
            }
        }

        {
            let mut active = self.active_maintenance_action_clients.lock();
            self.action_clients_with_pending_maintenance
                .extend(active.drain(..));
        }
        let action_clients_with_pending_maintenance =
            std::mem::take(&mut self.action_clients_with_pending_maintenance);
        for identity in action_clients_with_pending_maintenance {
            let Some(index) = self.action_client_identities.get(&identity).copied() else {
                continue;
            };
            let client = &self.action_clients[index];
            record_action_client_maintenance_poll();
            let maintenance_result = complete_action_client_operation(
                client,
                |client, completions| {
                    completions.extend(client.poll_maintenance());
                    Ok(())
                },
                |client, uuid| client.forget_goal(uuid),
            );
            if client.lock().has_pending_maintenance() {
                self.action_clients_with_pending_maintenance
                    .insert(identity);
            }
            maintenance_result?;
        }

        {
            let mut active = self.active_availability_action_clients.lock();
            self.action_clients_with_active_availability
                .extend(active.drain(..));
        }
        let active_action_clients =
            std::mem::take(&mut self.action_clients_with_active_availability);
        for identity in active_action_clients {
            let Some(index) = self.action_client_identities.get(&identity).copied() else {
                continue;
            };
            let client = &self.action_clients[index];
            complete_action_client_operation(
                client,
                |client, completions| {
                    completions.extend(client.poll_available(self.node_handle.as_mut()));
                    Ok(())
                },
                |client, uuid| client.forget_goal(uuid),
            )?;
            if client.lock().has_pending_availability() {
                self.action_clients_with_active_availability
                    .insert(identity);
            }
        }

        {
            let mut active = self.active_wait_publishers.lock();
            self.publishers_with_active_waits.extend(active.drain(..));
        }
        let publishers_with_active_waits = std::mem::take(&mut self.publishers_with_active_waits);
        let mut ack_query_pending = false;
        let mut publisher_matching_pending = false;
        for id in publishers_with_active_waits {
            let Some(index) = self.publisher_indexes.get(&id).copied() else {
                continue;
            };
            if self.pubs[index].poll_waits() {
                ack_query_pending |= self.pubs[index].has_pending_ack_waits();
                publisher_matching_pending |= self.pubs[index].has_pending_matching_waits();
                self.publishers_with_active_waits.insert(id);
            }
        }
        let subscriptions_with_active_waits =
            std::mem::take(&mut self.subscriptions_with_active_waits);
        for id in subscriptions_with_active_waits {
            let Some(index) = self.subscription_indexes.get(&id).copied() else {
                continue;
            };
            let subscriber = &mut self.subscribers[index];
            subscriber.poll_has_publishers();
            if subscriber.has_pending_publisher_waits() {
                self.subscriptions_with_active_waits.insert(id);
            }
        }

        // Completed local cancel work may wake application logic before the native wait.
        // Drain already-ready native events, then let the application observe the transition
        // without sleeping through the caller's timeout before it can publish a final result.
        let timeout_nanos = duration_to_rcl_nanos(timeout)?;
        let (timeout, timeout_nanos) = if local_action_progress {
            (Duration::ZERO, 0)
        } else if ack_query_pending {
            // RCL exposes acknowledgements as a query rather than a wait-set event. Only
            // pending ACK observers need this bounded fallback, including wake-driven runs.
            let wait = timeout.min(Duration::from_millis(10));
            (wait, duration_to_rcl_nanos(wait)?)
        } else {
            (timeout, timeout_nanos)
        };

        // #[doc = "* This function is thread-safe for unique wait sets with unique contents."]
        // #[doc = "* This function cannot operate on the same wait set in multiple threads, and"]
        // #[doc = "* the wait sets may not share content."]
        // #[doc = "* For example, calling rcl_wait() in two threads on two different wait sets"]
        // #[doc = "* that both contain a single, shared guard condition is undefined behavior."]

        let future_guard = self
            .future_wake_guard
            .as_ref()
            .filter(|guard| guard.active.load(std::sync::atomic::Ordering::Acquire))
            .cloned();
        let graph_interested = !self.graph_change_waiters.is_empty()
            || !self.clients_with_active_availability.is_empty()
            || !self.action_clients_with_active_availability.is_empty()
            || publisher_matching_pending
            || !self.subscriptions_with_active_waits.is_empty();
        let capacities = WaitSetCapacities {
            subscriptions: self.subscribers.len() + self.action_wait_set_capacities.subscriptions,
            guard_conditions: self.action_wait_set_capacities.guard_conditions
                + usize::from(graph_interested)
                + usize::from(future_guard.is_some()),
            timers: self.timers.len() + self.action_wait_set_capacities.timers,
            clients: self.clients.len() + self.action_wait_set_capacities.clients,
            services: self.services.len() + self.action_wait_set_capacities.services,
        };

        Ok(SpinPreparation {
            timeout,
            timeout_nanos,
            capacities,
            future_guard,
            graph_interested,
        })
    }
}
