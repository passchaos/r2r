//! Bounded ownership of native endpoint status notifications.

use futures::Stream;
use parking_lot::Mutex;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};

use crate::{
    Error, IncompatibleTypeStatus, LivelinessChangedStatus, LivelinessLostStatus, MatchedStatus,
    MessageLostStatus, NativeCallbackSubscription, Node, OfferedDeadlineMissedStatus,
    OfferedIncompatibleQosStatus, Publisher, PublisherUntyped, RequestedDeadlineMissedStatus,
    RequestedIncompatibleQosStatus, Result, Subscription, WrappedTypesupport,
};

pub(crate) mod native;

/// Native publisher status to observe. Unsupported RMW events fail during registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PublisherEventKind {
    IncompatibleQos,
    DeadlineMissed,
    LivelinessLost,
    #[cfg(not(r2r__ros__distro__humble))]
    Matched,
    #[cfg(not(r2r__ros__distro__humble))]
    IncompatibleType,
}

/// Native subscription status to observe. Unsupported RMW events fail during registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SubscriptionEventKind {
    IncompatibleQos,
    DeadlineMissed,
    LivelinessChanged,
    MessageLost,
    #[cfg(not(r2r__ros__distro__humble))]
    Matched,
    #[cfg(not(r2r__ros__distro__humble))]
    IncompatibleType,
}

/// A status snapshot taken once from the endpoint's native event counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum QosEvent {
    OfferedIncompatibleQos(OfferedIncompatibleQosStatus),
    OfferedDeadlineMissed(OfferedDeadlineMissedStatus),
    LivelinessLost(LivelinessLostStatus),
    RequestedIncompatibleQos(RequestedIncompatibleQosStatus),
    RequestedDeadlineMissed(RequestedDeadlineMissedStatus),
    LivelinessChanged(LivelinessChangedStatus),
    MessageLost(MessageLostStatus),
    Matched(MatchedStatus),
    IncompatibleType(IncompatibleTypeStatus),
}

#[derive(Default)]
struct EventSlot {
    pending: Option<QosEvent>,
    terminal: Option<Error>,
    closed: bool,
    receiver_dropped: bool,
    waker: Option<Waker>,
}

/// One taken status fits in this slot. While full, the native event is excluded from the wait
/// set, leaving its counters in the middleware until the consumer releases capacity.
#[derive(Default)]
pub(crate) struct EventChannel(Mutex<EventSlot>);

impl EventChannel {
    pub(crate) fn receiver_dropped(&self) -> bool {
        self.0.lock().receiver_dropped
    }
    pub(crate) fn can_take(&self) -> bool {
        let slot = self.0.lock();
        !slot.receiver_dropped && !slot.closed && slot.pending.is_none()
    }
    pub(crate) fn deliver(&self, event: QosEvent) {
        let wake = {
            let mut slot = self.0.lock();
            if slot.receiver_dropped || slot.closed {
                return;
            }
            assert!(slot.pending.is_none(), "native event was taken without slot capacity");
            slot.pending = Some(event);
            slot.waker.take()
        };
        if let Some(wake) = wake {
            wake.wake();
        }
    }
    pub(crate) fn close(&self, error: Error) {
        let wake = {
            let mut slot = self.0.lock();
            if slot.closed || slot.receiver_dropped {
                return;
            }
            slot.closed = true;
            slot.terminal = Some(error);
            slot.waker.take()
        };
        if let Some(wake) = wake {
            wake.wake();
        }
    }
}

/// A bounded, wake-driven stream of native QoS status snapshots.
///
/// The node/executor must be driven while awaiting. There is one Rust-side slot. A full stream
/// pauses native takes for this event only; the middleware continues accumulating status changes.
/// Reading the slot wakes the current executor to rearm the native event. No polling timer is
/// added. Each endpoint/event kind has one observer; duplicate registration and manual takes
/// while its stream exists return [`Error::QosEventAlreadyObserved`].
///
/// Endpoint destruction delivers any already-taken snapshot, then one endpoint-invalid error,
/// then terminates. Dropping the stream schedules native event cleanup at the next spin.
pub struct QosEventStream {
    pub(crate) id: uuid::Uuid,
    channel: Arc<EventChannel>,
    notifications: Arc<Mutex<crate::notifications::NotificationQueue<uuid::Uuid>>>,
}

impl QosEventStream {
    pub(crate) fn new(
        id: uuid::Uuid, channel: Arc<EventChannel>,
        notifications: Arc<Mutex<crate::notifications::NotificationQueue<uuid::Uuid>>>,
    ) -> Self {
        Self {
            id,
            channel,
            notifications,
        }
    }
}

impl Stream for QosEventStream {
    type Item = Result<QosEvent>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let (result, rearm, retired_waker) = {
            let mut slot = self.channel.0.lock();
            if let Some(event) = slot.pending.take() {
                (Poll::Ready(Some(Ok(event))), !slot.closed, None)
            } else if let Some(error) = slot.terminal.take() {
                (Poll::Ready(Some(Err(error))), false, None)
            } else if slot.closed {
                (Poll::Ready(None), false, None)
            } else {
                let retired = if !slot
                    .waker
                    .as_ref()
                    .is_some_and(|waker| waker.will_wake(cx.waker()))
                {
                    slot.waker.replace(cx.waker().clone())
                } else {
                    None
                };
                (Poll::Pending, false, retired)
            }
        };
        drop(retired_waker);
        if rearm {
            crate::notifications::enqueue(&self.notifications, self.id);
        }
        result
    }
}

impl Drop for QosEventStream {
    fn drop(&mut self) {
        let mut slot = self.channel.0.lock();
        slot.receiver_dropped = true;
        slot.pending.take();
        let waker = slot.waker.take();
        drop(slot);
        drop(waker);
        crate::notifications::enqueue(&self.notifications, self.id);
    }
}

impl<T: WrappedTypesupport> Publisher<T> {
    /// Observe one native QoS event through the owning node's wait set.
    pub fn qos_events(&self, node: &mut Node, kind: PublisherEventKind) -> Result<QosEventStream> {
        node.publisher_qos_events(
            self.handle
                .upgrade()
                .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?,
            kind,
        )
    }
}

impl PublisherUntyped {
    /// Observe one native QoS event through the owning node's wait set.
    pub fn qos_events(&self, node: &mut Node, kind: PublisherEventKind) -> Result<QosEventStream> {
        node.publisher_qos_events(
            self.handle
                .upgrade()
                .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?,
            kind,
        )
    }
}

impl<T> Subscription<T> {
    /// Observe one native QoS event independently of this message stream's capacity.
    pub fn qos_events(
        &self, node: &mut Node, kind: SubscriptionEventKind,
    ) -> Result<QosEventStream> {
        node.subscription_qos_events(self.id, kind)
    }
}

impl NativeCallbackSubscription {
    /// Observe one native QoS event independently of native callback dispatch.
    pub fn qos_events(
        &self, node: &mut Node, kind: SubscriptionEventKind,
    ) -> Result<QosEventStream> {
        node.subscription_qos_events(self.id, kind)
    }
}

#[cfg(test)]
mod tests;
