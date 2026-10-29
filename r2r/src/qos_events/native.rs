//! Share the established native typed event decoders and their RAII finalizers.

use super::*;
use crate::{
    publishers::{PublisherEvent, Publisher_},
    subscribers::SubscriptionEvent,
};
use r2r_rcl::*;

impl PublisherEventKind {
    pub(crate) fn native(self) -> rcl_publisher_event_type_t {
        use rcl_publisher_event_type_t::*;
        match self {
            Self::IncompatibleQos => RCL_PUBLISHER_OFFERED_INCOMPATIBLE_QOS,
            Self::DeadlineMissed => RCL_PUBLISHER_OFFERED_DEADLINE_MISSED,
            Self::LivelinessLost => RCL_PUBLISHER_LIVELINESS_LOST,
            #[cfg(not(r2r__ros__distro__humble))]
            Self::Matched => RCL_PUBLISHER_MATCHED,
            #[cfg(not(r2r__ros__distro__humble))]
            Self::IncompatibleType => RCL_PUBLISHER_INCOMPATIBLE_TYPE,
        }
    }
}

impl SubscriptionEventKind {
    pub(crate) fn native(self) -> rcl_subscription_event_type_t {
        use rcl_subscription_event_type_t::*;
        match self {
            Self::IncompatibleQos => RCL_SUBSCRIPTION_REQUESTED_INCOMPATIBLE_QOS,
            Self::DeadlineMissed => RCL_SUBSCRIPTION_REQUESTED_DEADLINE_MISSED,
            Self::LivelinessChanged => RCL_SUBSCRIPTION_LIVELINESS_CHANGED,
            Self::MessageLost => RCL_SUBSCRIPTION_MESSAGE_LOST,
            #[cfg(not(r2r__ros__distro__humble))]
            Self::Matched => RCL_SUBSCRIPTION_MATCHED,
            #[cfg(not(r2r__ros__distro__humble))]
            Self::IncompatibleType => RCL_SUBSCRIPTION_INCOMPATIBLE_TYPE,
        }
    }
}

pub(crate) enum NativeEvent {
    Publisher {
        event: PublisherEvent,
        owner: Arc<Publisher_>,
        kind: PublisherEventKind,
    },
    Subscription {
        event: SubscriptionEvent,
        kind: SubscriptionEventKind,
    },
}

impl NativeEvent {
    pub(crate) fn publisher(owner: Arc<Publisher_>, kind: PublisherEventKind) -> Result<Self> {
        let mut event = PublisherEvent::uninitialized(kind.native());
        owner.initialize_qos_event(&mut event, kind.native())?;
        Ok(Self::Publisher { event, owner, kind })
    }
    pub(crate) fn subscription(
        handle: &rcl_subscription_t, kind: SubscriptionEventKind,
    ) -> Result<Self> {
        let mut event = SubscriptionEvent::uninitialized(kind.native());
        event.ensure_initialized(handle)?;
        Ok(Self::Subscription { event, kind })
    }
    pub(crate) fn handle(&self) -> &rcl_event_t {
        match self {
            Self::Publisher { event, .. } => event.handle(),
            Self::Subscription { event, .. } => event.handle(),
        }
    }
    pub(crate) fn take(&self) -> Result<Option<QosEvent>> {
        match self {
            Self::Publisher { event, kind, .. } => match kind {
                PublisherEventKind::IncompatibleQos => event
                    .take_offered_incompatible_qos()
                    .map(|value| value.map(QosEvent::OfferedIncompatibleQos)),
                PublisherEventKind::DeadlineMissed => event
                    .take_offered_deadline_missed()
                    .map(|value| value.map(QosEvent::OfferedDeadlineMissed)),
                PublisherEventKind::LivelinessLost => event
                    .take_liveliness_lost()
                    .map(|value| value.map(QosEvent::LivelinessLost)),
                #[cfg(not(r2r__ros__distro__humble))]
                PublisherEventKind::Matched => event
                    .take_matched()
                    .map(|value| value.map(QosEvent::Matched)),
                #[cfg(not(r2r__ros__distro__humble))]
                PublisherEventKind::IncompatibleType => event
                    .take_incompatible_type()
                    .map(|value| value.map(QosEvent::IncompatibleType)),
            },
            Self::Subscription { event, kind } => match kind {
                SubscriptionEventKind::IncompatibleQos => event
                    .take_requested_incompatible_qos()
                    .map(|value| value.map(QosEvent::RequestedIncompatibleQos)),
                SubscriptionEventKind::DeadlineMissed => event
                    .take_requested_deadline_missed()
                    .map(|value| value.map(QosEvent::RequestedDeadlineMissed)),
                SubscriptionEventKind::LivelinessChanged => event
                    .take_liveliness_changed()
                    .map(|value| value.map(QosEvent::LivelinessChanged)),
                SubscriptionEventKind::MessageLost => event
                    .take_message_lost()
                    .map(|value| value.map(QosEvent::MessageLost)),
                #[cfg(not(r2r__ros__distro__humble))]
                SubscriptionEventKind::Matched => event
                    .take_matched()
                    .map(|value| value.map(QosEvent::Matched)),
                #[cfg(not(r2r__ros__distro__humble))]
                SubscriptionEventKind::IncompatibleType => event
                    .take_incompatible_type()
                    .map(|value| value.map(QosEvent::IncompatibleType)),
            },
        }
    }
}

impl Drop for NativeEvent {
    fn drop(&mut self) {
        if let Self::Publisher { event, owner, kind } = self {
            // Release native ownership before reopening the manual counter-take path to
            // publisher clones on other threads. The field's later Drop is idempotent.
            event.fini();
            owner.release_qos_event(kind.native());
        }
    }
}
