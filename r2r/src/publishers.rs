use futures::{channel::oneshot, Future, TryFutureExt};
use parking_lot::{ArcMutexGuard, Mutex, RawMutex};
#[cfg(test)]
use std::sync::atomic::AtomicUsize;
#[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
use std::time::Duration;
use std::{
    collections::VecDeque,
    ffi::{CStr, CString},
    fmt::Debug,
    marker::PhantomData,
    mem::MaybeUninit,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
};

use crate::{
    availability::Waiters, error::*, msg_types::*, nodes::Node, qos::QosProfile, QosPolicyKind,
};
use r2r_rcl::*;

/// Whether an endpoint should use a unique middleware network flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniqueNetworkFlowRequirement {
    NotRequired,
    StrictlyRequired,
    OptionallyRequired,
    SystemDefault,
}

/// Transport information for a middleware network flow used by an endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkFlowEndpoint {
    pub transport_protocol: String,
    pub internet_protocol: String,
    pub transport_port: u16,
    pub flow_label: u32,
    pub dscp: u8,
    pub internet_address: String,
}

impl From<UniqueNetworkFlowRequirement> for rmw_unique_network_flow_endpoints_requirement_t {
    fn from(requirement: UniqueNetworkFlowRequirement) -> Self {
        match requirement {
            UniqueNetworkFlowRequirement::NotRequired => {
                Self::RMW_UNIQUE_NETWORK_FLOW_ENDPOINTS_NOT_REQUIRED
            }
            UniqueNetworkFlowRequirement::StrictlyRequired => {
                Self::RMW_UNIQUE_NETWORK_FLOW_ENDPOINTS_STRICTLY_REQUIRED
            }
            UniqueNetworkFlowRequirement::OptionallyRequired => {
                Self::RMW_UNIQUE_NETWORK_FLOW_ENDPOINTS_OPTIONALLY_REQUIRED
            }
            UniqueNetworkFlowRequirement::SystemDefault => {
                Self::RMW_UNIQUE_NETWORK_FLOW_ENDPOINTS_SYSTEM_DEFAULT
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublisherOptions {
    pub unique_network_flow: UniqueNetworkFlowRequirement,
}

/// Cumulative status from an offered-incompatible-QoS publisher event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfferedIncompatibleQosStatus {
    pub total_count: i32,
    pub total_count_change: i32,
    pub last_policy_kind: QosPolicyKind,
}

/// Cumulative status from an offered-deadline-missed publisher event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfferedDeadlineMissedStatus {
    pub total_count: i32,
    pub total_count_change: i32,
}

/// Cumulative status from a liveliness-lost publisher event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LivelinessLostStatus {
    pub total_count: i32,
    pub total_count_change: i32,
}

impl Default for PublisherOptions {
    fn default() -> Self {
        Self {
            unique_network_flow: UniqueNetworkFlowRequirement::NotRequired,
        }
    }
}

impl PublisherOptions {
    pub const fn unique_network_flow(mut self, requirement: UniqueNetworkFlowRequirement) -> Self {
        self.unique_network_flow = requirement;
        self
    }
}

#[cfg(test)]
mod option_tests {
    use super::*;

    #[test]
    fn maps_unique_network_flow_requirements() {
        assert_eq!(
            rmw_unique_network_flow_endpoints_requirement_t::from(
                UniqueNetworkFlowRequirement::NotRequired
            ),
            rmw_unique_network_flow_endpoints_requirement_t::RMW_UNIQUE_NETWORK_FLOW_ENDPOINTS_NOT_REQUIRED
        );
        assert_eq!(
            rmw_unique_network_flow_endpoints_requirement_t::from(
                UniqueNetworkFlowRequirement::StrictlyRequired
            ),
            rmw_unique_network_flow_endpoints_requirement_t::RMW_UNIQUE_NETWORK_FLOW_ENDPOINTS_STRICTLY_REQUIRED
        );
        assert_eq!(
            rmw_unique_network_flow_endpoints_requirement_t::from(
                UniqueNetworkFlowRequirement::OptionallyRequired
            ),
            rmw_unique_network_flow_endpoints_requirement_t::RMW_UNIQUE_NETWORK_FLOW_ENDPOINTS_OPTIONALLY_REQUIRED
        );
        assert_eq!(
            rmw_unique_network_flow_endpoints_requirement_t::from(
                UniqueNetworkFlowRequirement::SystemDefault
            ),
            rmw_unique_network_flow_endpoints_requirement_t::RMW_UNIQUE_NETWORK_FLOW_ENDPOINTS_SYSTEM_DEFAULT
        );
    }

    #[cfg(not(r2r__ros__distro__foxy))]
    #[test]
    fn network_flow_endpoints_reject_invalid_native_addresses() {
        let mut endpoint = unsafe { rmw_get_zero_initialized_network_flow_endpoint() };
        endpoint.internet_address.fill(b'a' as std::os::raw::c_char);
        assert!(matches!(
            NetworkFlowEndpoint::try_from_native(&endpoint),
            Err(Error::RCL_RET_ERROR)
        ));

        endpoint.internet_address.fill(0);
        endpoint.internet_address[0] = 0xff_u8 as std::os::raw::c_char;
        assert!(matches!(
            NetworkFlowEndpoint::try_from_native(&endpoint),
            Err(Error::InvalidUtf8 {
                context: "network flow internet address",
                ..
            })
        ));
    }
}

// The publish function is thread safe. ROS2 docs state:
// =============
//
// This function is thread safe so long as access to both the
// publisher and the" `ros_message` is synchronized."  That means that
// calling rcl_publish() from multiple threads is allowed, but"
// calling rcl_publish() at the same time as non-thread safe
// publisher" functions is not, e.g. calling rcl_publish() and
// rcl_publisher_fini()" concurrently is not allowed."  Before calling
// rcl_publish() the message can change and after calling"
// rcl_publish() the message can change, but it cannot be changed
// during the" publish call."  The same `ros_message`, however, can be
// passed to multiple calls of" rcl_publish() simultaneously, even if
// the publishers differ."  The `ros_message` is unmodified by
// rcl_publish()."
//
// TODO: I guess there is a potential error source in destructuring
// while calling publish. I don't think its worth to protect with a
// mutex/rwlock for this though...
//
// Methods that mutate need to called from the thread owning the Node.
// I don't think we can count on Node being generally thread-safe.
// So keep pub/sub management and polling contained to one thread
// and send out publishers.

unsafe impl<T> Send for Publisher<T> where T: WrappedTypesupport {}

pub(crate) struct Publisher_ {
    pub(crate) id: uuid::Uuid,
    handle: Arc<Mutex<PublisherHandle>>,
    active_wait_publishers: Arc<Mutex<VecDeque<uuid::Uuid>>>,
    #[cfg(test)]
    pub(crate) wait_poll_count: AtomicUsize,

    poll_subscriber_channels: Mutex<Waiters<()>>,
    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    poll_ack_channels: Mutex<Waiters<Result<()>>>,
}

fn has_active_waiters<T>(waiters: &Mutex<Waiters<T>>) -> bool {
    let mut waiters = waiters.lock();
    waiters.prune_canceled();
    !waiters.is_empty()
}

fn take_waiters_through<T>(waiters: &Mutex<Waiters<T>>, boundary: u64) -> Vec<oneshot::Sender<T>> {
    waiters.lock().take_through(boundary)
}

fn complete_waiters<T: Clone>(waiters: Vec<oneshot::Sender<T>>, value: T) {
    for sender in waiters {
        let _ = sender.send(value.clone());
    }
}

#[cfg(test)]
mod waiter_tests {
    use super::*;
    use futures::{
        task::{waker_ref, ArcWake},
        Future,
    };
    use std::{
        pin::Pin,
        sync::atomic::{AtomicBool, Ordering},
        task::{Context, Poll},
    };

    struct RegisteringWake {
        waiters: Arc<Mutex<Waiters<()>>>,
        woke: AtomicBool,
    }

    impl ArcWake for RegisteringWake {
        fn wake_by_ref(arc_self: &Arc<Self>) {
            let (sender, _receiver) = oneshot::channel();
            arc_self.waiters.lock().register(sender);
            arc_self.woke.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn completing_waiters_allows_reentrant_registration() {
        let waiters = Arc::new(Mutex::new(Waiters::default()));
        let (sender, mut receiver) = oneshot::channel();
        waiters.lock().register(sender);
        let wake = Arc::new(RegisteringWake {
            waiters: Arc::clone(&waiters),
            woke: AtomicBool::new(false),
        });
        let waker = waker_ref(&wake);
        let mut context = Context::from_waker(&waker);
        assert!(matches!(Pin::new(&mut receiver).poll(&mut context), Poll::Pending));

        let boundary = waiters.lock().poll_boundary().unwrap();
        let detached = take_waiters_through(&waiters, boundary);
        assert!(waiters.try_lock().is_some());
        complete_waiters(detached, ());

        assert!(wake.woke.load(Ordering::SeqCst));
        assert_eq!(waiters.lock().len(), 1);
        assert!(matches!(Pin::new(&mut receiver).poll(&mut context), Poll::Ready(Ok(()))));
    }

    #[test]
    fn poll_boundary_preserves_concurrent_registrations() {
        let waiters = Mutex::new(Waiters::default());
        let (first, _first_receiver) = oneshot::channel::<()>();
        waiters.lock().register(first);
        let boundary = waiters.lock().poll_boundary().unwrap();
        let (second, _second_receiver) = oneshot::channel::<()>();
        waiters.lock().register(second);
        let detached = take_waiters_through(&waiters, boundary);

        assert_eq!(detached.len(), 1);
        assert_eq!(waiters.lock().len(), 1);
    }

    #[test]
    fn checking_active_waiters_releases_the_waiter_lock() {
        let waiters = Mutex::new(Waiters::default());
        let (sender, receiver) = oneshot::channel::<()>();
        drop(receiver);
        waiters.lock().register(sender);

        assert!(!has_active_waiters(&waiters));
        assert!(waiters.try_lock().is_some());
    }

    #[test]
    fn final_publisher_handle_drop_queues_its_identity_once() {
        let dropped_publishers = Arc::new(Mutex::new(VecDeque::new()));
        let publisher = Arc::new(Publisher_ {
            id: uuid::Uuid::from_u128(1),
            handle: Arc::new(Mutex::new(PublisherHandle {
                publisher: Some(unsafe { rcl_get_zero_initialized_publisher() }),
                offered_incompatible_qos: None,
                offered_deadline_missed: None,
                liveliness_lost: None,
            })),
            active_wait_publishers: Arc::new(Mutex::new(VecDeque::new())),
            wait_poll_count: AtomicUsize::new(0),
            poll_subscriber_channels: Mutex::new(Waiters::default()),
            #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
            poll_ack_channels: Mutex::new(Waiters::default()),
        });
        let identity = publisher.id;
        let handle = make_publisher::<crate::std_msgs::msg::String>(
            Arc::downgrade(&publisher),
            identity,
            Arc::clone(&dropped_publishers),
        );
        let clone = handle.clone();

        drop(handle);
        assert!(dropped_publishers.lock().is_empty());
        drop(clone);

        assert_eq!(
            dropped_publishers
                .lock()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            [identity]
        );
    }

    #[test]
    fn publisher_payloads_are_prepared_before_locking_the_native_handle() {
        let publisher = Publisher_ {
            id: uuid::Uuid::nil(),
            handle: Arc::new(Mutex::new(PublisherHandle {
                publisher: Some(unsafe { rcl_get_zero_initialized_publisher() }),
                offered_incompatible_qos: None,
                offered_deadline_missed: None,
                liveliness_lost: None,
            })),
            active_wait_publishers: Arc::new(Mutex::new(VecDeque::new())),
            wait_poll_count: AtomicUsize::new(0),
            poll_subscriber_channels: Mutex::new(Waiters::default()),
            #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
            poll_ack_channels: Mutex::new(Waiters::default()),
        };

        let prepared = publisher
            .with_prepared_handle(
                || {
                    assert!(publisher.handle.try_lock().is_some());
                    Ok(41)
                },
                |_handle, value| Ok(value + 1),
            )
            .unwrap();

        assert_eq!(prepared, 42);
    }
}

struct PublisherHandle {
    publisher: Option<rcl_publisher_t>,
    offered_incompatible_qos: Option<PublisherEvent>,
    offered_deadline_missed: Option<PublisherEvent>,
    liveliness_lost: Option<PublisherEvent>,
}

struct PublisherEvent {
    handle: rcl_event_t,
    event_type: rcl_publisher_event_type_t,
    initialized: bool,
}

impl PublisherEvent {
    fn uninitialized(event_type: rcl_publisher_event_type_t) -> Self {
        Self {
            handle: unsafe { rcl_get_zero_initialized_event() },
            event_type,
            initialized: false,
        }
    }

    fn ensure_initialized(&mut self, publisher: *const rcl_publisher_t) -> Result<()> {
        if self.initialized {
            return Ok(());
        }
        let ret = unsafe { rcl_publisher_event_init(&mut self.handle, publisher, self.event_type) };
        check_publisher_ret(ret)?;
        self.initialized = true;
        Ok(())
    }

    fn take_offered_incompatible_qos(&self) -> Result<Option<OfferedIncompatibleQosStatus>> {
        let mut status = MaybeUninit::<rmw_offered_qos_incompatible_event_status_t>::uninit();
        let ret = unsafe { rcl_take_event(&self.handle, status.as_mut_ptr().cast()) };
        if ret == RCL_RET_EVENT_TAKE_FAILED as i32 {
            return Ok(None);
        }
        check_publisher_ret(ret)?;
        let status = unsafe { status.assume_init() };
        if status.total_count_change == 0 {
            return Ok(None);
        }
        Ok(Some(OfferedIncompatibleQosStatus {
            total_count: status.total_count,
            total_count_change: status.total_count_change,
            last_policy_kind: status.last_policy_kind.into(),
        }))
    }

    fn take_offered_deadline_missed(&self) -> Result<Option<OfferedDeadlineMissedStatus>> {
        let mut status = MaybeUninit::<rmw_offered_deadline_missed_status_t>::uninit();
        let ret = unsafe { rcl_take_event(&self.handle, status.as_mut_ptr().cast()) };
        if ret == RCL_RET_EVENT_TAKE_FAILED as i32 {
            return Ok(None);
        }
        check_publisher_ret(ret)?;
        let status = unsafe { status.assume_init() };
        if status.total_count_change == 0 {
            return Ok(None);
        }
        Ok(Some(OfferedDeadlineMissedStatus {
            total_count: status.total_count,
            total_count_change: status.total_count_change,
        }))
    }

    fn take_liveliness_lost(&self) -> Result<Option<LivelinessLostStatus>> {
        let mut status = MaybeUninit::<rmw_liveliness_lost_status_t>::uninit();
        let ret = unsafe { rcl_take_event(&self.handle, status.as_mut_ptr().cast()) };
        if ret == RCL_RET_EVENT_TAKE_FAILED as i32 {
            return Ok(None);
        }
        check_publisher_ret(ret)?;
        let status = unsafe { status.assume_init() };
        if status.total_count_change == 0 {
            return Ok(None);
        }
        Ok(Some(LivelinessLostStatus {
            total_count: status.total_count,
            total_count_change: status.total_count_change,
        }))
    }
}

impl Drop for PublisherEvent {
    fn drop(&mut self) {
        if !self.initialized {
            return;
        }
        let ret = unsafe { rcl_event_fini(&mut self.handle) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize publisher event: {ret}");
        }
    }
}

// SAFETY: every access to the native publisher is serialized by the enclosing mutex.
unsafe impl Send for PublisherHandle {}

// SAFETY: all access to the RCL publisher handle is serialized by its mutex.
unsafe impl Send for Publisher_ {}
unsafe impl Sync for Publisher_ {}

impl Publisher_ {
    fn with_prepared_handle<P, T>(
        &self, prepare: impl FnOnce() -> Result<P>,
        operation: impl FnOnce(&rcl_publisher_t, P) -> Result<T>,
    ) -> Result<T> {
        let prepared = prepare()?;
        self.with_handle(|handle| operation(handle, prepared))
    }

    fn with_handle<T>(&self, operation: impl FnOnce(&rcl_publisher_t) -> Result<T>) -> Result<T> {
        let handle = self.handle.lock();
        let handle = handle
            .publisher
            .as_ref()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        operation(handle)
    }

    fn take_offered_incompatible_qos(&self) -> Result<Option<OfferedIncompatibleQosStatus>> {
        let mut handle = self.handle.lock();
        let publisher = handle
            .publisher
            .as_ref()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            as *const rcl_publisher_t;
        let event = handle.offered_incompatible_qos.get_or_insert_with(|| {
            PublisherEvent::uninitialized(
                rcl_publisher_event_type_t::RCL_PUBLISHER_OFFERED_INCOMPATIBLE_QOS,
            )
        });
        event.ensure_initialized(publisher)?;
        event.take_offered_incompatible_qos()
    }

    fn take_offered_deadline_missed(&self) -> Result<Option<OfferedDeadlineMissedStatus>> {
        let mut handle = self.handle.lock();
        let publisher = handle
            .publisher
            .as_ref()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            as *const rcl_publisher_t;
        let event = handle.offered_deadline_missed.get_or_insert_with(|| {
            PublisherEvent::uninitialized(
                rcl_publisher_event_type_t::RCL_PUBLISHER_OFFERED_DEADLINE_MISSED,
            )
        });
        event.ensure_initialized(publisher)?;
        event.take_offered_deadline_missed()
    }

    fn take_liveliness_lost(&self) -> Result<Option<LivelinessLostStatus>> {
        let mut handle = self.handle.lock();
        let publisher = handle
            .publisher
            .as_ref()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            as *const rcl_publisher_t;
        let event = handle.liveliness_lost.get_or_insert_with(|| {
            PublisherEvent::uninitialized(rcl_publisher_event_type_t::RCL_PUBLISHER_LIVELINESS_LOST)
        });
        event.ensure_initialized(publisher)?;
        event.take_liveliness_lost()
    }

    fn assert_liveliness(&self) -> Result<()> {
        self.with_handle(|handle| {
            let ret = unsafe { rcl_publisher_assert_liveliness(handle) };
            check_publisher_ret(ret)
        })
    }

    fn topic_name(&self) -> Result<String> {
        self.with_handle(|handle| {
            let name = unsafe { rcl_publisher_get_topic_name(handle) };
            if name.is_null() {
                return Err(Error::RCL_RET_PUBLISHER_INVALID);
            }
            unsafe { CStr::from_ptr(name) }
                .to_str()
                .map(str::to_owned)
                .map_err(|err| Error::InvalidUtf8 {
                    context: "publisher topic name",
                    err: err.to_string(),
                })
        })
    }

    fn can_loan_messages(&self) -> Result<bool> {
        self.with_handle(|handle| Ok(unsafe { rcl_publisher_can_loan_messages(handle) }))
    }

    fn network_flow_endpoints(&self) -> Result<Vec<NetworkFlowEndpoint>> {
        self.with_handle(publisher_network_flow_endpoints)
    }

    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    fn wait_for_all_acked(&self, timeout: Duration) -> Result<()> {
        let timeout =
            i64::try_from(timeout.as_nanos()).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        self.with_handle(|handle| {
            let ret = unsafe { rcl_publisher_wait_for_all_acked(handle, timeout) };
            check_publisher_ret(ret)
        })
    }

    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    fn register_ack_wait(&self, sender: oneshot::Sender<Result<()>>) {
        self.poll_ack_channels.lock().register(sender);
        self.active_wait_publishers.lock().push_back(self.id);
    }

    fn register_subscriber_wait(&self, sender: oneshot::Sender<()>) {
        self.poll_subscriber_channels.lock().register(sender);
        self.active_wait_publishers.lock().push_back(self.id);
    }

    fn get_actual_qos(&self) -> Result<QosProfile> {
        let handle = self.handle.lock();
        let handle = handle
            .publisher
            .as_ref()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        let qos = unsafe { rcl_publisher_get_actual_qos(handle) };
        let qos = unsafe { qos.as_ref() }.ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        QosProfile::try_from(qos)
    }

    fn get_subscription_count(&self) -> Result<usize> {
        // See https://github.com/ros2/rclcpp/issues/623

        let mut subscription_count = 0;
        let handle = self.handle.lock();
        let handle = handle
            .publisher
            .as_ref()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;

        let result = unsafe {
            rcl_publisher_get_subscription_count(handle, &mut subscription_count as *mut usize)
        };

        if result == RCL_RET_OK as i32 {
            Ok(subscription_count)
        } else {
            Err(Error::from_rcl_error(result))
        }
    }

    pub(crate) fn poll_has_subscribers(&self) {
        let Some(boundary) = self.poll_subscriber_channels.lock().poll_boundary() else {
            return;
        };
        match self.get_subscription_count() {
            Ok(0) => {}
            Ok(_) => {
                let waiters = take_waiters_through(&self.poll_subscriber_channels, boundary);
                complete_waiters(waiters, ());
            }
            Err(_) => {
                let waiters = take_waiters_through(&self.poll_subscriber_channels, boundary);
                drop(waiters);
            }
        }
    }

    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    pub(crate) fn poll_all_acked(&self) {
        let Some(boundary) = self.poll_ack_channels.lock().poll_boundary() else {
            return;
        };

        let result = self.wait_for_all_acked(Duration::ZERO);
        if matches!(result, Err(Error::RCL_RET_TIMEOUT)) {
            return;
        }
        let waiters = take_waiters_through(&self.poll_ack_channels, boundary);
        complete_waiters(waiters, result);
    }

    pub(crate) fn has_pending_waits(&self) -> bool {
        if has_active_waiters(&self.poll_subscriber_channels) {
            return true;
        }
        #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
        return has_active_waiters(&self.poll_ack_channels);
        #[cfg(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic))]
        false
    }

    pub(crate) fn poll_waits(&self) -> bool {
        #[cfg(test)]
        self.wait_poll_count.fetch_add(1, Ordering::Relaxed);
        self.poll_has_subscribers();
        #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
        self.poll_all_acked();
        self.has_pending_waits()
    }

    pub(crate) fn destroy(&self, node: &mut rcl_node_t) {
        let mut state = self.handle.lock();
        state.offered_incompatible_qos = None;
        state.offered_deadline_missed = None;
        state.liveliness_lost = None;
        if let Some(mut handle) = state.publisher.take() {
            let ret = unsafe { rcl_publisher_fini(&mut handle, node) };
            if ret != RCL_RET_OK as i32 {
                log::error!("could not finalize publisher: {}", ret);
            }
        }
    }
}

/// A ROS (typed) publisher.
///
/// This contains a `Weak Arc` to a typed publisher. As such it is safe to
/// move between threads.
#[derive(Debug)]
pub struct Publisher<T>
where
    T: WrappedTypesupport,
{
    pub(crate) handle: Weak<Publisher_>,
    lifecycle: Arc<PublisherHandleLifecycle>,
    type_: PhantomData<T>,
}

#[derive(Debug)]
pub(crate) struct PublisherHandleLifecycle {
    id: uuid::Uuid,
    dropped_publishers: Arc<Mutex<VecDeque<uuid::Uuid>>>,
    explicitly_destroyed: AtomicBool,
}

impl Drop for PublisherHandleLifecycle {
    fn drop(&mut self) {
        if !self.explicitly_destroyed.load(Ordering::Acquire) {
            self.dropped_publishers.lock().push_back(self.id);
        }
    }
}

impl PublisherHandleLifecycle {
    fn mark_explicitly_destroyed(&self) {
        self.explicitly_destroyed.store(true, Ordering::Release);
    }
}

impl<T> Clone for Publisher<T>
where
    T: WrappedTypesupport,
{
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            lifecycle: Arc::clone(&self.lifecycle),
            type_: PhantomData,
        }
    }
}

impl<T> Publisher<T>
where
    T: WrappedTypesupport,
{
    pub(crate) fn identity(&self) -> uuid::Uuid {
        self.lifecycle.id
    }

    pub(crate) fn mark_explicitly_destroyed(&self) {
        self.lifecycle.mark_explicitly_destroyed();
    }
}

/// A message buffer loaned by the middleware for zero-copy publishing.
///
/// The mutable borrow of the owning [`Node`] prevents its native resources from being finalized
/// before this loan is published or returned. Dropping the value without publishing returns the
/// loan to the middleware.
pub struct LoanedMessage<'node, T>
where
    T: WrappedTypesupport,
{
    msg: *mut T::CStruct,
    publisher: ArcMutexGuard<RawMutex, PublisherHandle>,
    _node: std::marker::PhantomData<&'node mut Node>,
}

impl<T> std::ops::Deref for LoanedMessage<'_, T>
where
    T: WrappedTypesupport,
{
    type Target = T::CStruct;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.msg }
    }
}

impl<T> std::ops::DerefMut for LoanedMessage<'_, T>
where
    T: WrappedTypesupport,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.msg }
    }
}

impl<T> LoanedMessage<'_, T>
where
    T: WrappedTypesupport,
{
    /// Publish this loan and transfer ownership back to the middleware.
    pub fn publish(mut self) -> Result<()> {
        let handle = self
            .publisher
            .publisher
            .as_ref()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        let msg = std::mem::replace(&mut self.msg, std::ptr::null_mut());
        let ret = unsafe { rcl_publish_loaned_message(handle, msg.cast(), std::ptr::null_mut()) };
        if ret == RCL_RET_OK as i32 {
            Ok(())
        } else {
            Err(Error::from_rcl_error(ret))
        }
    }
}

impl<T> Drop for LoanedMessage<'_, T>
where
    T: WrappedTypesupport,
{
    fn drop(&mut self) {
        if self.msg.is_null() {
            return;
        }
        let Some(handle) = self.publisher.publisher.as_ref() else {
            return;
        };
        let ret = unsafe { rcl_return_loaned_message_from_publisher(handle, self.msg.cast()) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not return publisher loaned message: {ret}");
        }
        self.msg = std::ptr::null_mut();
    }
}

unsafe impl Send for PublisherUntyped {}

/// A ROS (untyped) publisher.
///
/// This contains a `Weak Arc` to an "untyped" publisher. As such it is safe to
/// move between threads.
#[derive(Debug)]
pub struct PublisherUntyped {
    pub(crate) handle: Weak<Publisher_>,
    lifecycle: Arc<PublisherHandleLifecycle>,
    type_: String,
}

impl Clone for PublisherUntyped {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            lifecycle: Arc::clone(&self.lifecycle),
            type_: self.type_.clone(),
        }
    }
}

impl PublisherUntyped {
    pub(crate) fn identity(&self) -> uuid::Uuid {
        self.lifecycle.id
    }

    pub(crate) fn mark_explicitly_destroyed(&self) {
        self.lifecycle.mark_explicitly_destroyed();
    }
}

pub(crate) fn make_publisher<T>(
    handle: Weak<Publisher_>, id: uuid::Uuid, dropped_publishers: Arc<Mutex<VecDeque<uuid::Uuid>>>,
) -> Publisher<T>
where
    T: WrappedTypesupport,
{
    Publisher {
        handle,
        lifecycle: Arc::new(PublisherHandleLifecycle {
            id,
            dropped_publishers,
            explicitly_destroyed: AtomicBool::new(false),
        }),
        type_: PhantomData,
    }
}

pub(crate) fn make_publisher_untyped(
    handle: Weak<Publisher_>, id: uuid::Uuid, dropped_publishers: Arc<Mutex<VecDeque<uuid::Uuid>>>,
    type_: String,
) -> PublisherUntyped {
    PublisherUntyped {
        handle,
        lifecycle: Arc::new(PublisherHandleLifecycle {
            id,
            dropped_publishers,
            explicitly_destroyed: AtomicBool::new(false),
        }),
        type_,
    }
}

pub fn create_publisher_helper(
    node: &mut rcl_node_t, topic: &str, typesupport: *const rosidl_message_type_support_t,
    qos_profile: QosProfile, options: PublisherOptions, id: uuid::Uuid,
    active_wait_publishers: Arc<Mutex<VecDeque<uuid::Uuid>>>,
) -> Result<Publisher_> {
    let mut publisher_handle = unsafe { rcl_get_zero_initialized_publisher() };
    let topic_c_string = CString::new(topic).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;

    let result = unsafe {
        let mut publisher_options = rcl_publisher_get_default_options();
        publisher_options.qos = qos_profile.into();
        publisher_options
            .rmw_publisher_options
            .require_unique_network_flow_endpoints = options.unique_network_flow.into();
        rcl_publisher_init(
            &mut publisher_handle,
            node,
            typesupport,
            topic_c_string.as_ptr(),
            &publisher_options,
        )
    };
    if result == RCL_RET_OK as i32 {
        Ok(Publisher_ {
            id,
            handle: Arc::new(Mutex::new(PublisherHandle {
                publisher: Some(publisher_handle),
                offered_incompatible_qos: None,
                offered_deadline_missed: None,
                liveliness_lost: None,
            })),
            active_wait_publishers,
            #[cfg(test)]
            wait_poll_count: AtomicUsize::new(0),
            poll_subscriber_channels: Mutex::new(Waiters::default()),
            #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
            poll_ack_channels: Mutex::new(Waiters::default()),
        })
    } else {
        Err(Error::from_rcl_error(result))
    }
}

impl PublisherUntyped {
    /// Takes the latest offered-deadline-missed status without blocking.
    pub fn take_offered_deadline_missed(&self) -> Result<Option<OfferedDeadlineMissedStatus>> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .take_offered_deadline_missed()
    }

    /// Takes the latest liveliness-lost status without blocking.
    pub fn take_liveliness_lost(&self) -> Result<Option<LivelinessLostStatus>> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .take_liveliness_lost()
    }

    /// Takes the latest offered-incompatible-QoS status without blocking.
    ///
    /// The first call initializes the native event. Call this once before creating potentially
    /// incompatible subscriptions when every discovery event must be observed.
    pub fn take_offered_incompatible_qos(&self) -> Result<Option<OfferedIncompatibleQosStatus>> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .take_offered_incompatible_qos()
    }

    /// Returns the QoS profile resolved by the RMW implementation.
    pub fn get_actual_qos(&self) -> Result<QosProfile> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .get_actual_qos()
    }

    /// Returns the fully resolved topic name used by the native publisher.
    pub fn topic_name(&self) -> Result<String> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .topic_name()
    }

    /// Returns whether the middleware can loan messages for this publisher.
    pub fn can_loan_messages(&self) -> Result<bool> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .can_loan_messages()
    }

    /// Returns the network flows used to send messages, when supported by the RMW.
    pub fn get_network_flow_endpoints(&self) -> Result<Vec<NetworkFlowEndpoint>> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .network_flow_endpoints()
    }

    /// Manually asserts this publisher's liveliness.
    ///
    /// This is intended for publishers using [`crate::qos::LivelinessPolicy::ManualByTopic`].
    pub fn assert_liveliness(&self) -> Result<()> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .assert_liveliness()
    }

    /// Blocks until all previously published reliable data is acknowledged or `timeout` expires.
    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    pub fn wait_for_all_acked(&self, timeout: Duration) -> Result<()> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .wait_for_all_acked(timeout)
    }

    /// Asynchronously waits until all previously published reliable data is acknowledged.
    ///
    /// The owning node must be spun while awaiting. Unlike [`Self::wait_for_all_acked`], this does
    /// not block the calling thread.
    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    pub fn wait_for_all_acked_async(&self) -> Result<impl Future<Output = Result<()>>> {
        let publisher = self
            .handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        let (sender, receiver) = oneshot::channel();
        publisher.register_ack_wait(sender);
        Ok(async move {
            receiver
                .await
                .map_err(|_| Error::RCL_RET_PUBLISHER_INVALID)?
        })
    }

    /// Asynchronously waits for acknowledgements up to `timeout`. The node must be spun.
    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    pub fn wait_for_all_acked_async_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        let acked = self.wait_for_all_acked_async()?;
        node.with_timeout(acked, timeout)
    }

    /// Publish an "untyped" ROS message represented by a `serde_json::Value`.
    ///
    /// It is up to the user to make sure the fields are correct.
    pub fn publish(&self, msg: serde_json::Value) -> Result<()> {
        // upgrade to actual ref. if still alive
        let publisher = self
            .handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        publisher.with_prepared_handle(
            || {
                let mut native_msg = WrappedNativeMsgUntyped::new_from(&self.type_)?;
                native_msg.from_json(msg)?;
                Ok(native_msg)
            },
            |handle, native_msg| {
                let result =
                    unsafe { rcl_publish(handle, native_msg.void_ptr(), std::ptr::null_mut()) };
                if result == RCL_RET_OK as i32 {
                    Ok(())
                } else {
                    log::error!("could not publish {}", result);
                    Err(Error::from_rcl_error(result))
                }
            },
        )
    }

    /// Publish an pre-serialized ROS message represented by a `&[u8]`.
    ///
    /// It is up to the user to make sure data is a valid ROS serialized message.
    pub fn publish_raw(&self, data: &[u8]) -> Result<()> {
        // TODO should this be an unsafe function? I'm not sure what happens if the data is malformed ..

        // upgrade to actual ref. if still alive
        let publisher = self
            .handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        let handle = publisher.handle.lock();
        let handle = handle
            .publisher
            .as_ref()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;

        // Safety: Not retained beyond this function
        let msg_buf = rcl_serialized_message_t {
            buffer: data.as_ptr() as *mut u8,
            buffer_length: data.len(),
            buffer_capacity: data.len(),

            // Since its read only, this should never be used ..
            allocator: unsafe { rcutils_get_default_allocator() },
        };

        let result = unsafe {
            rcl_publish_serialized_message(
                handle,
                &msg_buf as *const rcl_serialized_message_t,
                std::ptr::null_mut(),
            )
        };

        if result == RCL_RET_OK as i32 {
            Ok(())
        } else {
            log::error!("could not publish {}", result);
            Err(Error::from_rcl_error(result))
        }
    }

    /// Gets the number of subscriptions matched to this publisher.
    pub fn get_subscription_count(&self) -> Result<usize> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .get_subscription_count()
    }

    /// Waits for at least one subscription to match this publisher.
    pub fn wait_for_subscribers(&self) -> Result<impl Future<Output = Result<()>>> {
        let (sender, receiver) = oneshot::channel();

        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .register_subscriber_wait(sender);

        Ok(receiver.map_err(|_| Error::RCL_RET_PUBLISHER_INVALID))
    }

    /// Wait for a matched subscription up to `timeout`. The node must be spun while awaiting.
    pub fn wait_for_subscribers_with_timeout(
        &self, node: &mut Node, timeout: std::time::Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        let available = self.wait_for_subscribers()?;
        node.with_timeout(available, timeout)
    }

    #[deprecated(note = "use get_subscription_count; the RCL count includes local matches")]
    pub fn get_inter_process_subscription_count(&self) -> Result<usize> {
        self.get_subscription_count()
    }

    #[deprecated(note = "use wait_for_subscribers; local matches are included")]
    pub fn wait_for_inter_process_subscribers(&self) -> Result<impl Future<Output = Result<()>>> {
        self.wait_for_subscribers()
    }

    #[deprecated(note = "use wait_for_subscribers_with_timeout; local matches are included")]
    pub fn wait_for_inter_process_subscribers_with_timeout(
        &self, node: &mut Node, timeout: std::time::Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        self.wait_for_subscribers_with_timeout(node, timeout)
    }
}

impl<T: 'static> Publisher<T>
where
    T: WrappedTypesupport,
{
    /// Takes the latest offered-deadline-missed status without blocking.
    pub fn take_offered_deadline_missed(&self) -> Result<Option<OfferedDeadlineMissedStatus>> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .take_offered_deadline_missed()
    }

    /// Takes the latest liveliness-lost status without blocking.
    pub fn take_liveliness_lost(&self) -> Result<Option<LivelinessLostStatus>> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .take_liveliness_lost()
    }

    /// Takes the latest offered-incompatible-QoS status without blocking.
    ///
    /// The first call initializes the native event. Call this once before creating potentially
    /// incompatible subscriptions when every discovery event must be observed.
    pub fn take_offered_incompatible_qos(&self) -> Result<Option<OfferedIncompatibleQosStatus>> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .take_offered_incompatible_qos()
    }

    /// Returns the QoS profile resolved by the RMW implementation.
    pub fn get_actual_qos(&self) -> Result<QosProfile> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .get_actual_qos()
    }

    /// Returns the fully resolved topic name used by the native publisher.
    pub fn topic_name(&self) -> Result<String> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .topic_name()
    }

    /// Returns whether the middleware can loan messages for this publisher and message type.
    pub fn can_loan_messages(&self) -> Result<bool> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .can_loan_messages()
    }

    /// Returns the network flows used to send messages, when supported by the RMW.
    pub fn get_network_flow_endpoints(&self) -> Result<Vec<NetworkFlowEndpoint>> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .network_flow_endpoints()
    }

    /// Manually asserts this publisher's liveliness.
    ///
    /// This is intended for publishers using [`crate::qos::LivelinessPolicy::ManualByTopic`].
    pub fn assert_liveliness(&self) -> Result<()> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .assert_liveliness()
    }

    /// Blocks until all previously published reliable data is acknowledged or `timeout` expires.
    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    pub fn wait_for_all_acked(&self, timeout: Duration) -> Result<()> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .wait_for_all_acked(timeout)
    }

    /// Asynchronously waits until all previously published reliable data is acknowledged.
    ///
    /// The owning node must be spun while awaiting. Unlike [`Self::wait_for_all_acked`], this does
    /// not block the calling thread.
    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    pub fn wait_for_all_acked_async(&self) -> Result<impl Future<Output = Result<()>>> {
        let publisher = self
            .handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        let (sender, receiver) = oneshot::channel();
        publisher.register_ack_wait(sender);
        Ok(async move {
            receiver
                .await
                .map_err(|_| Error::RCL_RET_PUBLISHER_INVALID)?
        })
    }

    /// Asynchronously waits for acknowledgements up to `timeout`. The node must be spun.
    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    pub fn wait_for_all_acked_async_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        let acked = self.wait_for_all_acked_async()?;
        node.with_timeout(acked, timeout)
    }

    /// Publish a ROS message.
    pub fn publish(&self, msg: &T) -> Result<()>
    where
        T: WrappedTypesupport,
    {
        // upgrade to actual ref. if still alive
        let publisher = self
            .handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        publisher.with_prepared_handle(
            || WrappedNativeMsg::<T>::try_from(msg),
            |handle, native_msg| {
                let result =
                    unsafe { rcl_publish(handle, native_msg.void_ptr(), std::ptr::null_mut()) };
                if result == RCL_RET_OK as i32 {
                    Ok(())
                } else {
                    log::error!("could not publish {}", result);
                    Err(Error::from_rcl_error(result))
                }
            },
        )
    }

    pub fn borrow_loaned_message<'node>(
        &self, node: &'node mut Node,
    ) -> Result<LoanedMessage<'node, T>>
    where
        T: WrappedTypesupport,
    {
        let publisher = self
            .handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        if !node.owns_publisher(&publisher) {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let publisher = publisher.handle.clone().lock_arc();
        let handle = publisher
            .publisher
            .as_ref()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        let mut msg = std::ptr::null_mut();
        let ret = unsafe { rcl_borrow_loaned_message(handle, T::get_ts(), &mut msg) };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        if msg.is_null() {
            return Err(Error::RCL_RET_BAD_ALLOC);
        }
        Ok(LoanedMessage {
            msg: msg.cast(),
            publisher,
            _node: PhantomData,
        })
    }

    /// Publish a "native" ROS message.
    ///
    /// This function is useful if you want to bypass the generated
    /// rust types as it lets you work with the raw C struct.
    pub fn publish_native(&self, msg: &mut WrappedNativeMsg<T>) -> Result<()>
    where
        T: WrappedTypesupport,
    {
        // upgrade to actual ref. if still alive
        let publisher = self
            .handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        let handle = publisher.handle.lock();
        let handle = handle
            .publisher
            .as_ref()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;

        let result = unsafe { rcl_publish(handle, msg.void_ptr(), std::ptr::null_mut()) };

        if result == RCL_RET_OK as i32 {
            Ok(())
        } else {
            log::error!("could not publish native {}", result);
            Err(Error::from_rcl_error(result))
        }
    }

    /// Gets the number of subscriptions matched to this publisher.
    pub fn get_subscription_count(&self) -> Result<usize> {
        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .get_subscription_count()
    }

    /// Waits for at least one subscription to match this publisher.
    pub fn wait_for_subscribers(&self) -> Result<impl Future<Output = Result<()>>> {
        let (sender, receiver) = oneshot::channel();

        self.handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?
            .register_subscriber_wait(sender);

        Ok(receiver.map_err(|_| Error::RCL_RET_PUBLISHER_INVALID))
    }

    /// Wait for a matched subscription up to `timeout`. The node must be spun while awaiting.
    pub fn wait_for_subscribers_with_timeout(
        &self, node: &mut Node, timeout: std::time::Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        let available = self.wait_for_subscribers()?;
        node.with_timeout(available, timeout)
    }

    #[deprecated(note = "use get_subscription_count; the RCL count includes local matches")]
    pub fn get_inter_process_subscription_count(&self) -> Result<usize> {
        self.get_subscription_count()
    }

    #[deprecated(note = "use wait_for_subscribers; local matches are included")]
    pub fn wait_for_inter_process_subscribers(&self) -> Result<impl Future<Output = Result<()>>> {
        self.wait_for_subscribers()
    }

    #[deprecated(note = "use wait_for_subscribers_with_timeout; local matches are included")]
    pub fn wait_for_inter_process_subscribers_with_timeout(
        &self, node: &mut Node, timeout: std::time::Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        self.wait_for_subscribers_with_timeout(node, timeout)
    }
}

fn check_publisher_ret(ret: rcl_ret_t) -> Result<()> {
    if ret == RCL_RET_OK as i32 {
        Ok(())
    } else {
        Err(Error::from_rcl_error(ret))
    }
}

#[cfg(not(r2r__ros__distro__foxy))]
struct NetworkFlowEndpointArray(rmw_network_flow_endpoint_array_t);

#[cfg(not(r2r__ros__distro__foxy))]
impl NetworkFlowEndpointArray {
    fn new() -> Self {
        Self(unsafe { rmw_get_zero_initialized_network_flow_endpoint_array() })
    }

    fn to_vec(&self) -> Result<Vec<NetworkFlowEndpoint>> {
        if self.0.size == 0 {
            return Ok(Vec::new());
        }
        if self.0.network_flow_endpoint.is_null() {
            return Err(Error::RCL_RET_ERROR);
        }
        unsafe { std::slice::from_raw_parts(self.0.network_flow_endpoint, self.0.size) }
            .iter()
            .map(NetworkFlowEndpoint::try_from_native)
            .collect()
    }
}

#[cfg(not(r2r__ros__distro__foxy))]
impl Drop for NetworkFlowEndpointArray {
    fn drop(&mut self) {
        if self.0.allocator.is_null() {
            return;
        }
        let ret = unsafe { rmw_network_flow_endpoint_array_fini(&mut self.0) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize network flow endpoint array: {ret}");
        }
    }
}

#[cfg(not(r2r__ros__distro__foxy))]
impl NetworkFlowEndpoint {
    fn try_from_native(endpoint: &rmw_network_flow_endpoint_t) -> Result<Self> {
        fn protocol_string(
            value: *const std::os::raw::c_char, context: &'static str,
        ) -> Result<String> {
            if value.is_null() {
                return Err(Error::RCL_RET_ERROR);
            }
            unsafe { CStr::from_ptr(value) }
                .to_str()
                .map(str::to_owned)
                .map_err(|err| Error::InvalidUtf8 {
                    context,
                    err: err.to_string(),
                })
        }

        let address = unsafe {
            std::slice::from_raw_parts(
                endpoint.internet_address.as_ptr().cast::<u8>(),
                endpoint.internet_address.len(),
            )
        };
        let internet_address = CStr::from_bytes_until_nul(address)
            .map_err(|_| Error::RCL_RET_ERROR)?
            .to_str()
            .map(str::to_owned)
            .map_err(|err| Error::InvalidUtf8 {
                context: "network flow internet address",
                err: err.to_string(),
            })?;

        Ok(Self {
            transport_protocol: protocol_string(
                unsafe {
                    rmw_network_flow_endpoint_get_transport_protocol_string(
                        endpoint.transport_protocol,
                    )
                },
                "network flow transport protocol",
            )?,
            internet_protocol: protocol_string(
                unsafe {
                    rmw_network_flow_endpoint_get_internet_protocol_string(
                        endpoint.internet_protocol,
                    )
                },
                "network flow internet protocol",
            )?,
            transport_port: endpoint.transport_port,
            flow_label: endpoint.flow_label,
            dscp: endpoint.dscp,
            internet_address,
        })
    }
}

fn publisher_network_flow_endpoints(handle: &rcl_publisher_t) -> Result<Vec<NetworkFlowEndpoint>> {
    #[cfg(not(r2r__ros__distro__foxy))]
    {
        let mut endpoints = NetworkFlowEndpointArray::new();
        let mut allocator = unsafe { rcutils_get_default_allocator() };
        let ret = unsafe {
            rcl_publisher_get_network_flow_endpoints(handle, &mut allocator, &mut endpoints.0)
        };
        if ret != RCL_RET_OK as i32 {
            let err = Error::from_rcl_error(ret);
            drop(endpoints);
            unsafe { rcutils_reset_error() };
            return Err(err);
        }
        endpoints.to_vec()
    }
    #[cfg(r2r__ros__distro__foxy)]
    {
        let _ = handle;
        Err(Error::RCL_RET_UNSUPPORTED)
    }
}

pub(crate) fn subscription_network_flow_endpoints(
    handle: &rcl_subscription_t,
) -> Result<Vec<NetworkFlowEndpoint>> {
    #[cfg(not(r2r__ros__distro__foxy))]
    {
        let mut endpoints = NetworkFlowEndpointArray::new();
        let mut allocator = unsafe { rcutils_get_default_allocator() };
        let ret = unsafe {
            rcl_subscription_get_network_flow_endpoints(handle, &mut allocator, &mut endpoints.0)
        };
        if ret != RCL_RET_OK as i32 {
            let err = Error::from_rcl_error(ret);
            drop(endpoints);
            unsafe { rcutils_reset_error() };
            return Err(err);
        }
        endpoints.to_vec()
    }
    #[cfg(r2r__ros__distro__foxy)]
    {
        let _ = handle;
        Err(Error::RCL_RET_UNSUPPORTED)
    }
}
