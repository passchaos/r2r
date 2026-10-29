//! Attach native wakes to background-owned observer and work queues.

use super::Node;
use std::task::Waker;

impl Node {
    pub(in crate::nodes) fn set_background_wake(&mut self, wake: Option<Waker>) {
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
