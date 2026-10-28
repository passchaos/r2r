//! Service response metadata checked against direct native RCL reads.

use super::*;
use crate::example_interfaces::srv::AddTwoInts;

/// Native request/response through the same client, independent of the Rust
/// response dispatcher. The public client and its upgraded owner remain alive
/// throughout, and no native handle lock is held while spinning the node.
fn native_response_guid(
    node: &mut Node, service: &mut crate::Service<AddTwoInts::Service>, client: *const rcl_client_t,
) -> [u8; 16] {
    let request =
        WrappedNativeMsg::<AddTwoInts::Request>::try_from(&AddTwoInts::Request { a: 20, b: 22 })
            .unwrap();
    let mut sequence = 0;
    check_rcl_ret(unsafe { rcl_send_request(client, request.void_ptr(), &mut sequence) }).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(Instant::now() < deadline, "native request timed out");
        node.try_spin_once(Duration::from_millis(1)).unwrap();
        if let Some(request) = service.next().now_or_never().flatten() {
            assert_eq!(request.request_info().sequence_number, sequence);
            assert_eq!(request.message.a + request.message.b, 42);
            request.respond(AddTwoInts::Response { sum: 42 }).unwrap();
            break;
        }
    }
    let mut response = WrappedNativeMsg::<AddTwoInts::Response>::try_new().unwrap();
    // All bits in this native structure are valid integer/byte values.
    let mut info: rmw_service_info_t = unsafe { std::mem::zeroed() };
    loop {
        assert!(Instant::now() < deadline, "native response timed out");
        let result =
            unsafe { rcl_take_response_with_info(client, &mut info, response.void_ptr_mut()) };
        if result != RCL_RET_CLIENT_TAKE_FAILED as i32 {
            check_rcl_ret(result).unwrap();
            break;
        }
        std::thread::yield_now();
    }
    assert_eq!(AddTwoInts::Response::from_native(&response).sum, 42);
    assert_eq!(info.request_id.sequence_number, sequence);
    info.request_id
        .writer_guid
        .map(|byte| byte.to_ne_bytes()[0])
}

#[test]
fn service_clients_can_return_response_metadata() {
    use crate::example_interfaces::srv::AddTwoInts;

    let context = Context::create().unwrap();
    let mut node = Node::create(context, "client_response_info_test", "").unwrap();
    let service_name = format!("/client_response_info_{}", uuid::Uuid::new_v4().simple());
    let mut service = node
        .create_service::<AddTwoInts::Service>(&service_name, QosProfile::default())
        .unwrap();
    let client = node
        .create_client::<AddTwoInts::Service>(&service_name, QosProfile::default())
        .unwrap();
    let untyped_client = node
        .create_client_untyped(
            &service_name,
            "example_interfaces/srv/AddTwoInts",
            QosProfile::default(),
        )
        .unwrap();
    let available = Node::is_available(&client).unwrap();
    node.spin_until_future_complete(available, Duration::from_secs(1))
        .unwrap();
    let available = Node::is_available(&untyped_client).unwrap();
    node.spin_until_future_complete(available, Duration::from_secs(1))
        .unwrap();

    let typed_owner = client.client.upgrade().unwrap();
    let native_client = typed_owner.lock().handle() as *const rcl_client_t;
    let expected_typed_guid = native_response_guid(&mut node, &mut service, native_client);

    let response = client
        .request_with_info_with_timeout(
            &mut node,
            &AddTwoInts::Request { a: 20, b: 22 },
            Duration::from_secs(1),
        )
        .unwrap();
    let mut response = Box::pin(response);
    let (response, request_info) = 'typed_response: loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(request) = service.next().now_or_never().flatten() {
            let request_info = request.request_info();
            request.respond(AddTwoInts::Response { sum: 42 }).unwrap();
            loop {
                node.try_spin_once(Duration::from_millis(10)).unwrap();
                if let Some(response) = response.as_mut().now_or_never() {
                    break 'typed_response (response.unwrap(), request_info);
                }
            }
        }
    };
    assert_eq!(response.response.sum, 42);
    assert_eq!(response.info.sequence_number, request_info.sequence_number);
    assert_eq!(response.info.writer_guid, expected_typed_guid);
    assert!(response.info.source_timestamp >= 0);
    assert!(response.info.received_timestamp >= 0);

    let legacy_response = client.request(&AddTwoInts::Request { a: 1, b: 2 }).unwrap();
    let mut legacy_response = Box::pin(legacy_response);
    let legacy_response = loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(request) = service.next().now_or_never().flatten() {
            request.respond(AddTwoInts::Response { sum: 3 }).unwrap();
        }
        if let Some(response) = legacy_response.as_mut().now_or_never() {
            break response.unwrap();
        }
    };
    assert_eq!(legacy_response.sum, 3);

    let untyped_owner = untyped_client.client.upgrade().unwrap();
    let native_client = untyped_owner.lock().handle() as *const rcl_client_t;
    let expected_untyped_guid = native_response_guid(&mut node, &mut service, native_client);

    let response = untyped_client
        .request_with_info_with_timeout(
            &mut node,
            serde_json::json!({ "a": 19, "b": 23 }),
            Duration::from_secs(1),
        )
        .unwrap();
    let mut response = Box::pin(response);
    let (response, request_info) = 'untyped_response: loop {
        node.try_spin_once(Duration::from_millis(10)).unwrap();
        if let Some(request) = service.next().now_or_never().flatten() {
            let request_info = request.request_info();
            request.respond(AddTwoInts::Response { sum: 42 }).unwrap();
            loop {
                node.try_spin_once(Duration::from_millis(10)).unwrap();
                if let Some(response) = response.as_mut().now_or_never() {
                    break 'untyped_response (response.unwrap(), request_info);
                }
            }
        }
    };
    assert_eq!(response.response.unwrap(), serde_json::json!({ "sum": 42 }));
    assert_eq!(response.info.sequence_number, request_info.sequence_number);
    assert_eq!(response.info.writer_guid, expected_untyped_guid);
    assert!(response.info.source_timestamp >= 0);
    assert!(response.info.received_timestamp >= 0);
}
