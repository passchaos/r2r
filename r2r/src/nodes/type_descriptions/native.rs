use super::*;
use crate::{msg_types::WrappedNativeMsg, type_description_interfaces::srv::GetTypeDescription};

pub(super) struct TypeDescriptionService {
    handle: rcl_service_t,
    node: *mut rcl_node_t,
    id: uuid::Uuid,
    response: WrappedNativeMsg<GetTypeDescription::Response>,
    #[cfg(not(feature = "doc-only"))]
    finish_response:
        unsafe extern "C" fn(*mut <GetTypeDescription::Response as WrappedTypesupport>::CStruct),
    sources: crate::type_descriptions::sources::SourceFiles,
}

// SAFETY: the owning Node keeps its boxed native node address valid, never shares an active
// spin, and finalizes every service before the node. All service operations use its mutex.
unsafe impl Send for TypeDescriptionService {}

impl TypeDescriptionService {
    pub(super) fn create(
        node: &mut rcl_node_t, id: uuid::Uuid, qos: Option<QosProfile>,
    ) -> Result<Arc<Mutex<dyn Service_>>> {
        #[cfg(not(feature = "doc-only"))]
        let finish_response =
            r2r_msg_gen::r2r_resolve_type_description_interfaces__srv__GetTypeDescription_Response__fini()
                .map_err(|error| Error::NativeTypeSupport {
                    interface: "type_description_interfaces/srv/GetTypeDescription_Response".into(),
                    err: error.to_string(),
                })?;
        let response = WrappedNativeMsg::try_new()?;
        let mut handle = unsafe { rcl_get_zero_initialized_service() };
        if let Some(qos) = qos {
            handle = create_service_helper(
                node,
                "~/get_type_description",
                GetTypeDescription::Service::try_get_ts()?,
                qos,
            )?;
        } else {
            check_rcl_ret(unsafe { rcl_node_type_description_service_init(&mut handle, node) })?;
        }
        Ok(Arc::new(Mutex::new(Self {
            handle,
            node,
            id,
            response,
            #[cfg(not(feature = "doc-only"))]
            finish_response,
            sources: Default::default(),
        })))
    }
}

impl Service_ for TypeDescriptionService {
    fn handle(&self) -> &rcl_service_t {
        &self.handle
    }
    fn id(&self) -> Option<uuid::Uuid> {
        Some(self.id)
    }
    fn send_response(&mut self, _id: rmw_request_id_t, _message: Box<dyn VoidPtr>) -> Result<()> {
        Err(Error::RCL_RET_SERVICE_INVALID)
    }
    fn handle_request(
        &mut self, _service: Arc<Mutex<dyn Service_>>,
    ) -> Result<ServiceRequestAction> {
        // Each RMW take receives a newly initialized, independently owned request container.
        let mut request = WrappedNativeMsg::<GetTypeDescription::Request>::try_new()?;
        let mut info = crate::native_metadata::service_info();
        let ret =
            unsafe { rcl_take_request_with_info(&self.handle, &mut info, request.void_ptr_mut()) };
        if ret == RCL_RET_SERVICE_TAKE_FAILED as i32 {
            return Ok(ServiceRequestAction::Idle);
        }
        check_rcl_ret(ret)?;
        unsafe {
            // RCL initializes an uninitialized response. Retire the previous initialized fields
            // first: handing it a freshly created or populated response would leak those fields.
            #[cfg(not(feature = "doc-only"))]
            (self.finish_response)(self.response.msg);
            #[cfg(feature = "doc-only")]
            type_description_interfaces__srv__GetTypeDescription_Response__fini(
                self.response.msg.cast(),
            );
            rcl_node_type_description_service_handle_request(
                self.node,
                &info.request_id,
                request.msg.cast(),
                self.response.msg.cast(),
            );
        }
        if self.response.failure_reason.data.is_null() {
            return Err(Error::RCL_RET_BAD_ALLOC);
        }
        if self.response.successful && self.response.type_sources.size != 0 {
            // This is the owning native response copy, not the immutable generated cache.
            let sources = unsafe {
                std::slice::from_raw_parts_mut(
                    self.response.type_sources.data,
                    self.response.type_sources.size,
                )
            };
            for source in sources {
                self.sources.repair(
                    source.type_name.to_str(),
                    source.encoding.to_str(),
                    &mut source.raw_file_contents,
                )?;
            }
        }
        check_rcl_ret(unsafe {
            rcl_send_response(&self.handle, &mut info.request_id, self.response.void_ptr_mut())
        })?;
        Ok(ServiceRequestAction::Handled)
    }
    fn destroy(&mut self, node: &mut rcl_node_t) {
        let ret = unsafe { rcl_service_fini(&mut self.handle, node) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize type-description service: {ret}");
        }
    }
}
