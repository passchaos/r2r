pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ServiceEventInfo {
        pub event_type: u8,
        pub stamp: builtin_interfaces::msg::Time,
        #[serde(with = "crate::msg_types::fixed_array")]
        pub client_gid: [u8; 16usize],
        pub sequence_number: i64,
    }
    unsafe impl WrappedTypesupport for ServiceEventInfo {
        type CStruct = service_msgs__msg__ServiceEventInfo;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__service_msgs__msg__ServiceEventInfo()
            }
        }
        fn create_msg() -> *mut service_msgs__msg__ServiceEventInfo {
            #[cfg(not(feature = "doc-only"))]
            unsafe { service_msgs__msg__ServiceEventInfo__create() }
            #[cfg(feature = "doc-only")] service_msgs__msg__ServiceEventInfo__create()
        }
        fn destroy_msg(msg: *mut service_msgs__msg__ServiceEventInfo) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { service_msgs__msg__ServiceEventInfo__destroy(msg) };
            #[cfg(feature = "doc-only")]
            service_msgs__msg__ServiceEventInfo__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> ServiceEventInfo {
            ServiceEventInfo {
                event_type: msg.event_type,
                stamp: builtin_interfaces::msg::Time::from_native(&msg.stamp),
                client_gid: msg.client_gid,
                sequence_number: msg.sequence_number,
            }
        }
        fn copy_to_native(&self, msg: &mut Self::CStruct) {
            self.try_copy_to_native(msg)
                .expect("failed to copy Rust message into native ROS storage");
        }
        fn try_copy_to_native(
            &self,
            #[allow(unused)]
            msg: &mut Self::CStruct,
        ) -> crate::Result<()> {
            msg.event_type = self.event_type;
            self.stamp.try_copy_to_native(&mut msg.stamp)?;
            msg.client_gid.copy_from_slice(&self.client_gid[..16usize]);
            msg.sequence_number = self.sequence_number;
            Ok(())
        }
    }
    impl Default for ServiceEventInfo {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<ServiceEventInfo>::new();
            ServiceEventInfo::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl ServiceEventInfo {
        pub const REQUEST_RECEIVED: _bindgen_ty_104 = service_msgs__msg__ServiceEventInfo__REQUEST_RECEIVED;
        pub const REQUEST_SENT: _bindgen_ty_103 = service_msgs__msg__ServiceEventInfo__REQUEST_SENT;
        pub const RESPONSE_RECEIVED: _bindgen_ty_106 = service_msgs__msg__ServiceEventInfo__RESPONSE_RECEIVED;
        pub const RESPONSE_SENT: _bindgen_ty_105 = service_msgs__msg__ServiceEventInfo__RESPONSE_SENT;
    }
}
