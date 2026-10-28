//! Native r2r implementations of the five built-in lifecycle services.
//!
//! Requests are taken, answered and finalized inline during the node's
//! ordinary wait handling: no mpsc buffers or spawned tasks are involved, so
//! readiness stays entirely inside the unified executor and each service call
//! costs one take plus one send.

use std::mem::MaybeUninit;
use std::sync::Arc;

use parking_lot::Mutex;

use r2r_rcl::*;

use crate::error::{Error, Result};
use crate::lifecycle::node::{LifecycleRuntime, LifecycleServiceKind};
use crate::lifecycle_msgs;
use crate::msg_types::{VoidPtr, WrappedNativeMsg};
use crate::services::{create_service_helper, ServiceRequestAction, Service_};
use crate::QosProfile;

use lifecycle_msgs::srv::{ChangeState, GetAvailableStates, GetAvailableTransitions, GetState};

/// One of the five lifecycle services, backed directly by an rcl service
/// handle and answered synchronously from the shared lifecycle runtime.
pub(crate) struct LifecycleService {
    pub rcl_handle: rcl_service_t,
    pub kind: LifecycleServiceKind,
    pub runtime: Arc<LifecycleRuntime>,
}

// SAFETY: native service operations are serialized through the enclosing node
// lock like every other Service_ implementation; all shared state lives behind
// Arc/Mutex.
unsafe impl Send for LifecycleService {}

impl LifecycleService {
    pub fn create(
        node: &mut rcl_node_t, service_name: &str, kind: LifecycleServiceKind,
        runtime: Arc<LifecycleRuntime>,
    ) -> Result<Arc<Mutex<dyn Service_>>> {
        let rcl_handle =
            create_service_helper(node, service_name, kind.type_support(), QosProfile::default())?;
        Ok(Arc::new(Mutex::new(LifecycleService {
            rcl_handle,
            kind,
            runtime,
        })))
    }

    /// Take one typed request, or return `None` when nothing is queued.
    fn take_request<T: crate::WrappedTypesupport>(
        &mut self,
    ) -> Result<Option<(rmw_request_id_t, T)>> {
        let mut request_info = MaybeUninit::<rmw_service_info_t>::uninit();
        let mut request_msg = WrappedNativeMsg::<T>::try_new()?;
        let ret = unsafe {
            rcl_take_request_with_info(
                &self.rcl_handle,
                request_info.as_mut_ptr(),
                request_msg.void_ptr_mut(),
            )
        };
        if ret == RCL_RET_SERVICE_TAKE_FAILED as i32 {
            return Ok(None);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        let request_info = unsafe { request_info.assume_init() };
        Ok(Some((request_info.request_id, T::from_native(&request_msg))))
    }

    fn send_response_typed<T: crate::WrappedTypesupport>(
        &mut self, mut request_id: rmw_request_id_t, response: T,
    ) -> Result<()> {
        let mut response_msg = WrappedNativeMsg::<T>::new();
        response.copy_to_native(&mut response_msg);
        let ret = unsafe {
            rcl_send_response(&self.rcl_handle, &mut request_id, response_msg.void_ptr_mut())
        };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        Ok(())
    }

    fn answer_change_state(&mut self) -> Result<bool> {
        let Some((request_id, request)) = self.take_request::<ChangeState::Request>()? else {
            return Ok(false);
        };
        let label = std::str::from_utf8(request.transition.label.as_bytes()).unwrap_or("");
        let label = (!label.is_empty()).then_some(label);
        let outcome = self.runtime.change_state(request.transition.id, label)?;
        let success = outcome == crate::lifecycle::CallbackReturn::Success;
        self.send_response_typed(request_id, ChangeState::Response { success })?;
        Ok(true)
    }

    fn answer_get_state(&mut self) -> Result<bool> {
        let Some((request_id, _request)) = self.take_request::<GetState::Request>()? else {
            return Ok(false);
        };
        let state = self.runtime.current_state_msg();
        self.send_response_typed(
            request_id,
            GetState::Response {
                current_state: state,
            },
        )?;
        Ok(true)
    }

    fn answer_get_available_states(&mut self) -> Result<bool> {
        let Some((request_id, _request)) = self.take_request::<GetAvailableStates::Request>()?
        else {
            return Ok(false);
        };
        self.send_response_typed(
            request_id,
            GetAvailableStates::Response {
                available_states: self.runtime.all_states_msgs(),
            },
        )?;
        Ok(true)
    }

    fn answer_transitions(&mut self, graph: bool) -> Result<bool> {
        let Some((request_id, _request)) =
            self.take_request::<GetAvailableTransitions::Request>()?
        else {
            return Ok(false);
        };
        let descriptions = if graph {
            self.runtime.transition_graph_descriptions()
        } else {
            self.runtime.available_transition_descriptions()
        };
        self.send_response_typed(
            request_id,
            GetAvailableTransitions::Response {
                available_transitions: descriptions,
            },
        )?;
        Ok(true)
    }
}

impl Service_ for LifecycleService {
    fn handle(&self) -> &rcl_service_t {
        &self.rcl_handle
    }

    fn send_response(
        &mut self, _request_id: rmw_request_id_t, _msg: Box<dyn VoidPtr>,
    ) -> Result<()> {
        // Responses are produced inline in handle_request; external code can
        // never answer an internal lifecycle service.
        Err(Error::RCL_RET_SERVICE_INVALID)
    }

    fn handle_request(
        &mut self, _service: Arc<Mutex<dyn Service_>>,
    ) -> Result<ServiceRequestAction> {
        let answered = match self.kind {
            LifecycleServiceKind::ChangeState => self.answer_change_state()?,
            LifecycleServiceKind::GetState => self.answer_get_state()?,
            LifecycleServiceKind::GetAvailableStates => self.answer_get_available_states()?,
            LifecycleServiceKind::GetAvailableTransitions => self.answer_transitions(false)?,
            LifecycleServiceKind::GetTransitionGraph => self.answer_transitions(true)?,
        };
        if !answered {
            return Ok(ServiceRequestAction::Idle);
        }
        // Nothing is delivered to a user stream. Report Idle to end the
        // per-spin take loop; further queued requests are handled on later
        // spins after the wait set re-triggers.
        Ok(ServiceRequestAction::Idle)
    }

    fn destroy(&mut self, node: &mut rcl_node_t) {
        unsafe {
            rcl_service_fini(&mut self.rcl_handle, node);
        }
    }
}
