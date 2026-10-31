use super::*;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Missing;
fn missing() -> Error {
    Error::NativeTypeSupport {
        interface: "missing/msg/Type".into(),
        err: "missing test interface library".into(),
    }
}
unsafe impl WrappedTypesupport for Missing {
    type CStruct = ();
    fn get_ts() -> &'static rosidl_message_type_support_t {
        panic!("fallible factory used infallible support")
    }
    fn try_get_ts() -> Result<&'static rosidl_message_type_support_t> {
        Err(missing())
    }
    fn create_msg() -> *mut () {
        panic!("fallible native creation used infallible constructor")
    }
    fn try_create_msg() -> Result<*mut ()> {
        Err(missing())
    }
    fn destroy_msg(_: *mut ()) {
        panic!("failed native creation installed a finalizer")
    }
    fn from_native(_: &()) -> Self {
        Self
    }
    fn copy_to_native(&self, _: &mut ()) {}
}
#[test]
fn lazy_typesupport_missing_constructor_reports_error_without_native_ownership() {
    assert!(matches!(
        WrappedNativeMsg::<Missing>::try_new(),
        Err(Error::NativeTypeSupport { .. })
    ));
    assert!(matches!(
        WrappedNativeMsgUntyped::try_new::<Missing>(),
        Err(Error::NativeTypeSupport { .. })
    ));
}
#[test]
fn lazy_typesupport_endpoint_factory_reports_error_before_registration() {
    let context = crate::Context::create().unwrap();
    let mut node = crate::Node::create(context, "missing_typesupport_factory", "").unwrap();
    assert!(matches!(
        node.create_publisher::<Missing>("/missing_publisher", crate::QosProfile::default()),
        Err(Error::NativeTypeSupport { .. })
    ));
    let mut group = crate::ReentrantCallbackGroup::new(2).unwrap();
    assert!(matches!(
        node.subscribe_native_reentrant_callback::<Missing, _>(
            &mut group,
            "/missing_subscriber",
            crate::QosProfile::default(),
            |_, _| Ok(())
        ),
        Err(Error::NativeTypeSupport { .. })
    ));
    assert!(group.is_empty());
}
