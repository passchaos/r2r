use super::*;

#[test]
fn type_description_service_has_no_implicit_endpoint_and_unsupported_versions_fail_cleanly() {
    let mut node =
        Node::create(Context::create().unwrap(), "type_description_default", "").unwrap();
    assert!(node.type_description_service_name().unwrap().is_none());
    assert!(!node.stop_type_description_service());
    #[cfg(any(r2r__ros__distro__humble, r2r__ros__distro__iron))]
    {
        assert!(matches!(node.start_type_description_service(), Err(Error::RCL_RET_UNSUPPORTED)));
        assert!(node.services.is_empty());
        assert!(matches!(
            Node::create_with_options(
                node.context.clone(),
                "unsupported_type_description",
                "",
                NodeOptions::default().start_type_description_service(true)
            ),
            Err(Error::RCL_RET_UNSUPPORTED)
        ));
    }
}

#[cfg(all(
    not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
    r2r__type_description_interfaces__srv__GetTypeDescription
))]
mod native;
