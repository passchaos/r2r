use super::*;
use std::marker::PhantomData;

pub(crate) struct NativeCallbackService<T, F>
where
    T: WrappedServiceTypeSupport,
{
    endpoint: Endpoint,
    response: WrappedNativeMsg<T::Response>,
    callback: F,
    marker: PhantomData<T>,
}

// SAFETY: the owning node exclusively drives this endpoint. Generated service support is
// immutable, native messages are Send, and the callback is required to be Send.
unsafe impl<T: WrappedServiceTypeSupport, F: Send> Send for NativeCallbackService<T, F> {}

impl<T, F> NativeCallbackService<T, F>
where
    T: WrappedServiceTypeSupport + 'static,
    F: for<'request> FnMut(
            NativeMessageRef<'request, T::Request>,
            &mut WrappedNativeMsg<T::Response>,
            ServiceRequestInfo,
        ) -> Result<()>
        + Send
        + 'static,
{
    pub(crate) fn create(
        node: &mut rcl_node_t, name: &str, qos: QosProfile, options: CallbackServiceOptions,
        callback: F,
    ) -> Result<Box<dyn CallbackService_>> {
        let options = options.validate()?;
        // Prepare owned response storage before creating the native endpoint, so allocation
        // failure leaves no unregistered service handle to finalize.
        let response = WrappedNativeMsg::try_new()?;
        let type_support = T::get_ts();
        let handle = create_service_helper(node, name, type_support, qos)?;
        Ok(Box::new(Self {
            endpoint: Endpoint {
                handle,
                #[cfg(not(r2r__ros__distro__humble))]
                type_support,
                id: uuid::Uuid::new_v4(),
                alive: Arc::new(AtomicBool::new(true)),
                options,
            },
            response,
            callback,
            marker: PhantomData,
        }))
    }
}

impl<T, F> CallbackService_ for NativeCallbackService<T, F>
where
    T: WrappedServiceTypeSupport + 'static,
    F: for<'request> FnMut(
            NativeMessageRef<'request, T::Request>,
            &mut WrappedNativeMsg<T::Response>,
            ServiceRequestInfo,
        ) -> Result<()>
        + Send
        + 'static,
{
    fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
    #[cfg(not(r2r__ros__distro__humble))]
    fn endpoint_mut(&mut self) -> &mut Endpoint {
        &mut self.endpoint
    }
    fn take_and_respond(&mut self) -> Result<bool> {
        if !self.endpoint.alive.load(Ordering::Acquire) {
            return Ok(false);
        }
        // A populated request buffer is never reused across RMW takes. Every take is separately
        // initialized and finalized; the scoped read-only view cannot transfer native ownership.
        let mut request = WrappedNativeMsg::<T::Request>::try_new()?;
        let mut info = crate::native_metadata::service_info();
        let ret = unsafe {
            rcl_take_request_with_info(&self.endpoint.handle, &mut info, request.void_ptr_mut())
        };
        if ret == RCL_RET_SERVICE_TAKE_FAILED as i32 {
            return Ok(false);
        }
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        (self.callback)(
            NativeMessageRef::from_owned(&request),
            &mut self.response,
            ServiceRequestInfo::from(&info),
        )?;
        let ret = unsafe {
            rcl_send_response(
                &self.endpoint.handle,
                &mut info.request_id,
                self.response.void_ptr_mut(),
            )
        };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        Ok(true)
    }
    fn destroy(&mut self, node: &mut rcl_node_t) {
        self.endpoint.destroy(node);
    }
}
