pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct UUID {
        #[serde(with = "crate::msg_types::fixed_array")]
        pub uuid: [u8; 16usize],
    }
    unsafe impl WrappedTypesupport for UUID {
        type CStruct = unique_identifier_msgs__msg__UUID;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__unique_identifier_msgs__msg__UUID()
            }
        }
        fn create_msg() -> *mut unique_identifier_msgs__msg__UUID {
            #[cfg(not(feature = "doc-only"))]
            unsafe { unique_identifier_msgs__msg__UUID__create() }
            #[cfg(feature = "doc-only")] unique_identifier_msgs__msg__UUID__create()
        }
        fn destroy_msg(msg: *mut unique_identifier_msgs__msg__UUID) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { unique_identifier_msgs__msg__UUID__destroy(msg) };
            #[cfg(feature = "doc-only")] unique_identifier_msgs__msg__UUID__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> UUID {
            UUID { uuid: msg.uuid }
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
            msg.uuid.copy_from_slice(&self.uuid[..16usize]);
            Ok(())
        }
    }
    impl Default for UUID {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<UUID>::new();
            UUID::from_native(&msg_native)
        }
    }
}
