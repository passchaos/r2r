use super::{
    WrappedNativeMsg, WrappedNativeMsgUntyped, WrappedServiceTypeSupport, WrappedTypesupport,
};

fn assert_roundtrip<T>(event: &T)
where
    T: WrappedTypesupport + PartialEq + std::fmt::Debug,
{
    let native = WrappedNativeMsg::<T>::try_from(event).unwrap();
    assert_eq!(event, &T::from_native(&native));
    let serialized = event.to_serialized_bytes().unwrap();
    assert_eq!(event, &T::from_serialized_bytes(&serialized).unwrap());
    let json = WrappedNativeMsgUntyped::try_from_typed(event)
        .unwrap()
        .to_json()
        .unwrap();
    assert_eq!(json, serde_json::to_value(event).unwrap());
}

#[cfg(r2r__example_interfaces__srv__AddTwoInts)]
#[test]
fn service_event_roundtrips_empty_and_populated_sequences() {
    use crate::{example_interfaces::srv::AddTwoInts, service_msgs::msg::ServiceEventInfo};

    assert_roundtrip(&AddTwoInts::Event::default());
    let event = AddTwoInts::Event {
        info: ServiceEventInfo {
            event_type: ServiceEventInfo::REQUEST_RECEIVED as u8,
            sequence_number: 42,
            ..Default::default()
        },
        request: vec![AddTwoInts::Request { a: 12, b: -7 }],
        response: vec![AddTwoInts::Response { sum: 5 }],
    };
    assert_roundtrip(&event);
    assert!(std::ptr::eq(
        <AddTwoInts::Service as WrappedServiceTypeSupport>::Event::get_ts(),
        AddTwoInts::Event::get_ts(),
    ));
}

#[cfg(r2r__example_interfaces__srv__AddTwoInts)]
#[test]
fn service_event_rejects_overfull_request_and_response_sequences() {
    use crate::{example_interfaces::srv::AddTwoInts, Error};

    let valid = AddTwoInts::Event {
        request: vec![AddTwoInts::Request { a: 1, b: 2 }],
        response: vec![AddTwoInts::Response { sum: 3 }],
        ..Default::default()
    };
    for is_request in [true, false] {
        let mut event = valid.clone();
        if is_request {
            event.request.push(AddTwoInts::Request::default());
        } else {
            event.response.push(AddTwoInts::Response::default());
        }
        assert!(matches!(
            WrappedNativeMsg::<_>::try_from(&event),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));

        let mut native = WrappedNativeMsg::<_>::try_from(&valid).unwrap();
        assert!(matches!(
            event.try_copy_to_native(&mut native),
            Err(Error::RCL_RET_INVALID_ARGUMENT)
        ));
        assert_eq!(valid, AddTwoInts::Event::from_native(&native));
    }
}

#[cfg(r2r__example_interfaces__action__Fibonacci)]
#[test]
fn action_service_events_roundtrip_nested_payloads() {
    use crate::example_interfaces::action::Fibonacci;

    let goal_event = Fibonacci::SendGoal::Event {
        request: vec![Fibonacci::SendGoal::Request {
            goal: Fibonacci::Goal { order: 5 },
            ..Default::default()
        }],
        response: vec![Fibonacci::SendGoal::Response {
            accepted: true,
            ..Default::default()
        }],
        ..Default::default()
    };
    assert_roundtrip(&Fibonacci::SendGoal::Event::default());
    assert_roundtrip(&goal_event);
    assert!(std::ptr::eq(
        <Fibonacci::SendGoal::Service as WrappedServiceTypeSupport>::Event::get_ts(),
        Fibonacci::SendGoal::Event::get_ts(),
    ));

    let result_event = Fibonacci::GetResult::Event {
        request: vec![Fibonacci::GetResult::Request::default()],
        response: vec![Fibonacci::GetResult::Response {
            status: 4,
            result: Fibonacci::Result {
                sequence: vec![0, 1, 1, 2, 3, 5],
            },
        }],
        ..Default::default()
    };
    assert_roundtrip(&Fibonacci::GetResult::Event::default());
    assert_roundtrip(&result_event);
    assert!(std::ptr::eq(
        <Fibonacci::GetResult::Service as WrappedServiceTypeSupport>::Event::get_ts(),
        Fibonacci::GetResult::Event::get_ts(),
    ));
}

#[cfg(r2r__example_interfaces__action__Fibonacci)]
#[test]
fn action_service_events_reject_overfull_sequences() {
    use crate::{example_interfaces::action::Fibonacci, Error};

    let goal_event = Fibonacci::SendGoal::Event {
        request: vec![Fibonacci::SendGoal::Request::default(); 2],
        ..Default::default()
    };
    assert!(matches!(
        WrappedNativeMsg::<_>::try_from(&goal_event),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    let result_event = Fibonacci::GetResult::Event {
        response: vec![Fibonacci::GetResult::Response::default(); 2],
        ..Default::default()
    };
    assert!(matches!(
        WrappedNativeMsg::<_>::try_from(&result_event),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
}
