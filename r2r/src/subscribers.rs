use futures::{
    channel::{mpsc, oneshot},
    Future, Stream, TryFutureExt,
};
use std::{
    ffi::CString,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use crate::{
    error::*,
    msg_types::*,
    nodes::{future_with_timeout, Node},
    qos::QosProfile,
};
use r2r_rcl::*;

/// A ROS subscription stream with graph matching helpers.
pub struct Subscription<T> {
    receiver: mpsc::Receiver<T>,
    id: uuid::Uuid,
}

impl<T> Subscription<T> {
    pub(crate) fn new(receiver: mpsc::Receiver<T>, id: uuid::Uuid) -> Self {
        Self { receiver, id }
    }

    /// Gets the number of publishers matched to this subscription.
    pub fn get_publisher_count(&self, node: &Node) -> Result<usize> {
        node.get_subscription_publisher_count(self.id)
    }

    /// Waits for at least one publisher to match this subscription.
    pub fn wait_for_publishers(&self, node: &mut Node) -> Result<impl Future<Output = Result<()>>> {
        let receiver = node.register_subscription_publisher_wait(self.id)?;
        Ok(receiver.map_err(|_| Error::RCL_RET_SUBSCRIPTION_INVALID))
    }

    /// Waits for a matched publisher up to `timeout`. The node must be spun while awaiting.
    pub fn wait_for_publishers_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        let available = self.wait_for_publishers(node)?;
        let timer = node.create_wall_timer(timeout)?;
        Ok(future_with_timeout(available, timer))
    }
}

impl<T> Stream for Subscription<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.receiver).poll_next(cx)
    }
}

pub(crate) struct SubscriptionMatchState {
    pub(crate) id: uuid::Uuid,
    pub(crate) waiters: Vec<oneshot::Sender<()>>,
}

impl SubscriptionMatchState {
    pub(crate) fn new(id: uuid::Uuid) -> Self {
        Self {
            id,
            waiters: Vec::new(),
        }
    }
}

pub(crate) trait Subscriber_ {
    fn handle(&self) -> &rcl_subscription_t;
    fn match_state(&self) -> Option<&SubscriptionMatchState> {
        None
    }
    fn match_state_mut(&mut self) -> Option<&mut SubscriptionMatchState> {
        None
    }
    fn publisher_count(&self) -> Result<usize> {
        let mut count = 0;
        let ret = unsafe { rcl_subscription_get_publisher_count(self.handle(), &mut count) };
        if ret == RCL_RET_OK as i32 {
            Ok(count)
        } else {
            Err(Error::from_rcl_error(ret))
        }
    }
    fn poll_has_publishers(&mut self) {
        {
            let Some(state) = self.match_state_mut() else {
                return;
            };
            state.waiters.retain(|sender| !sender.is_canceled());
            if state.waiters.is_empty() {
                return;
            }
        };

        match self.publisher_count() {
            Ok(0) => {}
            Ok(_) => {
                let state = self
                    .match_state_mut()
                    .expect("subscriber match state disappeared while polling");
                while let Some(sender) = state.waiters.pop() {
                    let _ = sender.send(());
                }
            }
            Err(_) => {
                self.match_state_mut()
                    .expect("subscriber match state disappeared while polling")
                    .waiters
                    .clear();
            }
        }
    }
    /// Returns true when the subscriber should be destroyed before entering the next wait.
    fn should_remove(&mut self) -> bool {
        false
    }
    /// Returns true if the subscriber stream has been dropped.
    fn handle_incoming(&mut self) -> bool;
    fn destroy(&mut self, node: &mut rcl_node_t);
}

pub struct TypedSubscriber<T>
where
    T: WrappedTypesupport,
{
    pub rcl_handle: rcl_subscription_t,
    pub sender: mpsc::Sender<T>,
    pub(crate) match_state: SubscriptionMatchState,
}

pub struct NativeSubscriber<T>
where
    T: WrappedTypesupport,
{
    pub rcl_handle: rcl_subscription_t,
    pub sender: mpsc::Sender<WrappedNativeMsg<T>>,
    pub(crate) match_state: SubscriptionMatchState,
}

pub struct UntypedSubscriber {
    pub rcl_handle: rcl_subscription_t,
    pub topic_type: String,
    pub sender: mpsc::Sender<Result<serde_json::Value>>,
    pub(crate) match_state: SubscriptionMatchState,
}

pub struct RawSubscriber {
    pub rcl_handle: rcl_subscription_t,
    pub msg_buf: rcl_serialized_message_t,
    pub sender: mpsc::Sender<Vec<u8>>,
    pub(crate) match_state: SubscriptionMatchState,
}

impl<T: 'static> Subscriber_ for TypedSubscriber<T>
where
    T: WrappedTypesupport,
{
    fn handle(&self) -> &rcl_subscription_t {
        &self.rcl_handle
    }

    fn match_state(&self) -> Option<&SubscriptionMatchState> {
        Some(&self.match_state)
    }

    fn match_state_mut(&mut self) -> Option<&mut SubscriptionMatchState> {
        Some(&mut self.match_state)
    }

    fn should_remove(&mut self) -> bool {
        self.sender.is_closed()
    }

    fn handle_incoming(&mut self) -> bool {
        let mut msg_info = rmw_message_info_t::default(); // we dont care for now
        let Ok(mut msg) = WrappedNativeMsg::<T>::try_new() else {
            log::error!("failed to allocate subscription message buffer");
            return false;
        };
        let ret = unsafe {
            rcl_take(&self.rcl_handle, msg.void_ptr_mut(), &mut msg_info, std::ptr::null_mut())
        };
        if ret == RCL_RET_OK as i32 {
            let msg = T::from_native(&msg);
            if let Err(e) = self.sender.try_send(msg) {
                if e.is_disconnected() {
                    // user dropped the handle to the stream, signal removal.
                    return true;
                }
                log::debug!("error {:?}", e)
            }
        }
        false
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_subscription_fini(&mut self.rcl_handle, node);
        }
    }
}

impl<T: 'static> Subscriber_ for NativeSubscriber<T>
where
    T: WrappedTypesupport,
{
    fn handle(&self) -> &rcl_subscription_t {
        &self.rcl_handle
    }

    fn match_state(&self) -> Option<&SubscriptionMatchState> {
        Some(&self.match_state)
    }

    fn match_state_mut(&mut self) -> Option<&mut SubscriptionMatchState> {
        Some(&mut self.match_state)
    }

    fn should_remove(&mut self) -> bool {
        self.sender.is_closed()
    }

    fn handle_incoming(&mut self) -> bool {
        // The returned stream may outlive the Node. Use caller-owned message storage so
        // dropping an item never has to access an already-finalized subscription handle.
        let mut msg_info = rmw_message_info_t::default();
        let Ok(mut msg) = WrappedNativeMsg::<T>::try_new() else {
            log::error!("failed to allocate native subscription message buffer");
            return false;
        };
        let ret = unsafe {
            rcl_take(&self.rcl_handle, msg.void_ptr_mut(), &mut msg_info, std::ptr::null_mut())
        };
        if ret != RCL_RET_OK as i32 {
            return false;
        }
        if let Err(e) = self.sender.try_send(msg) {
            if e.is_disconnected() {
                // user dropped the handle to the stream, signal removal.
                return true;
            }
            log::error!("error {:?}", e)
        }
        false
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_subscription_fini(&mut self.rcl_handle, node);
        }
    }
}

impl Subscriber_ for UntypedSubscriber {
    fn handle(&self) -> &rcl_subscription_t {
        &self.rcl_handle
    }

    fn match_state(&self) -> Option<&SubscriptionMatchState> {
        Some(&self.match_state)
    }

    fn match_state_mut(&mut self) -> Option<&mut SubscriptionMatchState> {
        Some(&mut self.match_state)
    }

    fn should_remove(&mut self) -> bool {
        self.sender.is_closed()
    }

    fn handle_incoming(&mut self) -> bool {
        let mut msg_info = rmw_message_info_t::default(); // we dont care for now
        let Ok(mut msg) = WrappedNativeMsgUntyped::new_from(&self.topic_type) else {
            log::error!("failed to allocate untyped subscription message buffer");
            return false;
        };
        let ret = unsafe {
            rcl_take(&self.rcl_handle, msg.void_ptr_mut(), &mut msg_info, std::ptr::null_mut())
        };
        if ret == RCL_RET_OK as i32 {
            let json = msg.to_json();
            if let Err(e) = self.sender.try_send(json) {
                if e.is_disconnected() {
                    // user dropped the handle to the stream, signal removal.
                    return true;
                }
                log::debug!("error {:?}", e)
            }
        }
        false
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_subscription_fini(&mut self.rcl_handle, node);
        }
    }
}

impl Subscriber_ for RawSubscriber {
    fn handle(&self) -> &rcl_subscription_t {
        &self.rcl_handle
    }

    fn match_state(&self) -> Option<&SubscriptionMatchState> {
        Some(&self.match_state)
    }

    fn match_state_mut(&mut self) -> Option<&mut SubscriptionMatchState> {
        Some(&mut self.match_state)
    }

    fn should_remove(&mut self) -> bool {
        self.sender.is_closed()
    }

    fn handle_incoming(&mut self) -> bool {
        let mut msg_info = rmw_message_info_t::default(); // we dont care for now
        let ret = unsafe {
            rcl_take_serialized_message(
                &self.rcl_handle,
                &mut self.msg_buf as *mut rcl_serialized_message_t,
                &mut msg_info,
                std::ptr::null_mut(),
            )
        };
        if ret != RCL_RET_OK as i32 {
            log::error!("failed to take serialized message");
            return false;
        }

        let data_bytes = if self.msg_buf.buffer.is_null() {
            Vec::new()
        } else {
            unsafe {
                std::slice::from_raw_parts(self.msg_buf.buffer, self.msg_buf.buffer_length).to_vec()
            }
        };

        if let Err(e) = self.sender.try_send(data_bytes) {
            if e.is_disconnected() {
                // user dropped the handle to the stream, signal removal.
                return true;
            }
            log::debug!("error {:?}", e)
        }

        false
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_subscription_fini(&mut self.rcl_handle, node);
            rcutils_uint8_array_fini(&mut self.msg_buf as *mut rcl_serialized_message_t);
        }
    }
}

pub fn create_subscription_helper(
    node: &mut rcl_node_t, topic: &str, ts: *const rosidl_message_type_support_t,
    qos_profile: QosProfile,
) -> Result<rcl_subscription_t> {
    let mut subscription_handle = unsafe { rcl_get_zero_initialized_subscription() };
    let topic_c_string = CString::new(topic).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;

    let result = unsafe {
        let mut subscription_options = rcl_subscription_get_default_options();
        subscription_options.qos = qos_profile.into();
        rcl_subscription_init(
            &mut subscription_handle,
            node,
            ts,
            topic_c_string.as_ptr(),
            &subscription_options,
        )
    };
    if result == RCL_RET_OK as i32 {
        Ok(subscription_handle)
    } else {
        Err(Error::from_rcl_error(result))
    }
}
