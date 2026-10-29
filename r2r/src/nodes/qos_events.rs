//! Native QoS event registration, readiness spans and endpoint cleanup.

use super::*;
use crate::qos_events::{
    native::NativeEvent, EventChannel, PublisherEventKind, QosEventStream, SubscriptionEventKind,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::nodes) enum EventEndpoint {
    Publisher(uuid::Uuid),
    Subscription(uuid::Uuid),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EventKind {
    Publisher(PublisherEventKind),
    Subscription(SubscriptionEventKind),
}

struct Registration {
    id: uuid::Uuid,
    endpoint: EventEndpoint,
    kind: EventKind,
    native: Option<NativeEvent>,
    channel: Arc<EventChannel>,
    notify_on_drop: bool,
}

impl Drop for Registration {
    fn drop(&mut self) {
        drop(self.native.take());
        if !self.notify_on_drop {
            return;
        }
        let error = match self.endpoint {
            EventEndpoint::Publisher(_) => Error::RCL_RET_PUBLISHER_INVALID,
            EventEndpoint::Subscription(_) => Error::RCL_RET_SUBSCRIPTION_INVALID,
        };
        self.channel.close(error);
    }
}

/// Nodes without observers allocate no event registry, channels, queue or wait-set event storage.
#[derive(Default)]
pub(in crate::nodes) struct QosEvents {
    // Stable storage for every rcl_event_t pointer retained by an active native wait set.
    #[allow(clippy::vec_box)] // Native wait sets retain each event address across registry growth.
    registrations: Vec<Box<Registration>>,
    indexes: HashMap<uuid::Uuid, usize>,
    owners: HashMap<(EventEndpoint, EventKind), uuid::Uuid>,
    active: Vec<usize>,
    notifications: Arc<Mutex<crate::notifications::NotificationQueue<uuid::Uuid>>>,
}

impl QosEvents {
    fn contains(&self, endpoint: EventEndpoint, kind: EventKind) -> bool {
        self.owners.contains_key(&(endpoint, kind))
    }

    fn insert(
        &mut self, endpoint: EventEndpoint, kind: EventKind, native: NativeEvent,
    ) -> QosEventStream {
        let id = uuid::Uuid::new_v4();
        let channel = Arc::new(EventChannel::default());
        self.indexes.insert(id, self.registrations.len());
        self.owners.insert((endpoint, kind), id);
        self.registrations.push(Box::new(Registration {
            id,
            endpoint,
            kind,
            native: Some(native),
            channel: channel.clone(),
            notify_on_drop: true,
        }));
        QosEventStream::new(id, channel, self.notifications.clone())
    }

    fn remove(&mut self, id: uuid::Uuid) {
        let Some(index) = self.indexes.remove(&id) else {
            return;
        };
        let registration = self.registrations.swap_remove(index);
        self.owners
            .remove(&(registration.endpoint, registration.kind));
        if let Some(moved) = self.registrations.get(index) {
            self.indexes.insert(moved.id, index);
        }
        drop(registration);
    }

    pub(in crate::nodes) fn remove_endpoint(&mut self, endpoint: EventEndpoint) {
        // Endpoint removal is cold; only event registrations are visited, never native endpoints.
        let ids: Vec<_> = self
            .registrations
            .iter()
            .filter(|event| event.endpoint == endpoint)
            .map(|event| event.id)
            .collect();
        for id in ids {
            self.remove(id);
        }
    }

    pub(in crate::nodes) fn defer_node_drop_notifications(
        &mut self,
    ) -> Vec<(Arc<EventChannel>, Error)> {
        self.registrations
            .iter_mut()
            .map(|registration| {
                registration.notify_on_drop = false;
                let error = match registration.endpoint {
                    EventEndpoint::Publisher(_) => Error::RCL_RET_PUBLISHER_INVALID,
                    EventEndpoint::Subscription(_) => Error::RCL_RET_SUBSCRIPTION_INVALID,
                };
                (registration.channel.clone(), error)
            })
            .collect()
    }

    pub(in crate::nodes) fn set_wake(&self, wake: Option<std::task::Waker>) {
        self.notifications.lock().set_wake(wake);
    }

    pub(in crate::nodes) fn prepare(&mut self) -> usize {
        // Clone the existing queue Arc to keep delivery/destruction callbacks outside its lock.
        let notifications = self.notifications.clone();
        loop {
            let id = notifications.lock().pop_front();
            let Some(id) = id else {
                break;
            };
            if let Some(&index) = self.indexes.get(&id) {
                if self.registrations[index].channel.receiver_dropped() {
                    self.remove(id);
                }
            }
        }
        self.active.clear();
        self.active.extend(
            self.registrations
                .iter()
                .enumerate()
                .filter_map(|(index, event)| event.channel.can_take().then_some(index)),
        );
        self.active.len()
    }

    pub(in crate::nodes) fn add_to_wait_set(&self, wait_set: &mut rcl_wait_set_t) -> Result<usize> {
        let mut first = 0;
        for (position, &index) in self.active.iter().enumerate() {
            let mut slot = 0;
            check_rcl_ret(unsafe {
                rcl_wait_set_add_event(
                    wait_set,
                    self.registrations[index].native.as_ref().unwrap().handle(),
                    &mut slot,
                )
            })?;
            if position == 0 {
                first = slot;
            }
        }
        Ok(first)
    }

    pub(in crate::nodes) fn dispatch(&self, wait_set: &rcl_wait_set_t, first: usize) -> Result<()> {
        if self.active.is_empty() {
            return Ok(());
        }
        if wait_set.events.is_null() || first + self.active.len() > wait_set.size_of_events {
            return Err(Error::RCL_RET_WAIT_SET_INVALID);
        }
        let slots =
            unsafe { std::slice::from_raw_parts(wait_set.events.add(first), self.active.len()) };
        let mut error = None;
        for (&index, ready) in self.active.iter().zip(slots) {
            if ready.is_null() {
                continue;
            }
            let registration = &self.registrations[index];
            // A dropped stream cannot cause another native status to be taken while cleanup waits.
            if !registration.channel.can_take() {
                continue;
            }
            match registration.native.as_ref().unwrap().take() {
                Ok(Some(status)) => registration.channel.deliver(status),
                Ok(None) => {}
                Err(failure) => {
                    registration.channel.close(failure.clone());
                    error.get_or_insert(failure);
                }
            }
        }
        error.map_or(Ok(()), Err)
    }
}

impl Node {
    pub(in crate::nodes) fn activate_qos_event_guard(&mut self) -> Result<Option<EventWait>> {
        let Some(events) = self
            .qos_events
            .as_ref()
            .filter(|events| !events.registrations.is_empty())
        else {
            return Ok(None);
        };
        let guard = match self.future_wake_guard.as_ref() {
            Some(guard) => guard.clone(),
            None => {
                let guard = future_executor::FutureGuard::new(&self.context)?;
                self.future_wake_guard = Some(guard.clone());
                guard
            }
        };
        let previous = guard.active.swap(true, std::sync::atomic::Ordering::AcqRel);
        events.set_wake(Some(guard.observer_waker()));
        Ok(Some(EventWait { guard, previous }))
    }
    pub(crate) fn publisher_qos_events(
        &mut self, publisher: Arc<Publisher_>, kind: PublisherEventKind,
    ) -> Result<QosEventStream> {
        if !self.owns_publisher(&publisher) {
            return Err(Error::RCL_RET_PUBLISHER_INVALID);
        }
        let endpoint = EventEndpoint::Publisher(publisher.id);
        let event_kind = EventKind::Publisher(kind);
        if self
            .qos_events
            .as_ref()
            .is_some_and(|events| events.contains(endpoint, event_kind))
        {
            return Err(Error::QosEventAlreadyObserved);
        }
        let native = NativeEvent::publisher(publisher, kind)?;
        Ok(self
            .qos_event_registry()
            .insert(endpoint, event_kind, native))
    }

    pub(crate) fn subscription_qos_events(
        &mut self, id: uuid::Uuid, kind: SubscriptionEventKind,
    ) -> Result<QosEventStream> {
        self.subscriber(id)?;
        let endpoint = EventEndpoint::Subscription(id);
        let event_kind = EventKind::Subscription(kind);
        self.check_subscription_event_observer(id, kind)?;
        let native = NativeEvent::subscription(self.subscriber(id)?.handle(), kind)?;
        Ok(self
            .qos_event_registry()
            .insert(endpoint, event_kind, native))
    }

    fn qos_event_registry(&mut self) -> &mut QosEvents {
        let guard = self
            .future_wake_guard
            .as_ref()
            .map(|guard| guard.observer_waker());
        let events = self
            .qos_events
            .get_or_insert_with(|| Box::new(QosEvents::default()));
        if !events.notifications.lock().has_wake() {
            events.set_wake(guard);
        }
        events
    }

    pub(in crate::nodes) fn check_subscription_event_observer(
        &self, id: uuid::Uuid, kind: SubscriptionEventKind,
    ) -> Result<()> {
        if self.qos_events.as_ref().is_some_and(|events| {
            events.contains(EventEndpoint::Subscription(id), EventKind::Subscription(kind))
        }) {
            Err(Error::QosEventAlreadyObserved)
        } else {
            Ok(())
        }
    }

    /// Immediately finalize this node's native event registration and close its stream.
    /// A stream from another node is dropped, scheduling cleanup in its owning node.
    pub fn destroy_qos_event_stream(&mut self, stream: QosEventStream) {
        if let Some(events) = &mut self.qos_events {
            events.remove(stream.id);
        }
    }
}

pub(in crate::nodes) struct EventWait {
    guard: Arc<future_executor::FutureGuard>,
    previous: bool,
}

impl Drop for EventWait {
    fn drop(&mut self) {
        self.guard
            .active
            .store(self.previous, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(test)]
mod tests;
