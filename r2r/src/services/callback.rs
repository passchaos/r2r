//! Scoped synchronous service handlers and their public endpoint ownership.

use super::*;
use std::sync::atomic::AtomicBool;

mod native;
pub(crate) use native::NativeCallbackService;

/// Native dispatch options for synchronous service callbacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackServiceOptions {
    /// Maximum requests handled for this endpoint in one spin dispatch.
    pub max_requests_per_spin: usize,
}

impl Default for CallbackServiceOptions {
    fn default() -> Self {
        Self {
            max_requests_per_spin: MAX_SERVICE_REQUESTS_PER_SPIN,
        }
    }
}

impl CallbackServiceOptions {
    pub const fn max_requests_per_spin(mut self, maximum: usize) -> Self {
        self.max_requests_per_spin = maximum;
        self
    }
    pub(crate) fn validate(self) -> Result<Self> {
        if self.max_requests_per_spin == 0 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        Ok(self)
    }
}

/// Ownership token for a typed or scoped native service callback.
///
/// Dropping it prevents further takes and wakes endpoint cleanup at the next node/executor spin.
/// Use [`Node::destroy_callback_service`] for immediate native teardown. A running callback may
/// finish its already-taken request before queued destruction is applied.
pub struct CallbackService {
    pub(crate) id: uuid::Uuid,
    alive: Arc<AtomicBool>,
    canceled: Arc<Mutex<crate::notifications::NotificationQueue<uuid::Uuid>>>,
}

impl CallbackService {
    pub(crate) fn new(
        id: uuid::Uuid, alive: Arc<AtomicBool>,
        canceled: Arc<Mutex<crate::notifications::NotificationQueue<uuid::Uuid>>>,
    ) -> Self {
        Self {
            id,
            alive,
            canceled,
        }
    }
    /// Fully resolved native service name, after namespace expansion and remapping.
    pub fn service_name(&self, node: &Node) -> Result<String> {
        node.callback_service_name(self.id)
    }
    pub fn get_request_subscription_actual_qos(&self, node: &Node) -> Result<QosProfile> {
        node.callback_service_request_qos(self.id)
    }
    pub fn get_response_publisher_actual_qos(&self, node: &Node) -> Result<QosProfile> {
        node.callback_service_response_qos(self.id)
    }

    #[cfg(not(r2r__ros__distro__humble))]
    /// Configure native service event publication with the same counters, identity and payloads.
    pub fn configure_introspection(
        &self, node: &mut Node, state: crate::ServiceIntrospectionState, qos: QosProfile,
    ) -> Result<()> {
        node.configure_callback_service_introspection(self.id, state, qos)
    }
}

impl Drop for CallbackService {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
        crate::notifications::enqueue(&self.canceled, self.id);
    }
}

pub(crate) struct Endpoint {
    pub(crate) handle: rcl_service_t,
    #[cfg(not(r2r__ros__distro__humble))]
    pub(crate) type_support: &'static rosidl_service_type_support_t,
    pub(crate) id: uuid::Uuid,
    pub(crate) alive: Arc<AtomicBool>,
    pub(crate) options: CallbackServiceOptions,
}

impl Endpoint {
    pub(crate) fn service_name(&self) -> Result<String> {
        let name = unsafe { rcl_service_get_service_name(&self.handle).as_ref() }
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        let name = unsafe { CStr::from_ptr(name) };
        name.to_str()
            .map(str::to_owned)
            .map_err(|error| Error::InvalidUtf8 {
                context: "callback service name",
                err: error.to_string(),
            })
    }
    pub(crate) fn request_qos(&self) -> Result<QosProfile> {
        let qos = unsafe { rcl_service_request_subscription_get_actual_qos(&self.handle).as_ref() }
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        QosProfile::try_from(qos)
    }
    pub(crate) fn response_qos(&self) -> Result<QosProfile> {
        let qos = unsafe { rcl_service_response_publisher_get_actual_qos(&self.handle).as_ref() }
            .ok_or(Error::RCL_RET_SERVICE_INVALID)?;
        QosProfile::try_from(qos)
    }
    pub(crate) fn destroy(&mut self, node: &mut rcl_node_t) {
        let ret = unsafe { rcl_service_fini(&mut self.handle, node) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize callback service: {ret}");
        }
    }
}

/// Node-owned dispatch only. No service, channel or handler lock is held during user callbacks.
pub(crate) trait CallbackService_: Send {
    fn endpoint(&self) -> &Endpoint;
    #[cfg(not(r2r__ros__distro__humble))]
    fn endpoint_mut(&mut self) -> &mut Endpoint;
    fn take_and_respond(&mut self) -> Result<bool>;
    fn destroy(&mut self, node: &mut rcl_node_t);
}

#[cfg(test)]
mod tests;
