use futures::{channel::oneshot, Future, TryFutureExt};
use parking_lot::{ArcMutexGuard, Mutex, RawMutex};
use std::{
    ffi::{CStr, CString},
    fmt::Debug,
    marker::PhantomData,
    sync::{Arc, Weak},
    time::Duration,
};

use crate::{error::*, msg_types::*, nodes::Node, qos::QosProfile};
use r2r_rcl::*;

/// Whether an endpoint should use a unique middleware network flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniqueNetworkFlowRequirement {
    NotRequired,
    StrictlyRequired,
    OptionallyRequired,
    SystemDefault,
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
    handle: Arc<Mutex<PublisherHandle>>,

    // TODO use a mpsc to avoid the mutex?
    poll_subscriber_channels: Mutex<Vec<oneshot::Sender<()>>>,
}

struct PublisherHandle(Option<rcl_publisher_t>);

// SAFETY: every access to the native publisher is serialized by the enclosing mutex.
unsafe impl Send for PublisherHandle {}

// SAFETY: all access to the RCL publisher handle is serialized by its mutex.
unsafe impl Send for Publisher_ {}
unsafe impl Sync for Publisher_ {}

impl Publisher_ {
    fn with_handle<T>(&self, operation: impl FnOnce(&rcl_publisher_t) -> Result<T>) -> Result<T> {
        let handle = self.handle.lock();
        let handle = handle.0.as_ref().ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        operation(handle)
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

    #[cfg(not(any(r2r__ros__distro__foxy, r2r__ros__distro__galactic)))]
    fn wait_for_all_acked(&self, timeout: Duration) -> Result<()> {
        let timeout =
            i64::try_from(timeout.as_nanos()).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        self.with_handle(|handle| {
            let ret = unsafe { rcl_publisher_wait_for_all_acked(handle, timeout) };
            check_publisher_ret(ret)
        })
    }

    fn get_actual_qos(&self) -> Result<QosProfile> {
        let handle = self.handle.lock();
        let handle = handle.0.as_ref().ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        let qos = unsafe { rcl_publisher_get_actual_qos(handle) };
        let qos = unsafe { qos.as_ref() }.ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        QosProfile::try_from(qos)
    }

    fn get_subscription_count(&self) -> Result<usize> {
        // See https://github.com/ros2/rclcpp/issues/623

        let mut subscription_count = 0;
        let handle = self.handle.lock();
        let handle = handle.0.as_ref().ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;

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
        let mut poll_subscriber_channels = self.poll_subscriber_channels.lock();
        poll_subscriber_channels.retain(|sender| !sender.is_canceled());

        if poll_subscriber_channels.is_empty() {
            return;
        }
        match self.get_subscription_count() {
            Ok(0) => {
                // not available...
            }
            Ok(_) => {
                // send ok and close channels
                while let Some(sender) = poll_subscriber_channels.pop() {
                    let _res = sender.send(()); // we ignore if receiver dropped.
                }
            }
            Err(_) => {
                // error, close all channels
                poll_subscriber_channels.clear();
            }
        }
    }

    pub(crate) fn has_pending_subscriber_waits(&self) -> bool {
        let mut waiters = self.poll_subscriber_channels.lock();
        waiters.retain(|sender| !sender.is_canceled());
        !waiters.is_empty()
    }

    pub(crate) fn destroy(&self, node: &mut rcl_node_t) {
        if let Some(mut handle) = self.handle.lock().0.take() {
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
#[derive(Debug, Clone)]
pub struct Publisher<T>
where
    T: WrappedTypesupport,
{
    pub(crate) handle: Weak<Publisher_>,
    type_: PhantomData<T>,
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
            .0
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
        let Some(handle) = self.publisher.0.as_ref() else {
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
#[derive(Debug, Clone)]
pub struct PublisherUntyped {
    pub(crate) handle: Weak<Publisher_>,
    type_: String,
}

pub fn make_publisher<T>(handle: Weak<Publisher_>) -> Publisher<T>
where
    T: WrappedTypesupport,
{
    Publisher {
        handle,
        type_: PhantomData,
    }
}

pub fn make_publisher_untyped(handle: Weak<Publisher_>, type_: String) -> PublisherUntyped {
    PublisherUntyped { handle, type_ }
}

pub fn create_publisher_helper(
    node: &mut rcl_node_t, topic: &str, typesupport: *const rosidl_message_type_support_t,
    qos_profile: QosProfile, options: PublisherOptions,
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
            handle: Arc::new(Mutex::new(PublisherHandle(Some(publisher_handle)))),
            poll_subscriber_channels: Mutex::new(Vec::new()),
        })
    } else {
        Err(Error::from_rcl_error(result))
    }
}

impl PublisherUntyped {
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

    /// Publish an "untyped" ROS message represented by a `serde_json::Value`.
    ///
    /// It is up to the user to make sure the fields are correct.
    pub fn publish(&self, msg: serde_json::Value) -> Result<()> {
        // upgrade to actual ref. if still alive
        let publisher = self
            .handle
            .upgrade()
            .ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        let handle = publisher.handle.lock();
        let handle = handle.0.as_ref().ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;

        let mut native_msg = WrappedNativeMsgUntyped::new_from(&self.type_)?;
        native_msg.from_json(msg)?;

        let result = unsafe { rcl_publish(handle, native_msg.void_ptr(), std::ptr::null_mut()) };

        if result == RCL_RET_OK as i32 {
            Ok(())
        } else {
            log::error!("could not publish {}", result);
            Err(Error::from_rcl_error(result))
        }
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
        let handle = handle.0.as_ref().ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;

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
            .poll_subscriber_channels
            .lock()
            .push(sender);

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
        let handle = publisher.handle.lock();
        let handle = handle.0.as_ref().ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;
        let native_msg = WrappedNativeMsg::<T>::try_from(msg)?;
        let result = unsafe { rcl_publish(handle, native_msg.void_ptr(), std::ptr::null_mut()) };

        if result == RCL_RET_OK as i32 {
            Ok(())
        } else {
            log::error!("could not publish {}", result);
            Err(Error::from_rcl_error(result))
        }
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
            .0
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
        let handle = handle.0.as_ref().ok_or(Error::RCL_RET_PUBLISHER_INVALID)?;

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
            .poll_subscriber_channels
            .lock()
            .push(sender);

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
