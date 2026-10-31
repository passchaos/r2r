use crate::error::*;
use r2r_msg_gen::*;
use r2r_rcl::{
    rcl_serialized_message_t, rosidl_action_type_support_t, rosidl_message_type_support_t,
    rosidl_service_type_support_t,
};
use serde::{Deserialize, Serialize};
use std::{
    boxed::Box,
    cell::RefCell,
    fmt::Debug,
    ops::{Deref, DerefMut},
};

pub mod generated_msgs {
    #![allow(clippy::all)]
    use super::*;
    include!(concat!(env!("OUT_DIR"), "/_r2r_generated_msgs.rs"));
    include!(concat!(env!("OUT_DIR"), "/_r2r_generated_untyped_helper.rs"));
    include!(concat!(env!("OUT_DIR"), "/_r2r_generated_service_helper.rs"));
    include!(concat!(env!("OUT_DIR"), "/_r2r_generated_action_helper.rs"));
}

use generated_msgs::{builtin_interfaces, unique_identifier_msgs};

pub(crate) fn uuid_msg_to_uuid(msg: &unique_identifier_msgs::msg::UUID) -> Result<uuid::Uuid> {
    uuid::Uuid::from_slice(&msg.uuid).map_err(|_| Error::InvalidActionUuidLength {
        actual: msg.uuid.len(),
    })
}

mod serialized_buffer;
pub(crate) use serialized_buffer::SerializedMessageBuffer;
mod fixed_array;

pub(crate) unsafe fn serialized_message_bytes(msg: &rcl_serialized_message_t) -> Result<&[u8]> {
    if msg.buffer_length == 0 {
        return Ok(&[]);
    }
    if msg.buffer.is_null() || msg.buffer_length > msg.buffer_capacity {
        return Err(Error::RCL_RET_ERROR);
    }
    Ok(std::slice::from_raw_parts(msg.buffer, msg.buffer_length))
}

thread_local! {
    static SERIALIZED_MESSAGE_CACHE: std::result::Result<RefCell<SerializedMessageBuffer>, i32> =
        SerializedMessageBuffer::with_capacity(0).map(RefCell::new);
}

/// Connects a Rust message type to its generated ROS C representation.
///
/// # Safety
///
/// Implementations must return the matching, process-lifetime type support, create and
/// destroy compatible native allocations, and only access valid `CStruct` values. Native
/// allocations owned by [`WrappedNativeMsg`] must also be safe to move to another thread,
/// and all static operations must be safe to call concurrently. An overridden
/// [`Self::try_reinitialize_native`] must preserve an initialized, destructible value on every
/// error and whenever it returns `Ok(false)`.
pub unsafe trait WrappedTypesupport:
    Serialize + for<'de> Deserialize<'de> + Default + Debug + Clone + Send + Sync + 'static
{
    type CStruct;

    fn get_ts() -> &'static rosidl_message_type_support_t;
    /// Resolves native type support, reporting missing runtime interface libraries.
    fn try_get_ts() -> Result<&'static rosidl_message_type_support_t> {
        Ok(Self::get_ts())
    }
    fn create_msg() -> *mut Self::CStruct;
    /// Prepares a matching native constructor/finalizer before creating owned storage.
    fn try_create_msg() -> Result<*mut Self::CStruct> {
        Ok(Self::create_msg())
    }
    fn destroy_msg(msg: *mut Self::CStruct);
    fn from_native(msg: &Self::CStruct) -> Self;
    /// Updates an existing Rust message from native storage.
    ///
    /// Custom implementations remain source compatible through this replacement default.
    /// Generated messages override it to retain reusable Vec/string/nested-message capacity.
    fn update_from_native(&mut self, msg: &Self::CStruct) {
        *self = Self::from_native(msg);
    }
    /// Reinitializes an owned native message in place when the implementation supports it.
    ///
    /// Returning `Ok(true)` guarantees that `msg` is freshly initialized and retains the same
    /// outer address. Returning `Ok(false)` leaves it unchanged and requests a replacement
    /// allocation. Any error must also leave `msg` initialized and safe to retry or destroy.
    fn try_reinitialize_native(_msg: &mut Self::CStruct) -> Result<bool> {
        Ok(false)
    }
    fn copy_to_native(&self, msg: &mut Self::CStruct);

    /// Tries to update an initialized native message from this Rust value.
    ///
    /// On error, `msg` must remain initialized and safe to destroy. Callers may retire the native
    /// allocation instead of reusing partially updated fields.
    fn try_copy_to_native(&self, msg: &mut Self::CStruct) -> Result<()> {
        self.copy_to_native(msg);
        Ok(())
    }

    /// This serializes the message using ROS2 methods.
    fn to_serialized_bytes(&self) -> Result<Vec<u8>> {
        SERIALIZED_MESSAGE_CACHE.with(|msg_buf| {
            let native_msg = WrappedNativeMsg::<Self>::try_from(self)?;

            let mut msg_buf = msg_buf
                .as_ref()
                .map_err(|err| Error::from_rcl_error(*err))?
                .borrow_mut();
            Ok(msg_buf.serialize(&native_msg)?.to_vec())
        })
    }

    /// This deserializes the message using ROS2 methods.
    fn from_serialized_bytes(data: &[u8]) -> Result<Self> {
        use r2r_rcl::*;

        let mut native_msg = WrappedNativeMsg::<Self>::try_new()?;

        let msg_buf = rcl_serialized_message_t {
            buffer: data.as_ptr() as *mut u8,
            buffer_length: data.len(),
            buffer_capacity: data.len(),

            // Since its read only, this should never be used ..
            allocator: unsafe { rcutils_get_default_allocator() },
        };

        // Note From the docs of rmw_deserialize, its not clear whether this reuses
        // any part of msg_buf. However it shouldn't matter since from_native
        // clones everything again anyway ..
        let result = unsafe {
            rmw_deserialize(
                &msg_buf as *const rcl_serialized_message_t,
                Self::try_get_ts()?,
                native_msg.void_ptr_mut(),
            )
        };

        if result == RCL_RET_OK as i32 {
            Ok(Self::from_native(&native_msg))
        } else {
            Err(Error::from_rcl_error(result))
        }
    }
}

#[allow(dead_code)] // Used by freshly generated bindings; old doc-only snapshots do not call it.
pub(crate) fn check_rosidl_runtime_ret(success: bool) -> Result<()> {
    if success {
        Ok(())
    } else {
        Err(Error::RCL_RET_BAD_ALLOC)
    }
}

/// Builds a fresh native value before retiring the current contents, then transfers the fresh
/// contents into the same outer address. Generated ROS initializers finalize their temporary on
/// failure; the current value is not touched until initialization succeeds.
///
/// # Safety
///
/// Both functions must operate on `C`, `message` must be initialized, `initialize` must accept
/// zeroed storage and clean its temporary on failure, and `finalize` must leave no live resources.
pub(crate) unsafe fn try_reinitialize_native_message<C>(
    message: &mut C, initialize: unsafe extern "C" fn(*mut C) -> bool,
    finalize: unsafe extern "C" fn(*mut C),
) -> Result<()> {
    let mut replacement = std::mem::MaybeUninit::<C>::zeroed();
    if !unsafe { initialize(replacement.as_mut_ptr()) } {
        return Err(Error::RCL_RET_BAD_ALLOC);
    }
    let destination = std::ptr::from_mut(message);
    unsafe {
        finalize(destination);
        std::ptr::copy_nonoverlapping(replacement.as_ptr(), destination, 1);
    }
    Ok(())
}

/// Connects a Rust service marker to its generated ROS C type support.
///
/// # Safety
///
/// `get_ts` must return the process-lifetime type support matching `Request` and `Response`.
/// On Iron and newer, `Event` must match the native service introspection event type as well.
pub unsafe trait WrappedServiceTypeSupport: Debug + Clone + Send + Sync {
    type Request: WrappedTypesupport;
    type Response: WrappedTypesupport;
    #[cfg(any(
        r2r__ros__distro__iron,
        r2r__ros__distro__jazzy,
        r2r__ros__distro__kilted,
        r2r__ros__distro__lyrical,
        r2r__ros__distro__rolling
    ))]
    /// Event message published by native service introspection on Iron and newer distributions.
    type Event: WrappedTypesupport;

    fn get_ts() -> &'static rosidl_service_type_support_t;
    /// Resolves native type support without changing custom implementation requirements.
    fn try_get_ts() -> Result<&'static rosidl_service_type_support_t> {
        Ok(Self::get_ts())
    }
}

/// Connects a Rust action marker to its generated ROS C type support.
///
/// # Safety
///
/// `get_ts` and every associated service/message type must describe the same action, and
/// the construction/destructuring methods must preserve their generated wire layouts.
/// A native feedback UUID accessor, when provided, must agree with the destructured goal ID
/// and access only the initialized native message's identity fields. If
/// `supports_native_feedback_delivery` returns `true`, the accessor must return `Some` for every
/// initialized feedback message.
pub unsafe trait WrappedActionTypeSupport: Debug + Clone + Send + Sync {
    type Goal: WrappedTypesupport;
    type Result: WrappedTypesupport;
    type Feedback: WrappedTypesupport;

    // internal...
    type FeedbackMessage: WrappedTypesupport;
    type SendGoal: WrappedServiceTypeSupport;
    type GetResult: WrappedServiceTypeSupport;

    fn get_ts() -> &'static rosidl_action_type_support_t;
    /// Resolves native type support without changing custom implementation requirements.
    fn try_get_ts() -> Result<&'static rosidl_action_type_support_t> {
        Ok(Self::get_ts())
    }

    fn make_goal_request_msg(
        goal_id: unique_identifier_msgs::msg::UUID, goal: Self::Goal,
    ) -> <<Self as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Request;
    fn make_goal_response_msg(
        accepted: bool, stamp: builtin_interfaces::msg::Time,
    ) -> <<Self as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response;
    fn make_feedback_msg(
        goal_id: unique_identifier_msgs::msg::UUID, feedback: Self::Feedback,
    ) -> Self::FeedbackMessage;
    fn make_result_request_msg(
        goal_id: unique_identifier_msgs::msg::UUID,
    ) -> <<Self as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Request;
    fn make_result_response_msg(
        status: i8, result: Self::Result,
    ) -> <<Self as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response;
    fn destructure_goal_request_msg(
        msg: <<Self as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Request,
    ) -> (unique_identifier_msgs::msg::UUID, Self::Goal);
    fn destructure_goal_response_msg(
        msg: <<Self as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response,
    ) -> (bool, builtin_interfaces::msg::Time);
    fn destructure_feedback_msg(
        msg: Self::FeedbackMessage,
    ) -> (unique_identifier_msgs::msg::UUID, Self::Feedback);
    /// Reads feedback identity without constructing the Rust payload. Custom action wrappers
    /// may retain the default to use complete decoding and routing instead.
    fn native_feedback_uuid(
        _msg: &<Self::FeedbackMessage as WrappedTypesupport>::CStruct,
    ) -> Option<[u8; 16]> {
        None
    }
    /// Reports whether [`Self::native_feedback_uuid`] can route an initialized native
    /// feedback message without first constructing its owned Rust payload.
    ///
    /// Generated action types return `true`. Custom action wrappers retain `false` until
    /// they provide a native identity accessor that satisfies this trait's safety contract.
    fn supports_native_feedback_delivery() -> bool {
        false
    }
    fn destructure_result_response_msg(
        msg: <<Self as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response,
    ) -> (i8, Self::Result);
    fn destructure_result_request_msg(
        msg: <<Self as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Request,
    ) -> unique_identifier_msgs::msg::UUID;
}

/// This struct wraps a RCL message.
///
/// It contains a pointer to a C struct.
pub struct WrappedNativeMsg<T>
where
    T: WrappedTypesupport,
{
    pub(crate) msg: *mut T::CStruct,
    deallocator: Option<Box<dyn FnOnce(*mut T::CStruct) + Send>>,
}

/// A read-only, non-owning view of a native ROS message received during subscription dispatch.
///
/// The lifetime prevents the native storage from escaping its callback. The owning allocation and
/// deallocator remain private to r2r, while [`Deref`] allows direct native-field inspection.
/// Pass this view to [`Publisher::publish_native_ref`](crate::Publisher::publish_native_ref)
/// to forward the same native message without constructing an owned Rust payload.
///
/// Native fields are intentionally read-only in safe Rust:
///
/// ```compile_fail
/// use r2r::{std_msgs::msg::Int32, NativeCallbackSubscription, Node, QosProfile};
///
/// fn install(node: &mut Node) -> r2r::Result<NativeCallbackSubscription> {
///     node.subscribe_native_callback::<Int32, _>(
///         "/input",
///         QosProfile::default(),
///         |mut message, _| {
///             message.data = 7;
///             Ok(())
///         },
///     )
/// }
/// ```
///
/// The callback lifetime also prevents retaining the view:
///
/// ```compile_fail
/// use r2r::{std_msgs::msg::Int32, NativeCallbackSubscription, Node, QosProfile};
///
/// fn install(node: &mut Node) -> r2r::Result<NativeCallbackSubscription> {
///     let mut escaped = None;
///     node.subscribe_native_callback::<Int32, _>(
///         "/input",
///         QosProfile::default(),
///         move |message, _| {
///             escaped = Some(message);
///             Ok(())
///         },
///     )
/// }
/// ```
pub struct NativeMessageRef<'message, T>
where
    T: WrappedTypesupport,
{
    message: &'message T::CStruct,
}

impl<'message, T> NativeMessageRef<'message, T>
where
    T: WrappedTypesupport,
{
    pub(crate) fn from_owned(message: &'message WrappedNativeMsg<T>) -> Self {
        Self { message }
    }

    pub(crate) fn void_ptr(&self) -> *const std::os::raw::c_void {
        std::ptr::from_ref::<T::CStruct>(self.message).cast()
    }
}

impl<T> Deref for NativeMessageRef<'_, T>
where
    T: WrappedTypesupport,
{
    type Target = T::CStruct;

    fn deref(&self) -> &Self::Target {
        self.message
    }
}

pub trait VoidPtr {
    fn void_ptr(&self) -> *const std::os::raw::c_void;
    fn void_ptr_mut(&mut self) -> *mut std::os::raw::c_void;
}

#[derive(Debug)]
pub struct WrappedNativeMsgUntyped {
    pub ts: &'static rosidl_message_type_support_t,
    msg: *mut std::os::raw::c_void,
    destroy: fn(*mut std::os::raw::c_void),
    msg_to_json: fn(
        native: *const std::os::raw::c_void,
    ) -> std::result::Result<serde_json::Value, serde_json::error::Error>,
    msg_from_json: fn(native: *mut std::os::raw::c_void, json: serde_json::Value) -> Result<()>,
}

// SAFETY: every constructor installs operations from one `WrappedTypesupport`
// implementation, whose unsafe contract requires owned native allocations to be
// movable across threads. The wrapper retains unique ownership of that allocation.
unsafe impl Send for WrappedNativeMsgUntyped {}

unsafe impl Send for UntypedServiceSupport {}
pub struct UntypedServiceSupport {
    pub ts: &'static rosidl_service_type_support_t,
    pub make_request_msg: fn() -> Result<WrappedNativeMsgUntyped>,
    pub make_response_msg: fn() -> Result<WrappedNativeMsgUntyped>,
}

impl UntypedServiceSupport {
    #[cfg(feature = "doc-only")]
    fn new<T>() -> Self
    where
        T: WrappedServiceTypeSupport,
    {
        Self::try_new::<T>().expect("native type support unavailable")
    }
    fn try_new<T>() -> Result<Self>
    where
        T: WrappedServiceTypeSupport,
    {
        let make_request_msg = WrappedNativeMsgUntyped::try_new::<T::Request>;
        let make_response_msg = WrappedNativeMsgUntyped::try_new::<T::Response>;

        Ok(UntypedServiceSupport {
            ts: T::try_get_ts()?,
            make_request_msg,
            make_response_msg,
        })
    }
}

// For now only the client side is implemented.
// SAFETY: generated type support has process lifetime and the erased operations below are
// explicitly thread-safe. The native RCL type support metadata is immutable after generation.
unsafe impl Send for UntypedActionSupport {}
pub struct UntypedActionSupport {
    pub(crate) ts: &'static rosidl_action_type_support_t,
    pub(crate) create_server: fn(
        &mut crate::Node,
        &str,
        crate::ActionServerOptions,
    ) -> Result<crate::action_servers::ActionServerUntyped>,

    pub(crate) make_goal_request_msg:
        fn(unique_identifier_msgs::msg::UUID, serde_json::Value) -> Result<WrappedNativeMsgUntyped>,
    pub(crate) make_goal_response_msg: fn() -> Result<WrappedNativeMsgUntyped>,
    pub(crate) destructure_goal_response_msg:
        fn(WrappedNativeMsgUntyped) -> (bool, builtin_interfaces::msg::Time),

    pub(crate) make_feedback_msg: fn() -> Result<WrappedNativeMsgUntyped>,
    pub(crate) native_feedback_uuid: fn(&WrappedNativeMsgUntyped) -> Option<[u8; 16]>,
    pub(crate) destructure_feedback_msg:
        fn(
            &WrappedNativeMsgUntyped,
        ) -> (unique_identifier_msgs::msg::UUID, Result<serde_json::Value>),

    pub(crate) make_result_request_msg:
        fn(unique_identifier_msgs::msg::UUID) -> Result<WrappedNativeMsgUntyped>,
    pub(crate) make_result_response_msg: fn() -> Result<WrappedNativeMsgUntyped>,
    pub(crate) destructure_result_response_msg:
        fn(WrappedNativeMsgUntyped) -> (i8, Result<serde_json::Value>),
}

fn make_action_goal_request<T>(
    goal_id: unique_identifier_msgs::msg::UUID, goal: serde_json::Value,
) -> Result<WrappedNativeMsgUntyped>
where
    T: WrappedActionTypeSupport,
{
    let goal = serde_json::from_value(goal).map_err(|err| Error::SerdeError {
        err: err.to_string(),
    })?;
    let request = T::make_goal_request_msg(goal_id, goal);
    WrappedNativeMsgUntyped::try_from_typed(&request)
}

fn make_action_goal_response<T>() -> Result<WrappedNativeMsgUntyped>
where
    T: WrappedActionTypeSupport,
{
    WrappedNativeMsgUntyped::try_new::<
        <<T as WrappedActionTypeSupport>::SendGoal as WrappedServiceTypeSupport>::Response,
    >()
}

fn destructure_action_goal_response<T>(
    msg: WrappedNativeMsgUntyped,
) -> (bool, builtin_interfaces::msg::Time)
where
    T: WrappedActionTypeSupport,
{
    let msg = unsafe {
        <<T::SendGoal as WrappedServiceTypeSupport>::Response>::from_native(
            &*(msg.msg
                as *const <<T::SendGoal as WrappedServiceTypeSupport>::Response as WrappedTypesupport>::CStruct),
        )
    };
    T::destructure_goal_response_msg(msg)
}

fn destructure_action_feedback<T>(
    msg: &WrappedNativeMsgUntyped,
) -> (unique_identifier_msgs::msg::UUID, Result<serde_json::Value>)
where
    T: WrappedActionTypeSupport,
{
    let msg = unsafe {
        T::FeedbackMessage::from_native(
            &*(msg.msg as *const <T::FeedbackMessage as WrappedTypesupport>::CStruct),
        )
    };
    let (uuid, feedback) = T::destructure_feedback_msg(msg);
    let json = serde_json::to_value(feedback).map_err(|err| Error::SerdeError {
        err: err.to_string(),
    });
    (uuid, json)
}

fn native_action_feedback_uuid<T: WrappedActionTypeSupport>(
    msg: &WrappedNativeMsgUntyped,
) -> Option<[u8; 16]> {
    // The constructor and this accessor are installed together for the same action type.
    let native =
        unsafe { &*(msg.msg as *const <T::FeedbackMessage as WrappedTypesupport>::CStruct) };
    T::native_feedback_uuid(native)
}

fn make_action_result_request<T>(
    goal_id: unique_identifier_msgs::msg::UUID,
) -> Result<WrappedNativeMsgUntyped>
where
    T: WrappedActionTypeSupport,
{
    let request = T::make_result_request_msg(goal_id);
    WrappedNativeMsgUntyped::try_from_typed(&request)
}

fn make_action_result_response<T>() -> Result<WrappedNativeMsgUntyped>
where
    T: WrappedActionTypeSupport,
{
    WrappedNativeMsgUntyped::try_new::<
        <<T as WrappedActionTypeSupport>::GetResult as WrappedServiceTypeSupport>::Response,
    >()
}

fn destructure_action_result_response<T>(
    msg: WrappedNativeMsgUntyped,
) -> (i8, Result<serde_json::Value>)
where
    T: WrappedActionTypeSupport,
{
    let msg = unsafe {
        <<T::GetResult as WrappedServiceTypeSupport>::Response>::from_native(
            &*(msg.msg
                as *const <<T::GetResult as WrappedServiceTypeSupport>::Response as WrappedTypesupport>::CStruct),
        )
    };
    let (status, result) = T::destructure_result_response_msg(msg);
    let json = serde_json::to_value(result).map_err(|err| Error::SerdeError {
        err: err.to_string(),
    });
    (status, json)
}

impl UntypedActionSupport {
    #[cfg(feature = "doc-only")]
    fn new<T>() -> Self
    where
        T: WrappedActionTypeSupport + 'static,
    {
        Self::try_new::<T>().expect("native type support unavailable")
    }
    fn try_new<T>() -> Result<Self>
    where
        T: WrappedActionTypeSupport + 'static,
    {
        Ok(UntypedActionSupport {
            ts: T::try_get_ts()?,
            create_server: crate::action_servers::create_action_server_untyped_for::<T>,
            make_goal_request_msg: make_action_goal_request::<T>,
            make_goal_response_msg: make_action_goal_response::<T>,
            destructure_goal_response_msg: destructure_action_goal_response::<T>,
            make_feedback_msg: WrappedNativeMsgUntyped::try_new::<T::FeedbackMessage>,
            native_feedback_uuid: native_action_feedback_uuid::<T>,
            destructure_feedback_msg: destructure_action_feedback::<T>,
            make_result_request_msg: make_action_result_request::<T>,
            make_result_response_msg: make_action_result_response::<T>,
            destructure_result_response_msg: destructure_action_result_response::<T>,
        })
    }
}

impl WrappedNativeMsgUntyped {
    fn try_from_typed<T>(msg: &T) -> Result<Self>
    where
        T: WrappedTypesupport,
    {
        let native = Self::try_new::<T>()?;
        unsafe { msg.try_copy_to_native(&mut *(native.msg as *mut T::CStruct))? };
        Ok(native)
    }

    #[allow(dead_code)] // Used by checked-in doc-only bindings generated by older releases.
    fn new<T>() -> Self
    where
        T: WrappedTypesupport,
    {
        Self::try_new::<T>().expect("failed to allocate native ROS message")
    }

    fn try_new<T>() -> Result<Self>
    where
        T: WrappedTypesupport,
    {
        let destroy = |native: *mut std::os::raw::c_void| {
            let native_msg = native as *mut T::CStruct;
            T::destroy_msg(native_msg);
        };

        let msg_to_json = |native: *const std::os::raw::c_void| {
            let msg = unsafe { T::from_native(&*(native as *const T::CStruct)) };
            serde_json::to_value(&msg)
        };

        let msg_from_json = |native: *mut std::os::raw::c_void, json: serde_json::Value| {
            let msg: T = serde_json::from_value(json).map_err(|serde_err| Error::SerdeError {
                err: serde_err.to_string(),
            })?;
            unsafe { msg.try_copy_to_native(&mut *(native as *mut T::CStruct)) }
        };

        let ts = T::try_get_ts()?;
        let msg = T::try_create_msg()? as *mut std::os::raw::c_void;
        if msg.is_null() {
            return Err(Error::RCL_RET_BAD_ALLOC);
        }

        Ok(WrappedNativeMsgUntyped {
            ts,
            msg,
            destroy,
            msg_to_json,
            msg_from_json,
        })
    }

    pub fn from_serialized_bytes(&mut self, data: &[u8]) -> Result<()> {
        // TODO: Copy paste from above, should refactor later.
        use r2r_rcl::*;

        let msg_buf = rcl_serialized_message_t {
            buffer: data.as_ptr() as *mut u8,
            buffer_length: data.len(),
            buffer_capacity: data.len(),

            // Since its read only, this should never be used ..
            allocator: unsafe { rcutils_get_default_allocator() },
        };

        // Note From the docs of rmw_deserialize, its not clear whether this reuses
        // any part of msg_buf. However it shouldn't matter since from_native
        // clones everything again anyway ..
        let result = unsafe {
            rmw_deserialize(&msg_buf as *const rcl_serialized_message_t, self.ts, self.msg)
        };

        if result == RCL_RET_OK as i32 {
            Ok(())
        } else {
            Err(Error::from_rcl_error(result))
        }
    }

    pub fn to_json(&self) -> Result<serde_json::Value> {
        let json = (self.msg_to_json)(self.msg);
        json.map_err(|serde_err| Error::SerdeError {
            err: serde_err.to_string(),
        })
    }

    pub fn from_json(&mut self, json: serde_json::Value) -> Result<()> {
        (self.msg_from_json)(self.msg, json)
    }
}

impl VoidPtr for WrappedNativeMsgUntyped {
    fn void_ptr(&self) -> *const std::os::raw::c_void {
        self.msg as *const _ as *const std::os::raw::c_void
    }

    fn void_ptr_mut(&mut self) -> *mut std::os::raw::c_void {
        self.msg as *mut _ as *mut std::os::raw::c_void
    }
}

impl Drop for WrappedNativeMsgUntyped {
    fn drop(&mut self) {
        (self.destroy)(self.msg);
    }
}

impl<T> WrappedNativeMsg<T>
where
    T: WrappedTypesupport + 'static,
{
    pub fn new() -> Self {
        Self::try_new().expect("failed to allocate native ROS message")
    }

    pub fn try_new() -> Result<Self> {
        let msg = T::try_create_msg()?;
        if msg.is_null() {
            return Err(Error::RCL_RET_BAD_ALLOC);
        }
        Ok(WrappedNativeMsg {
            msg,
            deallocator: Some(Box::new(T::destroy_msg)),
        })
    }

    pub fn from(msg: &T) -> Self {
        Self::try_from(msg).expect("failed to copy Rust message into native ROS storage")
    }

    pub fn try_from(msg: &T) -> Result<Self> {
        let mut native_msg = Self::try_new()?;
        msg.try_copy_to_native(&mut native_msg)?;
        Ok(native_msg)
    }

    /// Wrap a native message loan whose ownership is returned by `deallocator`.
    ///
    /// # Safety
    ///
    /// `msg` must be non-null, point to a valid `T::CStruct`, and remain valid until
    /// `deallocator` is invoked or ownership is released. The deallocator must accept
    /// exactly this allocation and may be called at most once.
    pub unsafe fn from_loaned(
        msg: *mut T::CStruct,
        deallocator: Box<dyn FnOnce(*mut <T as WrappedTypesupport>::CStruct) + Send>,
    ) -> Self {
        assert!(!msg.is_null(), "cannot wrap a null loaned ROS message");
        WrappedNativeMsg {
            msg,
            deallocator: Some(deallocator),
        }
    }
}

// SAFETY: the trait contract requires owned native allocations to be transferable between
// threads, and the stored deallocator is itself `Send`. The wrapper maintains unique ownership.
unsafe impl<T> Send for WrappedNativeMsg<T> where T: WrappedTypesupport {}

impl<T> Default for WrappedNativeMsg<T>
where
    T: WrappedTypesupport + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<T: 'static> VoidPtr for WrappedNativeMsg<T>
where
    T: WrappedTypesupport,
{
    fn void_ptr(&self) -> *const std::os::raw::c_void {
        self.msg as *const _ as *const std::os::raw::c_void
    }

    fn void_ptr_mut(&mut self) -> *mut std::os::raw::c_void {
        self.msg as *mut _ as *mut std::os::raw::c_void
    }
}

impl<T> Drop for WrappedNativeMsg<T>
where
    T: WrappedTypesupport,
{
    fn drop(&mut self) {
        if let Some(deallocator) = self.deallocator.take() {
            (deallocator)(self.msg);
        }
    }
}

impl<T> Deref for WrappedNativeMsg<T>
where
    T: WrappedTypesupport,
{
    type Target = T::CStruct;

    fn deref(&self) -> &Self::Target {
        unsafe { &(*self.msg) }
    }
}

impl<T> DerefMut for WrappedNativeMsg<T>
where
    T: WrappedTypesupport,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut (*self.msg) }
    }
}

#[cfg(test)]
mod tests {
    use super::{generated_msgs::*, *};
    use r2r_rcl::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[repr(C)]
    struct ResetFixture {
        value: usize,
    }

    static RESET_FINALIZATIONS: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn initialize_reset_fixture(message: *mut ResetFixture) -> bool {
        unsafe { message.write(ResetFixture { value: 73 }) };
        true
    }

    unsafe extern "C" fn fail_reset_fixture(message: *mut ResetFixture) -> bool {
        unsafe { message.write(ResetFixture { value: 99 }) };
        false
    }

    unsafe extern "C" fn finalize_reset_fixture(message: *mut ResetFixture) {
        RESET_FINALIZATIONS.fetch_add(1, Ordering::Relaxed);
        unsafe { (*message).value = 0 };
    }

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct CopyFailure;

    unsafe impl WrappedTypesupport for CopyFailure {
        type CStruct = u8;

        fn get_ts() -> &'static rosidl_message_type_support_t {
            unreachable!("the copy failure test does not access type support")
        }

        fn create_msg() -> *mut Self::CStruct {
            Box::into_raw(Box::new(0))
        }

        fn destroy_msg(msg: *mut Self::CStruct) {
            drop(unsafe { Box::from_raw(msg) });
        }

        fn from_native(_msg: &Self::CStruct) -> Self {
            Self
        }

        fn copy_to_native(&self, _msg: &mut Self::CStruct) {}

        fn try_copy_to_native(&self, _msg: &mut Self::CStruct) -> Result<()> {
            Err(Error::RCL_RET_BAD_ALLOC)
        }
    }

    #[test]
    fn native_message_copy_propagates_allocation_failure() {
        assert!(matches!(
            WrappedNativeMsg::<CopyFailure>::try_from(&CopyFailure),
            Err(Error::RCL_RET_BAD_ALLOC)
        ));
    }

    #[test]
    fn handwritten_types_keep_the_non_mutating_native_reset_fallback() {
        let mut native = 17_u8;
        assert!(matches!(CopyFailure::try_reinitialize_native(&mut native), Ok(false)));
        assert_eq!(native, 17);
    }

    #[test]
    fn native_reinitialize_prepares_replacement_before_retiring_current_value() {
        RESET_FINALIZATIONS.store(0, Ordering::Relaxed);
        let mut message = ResetFixture { value: 17 };
        let address = std::ptr::from_mut(&mut message);
        unsafe {
            try_reinitialize_native_message(
                &mut message,
                initialize_reset_fixture,
                finalize_reset_fixture,
            )
            .unwrap()
        };
        assert_eq!(std::ptr::from_mut(&mut message), address);
        assert_eq!(message.value, 73);
        assert_eq!(RESET_FINALIZATIONS.load(Ordering::Relaxed), 1);

        let mut unchanged = ResetFixture { value: 41 };
        assert!(matches!(
            unsafe {
                try_reinitialize_native_message(
                    &mut unchanged,
                    fail_reset_fixture,
                    finalize_reset_fixture,
                )
            },
            Err(Error::RCL_RET_BAD_ALLOC)
        ));
        assert_eq!(unchanged.value, 41);
        assert_eq!(RESET_FINALIZATIONS.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn action_uuid_json_rejects_wrong_fixed_array_lengths() {
        for count in [0, 15, 17] {
            let value = serde_json::json!({"uuid": vec![0; count]});
            assert!(serde_json::from_value::<unique_identifier_msgs::msg::UUID>(value).is_err());
        }
        let valid = unique_identifier_msgs::msg::UUID { uuid: [7; 16] };
        assert_eq!(uuid_msg_to_uuid(&valid).unwrap().as_bytes(), &[7; 16]);
    }

    #[test]
    fn test_ros_str() {
        let hej = "hej hopp";
        let mut msg = WrappedNativeMsg::<std_msgs::msg::String>::new();
        msg.data.assign(hej);
        assert_eq!(msg.data.to_str(), hej);
    }

    #[test]
    fn ros_string_strict_conversion_rejects_invalid_utf8() {
        let mut native = WrappedNativeMsg::<std_msgs::msg::String>::new();
        let invalid = [0xff_u8];
        assert!(unsafe {
            rosidl_runtime_c__String__assignn(
                &mut native.data,
                invalid.as_ptr().cast::<std::os::raw::c_char>(),
                invalid.len(),
            )
        });

        assert!(native.data.try_to_str().is_err());
        assert_eq!(native.data.to_str(), "");
    }

    #[test]
    fn generated_native_update_reuses_primitive_sequence_capacity() {
        let large = std_msgs::msg::UInt64MultiArray {
            layout: Default::default(),
            data: (0..4096).collect(),
        };
        let native = WrappedNativeMsg::<std_msgs::msg::UInt64MultiArray>::try_from(&large).unwrap();
        let mut target = std_msgs::msg::UInt64MultiArray::default();
        target.update_from_native(&native);
        let address = target.data.as_ptr();
        let capacity = target.data.capacity();
        assert_eq!(target, large);

        let small = std_msgs::msg::UInt64MultiArray {
            layout: Default::default(),
            data: vec![7; 128],
        };
        let native = WrappedNativeMsg::<std_msgs::msg::UInt64MultiArray>::try_from(&small).unwrap();
        target.update_from_native(&native);
        assert_eq!(target, small);
        assert_eq!(target.data.as_ptr(), address);
        assert_eq!(target.data.capacity(), capacity);
    }

    #[test]
    fn generated_native_reinitialize_preserves_outer_address_and_fresh_state() {
        let source = std_msgs::msg::UInt64MultiArray {
            layout: Default::default(),
            data: (0..4096).collect(),
        };
        let mut native =
            WrappedNativeMsg::<std_msgs::msg::UInt64MultiArray>::try_from(&source).unwrap();
        let outer = native.msg;
        assert_eq!(native.data.size, 4096);
        assert!(std_msgs::msg::UInt64MultiArray::try_reinitialize_native(&mut native).unwrap());
        assert_eq!(native.msg, outer);
        assert_eq!(native.data.size, 0);
        assert_eq!(native.data.capacity, 0);
        assert_eq!(native.layout.dim.size, 0);

        let replacement = std_msgs::msg::UInt64MultiArray {
            layout: Default::default(),
            data: vec![7; 32],
        };
        replacement.try_copy_to_native(&mut native).unwrap();
        assert_eq!(native.data.to_vec(), replacement.data);
    }

    #[test]
    fn generated_native_update_reuses_nested_sequence_and_string_capacity() {
        let dimension = |label: String, size| std_msgs::msg::MultiArrayDimension {
            label: crate::RosString::from(label),
            size,
            stride: size,
        };
        let large = std_msgs::msg::UInt64MultiArray {
            layout: std_msgs::msg::MultiArrayLayout {
                dim: vec![
                    dimension("x".repeat(1024), 4096),
                    dimension("tail".into(), 1),
                ],
                data_offset: 0,
            },
            data: vec![1; 4096],
        };
        let native = WrappedNativeMsg::<std_msgs::msg::UInt64MultiArray>::try_from(&large).unwrap();
        let mut target = std_msgs::msg::UInt64MultiArray::default();
        target.update_from_native(&native);
        let dimensions = target.layout.dim.as_ptr();
        let label = target.layout.dim[0].label.as_bytes().as_ptr();
        assert_eq!(target, large);

        let small = std_msgs::msg::UInt64MultiArray {
            layout: std_msgs::msg::MultiArrayLayout {
                dim: vec![dimension("short".into(), 8)],
                data_offset: 3,
            },
            data: vec![2; 8],
        };
        let native = WrappedNativeMsg::<std_msgs::msg::UInt64MultiArray>::try_from(&small).unwrap();
        target.update_from_native(&native);
        assert_eq!(target, small);
        assert_eq!(target.layout.dim.as_ptr(), dimensions);
        assert_eq!(target.layout.dim[0].label.as_bytes().as_ptr(), label);
    }

    #[cfg(r2r__test_msgs__msg__Arrays)]
    #[test]
    fn generated_native_update_reuses_string_and_nested_sequence_storage() {
        let mut large = test_msgs::msg::UnboundedSequences::default();
        large.string_values = vec![crate::RosString::from("x".repeat(1024)), "tail".into()];
        let mut first = test_msgs::msg::BasicTypes::default();
        first.int32_value = 41;
        let mut second = test_msgs::msg::BasicTypes::default();
        second.int32_value = 42;
        large.basic_types_values = vec![first, second];
        let native =
            WrappedNativeMsg::<test_msgs::msg::UnboundedSequences>::try_from(&large).unwrap();
        let mut target = test_msgs::msg::UnboundedSequences::default();
        target.update_from_native(&native);
        let strings_address = target.string_values.as_ptr();
        let first_string_address = target.string_values[0].as_bytes().as_ptr();
        let nested_address = target.basic_types_values.as_ptr();
        assert_eq!(target, large);

        let mut small = test_msgs::msg::UnboundedSequences::default();
        small.string_values = vec![crate::RosString::from("short")];
        let mut nested = test_msgs::msg::BasicTypes::default();
        nested.int32_value = -7;
        small.basic_types_values = vec![nested];
        let native =
            WrappedNativeMsg::<test_msgs::msg::UnboundedSequences>::try_from(&small).unwrap();
        target.update_from_native(&native);
        assert_eq!(target, small);
        assert_eq!(target.string_values.as_ptr(), strings_address);
        assert_eq!(target.string_values[0].as_bytes().as_ptr(), first_string_address);
        assert_eq!(target.basic_types_values.as_ptr(), nested_address);
    }

    #[cfg(r2r__test_msgs__msg__WStrings)]
    #[test]
    fn generated_native_update_reuses_wstring_scalar_array_and_sequence_storage() {
        let mut large = test_msgs::msg::WStrings::default();
        large.wstring_value = crate::RosWString::from(vec![65; 1024]);
        large.array_of_wstrings[0] = crate::RosWString::from(vec![66; 512]);
        large.unbounded_sequence_of_wstrings = vec![crate::RosWString::from(vec![67; 256])];
        let native = WrappedNativeMsg::<test_msgs::msg::WStrings>::try_from(&large).unwrap();
        let mut target = test_msgs::msg::WStrings::default();
        target.update_from_native(&native);
        let scalar = target.wstring_value.as_units().as_ptr();
        let array = target.array_of_wstrings[0].as_units().as_ptr();
        let sequence = target.unbounded_sequence_of_wstrings.as_ptr();
        let inner = target.unbounded_sequence_of_wstrings[0].as_units().as_ptr();
        assert_eq!(target, large);

        let mut small = test_msgs::msg::WStrings::default();
        small.wstring_value = crate::RosWString::from([1, 2]);
        small.array_of_wstrings[0] = crate::RosWString::from([3, 4]);
        small.unbounded_sequence_of_wstrings = vec![crate::RosWString::from([5, 6])];
        let native = WrappedNativeMsg::<test_msgs::msg::WStrings>::try_from(&small).unwrap();
        target.update_from_native(&native);
        assert_eq!(target, small);
        assert_eq!(target.wstring_value.as_units().as_ptr(), scalar);
        assert_eq!(target.array_of_wstrings[0].as_units().as_ptr(), array);
        assert_eq!(target.unbounded_sequence_of_wstrings.as_ptr(), sequence);
        assert_eq!(target.unbounded_sequence_of_wstrings[0].as_units().as_ptr(), inner);
    }

    #[test]
    fn test_copy_fields() {
        let msg_orig = std_msgs::msg::String { data: "hej".into() };
        let rosmsg = WrappedNativeMsg::<std_msgs::msg::String>::from(&msg_orig);
        let msg2 = std_msgs::msg::String::from_native(&rosmsg);
        assert_eq!(msg_orig, msg2);
    }

    #[test]
    fn test_introspection_string() {
        unsafe {
            use std::ffi::CStr;

            let x = rosidl_typesupport_introspection_c__get_message_type_support_handle__std_msgs__msg__String();
            let members = (*x).data as *const rosidl_typesupport_introspection_c__MessageMembers;
            println!("{:#?}", *members);

            assert_eq!((*members).member_count_, 1);

            let s = CStr::from_ptr((*members).message_namespace_)
                .to_str()
                .unwrap();
            assert_eq!(s, "std_msgs__msg");

            let s = CStr::from_ptr((*members).message_name_).to_str().unwrap();
            assert_eq!(s, "String");

            let member = (*members).members_;
            println!("member: {:#?}", *member);
            let field_name = CStr::from_ptr((*member).name_).to_str().unwrap();
            let type_id = (*member).type_id_;
            let is_array = (*member).is_array_;
            assert_eq!(field_name, "data");
            assert_eq!(type_id, 16u8); // rosidl_typesupport_introspection_c__ROS_TYPE_STRING as u8
            assert!(!is_array);
        }
    }

    #[test]
    fn fixed_array_native_cdr_and_json_roundtrip() {
        use geometry_msgs::msg::AccelWithCovariance;
        let message = AccelWithCovariance {
            covariance: std::array::from_fn(|index| index as f64 * 0.25),
            ..Default::default()
        };
        let _: &[f64; 36] = &message.covariance;
        let native = WrappedNativeMsg::<AccelWithCovariance>::try_from(&message).unwrap();
        assert_eq!(AccelWithCovariance::from_native(&native), message);
        let bytes = message.to_serialized_bytes().unwrap();
        assert_eq!(AccelWithCovariance::from_serialized_bytes(&bytes).unwrap(), message);
        let value = serde_json::to_value(&message).unwrap();
        assert_eq!(serde_json::from_value::<AccelWithCovariance>(value).unwrap(), message);
        for length in [0, 35, 37] {
            let value = serde_json::json!({"covariance":vec![0.0;length]});
            assert!(serde_json::from_value::<AccelWithCovariance>(value).is_err());
        }
    }

    #[test]
    #[should_panic] // we are testing that we cannot have to many elements in a fixed sized field
    fn test_capped_sequence() {
        // float64[<=3] dimensions in the .msg translates to a float64 sequence AND an array size field. handle it...
        unsafe {
            let x = rosidl_typesupport_introspection_c__get_message_type_support_handle__shape_msgs__msg__SolidPrimitive();
            let members = (*x).data as *const rosidl_typesupport_introspection_c__MessageMembers;
            println!("{:#?}", *members);

            let memberslice =
                std::slice::from_raw_parts((*members).members_, (*members).member_count_ as usize);
            for member in memberslice {
                println!("member: {:#?}", *member);
            }

            let msg_native = WrappedNativeMsg::<shape_msgs::msg::SolidPrimitive>::new();
            let mut msg = shape_msgs::msg::SolidPrimitive::from_native(&msg_native);
            println!("{:#?}", msg);
            msg.dimensions.push(1.0);
            msg.dimensions.push(1.0);
            msg.dimensions.push(1.0);
            msg.dimensions.push(1.0); // only three elements allowed
            let _msg_native2 = WrappedNativeMsg::<shape_msgs::msg::SolidPrimitive>::from(&msg);
        }
    }

    #[test]
    fn test_generation_string_use() {
        let msg = std_msgs::msg::String { data: "hej".into() };
        let msg2 = msg.clone();
        let msg_native = WrappedNativeMsg::<std_msgs::msg::String>::from(&msg2);
        let msg2 = std_msgs::msg::String::from_native(&msg_native);
        assert_eq!(msg, msg2)
    }

    #[test]
    fn generated_strings_preserve_embedded_nul_bytes() {
        let msg = std_msgs::msg::String {
            data: "before\0after".into(),
        };

        let native = WrappedNativeMsg::<std_msgs::msg::String>::from(&msg);
        let roundtrip = std_msgs::msg::String::from_native(&native);

        assert_eq!(roundtrip, msg);
    }

    #[test]
    fn test_generation_bool_use() {
        let msg = std_msgs::msg::Bool { data: true };
        let msg_native = WrappedNativeMsg::<std_msgs::msg::Bool>::from(&msg);
        let msg2 = std_msgs::msg::Bool::from_native(&msg_native);
        assert_eq!(msg, msg2);
    }

    #[test]
    fn test_float_sequence() {
        use trajectory_msgs::msg::*;
        let native = WrappedNativeMsg::<JointTrajectoryPoint>::new();
        let mut msg = JointTrajectoryPoint::from_native(&native);
        msg.positions.push(39.0);
        msg.positions.push(34.0);
        let new_native = WrappedNativeMsg::<JointTrajectoryPoint>::from(&msg);
        let new_msg = JointTrajectoryPoint::from_native(&new_native);
        println!("{:#?}", new_msg);
        assert_eq!(msg, new_msg);
    }

    #[test]
    fn test_deault() {
        use trajectory_msgs::msg::*;
        let mut msg: JointTrajectoryPoint = Default::default();
        msg.positions.push(39.0);
        msg.positions.push(34.0);
        let mut new_native = WrappedNativeMsg::<JointTrajectoryPoint>::from(&msg);
        unsafe { *new_native.positions.data = 88.9 };
        let new_msg = JointTrajectoryPoint::from_native(&new_native);
        println!("{:#?}", new_msg);
        assert_ne!(msg, new_msg);
    }

    #[test]
    fn test_untyped_json() {
        let mut msg = trajectory_msgs::msg::JointTrajectoryPoint::default();
        msg.positions.push(39.0);
        msg.positions.push(34.0);
        let json = serde_json::to_value(msg.clone()).unwrap();

        let mut native =
            WrappedNativeMsgUntyped::new_from("trajectory_msgs/msg/JointTrajectoryPoint").unwrap();
        native.from_json(json.clone()).unwrap();
        let json2 = native.to_json().unwrap();
        assert_eq!(json, json2);

        let msg2: trajectory_msgs::msg::JointTrajectoryPoint =
            serde_json::from_value(json2).unwrap();
        assert_eq!(msg, msg2);
    }

    #[test]
    fn test_from_loaned() {
        type MsgType = trajectory_msgs::msg::JointTrajectoryPoint;
        type CMsgType = <MsgType as WrappedTypesupport>::CStruct;

        let borrowed_msg = MsgType::create_msg();

        let native = unsafe {
            WrappedNativeMsg::<MsgType>::from_loaned(
                borrowed_msg as *mut CMsgType,
                Box::new(MsgType::destroy_msg),
            )
        };

        assert!(native.void_ptr() == borrowed_msg as *mut core::ffi::c_void);
    }

    #[test]
    fn test_serialization_fixed_size() {
        let message = std_msgs::msg::Int32 { data: 10 };

        let bytes = message.to_serialized_bytes().unwrap();

        let message_2 = std_msgs::msg::Int32::from_serialized_bytes(&bytes).unwrap();

        assert_eq!(message.data, message_2.data);

        let bytes_2 = message_2.to_serialized_bytes().unwrap();
        let bytes_3 = message_2.to_serialized_bytes().unwrap();

        assert_eq!(bytes, bytes_2);
        assert_eq!(bytes, bytes_3);
    }

    #[test]
    fn rejects_inconsistent_serialized_message_buffers() {
        let mut buffer = unsafe { r2r_rcl::rcutils_get_zero_initialized_uint8_array() };
        buffer.buffer_length = 1;
        assert!(matches!(
            unsafe { serialized_message_bytes(&buffer) },
            Err(Error::RCL_RET_ERROR)
        ));

        let mut byte = 42;
        buffer.buffer = &mut byte;
        buffer.buffer_capacity = 0;
        assert!(matches!(
            unsafe { serialized_message_bytes(&buffer) },
            Err(Error::RCL_RET_ERROR)
        ));

        buffer.buffer_length = 0;
        assert!(unsafe { serialized_message_bytes(&buffer) }
            .unwrap()
            .is_empty());
    }

    #[test]
    fn test_serialization_dynamic_size() {
        let message = std_msgs::msg::Int32MultiArray {
            layout: std_msgs::msg::MultiArrayLayout::default(),
            data: vec![10, 20, 30],
        };

        let bytes = message.to_serialized_bytes().unwrap();

        let message_2 = std_msgs::msg::Int32MultiArray::from_serialized_bytes(&bytes).unwrap();

        assert_eq!(message.data, message_2.data);

        let bytes_2 = message_2.to_serialized_bytes().unwrap();
        let bytes_3 = message_2.to_serialized_bytes().unwrap();

        assert_eq!(bytes, bytes_2);
        assert_eq!(bytes, bytes_3);
    }

    #[cfg(r2r__test_msgs__msg__Defaults)]
    #[test]
    fn test_untyped_json_default() {
        // from the msg definition file:
        // bool bool_value true
        // byte byte_value 50
        // char char_value 100
        // float32 float32_value 1.125
        // ...

        // let's try to change only a few fields.
        let json = r#"
        {
            "byte_value": 255,
            "float32_value": 3.25
        }"#;

        let mut native = WrappedNativeMsgUntyped::new_from("test_msgs/msg/Defaults").unwrap();
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        native.from_json(v).expect("could make default msg");
        let json2 = native.to_json().unwrap();
        let msg2: test_msgs::msg::Defaults = serde_json::from_value(json2).unwrap();

        assert!(msg2.bool_value); // the default
        assert_eq!(msg2.byte_value, 255); // from our json
        assert_eq!(msg2.char_value, 100); // the default
        assert_eq!(msg2.float32_value, 3.25); // from our json
    }

    #[cfg(r2r__test_msgs__msg__Arrays)]
    #[test]
    fn test_test_msgs_array() {
        let mut msg = test_msgs::msg::Arrays::default();
        println!("msg: {:?}", msg.string_values);
        msg.string_values = ["hej".into(), "hopp".into(), "stropp".into()];

        let msg_native = WrappedNativeMsg::<test_msgs::msg::Arrays>::from(&msg);
        let msg2 = test_msgs::msg::Arrays::from_native(&msg_native);

        assert_eq!(msg, msg2);
    }

    #[cfg(r2r__test_msgs__msg__Arrays)]
    #[test]
    fn test_test_msgs_array_too_few_elems() {
        let value = serde_json::json!({"string_values":["hej","hopp"]});
        assert!(serde_json::from_value::<test_msgs::msg::Arrays>(value).is_err());
    }

    #[cfg(r2r__test_msgs__msg__WStrings)]
    #[test]
    fn test_test_msgs_wstring() {
        let mut msg = test_msgs::msg::WStrings::default();
        let rust_str = "ハローワールド";
        msg.wstring_value = rust_str.into();
        let native = WrappedNativeMsg::<test_msgs::msg::WStrings>::from(&msg);
        println!("msg: {:?}", msg);
        let msg2 = test_msgs::msg::WStrings::from_native(&native);
        assert_eq!(msg.wstring_value, msg2.wstring_value);
    }

    #[cfg(r2r__example_interfaces__srv__AddTwoInts)]
    #[test]
    fn test_service_msgs() {
        use example_interfaces::srv::AddTwoInts;
        let req = AddTwoInts::Request {
            a: 5,
            ..Default::default()
        };
        let rn = WrappedNativeMsg::<_>::from(&req);
        let req2 = AddTwoInts::Request::from_native(&rn);
        println!("req2 {:?}", req2);
        assert_eq!(req, req2);

        let resp = AddTwoInts::Response { sum: 5 };
        let rn = WrappedNativeMsg::<_>::from(&resp);
        let resp2 = AddTwoInts::Response::from_native(&rn);
        println!("resp {:?}", resp2);
        assert_eq!(resp, resp2);
    }

    #[cfg(r2r__std_srvs__srv__Empty)]
    #[test]
    fn test_empty_msgs() {
        use std_srvs::srv::Empty;
        let req = Empty::Request::default();
        let resp = Empty::Response::default();
        println!("req {:?}", req);
        println!("resp {:?}", resp);
    }

    #[cfg(r2r__example_interfaces__action__Fibonacci)]
    #[test]
    fn test_action_msgs() {
        use example_interfaces::action::Fibonacci;
        let goal = Fibonacci::Goal { order: 5 };
        let gn = WrappedNativeMsg::<_>::from(&goal);
        let goal2 = Fibonacci::Goal::from_native(&gn);
        println!("goal2 {:?}", goal2);
        assert_eq!(goal, goal2);

        let res = Fibonacci::Result {
            sequence: vec![1, 2, 3],
        };
        let rn = WrappedNativeMsg::<_>::from(&res);
        let res2 = Fibonacci::Result::from_native(&rn);
        println!("res2 {:?}", res2);
        assert_eq!(res, res2);

        let fb = Fibonacci::Feedback {
            sequence: vec![4, 3, 6],
        };
        let fbn = WrappedNativeMsg::<_>::from(&fb);
        let fb2 = Fibonacci::Feedback::from_native(&fbn);
        println!("feedback2 {:?}", fb2);
        assert_eq!(fb, fb2);

        let fb = WrappedNativeMsg::<Fibonacci::Feedback>::new();
        let fb1 = Fibonacci::Feedback::default();
        let fb2 = Fibonacci::Feedback::from_native(&fb);
        assert_eq!(fb1, fb2);
    }

    #[cfg(r2r__example_interfaces__srv__AddTwoInts)]
    #[test]
    fn test_untyped_service_support() {
        let ts = UntypedServiceSupport::new_from("example_interfaces/srv/AddTwoInts").unwrap();
        let msg = (ts.make_request_msg)().unwrap();
        let json = msg.to_json();
        // the message should contain something (default msg)
        assert!(!json.unwrap().to_string().is_empty());
    }

    #[cfg(r2r__example_interfaces__action__Fibonacci)]
    #[test]
    fn test_untyped_action_support() {
        use example_interfaces::action::Fibonacci;

        let ts = UntypedActionSupport::new_from("example_interfaces/action/Fibonacci").unwrap();
        let uuid = unique_identifier_msgs::msg::UUID { uuid: [7; 16] };
        let goal = Fibonacci::Goal { order: 5 };
        let json_goal = serde_json::to_value(&goal).unwrap();
        let json_request = (ts.make_goal_request_msg)(uuid.clone(), json_goal)
            .unwrap()
            .to_json()
            .unwrap();
        let typed_request = Fibonacci::Action::make_goal_request_msg(uuid.clone(), goal);
        assert_eq!(json_request, serde_json::to_value(typed_request).unwrap());

        let json_result_request = (ts.make_result_request_msg)(uuid.clone())
            .unwrap()
            .to_json()
            .unwrap();
        let typed_result_request = Fibonacci::Action::make_result_request_msg(uuid);
        assert_eq!(json_result_request, serde_json::to_value(typed_result_request).unwrap());
    }

    #[cfg(r2r__example_interfaces__action__Fibonacci)]
    #[test]
    #[ignore = "manual microbenchmark; run with --release --ignored --nocapture"]
    fn benchmark_untyped_action_goal_request_conversion() {
        let iterations = std::env::var("R2R_BENCH_ITERATIONS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(200_000);
        let ts = UntypedActionSupport::new_from("example_interfaces/action/Fibonacci").unwrap();
        let uuid = unique_identifier_msgs::msg::UUID { uuid: [7; 16] };
        let goal = serde_json::json!({ "order": 21 });
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            let native = (ts.make_goal_request_msg)(
                std::hint::black_box(uuid.clone()),
                std::hint::black_box(goal.clone()),
            )
            .unwrap();
            std::hint::black_box(native);
        }
        let elapsed = start.elapsed();
        eprintln!(
            "untyped_action_goal_request: iterations={iterations} elapsed_ns={} ns_per_op={:.2}",
            elapsed.as_nanos(),
            elapsed.as_nanos() as f64 / iterations as f64
        );
    }

    #[cfg(r2r__example_interfaces__action__Fibonacci)]
    #[test]
    fn untyped_action_rejects_invalid_goal_json() {
        let ts = UntypedActionSupport::new_from("example_interfaces/action/Fibonacci").unwrap();
        let uuid = unique_identifier_msgs::msg::UUID::default();
        let invalid_goal = serde_json::json!({ "order": "not an integer" });

        let err = (ts.make_goal_request_msg)(uuid, invalid_goal).unwrap_err();

        assert!(matches!(err, Error::SerdeError { .. }));
    }

    #[cfg(r2r__action_msgs__msg__GoalStatus)]
    #[test]
    fn test_msg_constants() {
        use action_msgs::msg::GoalStatus;
        let gs = GoalStatus::default();

        assert_eq!(gs.status, GoalStatus::STATUS_UNKNOWN as i8);
        assert_eq!(0, GoalStatus::STATUS_UNKNOWN as i8);
        assert_eq!(1, GoalStatus::STATUS_ACCEPTED as i8);
        assert_eq!(2, GoalStatus::STATUS_EXECUTING as i8);
        assert_eq!(3, GoalStatus::STATUS_CANCELING as i8);
        assert_eq!(4, GoalStatus::STATUS_SUCCEEDED as i8);
        assert_eq!(5, GoalStatus::STATUS_CANCELED as i8);
        assert_eq!(6, GoalStatus::STATUS_ABORTED as i8);

        use action_msgs::srv::CancelGoal;
        let cgr = CancelGoal::Response::default();

        assert_eq!(cgr.return_code, CancelGoal::Response::ERROR_NONE as i8);
        assert_eq!(0, CancelGoal::Response::ERROR_NONE as i8);
        assert_eq!(1, CancelGoal::Response::ERROR_REJECTED as i8);
        assert_eq!(2, CancelGoal::Response::ERROR_UNKNOWN_GOAL_ID as i8);
        assert_eq!(3, CancelGoal::Response::ERROR_GOAL_TERMINATED as i8);
    }
}

#[cfg(all(
    test,
    not(feature = "doc-only"),
    any(
        r2r__ros__distro__iron,
        r2r__ros__distro__jazzy,
        r2r__ros__distro__kilted,
        r2r__ros__distro__lyrical,
        r2r__ros__distro__rolling
    )
))]
mod service_events;

#[cfg(all(test, not(feature = "doc-only")))]
mod native_sequences;

#[cfg(all(test, not(feature = "doc-only")))]
#[path = "msg_types/native_library_tests.rs"]
mod native_library_tests;
