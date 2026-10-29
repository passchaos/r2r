//! Attach the current native wake to observer and work queues.

use super::Node;
use std::task::Waker;

impl Node {
    pub(in crate::nodes) fn set_native_observer_wake(&mut self, wake: Option<Waker>) {
        // Queue storage already identifies the bound owner. Factories inherit that wake,
        // so repeated future runs need no entity scan or additional node metadata.
        let current = self.canceled_subscriptions.lock().wake();
        if match (&current, &wake) {
            (Some(current), Some(wake)) => current.will_wake(wake),
            (None, None) => true,
            _ => false,
        } {
            return;
        }
        if let Some(callbacks) = &self.service_callbacks {
            callbacks.set_wake(wake.clone());
        }
        if let Some(events) = &self.qos_events {
            events.set_wake(wake.clone());
        }
        if let Some(sleeps) = &self.ros_sleeps {
            sleeps.set_wake(wake.clone());
        }
        self.canceled_subscriptions.lock().set_wake(wake.clone());
        self.canceled_services.lock().set_wake(wake.clone());
        self.dropped_clients.lock().set_wake(wake.clone());
        self.dropped_publishers.lock().set_wake(wake.clone());
        self.canceled_timers.lock().set_wake(wake.clone());
        self.dropped_action_clients.lock().set_wake(wake.clone());
        self.dropped_action_servers.lock().set_wake(wake.clone());
        self.active_maintenance_action_clients
            .lock()
            .set_wake(wake.clone());
        self.graph_change_waiters.set_native_wake(wake.clone());
        for subscriber in &mut self.subscribers {
            if let Some(state) = subscriber.match_state_mut() {
                state.waiters.set_native_wake(wake.clone());
            }
        }
        for client in &self.clients {
            client.lock().set_availability_wake(wake.clone());
        }
        for client in &self.action_clients {
            client.lock().set_availability_wake(wake.clone());
        }
        for server in &self.action_servers {
            server.lock().set_maintenance_wake(wake.clone());
        }
        for publisher in &self.pubs {
            publisher.set_observer_wake(wake.clone());
        }
    }
}

#[cfg(all(test, not(feature = "doc-only")))]
mod tests;
