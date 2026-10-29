pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Field {
        pub name: crate::RosString,
        #[serde(rename = "type")]
        pub type_: type_description_interfaces::msg::FieldType,
        pub default_value: crate::RosString,
    }
    unsafe impl WrappedTypesupport for Field {
        type CStruct = type_description_interfaces__msg__Field;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__type_description_interfaces__msg__Field()
            }
        }
        fn create_msg() -> *mut type_description_interfaces__msg__Field {
            #[cfg(not(feature = "doc-only"))]
            unsafe { type_description_interfaces__msg__Field__create() }
            #[cfg(feature = "doc-only")]
            type_description_interfaces__msg__Field__create()
        }
        fn destroy_msg(msg: *mut type_description_interfaces__msg__Field) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { type_description_interfaces__msg__Field__destroy(msg) };
            #[cfg(feature = "doc-only")]
            type_description_interfaces__msg__Field__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Field {
            Field {
                name: crate::RosString::from(msg.name.as_bytes()),
                type_: type_description_interfaces::msg::FieldType::from_native(
                    &msg.type_,
                ),
                default_value: crate::RosString::from(msg.default_value.as_bytes()),
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
            check_rosidl_runtime_ret(msg.name.try_assign_bytes(self.name.as_bytes()))?;
            self.type_.try_copy_to_native(&mut msg.type_)?;
            check_rosidl_runtime_ret(
                msg.default_value.try_assign_bytes(self.default_value.as_bytes()),
            )?;
            Ok(())
        }
    }
    impl Default for Field {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Field>::new();
            Field::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct FieldType {
        pub type_id: u8,
        pub capacity: u64,
        pub string_capacity: u64,
        pub nested_type_name: crate::RosString,
    }
    unsafe impl WrappedTypesupport for FieldType {
        type CStruct = type_description_interfaces__msg__FieldType;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__type_description_interfaces__msg__FieldType()
            }
        }
        fn create_msg() -> *mut type_description_interfaces__msg__FieldType {
            #[cfg(not(feature = "doc-only"))]
            unsafe { type_description_interfaces__msg__FieldType__create() }
            #[cfg(feature = "doc-only")]
            type_description_interfaces__msg__FieldType__create()
        }
        fn destroy_msg(msg: *mut type_description_interfaces__msg__FieldType) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { type_description_interfaces__msg__FieldType__destroy(msg) };
            #[cfg(feature = "doc-only")]
            type_description_interfaces__msg__FieldType__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> FieldType {
            FieldType {
                type_id: msg.type_id,
                capacity: msg.capacity,
                string_capacity: msg.string_capacity,
                nested_type_name: crate::RosString::from(msg.nested_type_name.as_bytes()),
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
            msg.type_id = self.type_id;
            msg.capacity = self.capacity;
            msg.string_capacity = self.string_capacity;
            check_rosidl_runtime_ret(
                msg.nested_type_name.try_assign_bytes(self.nested_type_name.as_bytes()),
            )?;
            Ok(())
        }
    }
    impl Default for FieldType {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<FieldType>::new();
            FieldType::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl FieldType {
        pub const FIELD_TYPE_BOOLEAN: _bindgen_ty_463 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BOOLEAN;
        pub const FIELD_TYPE_BOOLEAN_ARRAY: _bindgen_ty_485 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BOOLEAN_ARRAY;
        pub const FIELD_TYPE_BOOLEAN_BOUNDED_SEQUENCE: _bindgen_ty_507 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BOOLEAN_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_BOOLEAN_UNBOUNDED_SEQUENCE: _bindgen_ty_529 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BOOLEAN_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_BOUNDED_STRING: _bindgen_ty_469 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BOUNDED_STRING;
        pub const FIELD_TYPE_BOUNDED_STRING_ARRAY: _bindgen_ty_491 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BOUNDED_STRING_ARRAY;
        pub const FIELD_TYPE_BOUNDED_STRING_BOUNDED_SEQUENCE: _bindgen_ty_513 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BOUNDED_STRING_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_BOUNDED_STRING_UNBOUNDED_SEQUENCE: _bindgen_ty_535 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BOUNDED_STRING_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_BOUNDED_WSTRING: _bindgen_ty_470 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BOUNDED_WSTRING;
        pub const FIELD_TYPE_BOUNDED_WSTRING_ARRAY: _bindgen_ty_492 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BOUNDED_WSTRING_ARRAY;
        pub const FIELD_TYPE_BOUNDED_WSTRING_BOUNDED_SEQUENCE: _bindgen_ty_514 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BOUNDED_WSTRING_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_BOUNDED_WSTRING_UNBOUNDED_SEQUENCE: _bindgen_ty_536 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BOUNDED_WSTRING_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_BYTE: _bindgen_ty_464 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BYTE;
        pub const FIELD_TYPE_BYTE_ARRAY: _bindgen_ty_486 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BYTE_ARRAY;
        pub const FIELD_TYPE_BYTE_BOUNDED_SEQUENCE: _bindgen_ty_508 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BYTE_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_BYTE_UNBOUNDED_SEQUENCE: _bindgen_ty_530 = type_description_interfaces__msg__FieldType__FIELD_TYPE_BYTE_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_CHAR: _bindgen_ty_461 = type_description_interfaces__msg__FieldType__FIELD_TYPE_CHAR;
        pub const FIELD_TYPE_CHAR_ARRAY: _bindgen_ty_483 = type_description_interfaces__msg__FieldType__FIELD_TYPE_CHAR_ARRAY;
        pub const FIELD_TYPE_CHAR_BOUNDED_SEQUENCE: _bindgen_ty_505 = type_description_interfaces__msg__FieldType__FIELD_TYPE_CHAR_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_CHAR_UNBOUNDED_SEQUENCE: _bindgen_ty_527 = type_description_interfaces__msg__FieldType__FIELD_TYPE_CHAR_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_DOUBLE: _bindgen_ty_459 = type_description_interfaces__msg__FieldType__FIELD_TYPE_DOUBLE;
        pub const FIELD_TYPE_DOUBLE_ARRAY: _bindgen_ty_481 = type_description_interfaces__msg__FieldType__FIELD_TYPE_DOUBLE_ARRAY;
        pub const FIELD_TYPE_DOUBLE_BOUNDED_SEQUENCE: _bindgen_ty_503 = type_description_interfaces__msg__FieldType__FIELD_TYPE_DOUBLE_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_DOUBLE_UNBOUNDED_SEQUENCE: _bindgen_ty_525 = type_description_interfaces__msg__FieldType__FIELD_TYPE_DOUBLE_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_FIXED_STRING: _bindgen_ty_467 = type_description_interfaces__msg__FieldType__FIELD_TYPE_FIXED_STRING;
        pub const FIELD_TYPE_FIXED_STRING_ARRAY: _bindgen_ty_489 = type_description_interfaces__msg__FieldType__FIELD_TYPE_FIXED_STRING_ARRAY;
        pub const FIELD_TYPE_FIXED_STRING_BOUNDED_SEQUENCE: _bindgen_ty_511 = type_description_interfaces__msg__FieldType__FIELD_TYPE_FIXED_STRING_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_FIXED_STRING_UNBOUNDED_SEQUENCE: _bindgen_ty_533 = type_description_interfaces__msg__FieldType__FIELD_TYPE_FIXED_STRING_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_FIXED_WSTRING: _bindgen_ty_468 = type_description_interfaces__msg__FieldType__FIELD_TYPE_FIXED_WSTRING;
        pub const FIELD_TYPE_FIXED_WSTRING_ARRAY: _bindgen_ty_490 = type_description_interfaces__msg__FieldType__FIELD_TYPE_FIXED_WSTRING_ARRAY;
        pub const FIELD_TYPE_FIXED_WSTRING_BOUNDED_SEQUENCE: _bindgen_ty_512 = type_description_interfaces__msg__FieldType__FIELD_TYPE_FIXED_WSTRING_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_FIXED_WSTRING_UNBOUNDED_SEQUENCE: _bindgen_ty_534 = type_description_interfaces__msg__FieldType__FIELD_TYPE_FIXED_WSTRING_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_FLOAT: _bindgen_ty_458 = type_description_interfaces__msg__FieldType__FIELD_TYPE_FLOAT;
        pub const FIELD_TYPE_FLOAT_ARRAY: _bindgen_ty_480 = type_description_interfaces__msg__FieldType__FIELD_TYPE_FLOAT_ARRAY;
        pub const FIELD_TYPE_FLOAT_BOUNDED_SEQUENCE: _bindgen_ty_502 = type_description_interfaces__msg__FieldType__FIELD_TYPE_FLOAT_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_FLOAT_UNBOUNDED_SEQUENCE: _bindgen_ty_524 = type_description_interfaces__msg__FieldType__FIELD_TYPE_FLOAT_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_INT16: _bindgen_ty_452 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT16;
        pub const FIELD_TYPE_INT16_ARRAY: _bindgen_ty_474 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT16_ARRAY;
        pub const FIELD_TYPE_INT16_BOUNDED_SEQUENCE: _bindgen_ty_496 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT16_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_INT16_UNBOUNDED_SEQUENCE: _bindgen_ty_518 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT16_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_INT32: _bindgen_ty_454 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT32;
        pub const FIELD_TYPE_INT32_ARRAY: _bindgen_ty_476 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT32_ARRAY;
        pub const FIELD_TYPE_INT32_BOUNDED_SEQUENCE: _bindgen_ty_498 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT32_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_INT32_UNBOUNDED_SEQUENCE: _bindgen_ty_520 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT32_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_INT64: _bindgen_ty_456 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT64;
        pub const FIELD_TYPE_INT64_ARRAY: _bindgen_ty_478 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT64_ARRAY;
        pub const FIELD_TYPE_INT64_BOUNDED_SEQUENCE: _bindgen_ty_500 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT64_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_INT64_UNBOUNDED_SEQUENCE: _bindgen_ty_522 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT64_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_INT8: _bindgen_ty_450 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT8;
        pub const FIELD_TYPE_INT8_ARRAY: _bindgen_ty_472 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT8_ARRAY;
        pub const FIELD_TYPE_INT8_BOUNDED_SEQUENCE: _bindgen_ty_494 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT8_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_INT8_UNBOUNDED_SEQUENCE: _bindgen_ty_516 = type_description_interfaces__msg__FieldType__FIELD_TYPE_INT8_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_LONG_DOUBLE: _bindgen_ty_460 = type_description_interfaces__msg__FieldType__FIELD_TYPE_LONG_DOUBLE;
        pub const FIELD_TYPE_LONG_DOUBLE_ARRAY: _bindgen_ty_482 = type_description_interfaces__msg__FieldType__FIELD_TYPE_LONG_DOUBLE_ARRAY;
        pub const FIELD_TYPE_LONG_DOUBLE_BOUNDED_SEQUENCE: _bindgen_ty_504 = type_description_interfaces__msg__FieldType__FIELD_TYPE_LONG_DOUBLE_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_LONG_DOUBLE_UNBOUNDED_SEQUENCE: _bindgen_ty_526 = type_description_interfaces__msg__FieldType__FIELD_TYPE_LONG_DOUBLE_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_NESTED_TYPE: _bindgen_ty_449 = type_description_interfaces__msg__FieldType__FIELD_TYPE_NESTED_TYPE;
        pub const FIELD_TYPE_NESTED_TYPE_ARRAY: _bindgen_ty_471 = type_description_interfaces__msg__FieldType__FIELD_TYPE_NESTED_TYPE_ARRAY;
        pub const FIELD_TYPE_NESTED_TYPE_BOUNDED_SEQUENCE: _bindgen_ty_493 = type_description_interfaces__msg__FieldType__FIELD_TYPE_NESTED_TYPE_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_NESTED_TYPE_UNBOUNDED_SEQUENCE: _bindgen_ty_515 = type_description_interfaces__msg__FieldType__FIELD_TYPE_NESTED_TYPE_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_NOT_SET: _bindgen_ty_448 = type_description_interfaces__msg__FieldType__FIELD_TYPE_NOT_SET;
        pub const FIELD_TYPE_STRING: _bindgen_ty_465 = type_description_interfaces__msg__FieldType__FIELD_TYPE_STRING;
        pub const FIELD_TYPE_STRING_ARRAY: _bindgen_ty_487 = type_description_interfaces__msg__FieldType__FIELD_TYPE_STRING_ARRAY;
        pub const FIELD_TYPE_STRING_BOUNDED_SEQUENCE: _bindgen_ty_509 = type_description_interfaces__msg__FieldType__FIELD_TYPE_STRING_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_STRING_UNBOUNDED_SEQUENCE: _bindgen_ty_531 = type_description_interfaces__msg__FieldType__FIELD_TYPE_STRING_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_UINT16: _bindgen_ty_453 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT16;
        pub const FIELD_TYPE_UINT16_ARRAY: _bindgen_ty_475 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT16_ARRAY;
        pub const FIELD_TYPE_UINT16_BOUNDED_SEQUENCE: _bindgen_ty_497 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT16_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_UINT16_UNBOUNDED_SEQUENCE: _bindgen_ty_519 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT16_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_UINT32: _bindgen_ty_455 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT32;
        pub const FIELD_TYPE_UINT32_ARRAY: _bindgen_ty_477 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT32_ARRAY;
        pub const FIELD_TYPE_UINT32_BOUNDED_SEQUENCE: _bindgen_ty_499 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT32_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_UINT32_UNBOUNDED_SEQUENCE: _bindgen_ty_521 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT32_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_UINT64: _bindgen_ty_457 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT64;
        pub const FIELD_TYPE_UINT64_ARRAY: _bindgen_ty_479 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT64_ARRAY;
        pub const FIELD_TYPE_UINT64_BOUNDED_SEQUENCE: _bindgen_ty_501 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT64_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_UINT64_UNBOUNDED_SEQUENCE: _bindgen_ty_523 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT64_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_UINT8: _bindgen_ty_451 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT8;
        pub const FIELD_TYPE_UINT8_ARRAY: _bindgen_ty_473 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT8_ARRAY;
        pub const FIELD_TYPE_UINT8_BOUNDED_SEQUENCE: _bindgen_ty_495 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT8_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_UINT8_UNBOUNDED_SEQUENCE: _bindgen_ty_517 = type_description_interfaces__msg__FieldType__FIELD_TYPE_UINT8_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_WCHAR: _bindgen_ty_462 = type_description_interfaces__msg__FieldType__FIELD_TYPE_WCHAR;
        pub const FIELD_TYPE_WCHAR_ARRAY: _bindgen_ty_484 = type_description_interfaces__msg__FieldType__FIELD_TYPE_WCHAR_ARRAY;
        pub const FIELD_TYPE_WCHAR_BOUNDED_SEQUENCE: _bindgen_ty_506 = type_description_interfaces__msg__FieldType__FIELD_TYPE_WCHAR_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_WCHAR_UNBOUNDED_SEQUENCE: _bindgen_ty_528 = type_description_interfaces__msg__FieldType__FIELD_TYPE_WCHAR_UNBOUNDED_SEQUENCE;
        pub const FIELD_TYPE_WSTRING: _bindgen_ty_466 = type_description_interfaces__msg__FieldType__FIELD_TYPE_WSTRING;
        pub const FIELD_TYPE_WSTRING_ARRAY: _bindgen_ty_488 = type_description_interfaces__msg__FieldType__FIELD_TYPE_WSTRING_ARRAY;
        pub const FIELD_TYPE_WSTRING_BOUNDED_SEQUENCE: _bindgen_ty_510 = type_description_interfaces__msg__FieldType__FIELD_TYPE_WSTRING_BOUNDED_SEQUENCE;
        pub const FIELD_TYPE_WSTRING_UNBOUNDED_SEQUENCE: _bindgen_ty_532 = type_description_interfaces__msg__FieldType__FIELD_TYPE_WSTRING_UNBOUNDED_SEQUENCE;
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct IndividualTypeDescription {
        pub type_name: crate::RosString,
        pub fields: Vec<type_description_interfaces::msg::Field>,
    }
    unsafe impl WrappedTypesupport for IndividualTypeDescription {
        type CStruct = type_description_interfaces__msg__IndividualTypeDescription;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__type_description_interfaces__msg__IndividualTypeDescription()
            }
        }
        fn create_msg() -> *mut type_description_interfaces__msg__IndividualTypeDescription {
            #[cfg(not(feature = "doc-only"))]
            unsafe {
                type_description_interfaces__msg__IndividualTypeDescription__create()
            }
            #[cfg(feature = "doc-only")]
            type_description_interfaces__msg__IndividualTypeDescription__create()
        }
        fn destroy_msg(
            msg: *mut type_description_interfaces__msg__IndividualTypeDescription,
        ) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe {
                type_description_interfaces__msg__IndividualTypeDescription__destroy(msg)
            };
            #[cfg(feature = "doc-only")]
            type_description_interfaces__msg__IndividualTypeDescription__destroy(msg)
        }
        fn from_native(
            #[allow(unused)]
            msg: &Self::CStruct,
        ) -> IndividualTypeDescription {
            IndividualTypeDescription {
                type_name: crate::RosString::from(msg.type_name.as_bytes()),
                fields: {
                    let mut temp = Vec::with_capacity(msg.fields.size);
                    if msg.fields.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(msg.fields.data, msg.fields.size)
                        };
                        for s in slice {
                            temp.push(
                                type_description_interfaces::msg::Field::from_native(s),
                            );
                        }
                    }
                    temp
                },
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
            check_rosidl_runtime_ret(
                msg.type_name.try_assign_bytes(self.type_name.as_bytes()),
            )?;
            unsafe {
                let length = self.fields.len();
                if length > msg.fields.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !type_description_interfaces__msg__Field__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    type_description_interfaces__msg__Field__Sequence__fini(
                        &mut msg.fields,
                    );
                    msg.fields = replacement;
                }
                msg.fields.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(msg.fields.data, length);
                    for (t, s) in slice.iter_mut().zip(&self.fields) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for IndividualTypeDescription {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<IndividualTypeDescription>::new();
            IndividualTypeDescription::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct KeyValue {
        pub key: crate::RosString,
        pub value: crate::RosString,
    }
    unsafe impl WrappedTypesupport for KeyValue {
        type CStruct = type_description_interfaces__msg__KeyValue;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__type_description_interfaces__msg__KeyValue()
            }
        }
        fn create_msg() -> *mut type_description_interfaces__msg__KeyValue {
            #[cfg(not(feature = "doc-only"))]
            unsafe { type_description_interfaces__msg__KeyValue__create() }
            #[cfg(feature = "doc-only")]
            type_description_interfaces__msg__KeyValue__create()
        }
        fn destroy_msg(msg: *mut type_description_interfaces__msg__KeyValue) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { type_description_interfaces__msg__KeyValue__destroy(msg) };
            #[cfg(feature = "doc-only")]
            type_description_interfaces__msg__KeyValue__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> KeyValue {
            KeyValue {
                key: crate::RosString::from(msg.key.as_bytes()),
                value: crate::RosString::from(msg.value.as_bytes()),
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
            check_rosidl_runtime_ret(msg.key.try_assign_bytes(self.key.as_bytes()))?;
            check_rosidl_runtime_ret(msg.value.try_assign_bytes(self.value.as_bytes()))?;
            Ok(())
        }
    }
    impl Default for KeyValue {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<KeyValue>::new();
            KeyValue::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct TypeDescription {
        pub type_description: type_description_interfaces::msg::IndividualTypeDescription,
        pub referenced_type_descriptions: Vec<
            type_description_interfaces::msg::IndividualTypeDescription,
        >,
    }
    unsafe impl WrappedTypesupport for TypeDescription {
        type CStruct = type_description_interfaces__msg__TypeDescription;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__type_description_interfaces__msg__TypeDescription()
            }
        }
        fn create_msg() -> *mut type_description_interfaces__msg__TypeDescription {
            #[cfg(not(feature = "doc-only"))]
            unsafe { type_description_interfaces__msg__TypeDescription__create() }
            #[cfg(feature = "doc-only")]
            type_description_interfaces__msg__TypeDescription__create()
        }
        fn destroy_msg(
            msg: *mut type_description_interfaces__msg__TypeDescription,
        ) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { type_description_interfaces__msg__TypeDescription__destroy(msg) };
            #[cfg(feature = "doc-only")]
            type_description_interfaces__msg__TypeDescription__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> TypeDescription {
            TypeDescription {
                type_description: type_description_interfaces::msg::IndividualTypeDescription::from_native(
                    &msg.type_description,
                ),
                referenced_type_descriptions: {
                    let mut temp = Vec::with_capacity(
                        msg.referenced_type_descriptions.size,
                    );
                    if msg.referenced_type_descriptions.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.referenced_type_descriptions.data,
                                msg.referenced_type_descriptions.size,
                            )
                        };
                        for s in slice {
                            temp.push(
                                type_description_interfaces::msg::IndividualTypeDescription::from_native(
                                    s,
                                ),
                            );
                        }
                    }
                    temp
                },
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
            self.type_description.try_copy_to_native(&mut msg.type_description)?;
            unsafe {
                let length = self.referenced_type_descriptions.len();
                if length > msg.referenced_type_descriptions.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !type_description_interfaces__msg__IndividualTypeDescription__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    type_description_interfaces__msg__IndividualTypeDescription__Sequence__fini(
                        &mut msg.referenced_type_descriptions,
                    );
                    msg.referenced_type_descriptions = replacement;
                }
                msg.referenced_type_descriptions.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.referenced_type_descriptions.data,
                        length,
                    );
                    for (t, s) in slice
                        .iter_mut()
                        .zip(&self.referenced_type_descriptions)
                    {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for TypeDescription {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<TypeDescription>::new();
            TypeDescription::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct TypeSource {
        pub type_name: crate::RosString,
        pub encoding: crate::RosString,
        pub raw_file_contents: crate::RosString,
    }
    unsafe impl WrappedTypesupport for TypeSource {
        type CStruct = type_description_interfaces__msg__TypeSource;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__type_description_interfaces__msg__TypeSource()
            }
        }
        fn create_msg() -> *mut type_description_interfaces__msg__TypeSource {
            #[cfg(not(feature = "doc-only"))]
            unsafe { type_description_interfaces__msg__TypeSource__create() }
            #[cfg(feature = "doc-only")]
            type_description_interfaces__msg__TypeSource__create()
        }
        fn destroy_msg(msg: *mut type_description_interfaces__msg__TypeSource) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { type_description_interfaces__msg__TypeSource__destroy(msg) };
            #[cfg(feature = "doc-only")]
            type_description_interfaces__msg__TypeSource__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> TypeSource {
            TypeSource {
                type_name: crate::RosString::from(msg.type_name.as_bytes()),
                encoding: crate::RosString::from(msg.encoding.as_bytes()),
                raw_file_contents: crate::RosString::from(
                    msg.raw_file_contents.as_bytes(),
                ),
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
            check_rosidl_runtime_ret(
                msg.type_name.try_assign_bytes(self.type_name.as_bytes()),
            )?;
            check_rosidl_runtime_ret(
                msg.encoding.try_assign_bytes(self.encoding.as_bytes()),
            )?;
            check_rosidl_runtime_ret(
                msg.raw_file_contents.try_assign_bytes(self.raw_file_contents.as_bytes()),
            )?;
            Ok(())
        }
    }
    impl Default for TypeSource {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<TypeSource>::new();
            TypeSource::from_native(&msg_native)
        }
    }
}
pub mod srv {
    #[allow(non_snake_case)]
    pub mod GetTypeDescription {
        use super::super::super::*;
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Service();
        unsafe impl WrappedServiceTypeSupport for Service {
            type Request = Request;
            type Response = Response;
            #[cfg(
                any(
                    r2r__ros__distro__iron,
                    r2r__ros__distro__jazzy,
                    r2r__ros__distro__kilted,
                    r2r__ros__distro__lyrical,
                    r2r__ros__distro__rolling
                )
            )]
            type Event = Event;
            fn get_ts() -> &'static rosidl_service_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_service_type_support_handle__type_description_interfaces__srv__GetTypeDescription()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub type_name: crate::RosString,
            pub type_hash: crate::RosString,
            pub include_type_sources: bool,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = type_description_interfaces__srv__GetTypeDescription_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__type_description_interfaces__srv__GetTypeDescription_Request()
                }
            }
            fn create_msg() -> *mut type_description_interfaces__srv__GetTypeDescription_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    type_description_interfaces__srv__GetTypeDescription_Request__create()
                }
                #[cfg(feature = "doc-only")]
                type_description_interfaces__srv__GetTypeDescription_Request__create()
            }
            fn destroy_msg(
                msg: *mut type_description_interfaces__srv__GetTypeDescription_Request,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    type_description_interfaces__srv__GetTypeDescription_Request__destroy(
                        msg,
                    )
                };
                #[cfg(feature = "doc-only")]
                type_description_interfaces__srv__GetTypeDescription_Request__destroy(
                    msg,
                )
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    type_name: crate::RosString::from(msg.type_name.as_bytes()),
                    type_hash: crate::RosString::from(msg.type_hash.as_bytes()),
                    include_type_sources: msg.include_type_sources,
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
                check_rosidl_runtime_ret(
                    msg.type_name.try_assign_bytes(self.type_name.as_bytes()),
                )?;
                check_rosidl_runtime_ret(
                    msg.type_hash.try_assign_bytes(self.type_hash.as_bytes()),
                )?;
                msg.include_type_sources = self.include_type_sources;
                Ok(())
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
            pub successful: bool,
            pub failure_reason: crate::RosString,
            pub type_description: type_description_interfaces::msg::TypeDescription,
            pub type_sources: Vec<type_description_interfaces::msg::TypeSource>,
            pub extra_information: Vec<type_description_interfaces::msg::KeyValue>,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = type_description_interfaces__srv__GetTypeDescription_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__type_description_interfaces__srv__GetTypeDescription_Response()
                }
            }
            fn create_msg() -> *mut type_description_interfaces__srv__GetTypeDescription_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    type_description_interfaces__srv__GetTypeDescription_Response__create()
                }
                #[cfg(feature = "doc-only")]
                type_description_interfaces__srv__GetTypeDescription_Response__create()
            }
            fn destroy_msg(
                msg: *mut type_description_interfaces__srv__GetTypeDescription_Response,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    type_description_interfaces__srv__GetTypeDescription_Response__destroy(
                        msg,
                    )
                };
                #[cfg(feature = "doc-only")]
                type_description_interfaces__srv__GetTypeDescription_Response__destroy(
                    msg,
                )
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    successful: msg.successful,
                    failure_reason: crate::RosString::from(
                        msg.failure_reason.as_bytes(),
                    ),
                    type_description: type_description_interfaces::msg::TypeDescription::from_native(
                        &msg.type_description,
                    ),
                    type_sources: {
                        let mut temp = Vec::with_capacity(msg.type_sources.size);
                        if msg.type_sources.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                    msg.type_sources.data,
                                    msg.type_sources.size,
                                )
                            };
                            for s in slice {
                                temp.push(
                                    type_description_interfaces::msg::TypeSource::from_native(s),
                                );
                            }
                        }
                        temp
                    },
                    extra_information: {
                        let mut temp = Vec::with_capacity(msg.extra_information.size);
                        if msg.extra_information.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                    msg.extra_information.data,
                                    msg.extra_information.size,
                                )
                            };
                            for s in slice {
                                temp.push(
                                    type_description_interfaces::msg::KeyValue::from_native(s),
                                );
                            }
                        }
                        temp
                    },
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
                msg.successful = self.successful;
                check_rosidl_runtime_ret(
                    msg.failure_reason.try_assign_bytes(self.failure_reason.as_bytes()),
                )?;
                self.type_description.try_copy_to_native(&mut msg.type_description)?;
                unsafe {
                    let length = self.type_sources.len();
                    if length > msg.type_sources.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !type_description_interfaces__msg__TypeSource__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        type_description_interfaces__msg__TypeSource__Sequence__fini(
                            &mut msg.type_sources,
                        );
                        msg.type_sources = replacement;
                    }
                    msg.type_sources.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.type_sources.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.type_sources) {
                            s.try_copy_to_native(t)?;
                        }
                    }
                }
                unsafe {
                    let length = self.extra_information.len();
                    if length > msg.extra_information.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !type_description_interfaces__msg__KeyValue__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        type_description_interfaces__msg__KeyValue__Sequence__fini(
                            &mut msg.extra_information,
                        );
                        msg.extra_information = replacement;
                    }
                    msg.extra_information.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.extra_information.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.extra_information) {
                            s.try_copy_to_native(t)?;
                        }
                    }
                }
                Ok(())
            }
        }
        impl Default for Response {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Response>::new();
                Response::from_native(&msg_native)
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Event {
            pub info: service_msgs::msg::ServiceEventInfo,
            pub request: Vec<
                type_description_interfaces::srv::GetTypeDescription::Request,
            >,
            pub response: Vec<
                type_description_interfaces::srv::GetTypeDescription::Response,
            >,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = type_description_interfaces__srv__GetTypeDescription_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__type_description_interfaces__srv__GetTypeDescription_Event()
                }
            }
            fn create_msg() -> *mut type_description_interfaces__srv__GetTypeDescription_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    type_description_interfaces__srv__GetTypeDescription_Event__create()
                }
                #[cfg(feature = "doc-only")]
                type_description_interfaces__srv__GetTypeDescription_Event__create()
            }
            fn destroy_msg(
                msg: *mut type_description_interfaces__srv__GetTypeDescription_Event,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    type_description_interfaces__srv__GetTypeDescription_Event__destroy(
                        msg,
                    )
                };
                #[cfg(feature = "doc-only")]
                type_description_interfaces__srv__GetTypeDescription_Event__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Event {
                Event {
                    info: service_msgs::msg::ServiceEventInfo::from_native(&msg.info),
                    request: {
                        let mut temp = Vec::with_capacity(msg.request.size);
                        if msg.request.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                    msg.request.data,
                                    msg.request.size,
                                )
                            };
                            for s in slice {
                                temp.push(
                                    type_description_interfaces::srv::GetTypeDescription::Request::from_native(
                                        s,
                                    ),
                                );
                            }
                        }
                        temp
                    },
                    response: {
                        let mut temp = Vec::with_capacity(msg.response.size);
                        if msg.response.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                    msg.response.data,
                                    msg.response.size,
                                )
                            };
                            for s in slice {
                                temp.push(
                                    type_description_interfaces::srv::GetTypeDescription::Response::from_native(
                                        s,
                                    ),
                                );
                            }
                        }
                        temp
                    },
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
                self.info.try_copy_to_native(&mut msg.info)?;
                if self.request.len() > 1usize {
                    return Err(Error::RCL_RET_INVALID_ARGUMENT);
                }
                unsafe {
                    let length = self.request.len();
                    if length > msg.request.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !type_description_interfaces__srv__GetTypeDescription_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        type_description_interfaces__srv__GetTypeDescription_Request__Sequence__fini(
                            &mut msg.request,
                        );
                        msg.request = replacement;
                    }
                    msg.request.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.request.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.request) {
                            s.try_copy_to_native(t)?;
                        }
                    }
                }
                if self.response.len() > 1usize {
                    return Err(Error::RCL_RET_INVALID_ARGUMENT);
                }
                unsafe {
                    let length = self.response.len();
                    if length > msg.response.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !type_description_interfaces__srv__GetTypeDescription_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        type_description_interfaces__srv__GetTypeDescription_Response__Sequence__fini(
                            &mut msg.response,
                        );
                        msg.response = replacement;
                    }
                    msg.response.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.response.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.response) {
                            s.try_copy_to_native(t)?;
                        }
                    }
                }
                Ok(())
            }
        }
        impl Default for Event {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Event>::new();
                Event::from_native(&msg_native)
            }
        }
    }
}
