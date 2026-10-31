use super::MessageInfo;
use crate::{
    error::*,
    msg_types::{VoidPtr, WrappedNativeMsg, WrappedTypesupport},
    native_buffer::{NativeBuffer, NativeLease},
};
use r2r_rcl::*;
use std::{any::TypeId, ops::Deref};

/// One asynchronously delivered batch of native ROS subscription messages.
///
/// The batch is filled by one `rcl_take_sequence` call. Its message containers belong to the
/// batch rather than the subscription endpoint, so it may outlive both the subscription and its
/// [`crate::Node`]. Dropping it returns its sequence storage to a one-slot pool owned jointly by
/// the stream and outstanding batches. Native message containers are refreshed before a reused
/// sequence is taken again because RMW implementations do not all preserve nested generated
/// storage across consecutive sequence takes.
pub struct NativeSubscriptionBatch<T>
where
    T: WrappedTypesupport,
{
    pub(crate) storage: NativeLease<NativeTakeBatch<T>>,
}

impl<T> NativeSubscriptionBatch<T>
where
    T: WrappedTypesupport,
{
    /// Number of messages taken by this batch.
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Returns true when the batch contains no messages.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Maximum number of messages that one native take can place in this batch.
    pub fn capacity(&self) -> usize {
        self.storage.capacity()
    }

    /// Borrows one generated native ROS message.
    pub fn message(&self, index: usize) -> Option<&T::CStruct> {
        self.storage.message(index)
    }

    /// Returns the RMW metadata associated with one message.
    pub fn info(&self, index: usize) -> Option<MessageInfo> {
        self.storage.info(index)
    }

    /// Iterates over native messages and their matching metadata in queue order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&T::CStruct, MessageInfo)> + '_ {
        self.storage.iter()
    }

    #[cfg(test)]
    pub(crate) fn last_refresh_count(&self) -> usize {
        self.storage.last_refresh_count()
    }
}

/// One asynchronously delivered batch of decoded Rust subscription messages.
///
/// Generated message fields are updated in place when the batch storage is reused, preserving
/// Vec, string and nested-message capacity across deliveries. Retaining several batches keeps
/// every Rust value independently owned.
///
/// ```no_run
/// use futures::StreamExt;
/// use r2r::{std_msgs::msg::UInt64MultiArray, QosProfile};
///
/// # async fn receive(node: &mut r2r::Node) -> r2r::Result<()> {
/// let mut batches = node.subscribe_batches::<UInt64MultiArray>(
///     "/samples",
///     QosProfile::default().keep_last(64),
///     16,
/// )?;
/// while let Some(batch) = batches.next().await {
///     for (message, info) in batch.iter() {
///         println!("{} values from {:?}", message.data.len(), info.publisher_gid);
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub struct SubscriptionBatch<T>
where
    T: WrappedTypesupport,
{
    pub(crate) storage: NativeLease<DecodedTakeBatch<T>>,
}

impl<T> SubscriptionBatch<T>
where
    T: WrappedTypesupport,
{
    /// Number of messages taken by this batch.
    pub fn len(&self) -> usize {
        self.storage.len
    }

    /// Returns true when the batch contains no messages.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Maximum number of messages that one native take can place in this batch.
    pub fn capacity(&self) -> usize {
        self.storage.capacity
    }

    /// Decoded Rust messages in native queue order.
    pub fn messages(&self) -> &[T] {
        &self.storage.messages[..self.storage.len]
    }

    /// RMW metadata in the same order as [`Self::messages`].
    pub fn infos(&self) -> &[MessageInfo] {
        &self.storage.infos[..self.storage.len]
    }

    /// Iterates over decoded messages and their matching metadata.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&T, &MessageInfo)> + '_ {
        self.messages().iter().zip(self.infos())
    }
}

impl<T> Deref for SubscriptionBatch<T>
where
    T: WrappedTypesupport,
{
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.messages()
    }
}

pub(crate) struct NativeTakeBatch<T>
where
    T: WrappedTypesupport,
{
    messages: Vec<WrappedNativeMsg<T>>,
    message_sequence: rmw_message_sequence_t,
    info_sequence: rmw_message_info_sequence_t,
    // Both RMW sequences retain this address until finalization.
    _allocator: Box<rcutils_allocator_t>,
    messages_to_refresh: usize,
    #[cfg(test)]
    last_refresh_count: usize,
}

// SAFETY: every native message has unique ownership and is Send by the WrappedTypesupport
// contract. The RMW arrays and boxed allocator are owned by this value and are only accessed
// through &mut self while taking. Moving the value does not move either Vec/Box allocation.
unsafe impl<T> Send for NativeTakeBatch<T> where T: WrappedTypesupport {}

impl<T> NativeTakeBatch<T>
where
    T: WrappedTypesupport,
{
    pub(crate) fn try_new(capacity: usize) -> Result<Self> {
        if capacity == 0 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }

        let mut messages = Vec::new();
        messages
            .try_reserve_exact(capacity)
            .map_err(|_| Error::RCL_RET_BAD_ALLOC)?;
        for _ in 0..capacity {
            messages.push(WrappedNativeMsg::<T>::try_new()?);
        }

        let mut allocator = Box::new(unsafe { rcutils_get_default_allocator() });
        if !unsafe { rcutils_allocator_is_valid(&*allocator) } {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let mut message_sequence = unsafe { rmw_get_zero_initialized_message_sequence() };
        let ret =
            unsafe { rmw_message_sequence_init(&mut message_sequence, capacity, &mut *allocator) };
        if ret != RMW_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let mut info_sequence = unsafe { rmw_get_zero_initialized_message_info_sequence() };
        let ret = unsafe {
            rmw_message_info_sequence_init(&mut info_sequence, capacity, &mut *allocator)
        };
        if ret != RMW_RET_OK as i32 {
            let fini_ret = unsafe { rmw_message_sequence_fini(&mut message_sequence) };
            if fini_ret != RMW_RET_OK as i32 {
                log::error!(
                    "could not finalize native message sequence after init failure: {fini_ret}"
                );
            }
            return Err(Error::from_rcl_error(ret));
        }

        if message_sequence.data.is_null()
            || info_sequence.data.is_null()
            || message_sequence.capacity < capacity
            || info_sequence.capacity < capacity
        {
            let _ = unsafe { rmw_message_info_sequence_fini(&mut info_sequence) };
            let _ = unsafe { rmw_message_sequence_fini(&mut message_sequence) };
            return Err(Error::RCL_RET_ERROR);
        }
        let pointers = unsafe {
            std::slice::from_raw_parts_mut(message_sequence.data, message_sequence.capacity)
        };
        for (pointer, message) in pointers.iter_mut().zip(&mut messages) {
            *pointer = message.void_ptr_mut();
        }

        Ok(Self {
            messages,
            message_sequence,
            info_sequence,
            _allocator: allocator,
            messages_to_refresh: 0,
            #[cfg(test)]
            last_refresh_count: 0,
        })
    }

    pub(crate) fn take(&mut self, subscription: &rcl_subscription_t) -> Result<bool> {
        if self.messages_to_refresh != 0 {
            self.refresh_messages(self.messages_to_refresh)?;
        }
        self.message_sequence.size = 0;
        self.info_sequence.size = 0;
        let ret = unsafe {
            rcl_take_sequence(
                subscription,
                self.capacity(),
                &mut self.message_sequence,
                &mut self.info_sequence,
                std::ptr::null_mut(),
            )
        };
        if ret == RCL_RET_SUBSCRIPTION_TAKE_FAILED as i32 {
            return Ok(false);
        }
        if ret != RCL_RET_OK as i32 {
            // A non-idle error may leave the sequences in an unknown but valid state. Refresh
            // every container before retrying.
            self.messages_to_refresh = self.capacity();
            return Err(Error::from_rcl_error(ret));
        }
        if self.message_sequence.size != self.info_sequence.size
            || self.message_sequence.size > self.capacity()
        {
            self.messages_to_refresh = self.capacity();
            return Err(Error::RCL_RET_ERROR);
        }
        // On success only the first `size` messages are valid output. Refresh that exact prefix
        // before reuse instead of paying generated init/fini costs for unused batch capacity.
        self.messages_to_refresh = self.message_sequence.size;
        Ok(self.message_sequence.size != 0)
    }

    pub(super) fn refresh_messages(&mut self, count: usize) -> Result<()> {
        debug_assert!(count <= self.capacity());
        let pointers = unsafe {
            std::slice::from_raw_parts_mut(
                self.message_sequence.data,
                self.message_sequence.capacity,
            )
        };
        for (pointer, message) in pointers.iter_mut().zip(&mut self.messages).take(count) {
            if !T::try_reinitialize_native(message)? {
                // The default for handwritten WrappedTypesupport implementations requests
                // replacement. Replace only the touched slot, then update its RMW pointer.
                *message = WrappedNativeMsg::<T>::try_new()?;
            }
            *pointer = message.void_ptr_mut();
        }
        self.messages_to_refresh = 0;
        #[cfg(test)]
        {
            self.last_refresh_count = count;
        }
        Ok(())
    }

    pub(crate) fn len(&self) -> usize {
        self.message_sequence.size
    }

    pub(crate) fn capacity(&self) -> usize {
        self.messages.len()
    }

    pub(crate) fn message(&self, index: usize) -> Option<&T::CStruct> {
        (index < self.len()).then(|| &*self.messages[index])
    }

    fn native_infos(&self) -> &[rmw_message_info_t] {
        if self.len() == 0 {
            &[]
        } else {
            debug_assert!(!self.info_sequence.data.is_null());
            unsafe { std::slice::from_raw_parts(self.info_sequence.data, self.len()) }
        }
    }

    fn native_messages(&self) -> impl ExactSizeIterator<Item = &T::CStruct> + '_ {
        self.messages[..self.len()].iter().map(|message| &**message)
    }

    pub(crate) fn info(&self, index: usize) -> Option<MessageInfo> {
        self.native_infos().get(index).copied().map(Into::into)
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = (&T::CStruct, MessageInfo)> + '_ {
        self.native_messages()
            .zip(self.native_infos().iter().copied().map(Into::into))
    }

    #[cfg(test)]
    pub(crate) fn last_refresh_count(&self) -> usize {
        self.last_refresh_count
    }

    #[cfg(test)]
    pub(crate) fn all_message_addresses(&self) -> Vec<*const T::CStruct> {
        self.messages
            .iter()
            .map(|message| std::ptr::from_ref(&**message))
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn message_mut_for_test(&mut self, index: usize) -> &mut WrappedNativeMsg<T> {
        &mut self.messages[index]
    }
}

impl<T> Drop for NativeTakeBatch<T>
where
    T: WrappedTypesupport,
{
    fn drop(&mut self) {
        let info_ret = unsafe { rmw_message_info_sequence_fini(&mut self.info_sequence) };
        if info_ret != RMW_RET_OK as i32 {
            log::error!("could not finalize native message-info sequence: {info_ret}");
        }
        let message_ret = unsafe { rmw_message_sequence_fini(&mut self.message_sequence) };
        if message_ret != RMW_RET_OK as i32 {
            log::error!("could not finalize native message sequence: {message_ret}");
        }
    }
}

pub(crate) struct DecodedTakeBatch<T>
where
    T: WrappedTypesupport,
{
    messages: Vec<T>,
    infos: Vec<MessageInfo>,
    len: usize,
    capacity: usize,
}

impl<T> DecodedTakeBatch<T>
where
    T: WrappedTypesupport,
{
    pub(crate) fn try_new(capacity: usize) -> Result<Self> {
        let mut messages = Vec::new();
        messages
            .try_reserve_exact(capacity)
            .map_err(|_| Error::RCL_RET_BAD_ALLOC)?;
        let mut infos = Vec::new();
        infos
            .try_reserve_exact(capacity)
            .map_err(|_| Error::RCL_RET_BAD_ALLOC)?;
        Ok(Self {
            messages,
            infos,
            len: 0,
            capacity,
        })
    }

    pub(crate) fn update_from_native(&mut self, native: &NativeTakeBatch<T>) {
        while self.messages.len() < native.len() {
            self.messages.push(T::default());
        }
        // Decode payloads without constructing metadata that would be discarded here. Metadata
        // is converted exactly once into the aligned output array below.
        for (message, native_message) in self.messages.iter_mut().zip(native.native_messages()) {
            message.update_from_native(native_message);
        }
        self.infos.clear();
        self.infos
            .extend(native.native_infos().iter().copied().map(Into::into));
        self.len = native.len();
    }
}

pub(crate) struct NativeBatchSubscriber<T>
where
    T: WrappedTypesupport,
{
    pub(crate) rcl_handle: rcl_subscription_t,
    pub(crate) sender: futures::channel::mpsc::Sender<NativeSubscriptionBatch<T>>,
    pub(crate) match_state: super::SubscriptionMatchState,
    pub(crate) batch_size: usize,
    pub(crate) buffer: NativeBuffer<NativeTakeBatch<T>>,
}

pub(crate) struct TypedBatchSubscriber<T>
where
    T: WrappedTypesupport,
{
    pub(crate) rcl_handle: rcl_subscription_t,
    pub(crate) sender: futures::channel::mpsc::Sender<SubscriptionBatch<T>>,
    pub(crate) match_state: super::SubscriptionMatchState,
    pub(crate) batch_size: usize,
    pub(crate) native_buffer: NativeBuffer<NativeTakeBatch<T>>,
    pub(crate) rust_buffer: NativeBuffer<DecodedTakeBatch<T>>,
}

impl<T> super::Subscriber_ for NativeBatchSubscriber<T>
where
    T: WrappedTypesupport,
{
    fn handle(&self) -> &rcl_subscription_t {
        &self.rcl_handle
    }

    fn message_type_id(&self) -> Option<TypeId> {
        Some(TypeId::of::<T>())
    }

    fn match_state(&self) -> Option<&super::SubscriptionMatchState> {
        Some(&self.match_state)
    }

    fn match_state_mut(&mut self) -> Option<&mut super::SubscriptionMatchState> {
        Some(&mut self.match_state)
    }

    fn should_remove(&mut self) -> bool {
        self.sender.is_closed()
    }

    fn handle_incoming(&mut self) -> Result<super::SubscriptionHandleResult> {
        match super::sender_ready(&mut self.sender) {
            Ok(true) => {}
            Ok(false) => return Ok(super::SubscriptionHandleResult::Backpressured),
            Err(_) => return Ok(super::SubscriptionHandleResult::Remove),
        }
        let mut storage = self
            .buffer
            .checkout(|| NativeTakeBatch::<T>::try_new(self.batch_size))?;
        if !storage.take(&self.rcl_handle)? {
            return Ok(super::SubscriptionHandleResult::Idle);
        }
        if let Err(error) = self.sender.try_send(NativeSubscriptionBatch { storage }) {
            if error.is_disconnected() {
                return Ok(super::SubscriptionHandleResult::Remove);
            }
            log::error!("could not deliver native subscription batch: {error:?}");
            return Ok(super::SubscriptionHandleResult::Backpressured);
        }
        Ok(super::SubscriptionHandleResult::Taken)
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        self.match_state.clear_events();
        let ret = unsafe { rcl_subscription_fini(&mut self.rcl_handle, node) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize native batch subscription: {ret}");
        }
    }
}

impl<T> super::Subscriber_ for TypedBatchSubscriber<T>
where
    T: WrappedTypesupport,
{
    fn handle(&self) -> &rcl_subscription_t {
        &self.rcl_handle
    }

    fn message_type_id(&self) -> Option<TypeId> {
        Some(TypeId::of::<T>())
    }

    fn match_state(&self) -> Option<&super::SubscriptionMatchState> {
        Some(&self.match_state)
    }

    fn match_state_mut(&mut self) -> Option<&mut super::SubscriptionMatchState> {
        Some(&mut self.match_state)
    }

    fn should_remove(&mut self) -> bool {
        self.sender.is_closed()
    }

    fn handle_incoming(&mut self) -> Result<super::SubscriptionHandleResult> {
        match super::sender_ready(&mut self.sender) {
            Ok(true) => {}
            Ok(false) => return Ok(super::SubscriptionHandleResult::Backpressured),
            Err(_) => return Ok(super::SubscriptionHandleResult::Remove),
        }
        let mut native = self
            .native_buffer
            .checkout(|| NativeTakeBatch::<T>::try_new(self.batch_size))?;
        if !native.take(&self.rcl_handle)? {
            return Ok(super::SubscriptionHandleResult::Idle);
        }
        let mut storage = self
            .rust_buffer
            .checkout(|| DecodedTakeBatch::try_new(self.batch_size))?;
        storage.update_from_native(&native);
        drop(native);
        if let Err(error) = self.sender.try_send(SubscriptionBatch { storage }) {
            if error.is_disconnected() {
                return Ok(super::SubscriptionHandleResult::Remove);
            }
            log::error!("could not deliver typed subscription batch: {error:?}");
            return Ok(super::SubscriptionHandleResult::Backpressured);
        }
        Ok(super::SubscriptionHandleResult::Taken)
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        self.match_state.clear_events();
        let ret = unsafe { rcl_subscription_fini(&mut self.rcl_handle, node) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize typed batch subscription: {ret}");
        }
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::*;
    use crate::std_msgs::msg::UInt64MultiArray;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex as StdMutex,
    };

    #[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
    struct Handwritten;

    static LOCK: StdMutex<()> = StdMutex::new(());
    static ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
    static CREATED: AtomicUsize = AtomicUsize::new(0);
    static DESTROYED: AtomicUsize = AtomicUsize::new(0);
    static FAIL_AT: AtomicUsize = AtomicUsize::new(usize::MAX);

    unsafe impl WrappedTypesupport for Handwritten {
        type CStruct = usize;

        fn get_ts() -> &'static rosidl_message_type_support_t {
            unreachable!("native refresh fixture does not access type support")
        }

        fn create_msg() -> *mut Self::CStruct {
            let attempt = ATTEMPTS.fetch_add(1, Ordering::Relaxed) + 1;
            if attempt == FAIL_AT.load(Ordering::Relaxed) {
                return std::ptr::null_mut();
            }
            CREATED.fetch_add(1, Ordering::Relaxed);
            Box::into_raw(Box::new(attempt))
        }

        fn destroy_msg(message: *mut Self::CStruct) {
            DESTROYED.fetch_add(1, Ordering::Relaxed);
            drop(unsafe { Box::from_raw(message) });
        }

        fn from_native(_message: &Self::CStruct) -> Self {
            Self
        }

        fn copy_to_native(&self, _message: &mut Self::CStruct) {}
    }

    #[test]
    fn handwritten_prefix_refresh_updates_each_pointer_and_remains_valid_after_failure() {
        let _lock = LOCK.lock().unwrap();
        ATTEMPTS.store(0, Ordering::Relaxed);
        CREATED.store(0, Ordering::Relaxed);
        DESTROYED.store(0, Ordering::Relaxed);
        FAIL_AT.store(usize::MAX, Ordering::Relaxed);

        let mut batch = NativeTakeBatch::<Handwritten>::try_new(4).unwrap();
        let before = unsafe { std::slice::from_raw_parts(batch.message_sequence.data, 4).to_vec() };

        FAIL_AT.store(6, Ordering::Relaxed);
        assert!(matches!(batch.refresh_messages(3), Err(Error::RCL_RET_BAD_ALLOC)));
        let after_failure =
            unsafe { std::slice::from_raw_parts(batch.message_sequence.data, 4).to_vec() };
        assert_ne!(after_failure[0], before[0]);
        assert_eq!(&after_failure[1..], &before[1..]);
        for (pointer, message) in after_failure.iter().zip(&mut batch.messages) {
            assert_eq!(*pointer, message.void_ptr_mut());
        }

        FAIL_AT.store(usize::MAX, Ordering::Relaxed);
        batch.refresh_messages(3).unwrap();
        let after_retry =
            unsafe { std::slice::from_raw_parts(batch.message_sequence.data, 4).to_vec() };
        assert_ne!(after_retry[0], after_failure[0]);
        assert_ne!(after_retry[1], after_failure[1]);
        assert_ne!(after_retry[2], after_failure[2]);
        assert_eq!(after_retry[3], before[3]);
        drop(batch);
        assert_eq!(CREATED.load(Ordering::Relaxed), DESTROYED.load(Ordering::Relaxed));
    }

    #[test]
    fn typed_batch_converts_each_message_info_once() {
        let mut native = NativeTakeBatch::<UInt64MultiArray>::try_new(4).unwrap();
        native.message_sequence.size = 4;
        native.info_sequence.size = 4;
        let mut decoded = DecodedTakeBatch::<UInt64MultiArray>::try_new(4).unwrap();
        crate::subscribers::reset_message_info_conversion_count();
        decoded.update_from_native(&native);
        assert_eq!(decoded.len, 4);
        assert_eq!(decoded.infos.len(), 4);
        assert_eq!(crate::subscribers::message_info_conversion_count(), 4);
    }
}
