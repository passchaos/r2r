use super::*;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct UnallocatedResponse(AddTwoInts::Response);
#[derive(Debug, Clone)]
struct UnallocatedService;

// SAFETY: both wrappers preserve the generated C representation and type support. The
// response allocator intentionally fails before any RMW take or native field access.
unsafe impl WrappedTypesupport for UnallocatedResponse {
    type CStruct = <AddTwoInts::Response as WrappedTypesupport>::CStruct;
    fn get_ts() -> &'static rosidl_message_type_support_t {
        AddTwoInts::Response::get_ts()
    }
    fn create_msg() -> *mut Self::CStruct {
        std::ptr::null_mut()
    }
    fn destroy_msg(message: *mut Self::CStruct) {
        AddTwoInts::Response::destroy_msg(message);
    }
    fn from_native(message: &Self::CStruct) -> Self {
        Self(AddTwoInts::Response::from_native(message))
    }
    fn copy_to_native(&self, message: &mut Self::CStruct) {
        self.0.copy_to_native(message);
    }
}
unsafe impl WrappedServiceTypeSupport for UnallocatedService {
    type Request = AddTwoInts::Request;
    type Response = UnallocatedResponse;
    #[cfg(not(r2r__ros__distro__humble))]
    type Event = AddTwoInts::Event;
    fn get_ts() -> &'static rosidl_service_type_support_t {
        AddTwoInts::Service::get_ts()
    }
}

#[test]
fn ready_dispatch_client_allocation_error_preserves_healthy_response_identity() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "dispatch_client_allocation", "").unwrap();
    let mut peer = Node::create(context, "dispatch_client_reply", "").unwrap();
    let name = format!("/dispatch_client_alloc_{}", uuid::Uuid::new_v4().simple());
    let mut bad_service = peer
        .create_service::<AddTwoInts::Service>(&name, QosProfile::services_default())
        .unwrap();
    let healthy_name = format!("{name}/healthy");
    let mut healthy_service = peer
        .create_service::<AddTwoInts::Service>(&healthy_name, QosProfile::services_default())
        .unwrap();
    let bad = node
        .create_client::<UnallocatedService>(&name, QosProfile::services_default())
        .unwrap();
    let healthy = node
        .create_client::<AddTwoInts::Service>(&healthy_name, QosProfile::services_default())
        .unwrap();
    node.spin_until_future_complete_with_options(
        async {
            futures::try_join!(bad.wait_for_service()?, healthy.wait_for_service()?)?;
            Ok(())
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let mut bad_response = Box::pin(bad.request(&AddTwoInts::Request { a: 17, b: 11 }).unwrap());
    let healthy_response = healthy
        .request_with_info(&AddTwoInts::Request { a: 31, b: 11 })
        .unwrap();
    let mut sequence = None;
    peer.spin_until_future_complete_with_options(
        async {
            for service in [&mut bad_service, &mut healthy_service] {
                let request = service.next().await.ok_or(Error::RCL_RET_SERVICE_INVALID)?;
                if request.message.a == 31 {
                    sequence = Some(request.request_info().sequence_number);
                }
                let sum = request.message.a + request.message.b;
                request.respond(AddTwoInts::Response { sum })?;
            }
            Ok(())
        },
        Duration::from_secs(3),
        FutureExecutionOptions::wake_driven(),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        assert!(Instant::now() < deadline);
        let preparation = node.prepare_spin(Duration::from_millis(10)).unwrap();
        let (wait_set, mut entities) = node.wait_entities();
        {
            let mut context = entities.context.context_handle.lock();
            wait_set
                .prepare(preparation.capacities, context.as_mut())
                .unwrap();
        }
        let offsets = entities
            .add_ordinary(&mut wait_set.handle, &preparation)
            .unwrap();
        wait_set.refresh_event_layout(entities.context).unwrap();
        let ret = unsafe { rcl_wait(&mut wait_set.handle, 10_000_000) };
        if ret == RCL_RET_TIMEOUT as i32 {
            continue;
        }
        check_rcl_ret(ret).unwrap();
        let ready = (0..2).all(|index| unsafe {
            !(*wait_set.handle.clients.add(offsets.clients + index)).is_null()
        });
        if ready {
            // Distinct reply topics exclude readiness caused by another client's response.
            // Both slots must identify these actual native endpoints.
            for (index, client) in entities.clients.iter().enumerate() {
                assert_eq!(
                    unsafe { *wait_set.handle.clients.add(offsets.clients + index) },
                    client.lock().handle() as *const rcl_client_t,
                );
            }
            assert!(matches!(
                entities.dispatch(&wait_set.handle, offsets),
                Err(Error::RCL_RET_BAD_ALLOC)
            ));
            break;
        }
    }
    assert!(
        bad_response.as_mut().now_or_never().is_none(),
        "allocation failure consumed the unread response"
    );
    let response = healthy_response
        .now_or_never()
        .expect("healthy native client response was skipped after its sibling failed")
        .unwrap();
    assert_eq!(response.response.sum, 42);
    assert_eq!(Some(response.info.sequence_number), sequence);
    assert_eq!(node.clients.len(), 2, "native take error destroyed an endpoint");
}
