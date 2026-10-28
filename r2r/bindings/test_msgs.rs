pub mod srv {
    #[allow(non_snake_case)]
    pub mod Arrays {
        use super::super::super::*;
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Service();
        unsafe impl WrappedServiceTypeSupport for Service {
            type Request = Request;
            type Response = Response;
            fn get_ts() -> &'static rosidl_service_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_service_type_support_handle__test_msgs__srv__Arrays()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            #[serde(with = "crate::msg_types::fixed_array")]
            pub bool_values: [bool; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub byte_values: [u8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub char_values: [u8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub float32_values: [f32; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub float64_values: [f64; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int8_values: [i8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint8_values: [u8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int16_values: [i16; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint16_values: [u16; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int32_values: [i32; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint32_values: [u32; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int64_values: [i64; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint64_values: [u64; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub string_values: [crate::RosString; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub basic_types_values: [test_msgs::msg::BasicTypes; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub constants_values: [test_msgs::msg::Constants; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub defaults_values: [test_msgs::msg::Defaults; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub bool_values_default: [bool; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub byte_values_default: [u8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub char_values_default: [u8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub float32_values_default: [f32; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub float64_values_default: [f64; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int8_values_default: [i8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint8_values_default: [u8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int16_values_default: [i16; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint16_values_default: [u16; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int32_values_default: [i32; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint32_values_default: [u32; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int64_values_default: [i64; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint64_values_default: [u64; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub string_values_default: [crate::RosString; 3usize],
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = test_msgs__srv__Arrays_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__srv__Arrays_Request()
                }
            }
            fn create_msg() -> *mut test_msgs__srv__Arrays_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__srv__Arrays_Request__create() }
                #[cfg(feature = "doc-only")] test_msgs__srv__Arrays_Request__create()
            }
            fn destroy_msg(msg: *mut test_msgs__srv__Arrays_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__srv__Arrays_Request__destroy(msg) };
                #[cfg(feature = "doc-only")] test_msgs__srv__Arrays_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    bool_values: msg.bool_values,
                    byte_values: msg.byte_values,
                    char_values: msg.char_values,
                    float32_values: msg.float32_values,
                    float64_values: msg.float64_values,
                    int8_values: msg.int8_values,
                    uint8_values: msg.uint8_values,
                    int16_values: msg.int16_values,
                    uint16_values: msg.uint16_values,
                    int32_values: msg.int32_values,
                    uint32_values: msg.uint32_values,
                    int64_values: msg.int64_values,
                    uint64_values: msg.uint64_values,
                    string_values: std::array::from_fn(|index| crate::RosString::from(
                        msg.string_values[index].as_bytes(),
                    )),
                    basic_types_values: std::array::from_fn(|index| test_msgs::msg::BasicTypes::from_native(
                        &msg.basic_types_values[index],
                    )),
                    constants_values: std::array::from_fn(|index| test_msgs::msg::Constants::from_native(
                        &msg.constants_values[index],
                    )),
                    defaults_values: std::array::from_fn(|index| test_msgs::msg::Defaults::from_native(
                        &msg.defaults_values[index],
                    )),
                    bool_values_default: msg.bool_values_default,
                    byte_values_default: msg.byte_values_default,
                    char_values_default: msg.char_values_default,
                    float32_values_default: msg.float32_values_default,
                    float64_values_default: msg.float64_values_default,
                    int8_values_default: msg.int8_values_default,
                    uint8_values_default: msg.uint8_values_default,
                    int16_values_default: msg.int16_values_default,
                    uint16_values_default: msg.uint16_values_default,
                    int32_values_default: msg.int32_values_default,
                    uint32_values_default: msg.uint32_values_default,
                    int64_values_default: msg.int64_values_default,
                    uint64_values_default: msg.uint64_values_default,
                    string_values_default: std::array::from_fn(|index| crate::RosString::from(
                        msg.string_values_default[index].as_bytes(),
                    )),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                assert_eq!(
                    self.bool_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "bool_values",
                    3usize,
                );
                msg.bool_values.copy_from_slice(&self.bool_values[..3usize]);
                assert_eq!(
                    self.byte_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "byte_values",
                    3usize,
                );
                msg.byte_values.copy_from_slice(&self.byte_values[..3usize]);
                assert_eq!(
                    self.char_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "char_values",
                    3usize,
                );
                msg.char_values.copy_from_slice(&self.char_values[..3usize]);
                assert_eq!(
                    self.float32_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "float32_values",
                    3usize,
                );
                msg.float32_values.copy_from_slice(&self.float32_values[..3usize]);
                assert_eq!(
                    self.float64_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "float64_values",
                    3usize,
                );
                msg.float64_values.copy_from_slice(&self.float64_values[..3usize]);
                assert_eq!(
                    self.int8_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int8_values",
                    3usize,
                );
                msg.int8_values.copy_from_slice(&self.int8_values[..3usize]);
                assert_eq!(
                    self.uint8_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint8_values",
                    3usize,
                );
                msg.uint8_values.copy_from_slice(&self.uint8_values[..3usize]);
                assert_eq!(
                    self.int16_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int16_values",
                    3usize,
                );
                msg.int16_values.copy_from_slice(&self.int16_values[..3usize]);
                assert_eq!(
                    self.uint16_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint16_values",
                    3usize,
                );
                msg.uint16_values.copy_from_slice(&self.uint16_values[..3usize]);
                assert_eq!(
                    self.int32_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int32_values",
                    3usize,
                );
                msg.int32_values.copy_from_slice(&self.int32_values[..3usize]);
                assert_eq!(
                    self.uint32_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint32_values",
                    3usize,
                );
                msg.uint32_values.copy_from_slice(&self.uint32_values[..3usize]);
                assert_eq!(
                    self.int64_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int64_values",
                    3usize,
                );
                msg.int64_values.copy_from_slice(&self.int64_values[..3usize]);
                assert_eq!(
                    self.uint64_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint64_values",
                    3usize,
                );
                msg.uint64_values.copy_from_slice(&self.uint64_values[..3usize]);
                assert_eq!(
                    self.string_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "string_values",
                    3usize,
                );
                for (t, s) in msg.string_values.iter_mut().zip(&self.string_values) {
                    t.try_assign_bytes((&s).as_bytes());
                }
                assert_eq!(
                    self.basic_types_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "basic_types_values",
                    3usize,
                );
                for (t, s) in msg
                    .basic_types_values
                    .iter_mut()
                    .zip(&self.basic_types_values)
                {
                    s.copy_to_native(t);
                }
                assert_eq!(
                    self.constants_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "constants_values",
                    3usize,
                );
                for (t, s) in msg.constants_values.iter_mut().zip(&self.constants_values)
                {
                    s.copy_to_native(t);
                }
                assert_eq!(
                    self.defaults_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "defaults_values",
                    3usize,
                );
                for (t, s) in msg.defaults_values.iter_mut().zip(&self.defaults_values) {
                    s.copy_to_native(t);
                }
                assert_eq!(
                    self.bool_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "bool_values_default",
                    3usize,
                );
                msg.bool_values_default
                    .copy_from_slice(&self.bool_values_default[..3usize]);
                assert_eq!(
                    self.byte_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "byte_values_default",
                    3usize,
                );
                msg.byte_values_default
                    .copy_from_slice(&self.byte_values_default[..3usize]);
                assert_eq!(
                    self.char_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "char_values_default",
                    3usize,
                );
                msg.char_values_default
                    .copy_from_slice(&self.char_values_default[..3usize]);
                assert_eq!(
                    self.float32_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "float32_values_default",
                    3usize,
                );
                msg.float32_values_default
                    .copy_from_slice(&self.float32_values_default[..3usize]);
                assert_eq!(
                    self.float64_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "float64_values_default",
                    3usize,
                );
                msg.float64_values_default
                    .copy_from_slice(&self.float64_values_default[..3usize]);
                assert_eq!(
                    self.int8_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int8_values_default",
                    3usize,
                );
                msg.int8_values_default
                    .copy_from_slice(&self.int8_values_default[..3usize]);
                assert_eq!(
                    self.uint8_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint8_values_default",
                    3usize,
                );
                msg.uint8_values_default
                    .copy_from_slice(&self.uint8_values_default[..3usize]);
                assert_eq!(
                    self.int16_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int16_values_default",
                    3usize,
                );
                msg.int16_values_default
                    .copy_from_slice(&self.int16_values_default[..3usize]);
                assert_eq!(
                    self.uint16_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint16_values_default",
                    3usize,
                );
                msg.uint16_values_default
                    .copy_from_slice(&self.uint16_values_default[..3usize]);
                assert_eq!(
                    self.int32_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int32_values_default",
                    3usize,
                );
                msg.int32_values_default
                    .copy_from_slice(&self.int32_values_default[..3usize]);
                assert_eq!(
                    self.uint32_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint32_values_default",
                    3usize,
                );
                msg.uint32_values_default
                    .copy_from_slice(&self.uint32_values_default[..3usize]);
                assert_eq!(
                    self.int64_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int64_values_default",
                    3usize,
                );
                msg.int64_values_default
                    .copy_from_slice(&self.int64_values_default[..3usize]);
                assert_eq!(
                    self.uint64_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint64_values_default",
                    3usize,
                );
                msg.uint64_values_default
                    .copy_from_slice(&self.uint64_values_default[..3usize]);
                assert_eq!(
                    self.string_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "string_values_default",
                    3usize,
                );
                for (t, s) in msg
                    .string_values_default
                    .iter_mut()
                    .zip(&self.string_values_default)
                {
                    t.try_assign_bytes((&s).as_bytes());
                }
            }
        }
        impl Default for Request {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Request>::new();
                Request::from_native(&msg_native)
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Response {
            #[serde(with = "crate::msg_types::fixed_array")]
            pub bool_values: [bool; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub byte_values: [u8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub char_values: [u8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub float32_values: [f32; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub float64_values: [f64; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int8_values: [i8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint8_values: [u8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int16_values: [i16; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint16_values: [u16; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int32_values: [i32; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint32_values: [u32; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int64_values: [i64; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint64_values: [u64; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub string_values: [crate::RosString; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub basic_types_values: [test_msgs::msg::BasicTypes; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub constants_values: [test_msgs::msg::Constants; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub defaults_values: [test_msgs::msg::Defaults; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub bool_values_default: [bool; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub byte_values_default: [u8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub char_values_default: [u8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub float32_values_default: [f32; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub float64_values_default: [f64; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int8_values_default: [i8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint8_values_default: [u8; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int16_values_default: [i16; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint16_values_default: [u16; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int32_values_default: [i32; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint32_values_default: [u32; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub int64_values_default: [i64; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub uint64_values_default: [u64; 3usize],
            #[serde(with = "crate::msg_types::fixed_array")]
            pub string_values_default: [crate::RosString; 3usize],
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = test_msgs__srv__Arrays_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__srv__Arrays_Response()
                }
            }
            fn create_msg() -> *mut test_msgs__srv__Arrays_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__srv__Arrays_Response__create() }
                #[cfg(feature = "doc-only")] test_msgs__srv__Arrays_Response__create()
            }
            fn destroy_msg(msg: *mut test_msgs__srv__Arrays_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__srv__Arrays_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                test_msgs__srv__Arrays_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    bool_values: msg.bool_values,
                    byte_values: msg.byte_values,
                    char_values: msg.char_values,
                    float32_values: msg.float32_values,
                    float64_values: msg.float64_values,
                    int8_values: msg.int8_values,
                    uint8_values: msg.uint8_values,
                    int16_values: msg.int16_values,
                    uint16_values: msg.uint16_values,
                    int32_values: msg.int32_values,
                    uint32_values: msg.uint32_values,
                    int64_values: msg.int64_values,
                    uint64_values: msg.uint64_values,
                    string_values: std::array::from_fn(|index| crate::RosString::from(
                        msg.string_values[index].as_bytes(),
                    )),
                    basic_types_values: std::array::from_fn(|index| test_msgs::msg::BasicTypes::from_native(
                        &msg.basic_types_values[index],
                    )),
                    constants_values: std::array::from_fn(|index| test_msgs::msg::Constants::from_native(
                        &msg.constants_values[index],
                    )),
                    defaults_values: std::array::from_fn(|index| test_msgs::msg::Defaults::from_native(
                        &msg.defaults_values[index],
                    )),
                    bool_values_default: msg.bool_values_default,
                    byte_values_default: msg.byte_values_default,
                    char_values_default: msg.char_values_default,
                    float32_values_default: msg.float32_values_default,
                    float64_values_default: msg.float64_values_default,
                    int8_values_default: msg.int8_values_default,
                    uint8_values_default: msg.uint8_values_default,
                    int16_values_default: msg.int16_values_default,
                    uint16_values_default: msg.uint16_values_default,
                    int32_values_default: msg.int32_values_default,
                    uint32_values_default: msg.uint32_values_default,
                    int64_values_default: msg.int64_values_default,
                    uint64_values_default: msg.uint64_values_default,
                    string_values_default: std::array::from_fn(|index| crate::RosString::from(
                        msg.string_values_default[index].as_bytes(),
                    )),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                assert_eq!(
                    self.bool_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "bool_values",
                    3usize,
                );
                msg.bool_values.copy_from_slice(&self.bool_values[..3usize]);
                assert_eq!(
                    self.byte_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "byte_values",
                    3usize,
                );
                msg.byte_values.copy_from_slice(&self.byte_values[..3usize]);
                assert_eq!(
                    self.char_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "char_values",
                    3usize,
                );
                msg.char_values.copy_from_slice(&self.char_values[..3usize]);
                assert_eq!(
                    self.float32_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "float32_values",
                    3usize,
                );
                msg.float32_values.copy_from_slice(&self.float32_values[..3usize]);
                assert_eq!(
                    self.float64_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "float64_values",
                    3usize,
                );
                msg.float64_values.copy_from_slice(&self.float64_values[..3usize]);
                assert_eq!(
                    self.int8_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int8_values",
                    3usize,
                );
                msg.int8_values.copy_from_slice(&self.int8_values[..3usize]);
                assert_eq!(
                    self.uint8_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint8_values",
                    3usize,
                );
                msg.uint8_values.copy_from_slice(&self.uint8_values[..3usize]);
                assert_eq!(
                    self.int16_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int16_values",
                    3usize,
                );
                msg.int16_values.copy_from_slice(&self.int16_values[..3usize]);
                assert_eq!(
                    self.uint16_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint16_values",
                    3usize,
                );
                msg.uint16_values.copy_from_slice(&self.uint16_values[..3usize]);
                assert_eq!(
                    self.int32_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int32_values",
                    3usize,
                );
                msg.int32_values.copy_from_slice(&self.int32_values[..3usize]);
                assert_eq!(
                    self.uint32_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint32_values",
                    3usize,
                );
                msg.uint32_values.copy_from_slice(&self.uint32_values[..3usize]);
                assert_eq!(
                    self.int64_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int64_values",
                    3usize,
                );
                msg.int64_values.copy_from_slice(&self.int64_values[..3usize]);
                assert_eq!(
                    self.uint64_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint64_values",
                    3usize,
                );
                msg.uint64_values.copy_from_slice(&self.uint64_values[..3usize]);
                assert_eq!(
                    self.string_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "string_values",
                    3usize,
                );
                for (t, s) in msg.string_values.iter_mut().zip(&self.string_values) {
                    t.try_assign_bytes((&s).as_bytes());
                }
                assert_eq!(
                    self.basic_types_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "basic_types_values",
                    3usize,
                );
                for (t, s) in msg
                    .basic_types_values
                    .iter_mut()
                    .zip(&self.basic_types_values)
                {
                    s.copy_to_native(t);
                }
                assert_eq!(
                    self.constants_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "constants_values",
                    3usize,
                );
                for (t, s) in msg.constants_values.iter_mut().zip(&self.constants_values)
                {
                    s.copy_to_native(t);
                }
                assert_eq!(
                    self.defaults_values.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "defaults_values",
                    3usize,
                );
                for (t, s) in msg.defaults_values.iter_mut().zip(&self.defaults_values) {
                    s.copy_to_native(t);
                }
                assert_eq!(
                    self.bool_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "bool_values_default",
                    3usize,
                );
                msg.bool_values_default
                    .copy_from_slice(&self.bool_values_default[..3usize]);
                assert_eq!(
                    self.byte_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "byte_values_default",
                    3usize,
                );
                msg.byte_values_default
                    .copy_from_slice(&self.byte_values_default[..3usize]);
                assert_eq!(
                    self.char_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "char_values_default",
                    3usize,
                );
                msg.char_values_default
                    .copy_from_slice(&self.char_values_default[..3usize]);
                assert_eq!(
                    self.float32_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "float32_values_default",
                    3usize,
                );
                msg.float32_values_default
                    .copy_from_slice(&self.float32_values_default[..3usize]);
                assert_eq!(
                    self.float64_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "float64_values_default",
                    3usize,
                );
                msg.float64_values_default
                    .copy_from_slice(&self.float64_values_default[..3usize]);
                assert_eq!(
                    self.int8_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int8_values_default",
                    3usize,
                );
                msg.int8_values_default
                    .copy_from_slice(&self.int8_values_default[..3usize]);
                assert_eq!(
                    self.uint8_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint8_values_default",
                    3usize,
                );
                msg.uint8_values_default
                    .copy_from_slice(&self.uint8_values_default[..3usize]);
                assert_eq!(
                    self.int16_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int16_values_default",
                    3usize,
                );
                msg.int16_values_default
                    .copy_from_slice(&self.int16_values_default[..3usize]);
                assert_eq!(
                    self.uint16_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint16_values_default",
                    3usize,
                );
                msg.uint16_values_default
                    .copy_from_slice(&self.uint16_values_default[..3usize]);
                assert_eq!(
                    self.int32_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int32_values_default",
                    3usize,
                );
                msg.int32_values_default
                    .copy_from_slice(&self.int32_values_default[..3usize]);
                assert_eq!(
                    self.uint32_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint32_values_default",
                    3usize,
                );
                msg.uint32_values_default
                    .copy_from_slice(&self.uint32_values_default[..3usize]);
                assert_eq!(
                    self.int64_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "int64_values_default",
                    3usize,
                );
                msg.int64_values_default
                    .copy_from_slice(&self.int64_values_default[..3usize]);
                assert_eq!(
                    self.uint64_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "uint64_values_default",
                    3usize,
                );
                msg.uint64_values_default
                    .copy_from_slice(&self.uint64_values_default[..3usize]);
                assert_eq!(
                    self.string_values_default.len(),
                    3usize,
                    "Field {} is fixed size of {}!",
                    "string_values_default",
                    3usize,
                );
                for (t, s) in msg
                    .string_values_default
                    .iter_mut()
                    .zip(&self.string_values_default)
                {
                    t.try_assign_bytes((&s).as_bytes());
                }
            }
        }
        impl Default for Response {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Response>::new();
                Response::from_native(&msg_native)
            }
        }
    }
    #[allow(non_snake_case)]
    pub mod BasicTypes {
        use super::super::super::*;
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Service();
        unsafe impl WrappedServiceTypeSupport for Service {
            type Request = Request;
            type Response = Response;
            fn get_ts() -> &'static rosidl_service_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_service_type_support_handle__test_msgs__srv__BasicTypes()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub bool_value: bool,
            pub byte_value: u8,
            pub char_value: u8,
            pub float32_value: f32,
            pub float64_value: f64,
            pub int8_value: i8,
            pub uint8_value: u8,
            pub int16_value: i16,
            pub uint16_value: u16,
            pub int32_value: i32,
            pub uint32_value: u32,
            pub int64_value: i64,
            pub uint64_value: u64,
            pub string_value: crate::RosString,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = test_msgs__srv__BasicTypes_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__srv__BasicTypes_Request()
                }
            }
            fn create_msg() -> *mut test_msgs__srv__BasicTypes_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__srv__BasicTypes_Request__create() }
                #[cfg(feature = "doc-only")] test_msgs__srv__BasicTypes_Request__create()
            }
            fn destroy_msg(msg: *mut test_msgs__srv__BasicTypes_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__srv__BasicTypes_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                test_msgs__srv__BasicTypes_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    bool_value: msg.bool_value,
                    byte_value: msg.byte_value,
                    char_value: msg.char_value,
                    float32_value: msg.float32_value,
                    float64_value: msg.float64_value,
                    int8_value: msg.int8_value,
                    uint8_value: msg.uint8_value,
                    int16_value: msg.int16_value,
                    uint16_value: msg.uint16_value,
                    int32_value: msg.int32_value,
                    uint32_value: msg.uint32_value,
                    int64_value: msg.int64_value,
                    uint64_value: msg.uint64_value,
                    string_value: crate::RosString::from(msg.string_value.as_bytes()),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                msg.bool_value = self.bool_value;
                msg.byte_value = self.byte_value;
                msg.char_value = self.char_value;
                msg.float32_value = self.float32_value;
                msg.float64_value = self.float64_value;
                msg.int8_value = self.int8_value;
                msg.uint8_value = self.uint8_value;
                msg.int16_value = self.int16_value;
                msg.uint16_value = self.uint16_value;
                msg.int32_value = self.int32_value;
                msg.uint32_value = self.uint32_value;
                msg.int64_value = self.int64_value;
                msg.uint64_value = self.uint64_value;
                msg.string_value.try_assign_bytes((&self.string_value).as_bytes());
            }
        }
        impl Default for Request {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Request>::new();
                Request::from_native(&msg_native)
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Response {
            pub bool_value: bool,
            pub byte_value: u8,
            pub char_value: u8,
            pub float32_value: f32,
            pub float64_value: f64,
            pub int8_value: i8,
            pub uint8_value: u8,
            pub int16_value: i16,
            pub uint16_value: u16,
            pub int32_value: i32,
            pub uint32_value: u32,
            pub int64_value: i64,
            pub uint64_value: u64,
            pub string_value: crate::RosString,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = test_msgs__srv__BasicTypes_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__srv__BasicTypes_Response()
                }
            }
            fn create_msg() -> *mut test_msgs__srv__BasicTypes_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__srv__BasicTypes_Response__create() }
                #[cfg(feature = "doc-only")]
                test_msgs__srv__BasicTypes_Response__create()
            }
            fn destroy_msg(msg: *mut test_msgs__srv__BasicTypes_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__srv__BasicTypes_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                test_msgs__srv__BasicTypes_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    bool_value: msg.bool_value,
                    byte_value: msg.byte_value,
                    char_value: msg.char_value,
                    float32_value: msg.float32_value,
                    float64_value: msg.float64_value,
                    int8_value: msg.int8_value,
                    uint8_value: msg.uint8_value,
                    int16_value: msg.int16_value,
                    uint16_value: msg.uint16_value,
                    int32_value: msg.int32_value,
                    uint32_value: msg.uint32_value,
                    int64_value: msg.int64_value,
                    uint64_value: msg.uint64_value,
                    string_value: crate::RosString::from(msg.string_value.as_bytes()),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                msg.bool_value = self.bool_value;
                msg.byte_value = self.byte_value;
                msg.char_value = self.char_value;
                msg.float32_value = self.float32_value;
                msg.float64_value = self.float64_value;
                msg.int8_value = self.int8_value;
                msg.uint8_value = self.uint8_value;
                msg.int16_value = self.int16_value;
                msg.uint16_value = self.uint16_value;
                msg.int32_value = self.int32_value;
                msg.uint32_value = self.uint32_value;
                msg.int64_value = self.int64_value;
                msg.uint64_value = self.uint64_value;
                msg.string_value.try_assign_bytes((&self.string_value).as_bytes());
            }
        }
        impl Default for Response {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Response>::new();
                Response::from_native(&msg_native)
            }
        }
    }
    #[allow(non_snake_case)]
    pub mod Empty {
        use super::super::super::*;
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Service();
        unsafe impl WrappedServiceTypeSupport for Service {
            type Request = Request;
            type Response = Response;
            fn get_ts() -> &'static rosidl_service_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_service_type_support_handle__test_msgs__srv__Empty()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {}
        unsafe impl WrappedTypesupport for Request {
            type CStruct = test_msgs__srv__Empty_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__srv__Empty_Request()
                }
            }
            fn create_msg() -> *mut test_msgs__srv__Empty_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__srv__Empty_Request__create() }
                #[cfg(feature = "doc-only")] test_msgs__srv__Empty_Request__create()
            }
            fn destroy_msg(msg: *mut test_msgs__srv__Empty_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__srv__Empty_Request__destroy(msg) };
                #[cfg(feature = "doc-only")] test_msgs__srv__Empty_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {}
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {}
        }
        impl Default for Request {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Request>::new();
                Request::from_native(&msg_native)
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Response {}
        unsafe impl WrappedTypesupport for Response {
            type CStruct = test_msgs__srv__Empty_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__srv__Empty_Response()
                }
            }
            fn create_msg() -> *mut test_msgs__srv__Empty_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__srv__Empty_Response__create() }
                #[cfg(feature = "doc-only")] test_msgs__srv__Empty_Response__create()
            }
            fn destroy_msg(msg: *mut test_msgs__srv__Empty_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__srv__Empty_Response__destroy(msg) };
                #[cfg(feature = "doc-only")] test_msgs__srv__Empty_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {}
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {}
        }
        impl Default for Response {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Response>::new();
                Response::from_native(&msg_native)
            }
        }
    }
}
pub mod action {
    #[allow(non_snake_case)]
    pub mod Fibonacci {
        use super::super::super::*;
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Action();
        unsafe impl WrappedActionTypeSupport for Action {
            type Goal = Goal;
            type Result = Result;
            type Feedback = Feedback;
            type FeedbackMessage = FeedbackMessage;
            type SendGoal = SendGoal::Service;
            type GetResult = GetResult::Service;
            fn get_ts() -> &'static rosidl_action_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_action_type_support_handle__test_msgs__action__Fibonacci()
                }
            }
            fn make_goal_request_msg(
                goal_id: unique_identifier_msgs::msg::UUID,
                goal: Goal,
            ) -> SendGoal::Request {
                SendGoal::Request { goal_id, goal }
            }
            fn make_goal_response_msg(
                accepted: bool,
                stamp: builtin_interfaces::msg::Time,
            ) -> SendGoal::Response {
                SendGoal::Response {
                    accepted,
                    stamp,
                }
            }
            fn make_feedback_msg(
                goal_id: unique_identifier_msgs::msg::UUID,
                feedback: Feedback,
            ) -> FeedbackMessage {
                FeedbackMessage {
                    goal_id,
                    feedback,
                }
            }
            fn make_result_request_msg(
                goal_id: unique_identifier_msgs::msg::UUID,
            ) -> GetResult::Request {
                GetResult::Request { goal_id }
            }
            fn make_result_response_msg(
                status: i8,
                result: Result,
            ) -> GetResult::Response {
                GetResult::Response {
                    status,
                    result,
                }
            }
            fn destructure_goal_request_msg(
                msg: SendGoal::Request,
            ) -> (unique_identifier_msgs::msg::UUID, Goal) {
                (msg.goal_id, msg.goal)
            }
            fn destructure_goal_response_msg(
                msg: SendGoal::Response,
            ) -> (bool, builtin_interfaces::msg::Time) {
                (msg.accepted, msg.stamp)
            }
            fn destructure_feedback_msg(
                msg: FeedbackMessage,
            ) -> (unique_identifier_msgs::msg::UUID, Feedback) {
                (msg.goal_id, msg.feedback)
            }
            fn destructure_result_response_msg(
                msg: GetResult::Response,
            ) -> (i8, Result) {
                (msg.status, msg.result)
            }
            fn destructure_result_request_msg(
                msg: GetResult::Request,
            ) -> unique_identifier_msgs::msg::UUID {
                msg.goal_id
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Goal {
            pub order: i32,
        }
        unsafe impl WrappedTypesupport for Goal {
            type CStruct = test_msgs__action__Fibonacci_Goal;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__Fibonacci_Goal()
                }
            }
            fn create_msg() -> *mut test_msgs__action__Fibonacci_Goal {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__Fibonacci_Goal__create() }
                #[cfg(feature = "doc-only")] test_msgs__action__Fibonacci_Goal__create()
            }
            fn destroy_msg(msg: *mut test_msgs__action__Fibonacci_Goal) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__Fibonacci_Goal__destroy(msg) };
                #[cfg(feature = "doc-only")]
                test_msgs__action__Fibonacci_Goal__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Goal {
                Goal { order: msg.order }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                msg.order = self.order;
            }
        }
        impl Default for Goal {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Goal>::new();
                Goal::from_native(&msg_native)
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Result {
            pub sequence: Vec<i32>,
        }
        unsafe impl WrappedTypesupport for Result {
            type CStruct = test_msgs__action__Fibonacci_Result;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__Fibonacci_Result()
                }
            }
            fn create_msg() -> *mut test_msgs__action__Fibonacci_Result {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__Fibonacci_Result__create() }
                #[cfg(feature = "doc-only")]
                test_msgs__action__Fibonacci_Result__create()
            }
            fn destroy_msg(msg: *mut test_msgs__action__Fibonacci_Result) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__Fibonacci_Result__destroy(msg) };
                #[cfg(feature = "doc-only")]
                test_msgs__action__Fibonacci_Result__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Result {
                Result {
                    sequence: msg.sequence.to_vec(),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                msg.sequence.update(&self.sequence);
            }
        }
        impl Default for Result {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Result>::new();
                Result::from_native(&msg_native)
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Feedback {
            pub sequence: Vec<i32>,
        }
        unsafe impl WrappedTypesupport for Feedback {
            type CStruct = test_msgs__action__Fibonacci_Feedback;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__Fibonacci_Feedback()
                }
            }
            fn create_msg() -> *mut test_msgs__action__Fibonacci_Feedback {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__Fibonacci_Feedback__create() }
                #[cfg(feature = "doc-only")]
                test_msgs__action__Fibonacci_Feedback__create()
            }
            fn destroy_msg(msg: *mut test_msgs__action__Fibonacci_Feedback) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__Fibonacci_Feedback__destroy(msg) };
                #[cfg(feature = "doc-only")]
                test_msgs__action__Fibonacci_Feedback__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Feedback {
                Feedback {
                    sequence: msg.sequence.to_vec(),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                msg.sequence.update(&self.sequence);
            }
        }
        impl Default for Feedback {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Feedback>::new();
                Feedback::from_native(&msg_native)
            }
        }
        #[allow(non_snake_case)]
        pub mod SendGoal {
            use super::super::super::super::*;
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            pub struct Service();
            unsafe impl WrappedServiceTypeSupport for Service {
                type Request = Request;
                type Response = Response;
                fn get_ts() -> &'static rosidl_service_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_service_type_support_handle__test_msgs__action__Fibonacci_SendGoal()
                    }
                }
            }
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            #[serde(default)]
            pub struct Request {
                pub goal_id: unique_identifier_msgs::msg::UUID,
                pub goal: test_msgs::action::Fibonacci::Goal,
            }
            unsafe impl WrappedTypesupport for Request {
                type CStruct = test_msgs__action__Fibonacci_SendGoal_Request;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__Fibonacci_SendGoal_Request()
                    }
                }
                fn create_msg() -> *mut test_msgs__action__Fibonacci_SendGoal_Request {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe { test_msgs__action__Fibonacci_SendGoal_Request__create() }
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__Fibonacci_SendGoal_Request__create()
                }
                fn destroy_msg(
                    msg: *mut test_msgs__action__Fibonacci_SendGoal_Request,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        test_msgs__action__Fibonacci_SendGoal_Request__destroy(msg)
                    };
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__Fibonacci_SendGoal_Request__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                    Request {
                        goal_id: unique_identifier_msgs::msg::UUID::from_native(
                            &msg.goal_id,
                        ),
                        goal: test_msgs::action::Fibonacci::Goal::from_native(&msg.goal),
                    }
                }
                fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                    self.goal_id.copy_to_native(&mut msg.goal_id);
                    self.goal.copy_to_native(&mut msg.goal);
                }
            }
            impl Default for Request {
                fn default() -> Self {
                    let msg_native = WrappedNativeMsg::<Request>::new();
                    Request::from_native(&msg_native)
                }
            }
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            #[serde(default)]
            pub struct Response {
                pub accepted: bool,
                pub stamp: builtin_interfaces::msg::Time,
            }
            unsafe impl WrappedTypesupport for Response {
                type CStruct = test_msgs__action__Fibonacci_SendGoal_Response;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__Fibonacci_SendGoal_Response()
                    }
                }
                fn create_msg() -> *mut test_msgs__action__Fibonacci_SendGoal_Response {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe { test_msgs__action__Fibonacci_SendGoal_Response__create() }
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__Fibonacci_SendGoal_Response__create()
                }
                fn destroy_msg(
                    msg: *mut test_msgs__action__Fibonacci_SendGoal_Response,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        test_msgs__action__Fibonacci_SendGoal_Response__destroy(msg)
                    };
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__Fibonacci_SendGoal_Response__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                    Response {
                        accepted: msg.accepted,
                        stamp: builtin_interfaces::msg::Time::from_native(&msg.stamp),
                    }
                }
                fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                    msg.accepted = self.accepted;
                    self.stamp.copy_to_native(&mut msg.stamp);
                }
            }
            impl Default for Response {
                fn default() -> Self {
                    let msg_native = WrappedNativeMsg::<Response>::new();
                    Response::from_native(&msg_native)
                }
            }
        }
        #[allow(non_snake_case)]
        pub mod GetResult {
            use super::super::super::super::*;
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            pub struct Service();
            unsafe impl WrappedServiceTypeSupport for Service {
                type Request = Request;
                type Response = Response;
                fn get_ts() -> &'static rosidl_service_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_service_type_support_handle__test_msgs__action__Fibonacci_GetResult()
                    }
                }
            }
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            #[serde(default)]
            pub struct Request {
                pub goal_id: unique_identifier_msgs::msg::UUID,
            }
            unsafe impl WrappedTypesupport for Request {
                type CStruct = test_msgs__action__Fibonacci_GetResult_Request;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__Fibonacci_GetResult_Request()
                    }
                }
                fn create_msg() -> *mut test_msgs__action__Fibonacci_GetResult_Request {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe { test_msgs__action__Fibonacci_GetResult_Request__create() }
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__Fibonacci_GetResult_Request__create()
                }
                fn destroy_msg(
                    msg: *mut test_msgs__action__Fibonacci_GetResult_Request,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        test_msgs__action__Fibonacci_GetResult_Request__destroy(msg)
                    };
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__Fibonacci_GetResult_Request__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                    Request {
                        goal_id: unique_identifier_msgs::msg::UUID::from_native(
                            &msg.goal_id,
                        ),
                    }
                }
                fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                    self.goal_id.copy_to_native(&mut msg.goal_id);
                }
            }
            impl Default for Request {
                fn default() -> Self {
                    let msg_native = WrappedNativeMsg::<Request>::new();
                    Request::from_native(&msg_native)
                }
            }
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            #[serde(default)]
            pub struct Response {
                pub status: i8,
                pub result: test_msgs::action::Fibonacci::Result,
            }
            unsafe impl WrappedTypesupport for Response {
                type CStruct = test_msgs__action__Fibonacci_GetResult_Response;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__Fibonacci_GetResult_Response()
                    }
                }
                fn create_msg() -> *mut test_msgs__action__Fibonacci_GetResult_Response {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe { test_msgs__action__Fibonacci_GetResult_Response__create() }
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__Fibonacci_GetResult_Response__create()
                }
                fn destroy_msg(
                    msg: *mut test_msgs__action__Fibonacci_GetResult_Response,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        test_msgs__action__Fibonacci_GetResult_Response__destroy(msg)
                    };
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__Fibonacci_GetResult_Response__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                    Response {
                        status: msg.status,
                        result: test_msgs::action::Fibonacci::Result::from_native(
                            &msg.result,
                        ),
                    }
                }
                fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                    msg.status = self.status;
                    self.result.copy_to_native(&mut msg.result);
                }
            }
            impl Default for Response {
                fn default() -> Self {
                    let msg_native = WrappedNativeMsg::<Response>::new();
                    Response::from_native(&msg_native)
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct FeedbackMessage {
            pub goal_id: unique_identifier_msgs::msg::UUID,
            pub feedback: test_msgs::action::Fibonacci::Feedback,
        }
        unsafe impl WrappedTypesupport for FeedbackMessage {
            type CStruct = test_msgs__action__Fibonacci_FeedbackMessage;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__Fibonacci_FeedbackMessage()
                }
            }
            fn create_msg() -> *mut test_msgs__action__Fibonacci_FeedbackMessage {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__Fibonacci_FeedbackMessage__create() }
                #[cfg(feature = "doc-only")]
                test_msgs__action__Fibonacci_FeedbackMessage__create()
            }
            fn destroy_msg(
                msg: *mut test_msgs__action__Fibonacci_FeedbackMessage,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__Fibonacci_FeedbackMessage__destroy(msg) };
                #[cfg(feature = "doc-only")]
                test_msgs__action__Fibonacci_FeedbackMessage__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> FeedbackMessage {
                FeedbackMessage {
                    goal_id: unique_identifier_msgs::msg::UUID::from_native(
                        &msg.goal_id,
                    ),
                    feedback: test_msgs::action::Fibonacci::Feedback::from_native(
                        &msg.feedback,
                    ),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                self.goal_id.copy_to_native(&mut msg.goal_id);
                self.feedback.copy_to_native(&mut msg.feedback);
            }
        }
        impl Default for FeedbackMessage {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<FeedbackMessage>::new();
                FeedbackMessage::from_native(&msg_native)
            }
        }
    }
    #[allow(non_snake_case)]
    pub mod NestedMessage {
        use super::super::super::*;
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Action();
        unsafe impl WrappedActionTypeSupport for Action {
            type Goal = Goal;
            type Result = Result;
            type Feedback = Feedback;
            type FeedbackMessage = FeedbackMessage;
            type SendGoal = SendGoal::Service;
            type GetResult = GetResult::Service;
            fn get_ts() -> &'static rosidl_action_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_action_type_support_handle__test_msgs__action__NestedMessage()
                }
            }
            fn make_goal_request_msg(
                goal_id: unique_identifier_msgs::msg::UUID,
                goal: Goal,
            ) -> SendGoal::Request {
                SendGoal::Request { goal_id, goal }
            }
            fn make_goal_response_msg(
                accepted: bool,
                stamp: builtin_interfaces::msg::Time,
            ) -> SendGoal::Response {
                SendGoal::Response {
                    accepted,
                    stamp,
                }
            }
            fn make_feedback_msg(
                goal_id: unique_identifier_msgs::msg::UUID,
                feedback: Feedback,
            ) -> FeedbackMessage {
                FeedbackMessage {
                    goal_id,
                    feedback,
                }
            }
            fn make_result_request_msg(
                goal_id: unique_identifier_msgs::msg::UUID,
            ) -> GetResult::Request {
                GetResult::Request { goal_id }
            }
            fn make_result_response_msg(
                status: i8,
                result: Result,
            ) -> GetResult::Response {
                GetResult::Response {
                    status,
                    result,
                }
            }
            fn destructure_goal_request_msg(
                msg: SendGoal::Request,
            ) -> (unique_identifier_msgs::msg::UUID, Goal) {
                (msg.goal_id, msg.goal)
            }
            fn destructure_goal_response_msg(
                msg: SendGoal::Response,
            ) -> (bool, builtin_interfaces::msg::Time) {
                (msg.accepted, msg.stamp)
            }
            fn destructure_feedback_msg(
                msg: FeedbackMessage,
            ) -> (unique_identifier_msgs::msg::UUID, Feedback) {
                (msg.goal_id, msg.feedback)
            }
            fn destructure_result_response_msg(
                msg: GetResult::Response,
            ) -> (i8, Result) {
                (msg.status, msg.result)
            }
            fn destructure_result_request_msg(
                msg: GetResult::Request,
            ) -> unique_identifier_msgs::msg::UUID {
                msg.goal_id
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Goal {
            pub nested_field_no_pkg: test_msgs::msg::Builtins,
            pub nested_field: test_msgs::msg::BasicTypes,
            pub nested_different_pkg: builtin_interfaces::msg::Time,
        }
        unsafe impl WrappedTypesupport for Goal {
            type CStruct = test_msgs__action__NestedMessage_Goal;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__NestedMessage_Goal()
                }
            }
            fn create_msg() -> *mut test_msgs__action__NestedMessage_Goal {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__NestedMessage_Goal__create() }
                #[cfg(feature = "doc-only")]
                test_msgs__action__NestedMessage_Goal__create()
            }
            fn destroy_msg(msg: *mut test_msgs__action__NestedMessage_Goal) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__NestedMessage_Goal__destroy(msg) };
                #[cfg(feature = "doc-only")]
                test_msgs__action__NestedMessage_Goal__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Goal {
                Goal {
                    nested_field_no_pkg: test_msgs::msg::Builtins::from_native(
                        &msg.nested_field_no_pkg,
                    ),
                    nested_field: test_msgs::msg::BasicTypes::from_native(
                        &msg.nested_field,
                    ),
                    nested_different_pkg: builtin_interfaces::msg::Time::from_native(
                        &msg.nested_different_pkg,
                    ),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                self.nested_field_no_pkg.copy_to_native(&mut msg.nested_field_no_pkg);
                self.nested_field.copy_to_native(&mut msg.nested_field);
                self.nested_different_pkg.copy_to_native(&mut msg.nested_different_pkg);
            }
        }
        impl Default for Goal {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Goal>::new();
                Goal::from_native(&msg_native)
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Result {
            pub nested_field_no_pkg: test_msgs::msg::Builtins,
            pub nested_field: test_msgs::msg::BasicTypes,
            pub nested_different_pkg: builtin_interfaces::msg::Time,
        }
        unsafe impl WrappedTypesupport for Result {
            type CStruct = test_msgs__action__NestedMessage_Result;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__NestedMessage_Result()
                }
            }
            fn create_msg() -> *mut test_msgs__action__NestedMessage_Result {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__NestedMessage_Result__create() }
                #[cfg(feature = "doc-only")]
                test_msgs__action__NestedMessage_Result__create()
            }
            fn destroy_msg(msg: *mut test_msgs__action__NestedMessage_Result) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__NestedMessage_Result__destroy(msg) };
                #[cfg(feature = "doc-only")]
                test_msgs__action__NestedMessage_Result__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Result {
                Result {
                    nested_field_no_pkg: test_msgs::msg::Builtins::from_native(
                        &msg.nested_field_no_pkg,
                    ),
                    nested_field: test_msgs::msg::BasicTypes::from_native(
                        &msg.nested_field,
                    ),
                    nested_different_pkg: builtin_interfaces::msg::Time::from_native(
                        &msg.nested_different_pkg,
                    ),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                self.nested_field_no_pkg.copy_to_native(&mut msg.nested_field_no_pkg);
                self.nested_field.copy_to_native(&mut msg.nested_field);
                self.nested_different_pkg.copy_to_native(&mut msg.nested_different_pkg);
            }
        }
        impl Default for Result {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Result>::new();
                Result::from_native(&msg_native)
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Feedback {
            pub nested_field_no_pkg: test_msgs::msg::Builtins,
            pub nested_field: test_msgs::msg::BasicTypes,
            pub nested_different_pkg: builtin_interfaces::msg::Time,
        }
        unsafe impl WrappedTypesupport for Feedback {
            type CStruct = test_msgs__action__NestedMessage_Feedback;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__NestedMessage_Feedback()
                }
            }
            fn create_msg() -> *mut test_msgs__action__NestedMessage_Feedback {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__NestedMessage_Feedback__create() }
                #[cfg(feature = "doc-only")]
                test_msgs__action__NestedMessage_Feedback__create()
            }
            fn destroy_msg(msg: *mut test_msgs__action__NestedMessage_Feedback) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__NestedMessage_Feedback__destroy(msg) };
                #[cfg(feature = "doc-only")]
                test_msgs__action__NestedMessage_Feedback__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Feedback {
                Feedback {
                    nested_field_no_pkg: test_msgs::msg::Builtins::from_native(
                        &msg.nested_field_no_pkg,
                    ),
                    nested_field: test_msgs::msg::BasicTypes::from_native(
                        &msg.nested_field,
                    ),
                    nested_different_pkg: builtin_interfaces::msg::Time::from_native(
                        &msg.nested_different_pkg,
                    ),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                self.nested_field_no_pkg.copy_to_native(&mut msg.nested_field_no_pkg);
                self.nested_field.copy_to_native(&mut msg.nested_field);
                self.nested_different_pkg.copy_to_native(&mut msg.nested_different_pkg);
            }
        }
        impl Default for Feedback {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Feedback>::new();
                Feedback::from_native(&msg_native)
            }
        }
        #[allow(non_snake_case)]
        pub mod SendGoal {
            use super::super::super::super::*;
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            pub struct Service();
            unsafe impl WrappedServiceTypeSupport for Service {
                type Request = Request;
                type Response = Response;
                fn get_ts() -> &'static rosidl_service_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_service_type_support_handle__test_msgs__action__NestedMessage_SendGoal()
                    }
                }
            }
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            #[serde(default)]
            pub struct Request {
                pub goal_id: unique_identifier_msgs::msg::UUID,
                pub goal: test_msgs::action::NestedMessage::Goal,
            }
            unsafe impl WrappedTypesupport for Request {
                type CStruct = test_msgs__action__NestedMessage_SendGoal_Request;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__NestedMessage_SendGoal_Request()
                    }
                }
                fn create_msg() -> *mut test_msgs__action__NestedMessage_SendGoal_Request {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        test_msgs__action__NestedMessage_SendGoal_Request__create()
                    }
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__NestedMessage_SendGoal_Request__create()
                }
                fn destroy_msg(
                    msg: *mut test_msgs__action__NestedMessage_SendGoal_Request,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        test_msgs__action__NestedMessage_SendGoal_Request__destroy(msg)
                    };
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__NestedMessage_SendGoal_Request__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                    Request {
                        goal_id: unique_identifier_msgs::msg::UUID::from_native(
                            &msg.goal_id,
                        ),
                        goal: test_msgs::action::NestedMessage::Goal::from_native(
                            &msg.goal,
                        ),
                    }
                }
                fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                    self.goal_id.copy_to_native(&mut msg.goal_id);
                    self.goal.copy_to_native(&mut msg.goal);
                }
            }
            impl Default for Request {
                fn default() -> Self {
                    let msg_native = WrappedNativeMsg::<Request>::new();
                    Request::from_native(&msg_native)
                }
            }
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            #[serde(default)]
            pub struct Response {
                pub accepted: bool,
                pub stamp: builtin_interfaces::msg::Time,
            }
            unsafe impl WrappedTypesupport for Response {
                type CStruct = test_msgs__action__NestedMessage_SendGoal_Response;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__NestedMessage_SendGoal_Response()
                    }
                }
                fn create_msg() -> *mut test_msgs__action__NestedMessage_SendGoal_Response {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        test_msgs__action__NestedMessage_SendGoal_Response__create()
                    }
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__NestedMessage_SendGoal_Response__create()
                }
                fn destroy_msg(
                    msg: *mut test_msgs__action__NestedMessage_SendGoal_Response,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        test_msgs__action__NestedMessage_SendGoal_Response__destroy(msg)
                    };
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__NestedMessage_SendGoal_Response__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                    Response {
                        accepted: msg.accepted,
                        stamp: builtin_interfaces::msg::Time::from_native(&msg.stamp),
                    }
                }
                fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                    msg.accepted = self.accepted;
                    self.stamp.copy_to_native(&mut msg.stamp);
                }
            }
            impl Default for Response {
                fn default() -> Self {
                    let msg_native = WrappedNativeMsg::<Response>::new();
                    Response::from_native(&msg_native)
                }
            }
        }
        #[allow(non_snake_case)]
        pub mod GetResult {
            use super::super::super::super::*;
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            pub struct Service();
            unsafe impl WrappedServiceTypeSupport for Service {
                type Request = Request;
                type Response = Response;
                fn get_ts() -> &'static rosidl_service_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_service_type_support_handle__test_msgs__action__NestedMessage_GetResult()
                    }
                }
            }
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            #[serde(default)]
            pub struct Request {
                pub goal_id: unique_identifier_msgs::msg::UUID,
            }
            unsafe impl WrappedTypesupport for Request {
                type CStruct = test_msgs__action__NestedMessage_GetResult_Request;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__NestedMessage_GetResult_Request()
                    }
                }
                fn create_msg() -> *mut test_msgs__action__NestedMessage_GetResult_Request {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        test_msgs__action__NestedMessage_GetResult_Request__create()
                    }
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__NestedMessage_GetResult_Request__create()
                }
                fn destroy_msg(
                    msg: *mut test_msgs__action__NestedMessage_GetResult_Request,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        test_msgs__action__NestedMessage_GetResult_Request__destroy(msg)
                    };
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__NestedMessage_GetResult_Request__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                    Request {
                        goal_id: unique_identifier_msgs::msg::UUID::from_native(
                            &msg.goal_id,
                        ),
                    }
                }
                fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                    self.goal_id.copy_to_native(&mut msg.goal_id);
                }
            }
            impl Default for Request {
                fn default() -> Self {
                    let msg_native = WrappedNativeMsg::<Request>::new();
                    Request::from_native(&msg_native)
                }
            }
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            #[serde(default)]
            pub struct Response {
                pub status: i8,
                pub result: test_msgs::action::NestedMessage::Result,
            }
            unsafe impl WrappedTypesupport for Response {
                type CStruct = test_msgs__action__NestedMessage_GetResult_Response;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__NestedMessage_GetResult_Response()
                    }
                }
                fn create_msg() -> *mut test_msgs__action__NestedMessage_GetResult_Response {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        test_msgs__action__NestedMessage_GetResult_Response__create()
                    }
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__NestedMessage_GetResult_Response__create()
                }
                fn destroy_msg(
                    msg: *mut test_msgs__action__NestedMessage_GetResult_Response,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        test_msgs__action__NestedMessage_GetResult_Response__destroy(msg)
                    };
                    #[cfg(feature = "doc-only")]
                    test_msgs__action__NestedMessage_GetResult_Response__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                    Response {
                        status: msg.status,
                        result: test_msgs::action::NestedMessage::Result::from_native(
                            &msg.result,
                        ),
                    }
                }
                fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                    msg.status = self.status;
                    self.result.copy_to_native(&mut msg.result);
                }
            }
            impl Default for Response {
                fn default() -> Self {
                    let msg_native = WrappedNativeMsg::<Response>::new();
                    Response::from_native(&msg_native)
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct FeedbackMessage {
            pub goal_id: unique_identifier_msgs::msg::UUID,
            pub feedback: test_msgs::action::NestedMessage::Feedback,
        }
        unsafe impl WrappedTypesupport for FeedbackMessage {
            type CStruct = test_msgs__action__NestedMessage_FeedbackMessage;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__action__NestedMessage_FeedbackMessage()
                }
            }
            fn create_msg() -> *mut test_msgs__action__NestedMessage_FeedbackMessage {
                #[cfg(not(feature = "doc-only"))]
                unsafe { test_msgs__action__NestedMessage_FeedbackMessage__create() }
                #[cfg(feature = "doc-only")]
                test_msgs__action__NestedMessage_FeedbackMessage__create()
            }
            fn destroy_msg(
                msg: *mut test_msgs__action__NestedMessage_FeedbackMessage,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    test_msgs__action__NestedMessage_FeedbackMessage__destroy(msg)
                };
                #[cfg(feature = "doc-only")]
                test_msgs__action__NestedMessage_FeedbackMessage__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> FeedbackMessage {
                FeedbackMessage {
                    goal_id: unique_identifier_msgs::msg::UUID::from_native(
                        &msg.goal_id,
                    ),
                    feedback: test_msgs::action::NestedMessage::Feedback::from_native(
                        &msg.feedback,
                    ),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                self.goal_id.copy_to_native(&mut msg.goal_id);
                self.feedback.copy_to_native(&mut msg.feedback);
            }
        }
        impl Default for FeedbackMessage {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<FeedbackMessage>::new();
                FeedbackMessage::from_native(&msg_native)
            }
        }
    }
}
pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Arrays {
        #[serde(with = "crate::msg_types::fixed_array")]
        pub bool_values: [bool; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub byte_values: [u8; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub char_values: [u8; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub float32_values: [f32; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub float64_values: [f64; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub int8_values: [i8; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub uint8_values: [u8; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub int16_values: [i16; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub uint16_values: [u16; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub int32_values: [i32; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub uint32_values: [u32; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub int64_values: [i64; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub uint64_values: [u64; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub string_values: [crate::RosString; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub basic_types_values: [test_msgs::msg::BasicTypes; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub constants_values: [test_msgs::msg::Constants; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub defaults_values: [test_msgs::msg::Defaults; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub bool_values_default: [bool; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub byte_values_default: [u8; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub char_values_default: [u8; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub float32_values_default: [f32; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub float64_values_default: [f64; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub int8_values_default: [i8; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub uint8_values_default: [u8; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub int16_values_default: [i16; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub uint16_values_default: [u16; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub int32_values_default: [i32; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub uint32_values_default: [u32; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub int64_values_default: [i64; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub uint64_values_default: [u64; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub string_values_default: [crate::RosString; 3usize],
        pub alignment_check: i32,
    }
    unsafe impl WrappedTypesupport for Arrays {
        type CStruct = test_msgs__msg__Arrays;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__Arrays()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__Arrays {
            #[cfg(not(feature = "doc-only"))] unsafe { test_msgs__msg__Arrays__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__Arrays__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__Arrays) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__Arrays__destroy(msg) };
            #[cfg(feature = "doc-only")] test_msgs__msg__Arrays__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Arrays {
            Arrays {
                bool_values: msg.bool_values,
                byte_values: msg.byte_values,
                char_values: msg.char_values,
                float32_values: msg.float32_values,
                float64_values: msg.float64_values,
                int8_values: msg.int8_values,
                uint8_values: msg.uint8_values,
                int16_values: msg.int16_values,
                uint16_values: msg.uint16_values,
                int32_values: msg.int32_values,
                uint32_values: msg.uint32_values,
                int64_values: msg.int64_values,
                uint64_values: msg.uint64_values,
                string_values: std::array::from_fn(|index| crate::RosString::from(
                    msg.string_values[index].as_bytes(),
                )),
                basic_types_values: std::array::from_fn(|index| test_msgs::msg::BasicTypes::from_native(
                    &msg.basic_types_values[index],
                )),
                constants_values: std::array::from_fn(|index| test_msgs::msg::Constants::from_native(
                    &msg.constants_values[index],
                )),
                defaults_values: std::array::from_fn(|index| test_msgs::msg::Defaults::from_native(
                    &msg.defaults_values[index],
                )),
                bool_values_default: msg.bool_values_default,
                byte_values_default: msg.byte_values_default,
                char_values_default: msg.char_values_default,
                float32_values_default: msg.float32_values_default,
                float64_values_default: msg.float64_values_default,
                int8_values_default: msg.int8_values_default,
                uint8_values_default: msg.uint8_values_default,
                int16_values_default: msg.int16_values_default,
                uint16_values_default: msg.uint16_values_default,
                int32_values_default: msg.int32_values_default,
                uint32_values_default: msg.uint32_values_default,
                int64_values_default: msg.int64_values_default,
                uint64_values_default: msg.uint64_values_default,
                string_values_default: std::array::from_fn(|index| crate::RosString::from(
                    msg.string_values_default[index].as_bytes(),
                )),
                alignment_check: msg.alignment_check,
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            assert_eq!(
                self.bool_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "bool_values",
                3usize,
            );
            msg.bool_values.copy_from_slice(&self.bool_values[..3usize]);
            assert_eq!(
                self.byte_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "byte_values",
                3usize,
            );
            msg.byte_values.copy_from_slice(&self.byte_values[..3usize]);
            assert_eq!(
                self.char_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "char_values",
                3usize,
            );
            msg.char_values.copy_from_slice(&self.char_values[..3usize]);
            assert_eq!(
                self.float32_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "float32_values",
                3usize,
            );
            msg.float32_values.copy_from_slice(&self.float32_values[..3usize]);
            assert_eq!(
                self.float64_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "float64_values",
                3usize,
            );
            msg.float64_values.copy_from_slice(&self.float64_values[..3usize]);
            assert_eq!(
                self.int8_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "int8_values",
                3usize,
            );
            msg.int8_values.copy_from_slice(&self.int8_values[..3usize]);
            assert_eq!(
                self.uint8_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "uint8_values",
                3usize,
            );
            msg.uint8_values.copy_from_slice(&self.uint8_values[..3usize]);
            assert_eq!(
                self.int16_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "int16_values",
                3usize,
            );
            msg.int16_values.copy_from_slice(&self.int16_values[..3usize]);
            assert_eq!(
                self.uint16_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "uint16_values",
                3usize,
            );
            msg.uint16_values.copy_from_slice(&self.uint16_values[..3usize]);
            assert_eq!(
                self.int32_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "int32_values",
                3usize,
            );
            msg.int32_values.copy_from_slice(&self.int32_values[..3usize]);
            assert_eq!(
                self.uint32_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "uint32_values",
                3usize,
            );
            msg.uint32_values.copy_from_slice(&self.uint32_values[..3usize]);
            assert_eq!(
                self.int64_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "int64_values",
                3usize,
            );
            msg.int64_values.copy_from_slice(&self.int64_values[..3usize]);
            assert_eq!(
                self.uint64_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "uint64_values",
                3usize,
            );
            msg.uint64_values.copy_from_slice(&self.uint64_values[..3usize]);
            assert_eq!(
                self.string_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "string_values",
                3usize,
            );
            for (t, s) in msg.string_values.iter_mut().zip(&self.string_values) {
                t.try_assign_bytes((&s).as_bytes());
            }
            assert_eq!(
                self.basic_types_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "basic_types_values",
                3usize,
            );
            for (t, s) in msg.basic_types_values.iter_mut().zip(&self.basic_types_values)
            {
                s.copy_to_native(t);
            }
            assert_eq!(
                self.constants_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "constants_values",
                3usize,
            );
            for (t, s) in msg.constants_values.iter_mut().zip(&self.constants_values) {
                s.copy_to_native(t);
            }
            assert_eq!(
                self.defaults_values.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "defaults_values",
                3usize,
            );
            for (t, s) in msg.defaults_values.iter_mut().zip(&self.defaults_values) {
                s.copy_to_native(t);
            }
            assert_eq!(
                self.bool_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "bool_values_default",
                3usize,
            );
            msg.bool_values_default.copy_from_slice(&self.bool_values_default[..3usize]);
            assert_eq!(
                self.byte_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "byte_values_default",
                3usize,
            );
            msg.byte_values_default.copy_from_slice(&self.byte_values_default[..3usize]);
            assert_eq!(
                self.char_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "char_values_default",
                3usize,
            );
            msg.char_values_default.copy_from_slice(&self.char_values_default[..3usize]);
            assert_eq!(
                self.float32_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "float32_values_default",
                3usize,
            );
            msg.float32_values_default
                .copy_from_slice(&self.float32_values_default[..3usize]);
            assert_eq!(
                self.float64_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "float64_values_default",
                3usize,
            );
            msg.float64_values_default
                .copy_from_slice(&self.float64_values_default[..3usize]);
            assert_eq!(
                self.int8_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "int8_values_default",
                3usize,
            );
            msg.int8_values_default.copy_from_slice(&self.int8_values_default[..3usize]);
            assert_eq!(
                self.uint8_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "uint8_values_default",
                3usize,
            );
            msg.uint8_values_default
                .copy_from_slice(&self.uint8_values_default[..3usize]);
            assert_eq!(
                self.int16_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "int16_values_default",
                3usize,
            );
            msg.int16_values_default
                .copy_from_slice(&self.int16_values_default[..3usize]);
            assert_eq!(
                self.uint16_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "uint16_values_default",
                3usize,
            );
            msg.uint16_values_default
                .copy_from_slice(&self.uint16_values_default[..3usize]);
            assert_eq!(
                self.int32_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "int32_values_default",
                3usize,
            );
            msg.int32_values_default
                .copy_from_slice(&self.int32_values_default[..3usize]);
            assert_eq!(
                self.uint32_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "uint32_values_default",
                3usize,
            );
            msg.uint32_values_default
                .copy_from_slice(&self.uint32_values_default[..3usize]);
            assert_eq!(
                self.int64_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "int64_values_default",
                3usize,
            );
            msg.int64_values_default
                .copy_from_slice(&self.int64_values_default[..3usize]);
            assert_eq!(
                self.uint64_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "uint64_values_default",
                3usize,
            );
            msg.uint64_values_default
                .copy_from_slice(&self.uint64_values_default[..3usize]);
            assert_eq!(
                self.string_values_default.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "string_values_default",
                3usize,
            );
            for (t, s) in msg
                .string_values_default
                .iter_mut()
                .zip(&self.string_values_default)
            {
                t.try_assign_bytes((&s).as_bytes());
            }
            msg.alignment_check = self.alignment_check;
        }
    }
    impl Default for Arrays {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Arrays>::new();
            Arrays::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct BasicTypes {
        pub bool_value: bool,
        pub byte_value: u8,
        pub char_value: u8,
        pub float32_value: f32,
        pub float64_value: f64,
        pub int8_value: i8,
        pub uint8_value: u8,
        pub int16_value: i16,
        pub uint16_value: u16,
        pub int32_value: i32,
        pub uint32_value: u32,
        pub int64_value: i64,
        pub uint64_value: u64,
    }
    unsafe impl WrappedTypesupport for BasicTypes {
        type CStruct = test_msgs__msg__BasicTypes;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__BasicTypes()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__BasicTypes {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__BasicTypes__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__BasicTypes__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__BasicTypes) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__BasicTypes__destroy(msg) };
            #[cfg(feature = "doc-only")] test_msgs__msg__BasicTypes__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> BasicTypes {
            BasicTypes {
                bool_value: msg.bool_value,
                byte_value: msg.byte_value,
                char_value: msg.char_value,
                float32_value: msg.float32_value,
                float64_value: msg.float64_value,
                int8_value: msg.int8_value,
                uint8_value: msg.uint8_value,
                int16_value: msg.int16_value,
                uint16_value: msg.uint16_value,
                int32_value: msg.int32_value,
                uint32_value: msg.uint32_value,
                int64_value: msg.int64_value,
                uint64_value: msg.uint64_value,
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            msg.bool_value = self.bool_value;
            msg.byte_value = self.byte_value;
            msg.char_value = self.char_value;
            msg.float32_value = self.float32_value;
            msg.float64_value = self.float64_value;
            msg.int8_value = self.int8_value;
            msg.uint8_value = self.uint8_value;
            msg.int16_value = self.int16_value;
            msg.uint16_value = self.uint16_value;
            msg.int32_value = self.int32_value;
            msg.uint32_value = self.uint32_value;
            msg.int64_value = self.int64_value;
            msg.uint64_value = self.uint64_value;
        }
    }
    impl Default for BasicTypes {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<BasicTypes>::new();
            BasicTypes::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct BoundedPlainSequences {
        pub bool_values: Vec<bool>,
        pub byte_values: Vec<u8>,
        pub char_values: Vec<u8>,
        pub float32_values: Vec<f32>,
        pub float64_values: Vec<f64>,
        pub int8_values: Vec<i8>,
        pub uint8_values: Vec<u8>,
        pub int16_values: Vec<i16>,
        pub uint16_values: Vec<u16>,
        pub int32_values: Vec<i32>,
        pub uint32_values: Vec<u32>,
        pub int64_values: Vec<i64>,
        pub uint64_values: Vec<u64>,
        pub basic_types_values: Vec<test_msgs::msg::BasicTypes>,
        pub constants_values: Vec<test_msgs::msg::Constants>,
        pub defaults_values: Vec<test_msgs::msg::Defaults>,
        pub bool_values_default: Vec<bool>,
        pub byte_values_default: Vec<u8>,
        pub char_values_default: Vec<u8>,
        pub float32_values_default: Vec<f32>,
        pub float64_values_default: Vec<f64>,
        pub int8_values_default: Vec<i8>,
        pub uint8_values_default: Vec<u8>,
        pub int16_values_default: Vec<i16>,
        pub uint16_values_default: Vec<u16>,
        pub int32_values_default: Vec<i32>,
        pub uint32_values_default: Vec<u32>,
        pub int64_values_default: Vec<i64>,
        pub uint64_values_default: Vec<u64>,
        pub alignment_check: i32,
    }
    unsafe impl WrappedTypesupport for BoundedPlainSequences {
        type CStruct = test_msgs__msg__BoundedPlainSequences;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__BoundedPlainSequences()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__BoundedPlainSequences {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__BoundedPlainSequences__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__BoundedPlainSequences__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__BoundedPlainSequences) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__BoundedPlainSequences__destroy(msg) };
            #[cfg(feature = "doc-only")]
            test_msgs__msg__BoundedPlainSequences__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> BoundedPlainSequences {
            BoundedPlainSequences {
                bool_values: msg.bool_values.to_vec(),
                byte_values: msg.byte_values.to_vec(),
                char_values: msg.char_values.to_vec(),
                float32_values: msg.float32_values.to_vec(),
                float64_values: msg.float64_values.to_vec(),
                int8_values: msg.int8_values.to_vec(),
                uint8_values: msg.uint8_values.to_vec(),
                int16_values: msg.int16_values.to_vec(),
                uint16_values: msg.uint16_values.to_vec(),
                int32_values: msg.int32_values.to_vec(),
                uint32_values: msg.uint32_values.to_vec(),
                int64_values: msg.int64_values.to_vec(),
                uint64_values: msg.uint64_values.to_vec(),
                basic_types_values: {
                    let mut temp = Vec::with_capacity(msg.basic_types_values.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.basic_types_values.data,
                            msg.basic_types_values.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::BasicTypes::from_native(s));
                    }
                    temp
                },
                constants_values: {
                    let mut temp = Vec::with_capacity(msg.constants_values.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.constants_values.data,
                            msg.constants_values.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::Constants::from_native(s));
                    }
                    temp
                },
                defaults_values: {
                    let mut temp = Vec::with_capacity(msg.defaults_values.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.defaults_values.data,
                            msg.defaults_values.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::Defaults::from_native(s));
                    }
                    temp
                },
                bool_values_default: msg.bool_values_default.to_vec(),
                byte_values_default: msg.byte_values_default.to_vec(),
                char_values_default: msg.char_values_default.to_vec(),
                float32_values_default: msg.float32_values_default.to_vec(),
                float64_values_default: msg.float64_values_default.to_vec(),
                int8_values_default: msg.int8_values_default.to_vec(),
                uint8_values_default: msg.uint8_values_default.to_vec(),
                int16_values_default: msg.int16_values_default.to_vec(),
                uint16_values_default: msg.uint16_values_default.to_vec(),
                int32_values_default: msg.int32_values_default.to_vec(),
                uint32_values_default: msg.uint32_values_default.to_vec(),
                int64_values_default: msg.int64_values_default.to_vec(),
                uint64_values_default: msg.uint64_values_default.to_vec(),
                alignment_check: msg.alignment_check,
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            assert!(
                self.bool_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "bool_values",
                3usize,
            );
            msg.bool_values.update(&self.bool_values);
            assert!(
                self.byte_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "byte_values",
                3usize,
            );
            msg.byte_values.update(&self.byte_values);
            assert!(
                self.char_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "char_values",
                3usize,
            );
            msg.char_values.update(&self.char_values);
            assert!(
                self.float32_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "float32_values",
                3usize,
            );
            msg.float32_values.update(&self.float32_values);
            assert!(
                self.float64_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "float64_values",
                3usize,
            );
            msg.float64_values.update(&self.float64_values);
            assert!(
                self.int8_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int8_values",
                3usize,
            );
            msg.int8_values.update(&self.int8_values);
            assert!(
                self.uint8_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint8_values",
                3usize,
            );
            msg.uint8_values.update(&self.uint8_values);
            assert!(
                self.int16_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int16_values",
                3usize,
            );
            msg.int16_values.update(&self.int16_values);
            assert!(
                self.uint16_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint16_values",
                3usize,
            );
            msg.uint16_values.update(&self.uint16_values);
            assert!(
                self.int32_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int32_values",
                3usize,
            );
            msg.int32_values.update(&self.int32_values);
            assert!(
                self.uint32_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint32_values",
                3usize,
            );
            msg.uint32_values.update(&self.uint32_values);
            assert!(
                self.int64_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int64_values",
                3usize,
            );
            msg.int64_values.update(&self.int64_values);
            assert!(
                self.uint64_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint64_values",
                3usize,
            );
            msg.uint64_values.update(&self.uint64_values);
            unsafe {
                test_msgs__msg__BasicTypes__Sequence__fini(&mut msg.basic_types_values);
                test_msgs__msg__BasicTypes__Sequence__init(
                    &mut msg.basic_types_values,
                    self.basic_types_values.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.basic_types_values.data,
                    msg.basic_types_values.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.basic_types_values) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                test_msgs__msg__Constants__Sequence__fini(&mut msg.constants_values);
                test_msgs__msg__Constants__Sequence__init(
                    &mut msg.constants_values,
                    self.constants_values.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.constants_values.data,
                    msg.constants_values.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.constants_values) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                test_msgs__msg__Defaults__Sequence__fini(&mut msg.defaults_values);
                test_msgs__msg__Defaults__Sequence__init(
                    &mut msg.defaults_values,
                    self.defaults_values.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.defaults_values.data,
                    msg.defaults_values.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.defaults_values) {
                    s.copy_to_native(t);
                }
            }
            assert!(
                self.bool_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "bool_values_default",
                3usize,
            );
            msg.bool_values_default.update(&self.bool_values_default);
            assert!(
                self.byte_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "byte_values_default",
                3usize,
            );
            msg.byte_values_default.update(&self.byte_values_default);
            assert!(
                self.char_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "char_values_default",
                3usize,
            );
            msg.char_values_default.update(&self.char_values_default);
            assert!(
                self.float32_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "float32_values_default",
                3usize,
            );
            msg.float32_values_default.update(&self.float32_values_default);
            assert!(
                self.float64_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "float64_values_default",
                3usize,
            );
            msg.float64_values_default.update(&self.float64_values_default);
            assert!(
                self.int8_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int8_values_default",
                3usize,
            );
            msg.int8_values_default.update(&self.int8_values_default);
            assert!(
                self.uint8_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint8_values_default",
                3usize,
            );
            msg.uint8_values_default.update(&self.uint8_values_default);
            assert!(
                self.int16_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int16_values_default",
                3usize,
            );
            msg.int16_values_default.update(&self.int16_values_default);
            assert!(
                self.uint16_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint16_values_default",
                3usize,
            );
            msg.uint16_values_default.update(&self.uint16_values_default);
            assert!(
                self.int32_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int32_values_default",
                3usize,
            );
            msg.int32_values_default.update(&self.int32_values_default);
            assert!(
                self.uint32_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint32_values_default",
                3usize,
            );
            msg.uint32_values_default.update(&self.uint32_values_default);
            assert!(
                self.int64_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int64_values_default",
                3usize,
            );
            msg.int64_values_default.update(&self.int64_values_default);
            assert!(
                self.uint64_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint64_values_default",
                3usize,
            );
            msg.uint64_values_default.update(&self.uint64_values_default);
            msg.alignment_check = self.alignment_check;
        }
    }
    impl Default for BoundedPlainSequences {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<BoundedPlainSequences>::new();
            BoundedPlainSequences::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct BoundedSequences {
        pub bool_values: Vec<bool>,
        pub byte_values: Vec<u8>,
        pub char_values: Vec<u8>,
        pub float32_values: Vec<f32>,
        pub float64_values: Vec<f64>,
        pub int8_values: Vec<i8>,
        pub uint8_values: Vec<u8>,
        pub int16_values: Vec<i16>,
        pub uint16_values: Vec<u16>,
        pub int32_values: Vec<i32>,
        pub uint32_values: Vec<u32>,
        pub int64_values: Vec<i64>,
        pub uint64_values: Vec<u64>,
        pub string_values: Vec<crate::RosString>,
        pub basic_types_values: Vec<test_msgs::msg::BasicTypes>,
        pub constants_values: Vec<test_msgs::msg::Constants>,
        pub defaults_values: Vec<test_msgs::msg::Defaults>,
        pub bool_values_default: Vec<bool>,
        pub byte_values_default: Vec<u8>,
        pub char_values_default: Vec<u8>,
        pub float32_values_default: Vec<f32>,
        pub float64_values_default: Vec<f64>,
        pub int8_values_default: Vec<i8>,
        pub uint8_values_default: Vec<u8>,
        pub int16_values_default: Vec<i16>,
        pub uint16_values_default: Vec<u16>,
        pub int32_values_default: Vec<i32>,
        pub uint32_values_default: Vec<u32>,
        pub int64_values_default: Vec<i64>,
        pub uint64_values_default: Vec<u64>,
        pub string_values_default: Vec<crate::RosString>,
        pub alignment_check: i32,
    }
    unsafe impl WrappedTypesupport for BoundedSequences {
        type CStruct = test_msgs__msg__BoundedSequences;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__BoundedSequences()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__BoundedSequences {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__BoundedSequences__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__BoundedSequences__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__BoundedSequences) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__BoundedSequences__destroy(msg) };
            #[cfg(feature = "doc-only")] test_msgs__msg__BoundedSequences__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> BoundedSequences {
            BoundedSequences {
                bool_values: msg.bool_values.to_vec(),
                byte_values: msg.byte_values.to_vec(),
                char_values: msg.char_values.to_vec(),
                float32_values: msg.float32_values.to_vec(),
                float64_values: msg.float64_values.to_vec(),
                int8_values: msg.int8_values.to_vec(),
                uint8_values: msg.uint8_values.to_vec(),
                int16_values: msg.int16_values.to_vec(),
                uint16_values: msg.uint16_values.to_vec(),
                int32_values: msg.int32_values.to_vec(),
                uint32_values: msg.uint32_values.to_vec(),
                int64_values: msg.int64_values.to_vec(),
                uint64_values: msg.uint64_values.to_vec(),
                string_values: msg.string_values.to_bytes_vec::<crate::RosString>(),
                basic_types_values: {
                    let mut temp = Vec::with_capacity(msg.basic_types_values.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.basic_types_values.data,
                            msg.basic_types_values.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::BasicTypes::from_native(s));
                    }
                    temp
                },
                constants_values: {
                    let mut temp = Vec::with_capacity(msg.constants_values.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.constants_values.data,
                            msg.constants_values.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::Constants::from_native(s));
                    }
                    temp
                },
                defaults_values: {
                    let mut temp = Vec::with_capacity(msg.defaults_values.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.defaults_values.data,
                            msg.defaults_values.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::Defaults::from_native(s));
                    }
                    temp
                },
                bool_values_default: msg.bool_values_default.to_vec(),
                byte_values_default: msg.byte_values_default.to_vec(),
                char_values_default: msg.char_values_default.to_vec(),
                float32_values_default: msg.float32_values_default.to_vec(),
                float64_values_default: msg.float64_values_default.to_vec(),
                int8_values_default: msg.int8_values_default.to_vec(),
                uint8_values_default: msg.uint8_values_default.to_vec(),
                int16_values_default: msg.int16_values_default.to_vec(),
                uint16_values_default: msg.uint16_values_default.to_vec(),
                int32_values_default: msg.int32_values_default.to_vec(),
                uint32_values_default: msg.uint32_values_default.to_vec(),
                int64_values_default: msg.int64_values_default.to_vec(),
                uint64_values_default: msg.uint64_values_default.to_vec(),
                string_values_default: msg
                    .string_values_default
                    .to_bytes_vec::<crate::RosString>(),
                alignment_check: msg.alignment_check,
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            assert!(
                self.bool_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "bool_values",
                3usize,
            );
            msg.bool_values.update(&self.bool_values);
            assert!(
                self.byte_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "byte_values",
                3usize,
            );
            msg.byte_values.update(&self.byte_values);
            assert!(
                self.char_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "char_values",
                3usize,
            );
            msg.char_values.update(&self.char_values);
            assert!(
                self.float32_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "float32_values",
                3usize,
            );
            msg.float32_values.update(&self.float32_values);
            assert!(
                self.float64_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "float64_values",
                3usize,
            );
            msg.float64_values.update(&self.float64_values);
            assert!(
                self.int8_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int8_values",
                3usize,
            );
            msg.int8_values.update(&self.int8_values);
            assert!(
                self.uint8_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint8_values",
                3usize,
            );
            msg.uint8_values.update(&self.uint8_values);
            assert!(
                self.int16_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int16_values",
                3usize,
            );
            msg.int16_values.update(&self.int16_values);
            assert!(
                self.uint16_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint16_values",
                3usize,
            );
            msg.uint16_values.update(&self.uint16_values);
            assert!(
                self.int32_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int32_values",
                3usize,
            );
            msg.int32_values.update(&self.int32_values);
            assert!(
                self.uint32_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint32_values",
                3usize,
            );
            msg.uint32_values.update(&self.uint32_values);
            assert!(
                self.int64_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int64_values",
                3usize,
            );
            msg.int64_values.update(&self.int64_values);
            assert!(
                self.uint64_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint64_values",
                3usize,
            );
            msg.uint64_values.update(&self.uint64_values);
            assert!(
                self.string_values.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "string_values",
                3usize,
            );
            msg.string_values.try_update_bytes(&self.string_values);
            unsafe {
                test_msgs__msg__BasicTypes__Sequence__fini(&mut msg.basic_types_values);
                test_msgs__msg__BasicTypes__Sequence__init(
                    &mut msg.basic_types_values,
                    self.basic_types_values.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.basic_types_values.data,
                    msg.basic_types_values.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.basic_types_values) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                test_msgs__msg__Constants__Sequence__fini(&mut msg.constants_values);
                test_msgs__msg__Constants__Sequence__init(
                    &mut msg.constants_values,
                    self.constants_values.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.constants_values.data,
                    msg.constants_values.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.constants_values) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                test_msgs__msg__Defaults__Sequence__fini(&mut msg.defaults_values);
                test_msgs__msg__Defaults__Sequence__init(
                    &mut msg.defaults_values,
                    self.defaults_values.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.defaults_values.data,
                    msg.defaults_values.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.defaults_values) {
                    s.copy_to_native(t);
                }
            }
            assert!(
                self.bool_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "bool_values_default",
                3usize,
            );
            msg.bool_values_default.update(&self.bool_values_default);
            assert!(
                self.byte_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "byte_values_default",
                3usize,
            );
            msg.byte_values_default.update(&self.byte_values_default);
            assert!(
                self.char_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "char_values_default",
                3usize,
            );
            msg.char_values_default.update(&self.char_values_default);
            assert!(
                self.float32_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "float32_values_default",
                3usize,
            );
            msg.float32_values_default.update(&self.float32_values_default);
            assert!(
                self.float64_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "float64_values_default",
                3usize,
            );
            msg.float64_values_default.update(&self.float64_values_default);
            assert!(
                self.int8_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int8_values_default",
                3usize,
            );
            msg.int8_values_default.update(&self.int8_values_default);
            assert!(
                self.uint8_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint8_values_default",
                3usize,
            );
            msg.uint8_values_default.update(&self.uint8_values_default);
            assert!(
                self.int16_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int16_values_default",
                3usize,
            );
            msg.int16_values_default.update(&self.int16_values_default);
            assert!(
                self.uint16_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint16_values_default",
                3usize,
            );
            msg.uint16_values_default.update(&self.uint16_values_default);
            assert!(
                self.int32_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int32_values_default",
                3usize,
            );
            msg.int32_values_default.update(&self.int32_values_default);
            assert!(
                self.uint32_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint32_values_default",
                3usize,
            );
            msg.uint32_values_default.update(&self.uint32_values_default);
            assert!(
                self.int64_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "int64_values_default",
                3usize,
            );
            msg.int64_values_default.update(&self.int64_values_default);
            assert!(
                self.uint64_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "uint64_values_default",
                3usize,
            );
            msg.uint64_values_default.update(&self.uint64_values_default);
            assert!(
                self.string_values_default.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "string_values_default",
                3usize,
            );
            msg.string_values_default.try_update_bytes(&self.string_values_default);
            msg.alignment_check = self.alignment_check;
        }
    }
    impl Default for BoundedSequences {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<BoundedSequences>::new();
            BoundedSequences::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Builtins {
        pub duration_value: builtin_interfaces::msg::Duration,
        pub time_value: builtin_interfaces::msg::Time,
    }
    unsafe impl WrappedTypesupport for Builtins {
        type CStruct = test_msgs__msg__Builtins;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__Builtins()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__Builtins {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__Builtins__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__Builtins__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__Builtins) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__Builtins__destroy(msg) };
            #[cfg(feature = "doc-only")] test_msgs__msg__Builtins__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Builtins {
            Builtins {
                duration_value: builtin_interfaces::msg::Duration::from_native(
                    &msg.duration_value,
                ),
                time_value: builtin_interfaces::msg::Time::from_native(&msg.time_value),
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            self.duration_value.copy_to_native(&mut msg.duration_value);
            self.time_value.copy_to_native(&mut msg.time_value);
        }
    }
    impl Default for Builtins {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Builtins>::new();
            Builtins::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Constants {}
    unsafe impl WrappedTypesupport for Constants {
        type CStruct = test_msgs__msg__Constants;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__Constants()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__Constants {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__Constants__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__Constants__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__Constants) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__Constants__destroy(msg) };
            #[cfg(feature = "doc-only")] test_msgs__msg__Constants__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Constants {
            Constants {}
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {}
    }
    impl Default for Constants {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Constants>::new();
            Constants::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl Constants {
        pub const BOOL_CONST: bool = test_msgs__msg__Constants__BOOL_CONST;
        pub const BYTE_CONST: _bindgen_ty_147 = test_msgs__msg__Constants__BYTE_CONST;
        pub const CHAR_CONST: _bindgen_ty_148 = test_msgs__msg__Constants__CHAR_CONST;
        pub const FLOAT32_CONST: f32 = test_msgs__msg__Constants__FLOAT32_CONST;
        pub const FLOAT64_CONST: f64 = test_msgs__msg__Constants__FLOAT64_CONST;
        pub const INT16_CONST: _bindgen_ty_151 = test_msgs__msg__Constants__INT16_CONST;
        pub const INT32_CONST: _bindgen_ty_153 = test_msgs__msg__Constants__INT32_CONST;
        pub const INT64_CONST: _bindgen_ty_155 = test_msgs__msg__Constants__INT64_CONST;
        pub const INT8_CONST: _bindgen_ty_149 = test_msgs__msg__Constants__INT8_CONST;
        pub const UINT16_CONST: _bindgen_ty_152 = test_msgs__msg__Constants__UINT16_CONST;
        pub const UINT32_CONST: _bindgen_ty_154 = test_msgs__msg__Constants__UINT32_CONST;
        pub const UINT64_CONST: _bindgen_ty_156 = test_msgs__msg__Constants__UINT64_CONST;
        pub const UINT8_CONST: _bindgen_ty_150 = test_msgs__msg__Constants__UINT8_CONST;
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Defaults {
        pub bool_value: bool,
        pub byte_value: u8,
        pub char_value: u8,
        pub float32_value: f32,
        pub float64_value: f64,
        pub int8_value: i8,
        pub uint8_value: u8,
        pub int16_value: i16,
        pub uint16_value: u16,
        pub int32_value: i32,
        pub uint32_value: u32,
        pub int64_value: i64,
        pub uint64_value: u64,
    }
    unsafe impl WrappedTypesupport for Defaults {
        type CStruct = test_msgs__msg__Defaults;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__Defaults()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__Defaults {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__Defaults__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__Defaults__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__Defaults) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__Defaults__destroy(msg) };
            #[cfg(feature = "doc-only")] test_msgs__msg__Defaults__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Defaults {
            Defaults {
                bool_value: msg.bool_value,
                byte_value: msg.byte_value,
                char_value: msg.char_value,
                float32_value: msg.float32_value,
                float64_value: msg.float64_value,
                int8_value: msg.int8_value,
                uint8_value: msg.uint8_value,
                int16_value: msg.int16_value,
                uint16_value: msg.uint16_value,
                int32_value: msg.int32_value,
                uint32_value: msg.uint32_value,
                int64_value: msg.int64_value,
                uint64_value: msg.uint64_value,
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            msg.bool_value = self.bool_value;
            msg.byte_value = self.byte_value;
            msg.char_value = self.char_value;
            msg.float32_value = self.float32_value;
            msg.float64_value = self.float64_value;
            msg.int8_value = self.int8_value;
            msg.uint8_value = self.uint8_value;
            msg.int16_value = self.int16_value;
            msg.uint16_value = self.uint16_value;
            msg.int32_value = self.int32_value;
            msg.uint32_value = self.uint32_value;
            msg.int64_value = self.int64_value;
            msg.uint64_value = self.uint64_value;
        }
    }
    impl Default for Defaults {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Defaults>::new();
            Defaults::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Empty {}
    unsafe impl WrappedTypesupport for Empty {
        type CStruct = test_msgs__msg__Empty;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__Empty()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__Empty {
            #[cfg(not(feature = "doc-only"))] unsafe { test_msgs__msg__Empty__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__Empty__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__Empty) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__Empty__destroy(msg) };
            #[cfg(feature = "doc-only")] test_msgs__msg__Empty__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Empty {
            Empty {}
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {}
    }
    impl Default for Empty {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Empty>::new();
            Empty::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct MultiNested {
        #[serde(with = "crate::msg_types::fixed_array")]
        pub array_of_arrays: [test_msgs::msg::Arrays; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub array_of_bounded_sequences: [test_msgs::msg::BoundedSequences; 3usize],
        #[serde(with = "crate::msg_types::fixed_array")]
        pub array_of_unbounded_sequences: [test_msgs::msg::UnboundedSequences; 3usize],
        pub bounded_sequence_of_arrays: Vec<test_msgs::msg::Arrays>,
        pub bounded_sequence_of_bounded_sequences: Vec<test_msgs::msg::BoundedSequences>,
        pub bounded_sequence_of_unbounded_sequences: Vec<
            test_msgs::msg::UnboundedSequences,
        >,
        pub unbounded_sequence_of_arrays: Vec<test_msgs::msg::Arrays>,
        pub unbounded_sequence_of_bounded_sequences: Vec<
            test_msgs::msg::BoundedSequences,
        >,
        pub unbounded_sequence_of_unbounded_sequences: Vec<
            test_msgs::msg::UnboundedSequences,
        >,
    }
    unsafe impl WrappedTypesupport for MultiNested {
        type CStruct = test_msgs__msg__MultiNested;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__MultiNested()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__MultiNested {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__MultiNested__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__MultiNested__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__MultiNested) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__MultiNested__destroy(msg) };
            #[cfg(feature = "doc-only")] test_msgs__msg__MultiNested__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> MultiNested {
            MultiNested {
                array_of_arrays: std::array::from_fn(|index| test_msgs::msg::Arrays::from_native(
                    &msg.array_of_arrays[index],
                )),
                array_of_bounded_sequences: std::array::from_fn(|index| test_msgs::msg::BoundedSequences::from_native(
                    &msg.array_of_bounded_sequences[index],
                )),
                array_of_unbounded_sequences: std::array::from_fn(|index| test_msgs::msg::UnboundedSequences::from_native(
                    &msg.array_of_unbounded_sequences[index],
                )),
                bounded_sequence_of_arrays: {
                    let mut temp = Vec::with_capacity(
                        msg.bounded_sequence_of_arrays.size,
                    );
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.bounded_sequence_of_arrays.data,
                            msg.bounded_sequence_of_arrays.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::Arrays::from_native(s));
                    }
                    temp
                },
                bounded_sequence_of_bounded_sequences: {
                    let mut temp = Vec::with_capacity(
                        msg.bounded_sequence_of_bounded_sequences.size,
                    );
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.bounded_sequence_of_bounded_sequences.data,
                            msg.bounded_sequence_of_bounded_sequences.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::BoundedSequences::from_native(s));
                    }
                    temp
                },
                bounded_sequence_of_unbounded_sequences: {
                    let mut temp = Vec::with_capacity(
                        msg.bounded_sequence_of_unbounded_sequences.size,
                    );
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.bounded_sequence_of_unbounded_sequences.data,
                            msg.bounded_sequence_of_unbounded_sequences.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::UnboundedSequences::from_native(s));
                    }
                    temp
                },
                unbounded_sequence_of_arrays: {
                    let mut temp = Vec::with_capacity(
                        msg.unbounded_sequence_of_arrays.size,
                    );
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.unbounded_sequence_of_arrays.data,
                            msg.unbounded_sequence_of_arrays.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::Arrays::from_native(s));
                    }
                    temp
                },
                unbounded_sequence_of_bounded_sequences: {
                    let mut temp = Vec::with_capacity(
                        msg.unbounded_sequence_of_bounded_sequences.size,
                    );
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.unbounded_sequence_of_bounded_sequences.data,
                            msg.unbounded_sequence_of_bounded_sequences.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::BoundedSequences::from_native(s));
                    }
                    temp
                },
                unbounded_sequence_of_unbounded_sequences: {
                    let mut temp = Vec::with_capacity(
                        msg.unbounded_sequence_of_unbounded_sequences.size,
                    );
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.unbounded_sequence_of_unbounded_sequences.data,
                            msg.unbounded_sequence_of_unbounded_sequences.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::UnboundedSequences::from_native(s));
                    }
                    temp
                },
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            assert_eq!(
                self.array_of_arrays.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "array_of_arrays",
                3usize,
            );
            for (t, s) in msg.array_of_arrays.iter_mut().zip(&self.array_of_arrays) {
                s.copy_to_native(t);
            }
            assert_eq!(
                self.array_of_bounded_sequences.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "array_of_bounded_sequences",
                3usize,
            );
            for (t, s) in msg
                .array_of_bounded_sequences
                .iter_mut()
                .zip(&self.array_of_bounded_sequences)
            {
                s.copy_to_native(t);
            }
            assert_eq!(
                self.array_of_unbounded_sequences.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "array_of_unbounded_sequences",
                3usize,
            );
            for (t, s) in msg
                .array_of_unbounded_sequences
                .iter_mut()
                .zip(&self.array_of_unbounded_sequences)
            {
                s.copy_to_native(t);
            }
            unsafe {
                test_msgs__msg__Arrays__Sequence__fini(
                    &mut msg.bounded_sequence_of_arrays,
                );
                test_msgs__msg__Arrays__Sequence__init(
                    &mut msg.bounded_sequence_of_arrays,
                    self.bounded_sequence_of_arrays.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.bounded_sequence_of_arrays.data,
                    msg.bounded_sequence_of_arrays.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.bounded_sequence_of_arrays) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                test_msgs__msg__BoundedSequences__Sequence__fini(
                    &mut msg.bounded_sequence_of_bounded_sequences,
                );
                test_msgs__msg__BoundedSequences__Sequence__init(
                    &mut msg.bounded_sequence_of_bounded_sequences,
                    self.bounded_sequence_of_bounded_sequences.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.bounded_sequence_of_bounded_sequences.data,
                    msg.bounded_sequence_of_bounded_sequences.size,
                );
                for (t, s) in slice
                    .iter_mut()
                    .zip(&self.bounded_sequence_of_bounded_sequences)
                {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                test_msgs__msg__UnboundedSequences__Sequence__fini(
                    &mut msg.bounded_sequence_of_unbounded_sequences,
                );
                test_msgs__msg__UnboundedSequences__Sequence__init(
                    &mut msg.bounded_sequence_of_unbounded_sequences,
                    self.bounded_sequence_of_unbounded_sequences.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.bounded_sequence_of_unbounded_sequences.data,
                    msg.bounded_sequence_of_unbounded_sequences.size,
                );
                for (t, s) in slice
                    .iter_mut()
                    .zip(&self.bounded_sequence_of_unbounded_sequences)
                {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                test_msgs__msg__Arrays__Sequence__fini(
                    &mut msg.unbounded_sequence_of_arrays,
                );
                test_msgs__msg__Arrays__Sequence__init(
                    &mut msg.unbounded_sequence_of_arrays,
                    self.unbounded_sequence_of_arrays.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.unbounded_sequence_of_arrays.data,
                    msg.unbounded_sequence_of_arrays.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.unbounded_sequence_of_arrays) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                test_msgs__msg__BoundedSequences__Sequence__fini(
                    &mut msg.unbounded_sequence_of_bounded_sequences,
                );
                test_msgs__msg__BoundedSequences__Sequence__init(
                    &mut msg.unbounded_sequence_of_bounded_sequences,
                    self.unbounded_sequence_of_bounded_sequences.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.unbounded_sequence_of_bounded_sequences.data,
                    msg.unbounded_sequence_of_bounded_sequences.size,
                );
                for (t, s) in slice
                    .iter_mut()
                    .zip(&self.unbounded_sequence_of_bounded_sequences)
                {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                test_msgs__msg__UnboundedSequences__Sequence__fini(
                    &mut msg.unbounded_sequence_of_unbounded_sequences,
                );
                test_msgs__msg__UnboundedSequences__Sequence__init(
                    &mut msg.unbounded_sequence_of_unbounded_sequences,
                    self.unbounded_sequence_of_unbounded_sequences.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.unbounded_sequence_of_unbounded_sequences.data,
                    msg.unbounded_sequence_of_unbounded_sequences.size,
                );
                for (t, s) in slice
                    .iter_mut()
                    .zip(&self.unbounded_sequence_of_unbounded_sequences)
                {
                    s.copy_to_native(t);
                }
            }
        }
    }
    impl Default for MultiNested {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<MultiNested>::new();
            MultiNested::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Nested {
        pub basic_types_value: test_msgs::msg::BasicTypes,
    }
    unsafe impl WrappedTypesupport for Nested {
        type CStruct = test_msgs__msg__Nested;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__Nested()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__Nested {
            #[cfg(not(feature = "doc-only"))] unsafe { test_msgs__msg__Nested__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__Nested__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__Nested) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__Nested__destroy(msg) };
            #[cfg(feature = "doc-only")] test_msgs__msg__Nested__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Nested {
            Nested {
                basic_types_value: test_msgs::msg::BasicTypes::from_native(
                    &msg.basic_types_value,
                ),
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            self.basic_types_value.copy_to_native(&mut msg.basic_types_value);
        }
    }
    impl Default for Nested {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Nested>::new();
            Nested::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Strings {
        pub string_value: crate::RosString,
        pub string_value_default1: crate::RosString,
        pub string_value_default2: crate::RosString,
        pub string_value_default3: crate::RosString,
        pub string_value_default4: crate::RosString,
        pub string_value_default5: crate::RosString,
        pub bounded_string_value: crate::RosString,
        pub bounded_string_value_default1: crate::RosString,
        pub bounded_string_value_default2: crate::RosString,
        pub bounded_string_value_default3: crate::RosString,
        pub bounded_string_value_default4: crate::RosString,
        pub bounded_string_value_default5: crate::RosString,
    }
    unsafe impl WrappedTypesupport for Strings {
        type CStruct = test_msgs__msg__Strings;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__Strings()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__Strings {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__Strings__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__Strings__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__Strings) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__Strings__destroy(msg) };
            #[cfg(feature = "doc-only")] test_msgs__msg__Strings__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Strings {
            Strings {
                string_value: crate::RosString::from(msg.string_value.as_bytes()),
                string_value_default1: crate::RosString::from(
                    msg.string_value_default1.as_bytes(),
                ),
                string_value_default2: crate::RosString::from(
                    msg.string_value_default2.as_bytes(),
                ),
                string_value_default3: crate::RosString::from(
                    msg.string_value_default3.as_bytes(),
                ),
                string_value_default4: crate::RosString::from(
                    msg.string_value_default4.as_bytes(),
                ),
                string_value_default5: crate::RosString::from(
                    msg.string_value_default5.as_bytes(),
                ),
                bounded_string_value: crate::RosString::from(
                    msg.bounded_string_value.as_bytes(),
                ),
                bounded_string_value_default1: crate::RosString::from(
                    msg.bounded_string_value_default1.as_bytes(),
                ),
                bounded_string_value_default2: crate::RosString::from(
                    msg.bounded_string_value_default2.as_bytes(),
                ),
                bounded_string_value_default3: crate::RosString::from(
                    msg.bounded_string_value_default3.as_bytes(),
                ),
                bounded_string_value_default4: crate::RosString::from(
                    msg.bounded_string_value_default4.as_bytes(),
                ),
                bounded_string_value_default5: crate::RosString::from(
                    msg.bounded_string_value_default5.as_bytes(),
                ),
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            msg.string_value.try_assign_bytes((&self.string_value).as_bytes());
            msg.string_value_default1
                .try_assign_bytes((&self.string_value_default1).as_bytes());
            msg.string_value_default2
                .try_assign_bytes((&self.string_value_default2).as_bytes());
            msg.string_value_default3
                .try_assign_bytes((&self.string_value_default3).as_bytes());
            msg.string_value_default4
                .try_assign_bytes((&self.string_value_default4).as_bytes());
            msg.string_value_default5
                .try_assign_bytes((&self.string_value_default5).as_bytes());
            msg.bounded_string_value
                .try_assign_bytes((&self.bounded_string_value).as_bytes());
            msg.bounded_string_value_default1
                .try_assign_bytes((&self.bounded_string_value_default1).as_bytes());
            msg.bounded_string_value_default2
                .try_assign_bytes((&self.bounded_string_value_default2).as_bytes());
            msg.bounded_string_value_default3
                .try_assign_bytes((&self.bounded_string_value_default3).as_bytes());
            msg.bounded_string_value_default4
                .try_assign_bytes((&self.bounded_string_value_default4).as_bytes());
            msg.bounded_string_value_default5
                .try_assign_bytes((&self.bounded_string_value_default5).as_bytes());
        }
    }
    impl Default for Strings {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Strings>::new();
            Strings::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl Strings {
        pub const STRING_CONST: &[u8; 13usize] = test_msgs__msg__Strings__STRING_CONST;
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct UnboundedSequences {
        pub bool_values: Vec<bool>,
        pub byte_values: Vec<u8>,
        pub char_values: Vec<u8>,
        pub float32_values: Vec<f32>,
        pub float64_values: Vec<f64>,
        pub int8_values: Vec<i8>,
        pub uint8_values: Vec<u8>,
        pub int16_values: Vec<i16>,
        pub uint16_values: Vec<u16>,
        pub int32_values: Vec<i32>,
        pub uint32_values: Vec<u32>,
        pub int64_values: Vec<i64>,
        pub uint64_values: Vec<u64>,
        pub string_values: Vec<crate::RosString>,
        pub basic_types_values: Vec<test_msgs::msg::BasicTypes>,
        pub constants_values: Vec<test_msgs::msg::Constants>,
        pub defaults_values: Vec<test_msgs::msg::Defaults>,
        pub bool_values_default: Vec<bool>,
        pub byte_values_default: Vec<u8>,
        pub char_values_default: Vec<u8>,
        pub float32_values_default: Vec<f32>,
        pub float64_values_default: Vec<f64>,
        pub int8_values_default: Vec<i8>,
        pub uint8_values_default: Vec<u8>,
        pub int16_values_default: Vec<i16>,
        pub uint16_values_default: Vec<u16>,
        pub int32_values_default: Vec<i32>,
        pub uint32_values_default: Vec<u32>,
        pub int64_values_default: Vec<i64>,
        pub uint64_values_default: Vec<u64>,
        pub string_values_default: Vec<crate::RosString>,
        pub alignment_check: i32,
    }
    unsafe impl WrappedTypesupport for UnboundedSequences {
        type CStruct = test_msgs__msg__UnboundedSequences;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__UnboundedSequences()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__UnboundedSequences {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__UnboundedSequences__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__UnboundedSequences__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__UnboundedSequences) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__UnboundedSequences__destroy(msg) };
            #[cfg(feature = "doc-only")] test_msgs__msg__UnboundedSequences__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> UnboundedSequences {
            UnboundedSequences {
                bool_values: msg.bool_values.to_vec(),
                byte_values: msg.byte_values.to_vec(),
                char_values: msg.char_values.to_vec(),
                float32_values: msg.float32_values.to_vec(),
                float64_values: msg.float64_values.to_vec(),
                int8_values: msg.int8_values.to_vec(),
                uint8_values: msg.uint8_values.to_vec(),
                int16_values: msg.int16_values.to_vec(),
                uint16_values: msg.uint16_values.to_vec(),
                int32_values: msg.int32_values.to_vec(),
                uint32_values: msg.uint32_values.to_vec(),
                int64_values: msg.int64_values.to_vec(),
                uint64_values: msg.uint64_values.to_vec(),
                string_values: msg.string_values.to_bytes_vec::<crate::RosString>(),
                basic_types_values: {
                    let mut temp = Vec::with_capacity(msg.basic_types_values.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.basic_types_values.data,
                            msg.basic_types_values.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::BasicTypes::from_native(s));
                    }
                    temp
                },
                constants_values: {
                    let mut temp = Vec::with_capacity(msg.constants_values.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.constants_values.data,
                            msg.constants_values.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::Constants::from_native(s));
                    }
                    temp
                },
                defaults_values: {
                    let mut temp = Vec::with_capacity(msg.defaults_values.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.defaults_values.data,
                            msg.defaults_values.size,
                        )
                    };
                    for s in slice {
                        temp.push(test_msgs::msg::Defaults::from_native(s));
                    }
                    temp
                },
                bool_values_default: msg.bool_values_default.to_vec(),
                byte_values_default: msg.byte_values_default.to_vec(),
                char_values_default: msg.char_values_default.to_vec(),
                float32_values_default: msg.float32_values_default.to_vec(),
                float64_values_default: msg.float64_values_default.to_vec(),
                int8_values_default: msg.int8_values_default.to_vec(),
                uint8_values_default: msg.uint8_values_default.to_vec(),
                int16_values_default: msg.int16_values_default.to_vec(),
                uint16_values_default: msg.uint16_values_default.to_vec(),
                int32_values_default: msg.int32_values_default.to_vec(),
                uint32_values_default: msg.uint32_values_default.to_vec(),
                int64_values_default: msg.int64_values_default.to_vec(),
                uint64_values_default: msg.uint64_values_default.to_vec(),
                string_values_default: msg
                    .string_values_default
                    .to_bytes_vec::<crate::RosString>(),
                alignment_check: msg.alignment_check,
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            msg.bool_values.update(&self.bool_values);
            msg.byte_values.update(&self.byte_values);
            msg.char_values.update(&self.char_values);
            msg.float32_values.update(&self.float32_values);
            msg.float64_values.update(&self.float64_values);
            msg.int8_values.update(&self.int8_values);
            msg.uint8_values.update(&self.uint8_values);
            msg.int16_values.update(&self.int16_values);
            msg.uint16_values.update(&self.uint16_values);
            msg.int32_values.update(&self.int32_values);
            msg.uint32_values.update(&self.uint32_values);
            msg.int64_values.update(&self.int64_values);
            msg.uint64_values.update(&self.uint64_values);
            msg.string_values.try_update_bytes(&self.string_values);
            unsafe {
                test_msgs__msg__BasicTypes__Sequence__fini(&mut msg.basic_types_values);
                test_msgs__msg__BasicTypes__Sequence__init(
                    &mut msg.basic_types_values,
                    self.basic_types_values.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.basic_types_values.data,
                    msg.basic_types_values.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.basic_types_values) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                test_msgs__msg__Constants__Sequence__fini(&mut msg.constants_values);
                test_msgs__msg__Constants__Sequence__init(
                    &mut msg.constants_values,
                    self.constants_values.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.constants_values.data,
                    msg.constants_values.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.constants_values) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                test_msgs__msg__Defaults__Sequence__fini(&mut msg.defaults_values);
                test_msgs__msg__Defaults__Sequence__init(
                    &mut msg.defaults_values,
                    self.defaults_values.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.defaults_values.data,
                    msg.defaults_values.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.defaults_values) {
                    s.copy_to_native(t);
                }
            }
            msg.bool_values_default.update(&self.bool_values_default);
            msg.byte_values_default.update(&self.byte_values_default);
            msg.char_values_default.update(&self.char_values_default);
            msg.float32_values_default.update(&self.float32_values_default);
            msg.float64_values_default.update(&self.float64_values_default);
            msg.int8_values_default.update(&self.int8_values_default);
            msg.uint8_values_default.update(&self.uint8_values_default);
            msg.int16_values_default.update(&self.int16_values_default);
            msg.uint16_values_default.update(&self.uint16_values_default);
            msg.int32_values_default.update(&self.int32_values_default);
            msg.uint32_values_default.update(&self.uint32_values_default);
            msg.int64_values_default.update(&self.int64_values_default);
            msg.uint64_values_default.update(&self.uint64_values_default);
            msg.string_values_default.try_update_bytes(&self.string_values_default);
            msg.alignment_check = self.alignment_check;
        }
    }
    impl Default for UnboundedSequences {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<UnboundedSequences>::new();
            UnboundedSequences::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct WStrings {
        pub wstring_value: crate::RosWString,
        pub wstring_value_default1: crate::RosWString,
        pub wstring_value_default2: crate::RosWString,
        pub wstring_value_default3: crate::RosWString,
        #[serde(with = "crate::msg_types::fixed_array")]
        pub array_of_wstrings: [crate::RosWString; 3usize],
        pub bounded_sequence_of_wstrings: Vec<crate::RosWString>,
        pub unbounded_sequence_of_wstrings: Vec<crate::RosWString>,
    }
    unsafe impl WrappedTypesupport for WStrings {
        type CStruct = test_msgs__msg__WStrings;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__test_msgs__msg__WStrings()
            }
        }
        fn create_msg() -> *mut test_msgs__msg__WStrings {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__WStrings__create() }
            #[cfg(feature = "doc-only")] test_msgs__msg__WStrings__create()
        }
        fn destroy_msg(msg: *mut test_msgs__msg__WStrings) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { test_msgs__msg__WStrings__destroy(msg) };
            #[cfg(feature = "doc-only")] test_msgs__msg__WStrings__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> WStrings {
            WStrings {
                wstring_value: crate::RosWString::from(msg.wstring_value.as_units()),
                wstring_value_default1: crate::RosWString::from(
                    msg.wstring_value_default1.as_units(),
                ),
                wstring_value_default2: crate::RosWString::from(
                    msg.wstring_value_default2.as_units(),
                ),
                wstring_value_default3: crate::RosWString::from(
                    msg.wstring_value_default3.as_units(),
                ),
                array_of_wstrings: std::array::from_fn(|index| crate::RosWString::from(
                    msg.array_of_wstrings[index].as_units(),
                )),
                bounded_sequence_of_wstrings: msg
                    .bounded_sequence_of_wstrings
                    .to_units_vec::<crate::RosWString>(),
                unbounded_sequence_of_wstrings: msg
                    .unbounded_sequence_of_wstrings
                    .to_units_vec::<crate::RosWString>(),
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            msg.wstring_value.try_assign_units((&self.wstring_value).as_units());
            msg.wstring_value_default1
                .try_assign_units((&self.wstring_value_default1).as_units());
            msg.wstring_value_default2
                .try_assign_units((&self.wstring_value_default2).as_units());
            msg.wstring_value_default3
                .try_assign_units((&self.wstring_value_default3).as_units());
            assert_eq!(
                self.array_of_wstrings.len(),
                3usize,
                "Field {} is fixed size of {}!",
                "array_of_wstrings",
                3usize,
            );
            for (t, s) in msg.array_of_wstrings.iter_mut().zip(&self.array_of_wstrings) {
                t.try_assign_units((&s).as_units());
            }
            assert!(
                self.bounded_sequence_of_wstrings.len() <= 3usize,
                "Field {} is upper bounded by {}!",
                "bounded_sequence_of_wstrings",
                3usize,
            );
            msg.bounded_sequence_of_wstrings
                .try_update_units(&self.bounded_sequence_of_wstrings);
            msg.unbounded_sequence_of_wstrings
                .try_update_units(&self.unbounded_sequence_of_wstrings);
        }
    }
    impl Default for WStrings {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<WStrings>::new();
            WStrings::from_native(&msg_native)
        }
    }
}
