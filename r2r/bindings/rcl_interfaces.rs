pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct FloatingPointRange {
        pub from_value: f64,
        pub to_value: f64,
        pub step: f64,
    }
    unsafe impl WrappedTypesupport for FloatingPointRange {
        type CStruct = rcl_interfaces__msg__FloatingPointRange;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__FloatingPointRange()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__FloatingPointRange {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__FloatingPointRange__create() }
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__FloatingPointRange__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__FloatingPointRange) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__FloatingPointRange__destroy(msg) };
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__FloatingPointRange__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> FloatingPointRange {
            FloatingPointRange {
                from_value: msg.from_value,
                to_value: msg.to_value,
                step: msg.step,
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            msg.from_value = self.from_value;
            msg.to_value = self.to_value;
            msg.step = self.step;
        }
    }
    impl Default for FloatingPointRange {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<FloatingPointRange>::new();
            FloatingPointRange::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct IntegerRange {
        pub from_value: i64,
        pub to_value: i64,
        pub step: u64,
    }
    unsafe impl WrappedTypesupport for IntegerRange {
        type CStruct = rcl_interfaces__msg__IntegerRange;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__IntegerRange()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__IntegerRange {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__IntegerRange__create() }
            #[cfg(feature = "doc-only")] rcl_interfaces__msg__IntegerRange__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__IntegerRange) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__IntegerRange__destroy(msg) };
            #[cfg(feature = "doc-only")] rcl_interfaces__msg__IntegerRange__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> IntegerRange {
            IntegerRange {
                from_value: msg.from_value,
                to_value: msg.to_value,
                step: msg.step,
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            msg.from_value = self.from_value;
            msg.to_value = self.to_value;
            msg.step = self.step;
        }
    }
    impl Default for IntegerRange {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<IntegerRange>::new();
            IntegerRange::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ListParametersResult {
        pub names: Vec<crate::RosString>,
        pub prefixes: Vec<crate::RosString>,
    }
    unsafe impl WrappedTypesupport for ListParametersResult {
        type CStruct = rcl_interfaces__msg__ListParametersResult;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__ListParametersResult()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__ListParametersResult {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__ListParametersResult__create() }
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__ListParametersResult__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__ListParametersResult) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__ListParametersResult__destroy(msg) };
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__ListParametersResult__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> ListParametersResult {
            ListParametersResult {
                names: msg.names.to_bytes_vec::<crate::RosString>(),
                prefixes: msg.prefixes.to_bytes_vec::<crate::RosString>(),
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            msg.names.try_update_bytes(&self.names);
            msg.prefixes.try_update_bytes(&self.prefixes);
        }
    }
    impl Default for ListParametersResult {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<ListParametersResult>::new();
            ListParametersResult::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Log {
        pub stamp: builtin_interfaces::msg::Time,
        pub level: u8,
        pub name: crate::RosString,
        pub msg: crate::RosString,
        pub file: crate::RosString,
        pub function: crate::RosString,
        pub line: u32,
    }
    unsafe impl WrappedTypesupport for Log {
        type CStruct = rcl_interfaces__msg__Log;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__Log()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__Log {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__Log__create() }
            #[cfg(feature = "doc-only")] rcl_interfaces__msg__Log__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__Log) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__Log__destroy(msg) };
            #[cfg(feature = "doc-only")] rcl_interfaces__msg__Log__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Log {
            Log {
                stamp: builtin_interfaces::msg::Time::from_native(&msg.stamp),
                level: msg.level,
                name: crate::RosString::from(msg.name.as_bytes()),
                msg: crate::RosString::from(msg.msg.as_bytes()),
                file: crate::RosString::from(msg.file.as_bytes()),
                function: crate::RosString::from(msg.function.as_bytes()),
                line: msg.line,
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            self.stamp.copy_to_native(&mut msg.stamp);
            msg.level = self.level;
            msg.name.try_assign_bytes((&self.name).as_bytes());
            msg.msg.try_assign_bytes((&self.msg).as_bytes());
            msg.file.try_assign_bytes((&self.file).as_bytes());
            msg.function.try_assign_bytes((&self.function).as_bytes());
            msg.line = self.line;
        }
    }
    impl Default for Log {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Log>::new();
            Log::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl Log {
        pub const DEBUG: _bindgen_ty_62 = rcl_interfaces__msg__Log__DEBUG;
        pub const ERROR: _bindgen_ty_65 = rcl_interfaces__msg__Log__ERROR;
        pub const FATAL: _bindgen_ty_66 = rcl_interfaces__msg__Log__FATAL;
        pub const INFO: _bindgen_ty_63 = rcl_interfaces__msg__Log__INFO;
        pub const WARN: _bindgen_ty_64 = rcl_interfaces__msg__Log__WARN;
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Parameter {
        pub name: crate::RosString,
        pub value: rcl_interfaces::msg::ParameterValue,
    }
    unsafe impl WrappedTypesupport for Parameter {
        type CStruct = rcl_interfaces__msg__Parameter;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__Parameter()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__Parameter {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__Parameter__create() }
            #[cfg(feature = "doc-only")] rcl_interfaces__msg__Parameter__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__Parameter) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__Parameter__destroy(msg) };
            #[cfg(feature = "doc-only")] rcl_interfaces__msg__Parameter__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Parameter {
            Parameter {
                name: crate::RosString::from(msg.name.as_bytes()),
                value: rcl_interfaces::msg::ParameterValue::from_native(&msg.value),
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            msg.name.try_assign_bytes((&self.name).as_bytes());
            self.value.copy_to_native(&mut msg.value);
        }
    }
    impl Default for Parameter {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Parameter>::new();
            Parameter::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ParameterDescriptor {
        pub name: crate::RosString,
        #[serde(rename = "type")]
        pub type_: u8,
        pub description: crate::RosString,
        pub additional_constraints: crate::RosString,
        pub read_only: bool,
        pub dynamic_typing: bool,
        pub floating_point_range: Vec<rcl_interfaces::msg::FloatingPointRange>,
        pub integer_range: Vec<rcl_interfaces::msg::IntegerRange>,
    }
    unsafe impl WrappedTypesupport for ParameterDescriptor {
        type CStruct = rcl_interfaces__msg__ParameterDescriptor;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__ParameterDescriptor()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__ParameterDescriptor {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__ParameterDescriptor__create() }
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__ParameterDescriptor__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__ParameterDescriptor) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__ParameterDescriptor__destroy(msg) };
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__ParameterDescriptor__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> ParameterDescriptor {
            ParameterDescriptor {
                name: crate::RosString::from(msg.name.as_bytes()),
                type_: msg.type_,
                description: crate::RosString::from(msg.description.as_bytes()),
                additional_constraints: crate::RosString::from(
                    msg.additional_constraints.as_bytes(),
                ),
                read_only: msg.read_only,
                dynamic_typing: msg.dynamic_typing,
                floating_point_range: {
                    let mut temp = Vec::with_capacity(msg.floating_point_range.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.floating_point_range.data,
                            msg.floating_point_range.size,
                        )
                    };
                    for s in slice {
                        temp.push(
                            rcl_interfaces::msg::FloatingPointRange::from_native(s),
                        );
                    }
                    temp
                },
                integer_range: {
                    let mut temp = Vec::with_capacity(msg.integer_range.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.integer_range.data,
                            msg.integer_range.size,
                        )
                    };
                    for s in slice {
                        temp.push(rcl_interfaces::msg::IntegerRange::from_native(s));
                    }
                    temp
                },
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            msg.name.try_assign_bytes((&self.name).as_bytes());
            msg.type_ = self.type_;
            msg.description.try_assign_bytes((&self.description).as_bytes());
            msg.additional_constraints
                .try_assign_bytes((&self.additional_constraints).as_bytes());
            msg.read_only = self.read_only;
            msg.dynamic_typing = self.dynamic_typing;
            unsafe {
                rcl_interfaces__msg__FloatingPointRange__Sequence__fini(
                    &mut msg.floating_point_range,
                );
                rcl_interfaces__msg__FloatingPointRange__Sequence__init(
                    &mut msg.floating_point_range,
                    self.floating_point_range.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.floating_point_range.data,
                    msg.floating_point_range.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.floating_point_range) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                rcl_interfaces__msg__IntegerRange__Sequence__fini(
                    &mut msg.integer_range,
                );
                rcl_interfaces__msg__IntegerRange__Sequence__init(
                    &mut msg.integer_range,
                    self.integer_range.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.integer_range.data,
                    msg.integer_range.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.integer_range) {
                    s.copy_to_native(t);
                }
            }
        }
    }
    impl Default for ParameterDescriptor {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<ParameterDescriptor>::new();
            ParameterDescriptor::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ParameterEvent {
        pub stamp: builtin_interfaces::msg::Time,
        pub node: crate::RosString,
        pub new_parameters: Vec<rcl_interfaces::msg::Parameter>,
        pub changed_parameters: Vec<rcl_interfaces::msg::Parameter>,
        pub deleted_parameters: Vec<rcl_interfaces::msg::Parameter>,
    }
    unsafe impl WrappedTypesupport for ParameterEvent {
        type CStruct = rcl_interfaces__msg__ParameterEvent;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__ParameterEvent()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__ParameterEvent {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__ParameterEvent__create() }
            #[cfg(feature = "doc-only")] rcl_interfaces__msg__ParameterEvent__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__ParameterEvent) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__ParameterEvent__destroy(msg) };
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__ParameterEvent__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> ParameterEvent {
            ParameterEvent {
                stamp: builtin_interfaces::msg::Time::from_native(&msg.stamp),
                node: crate::RosString::from(msg.node.as_bytes()),
                new_parameters: {
                    let mut temp = Vec::with_capacity(msg.new_parameters.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.new_parameters.data,
                            msg.new_parameters.size,
                        )
                    };
                    for s in slice {
                        temp.push(rcl_interfaces::msg::Parameter::from_native(s));
                    }
                    temp
                },
                changed_parameters: {
                    let mut temp = Vec::with_capacity(msg.changed_parameters.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.changed_parameters.data,
                            msg.changed_parameters.size,
                        )
                    };
                    for s in slice {
                        temp.push(rcl_interfaces::msg::Parameter::from_native(s));
                    }
                    temp
                },
                deleted_parameters: {
                    let mut temp = Vec::with_capacity(msg.deleted_parameters.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.deleted_parameters.data,
                            msg.deleted_parameters.size,
                        )
                    };
                    for s in slice {
                        temp.push(rcl_interfaces::msg::Parameter::from_native(s));
                    }
                    temp
                },
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            self.stamp.copy_to_native(&mut msg.stamp);
            msg.node.try_assign_bytes((&self.node).as_bytes());
            unsafe {
                rcl_interfaces__msg__Parameter__Sequence__fini(&mut msg.new_parameters);
                rcl_interfaces__msg__Parameter__Sequence__init(
                    &mut msg.new_parameters,
                    self.new_parameters.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.new_parameters.data,
                    msg.new_parameters.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.new_parameters) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                rcl_interfaces__msg__Parameter__Sequence__fini(
                    &mut msg.changed_parameters,
                );
                rcl_interfaces__msg__Parameter__Sequence__init(
                    &mut msg.changed_parameters,
                    self.changed_parameters.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.changed_parameters.data,
                    msg.changed_parameters.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.changed_parameters) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                rcl_interfaces__msg__Parameter__Sequence__fini(
                    &mut msg.deleted_parameters,
                );
                rcl_interfaces__msg__Parameter__Sequence__init(
                    &mut msg.deleted_parameters,
                    self.deleted_parameters.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.deleted_parameters.data,
                    msg.deleted_parameters.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.deleted_parameters) {
                    s.copy_to_native(t);
                }
            }
        }
    }
    impl Default for ParameterEvent {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<ParameterEvent>::new();
            ParameterEvent::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ParameterEventDescriptors {
        pub new_parameters: Vec<rcl_interfaces::msg::ParameterDescriptor>,
        pub changed_parameters: Vec<rcl_interfaces::msg::ParameterDescriptor>,
        pub deleted_parameters: Vec<rcl_interfaces::msg::ParameterDescriptor>,
    }
    unsafe impl WrappedTypesupport for ParameterEventDescriptors {
        type CStruct = rcl_interfaces__msg__ParameterEventDescriptors;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__ParameterEventDescriptors()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__ParameterEventDescriptors {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__ParameterEventDescriptors__create() }
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__ParameterEventDescriptors__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__ParameterEventDescriptors) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__ParameterEventDescriptors__destroy(msg) };
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__ParameterEventDescriptors__destroy(msg)
        }
        fn from_native(
            #[allow(unused)]
            msg: &Self::CStruct,
        ) -> ParameterEventDescriptors {
            ParameterEventDescriptors {
                new_parameters: {
                    let mut temp = Vec::with_capacity(msg.new_parameters.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.new_parameters.data,
                            msg.new_parameters.size,
                        )
                    };
                    for s in slice {
                        temp.push(
                            rcl_interfaces::msg::ParameterDescriptor::from_native(s),
                        );
                    }
                    temp
                },
                changed_parameters: {
                    let mut temp = Vec::with_capacity(msg.changed_parameters.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.changed_parameters.data,
                            msg.changed_parameters.size,
                        )
                    };
                    for s in slice {
                        temp.push(
                            rcl_interfaces::msg::ParameterDescriptor::from_native(s),
                        );
                    }
                    temp
                },
                deleted_parameters: {
                    let mut temp = Vec::with_capacity(msg.deleted_parameters.size);
                    let slice = unsafe {
                        std::slice::from_raw_parts(
                            msg.deleted_parameters.data,
                            msg.deleted_parameters.size,
                        )
                    };
                    for s in slice {
                        temp.push(
                            rcl_interfaces::msg::ParameterDescriptor::from_native(s),
                        );
                    }
                    temp
                },
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            unsafe {
                rcl_interfaces__msg__ParameterDescriptor__Sequence__fini(
                    &mut msg.new_parameters,
                );
                rcl_interfaces__msg__ParameterDescriptor__Sequence__init(
                    &mut msg.new_parameters,
                    self.new_parameters.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.new_parameters.data,
                    msg.new_parameters.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.new_parameters) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                rcl_interfaces__msg__ParameterDescriptor__Sequence__fini(
                    &mut msg.changed_parameters,
                );
                rcl_interfaces__msg__ParameterDescriptor__Sequence__init(
                    &mut msg.changed_parameters,
                    self.changed_parameters.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.changed_parameters.data,
                    msg.changed_parameters.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.changed_parameters) {
                    s.copy_to_native(t);
                }
            }
            unsafe {
                rcl_interfaces__msg__ParameterDescriptor__Sequence__fini(
                    &mut msg.deleted_parameters,
                );
                rcl_interfaces__msg__ParameterDescriptor__Sequence__init(
                    &mut msg.deleted_parameters,
                    self.deleted_parameters.len(),
                );
                let slice = std::slice::from_raw_parts_mut(
                    msg.deleted_parameters.data,
                    msg.deleted_parameters.size,
                );
                for (t, s) in slice.iter_mut().zip(&self.deleted_parameters) {
                    s.copy_to_native(t);
                }
            }
        }
    }
    impl Default for ParameterEventDescriptors {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<ParameterEventDescriptors>::new();
            ParameterEventDescriptors::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ParameterType {}
    unsafe impl WrappedTypesupport for ParameterType {
        type CStruct = rcl_interfaces__msg__ParameterType;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__ParameterType()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__ParameterType {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__ParameterType__create() }
            #[cfg(feature = "doc-only")] rcl_interfaces__msg__ParameterType__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__ParameterType) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__ParameterType__destroy(msg) };
            #[cfg(feature = "doc-only")] rcl_interfaces__msg__ParameterType__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> ParameterType {
            ParameterType {}
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {}
    }
    impl Default for ParameterType {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<ParameterType>::new();
            ParameterType::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl ParameterType {
        pub const PARAMETER_BOOL: _bindgen_ty_70 = rcl_interfaces__msg__ParameterType__PARAMETER_BOOL;
        pub const PARAMETER_BOOL_ARRAY: _bindgen_ty_75 = rcl_interfaces__msg__ParameterType__PARAMETER_BOOL_ARRAY;
        pub const PARAMETER_BYTE_ARRAY: _bindgen_ty_74 = rcl_interfaces__msg__ParameterType__PARAMETER_BYTE_ARRAY;
        pub const PARAMETER_DOUBLE: _bindgen_ty_72 = rcl_interfaces__msg__ParameterType__PARAMETER_DOUBLE;
        pub const PARAMETER_DOUBLE_ARRAY: _bindgen_ty_77 = rcl_interfaces__msg__ParameterType__PARAMETER_DOUBLE_ARRAY;
        pub const PARAMETER_INTEGER: _bindgen_ty_71 = rcl_interfaces__msg__ParameterType__PARAMETER_INTEGER;
        pub const PARAMETER_INTEGER_ARRAY: _bindgen_ty_76 = rcl_interfaces__msg__ParameterType__PARAMETER_INTEGER_ARRAY;
        pub const PARAMETER_NOT_SET: _bindgen_ty_69 = rcl_interfaces__msg__ParameterType__PARAMETER_NOT_SET;
        pub const PARAMETER_STRING: _bindgen_ty_73 = rcl_interfaces__msg__ParameterType__PARAMETER_STRING;
        pub const PARAMETER_STRING_ARRAY: _bindgen_ty_78 = rcl_interfaces__msg__ParameterType__PARAMETER_STRING_ARRAY;
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ParameterValue {
        #[serde(rename = "type")]
        pub type_: u8,
        pub bool_value: bool,
        pub integer_value: i64,
        pub double_value: f64,
        pub string_value: crate::RosString,
        pub byte_array_value: Vec<u8>,
        pub bool_array_value: Vec<bool>,
        pub integer_array_value: Vec<i64>,
        pub double_array_value: Vec<f64>,
        pub string_array_value: Vec<crate::RosString>,
    }
    unsafe impl WrappedTypesupport for ParameterValue {
        type CStruct = rcl_interfaces__msg__ParameterValue;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__ParameterValue()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__ParameterValue {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__ParameterValue__create() }
            #[cfg(feature = "doc-only")] rcl_interfaces__msg__ParameterValue__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__ParameterValue) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__ParameterValue__destroy(msg) };
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__ParameterValue__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> ParameterValue {
            ParameterValue {
                type_: msg.type_,
                bool_value: msg.bool_value,
                integer_value: msg.integer_value,
                double_value: msg.double_value,
                string_value: crate::RosString::from(msg.string_value.as_bytes()),
                byte_array_value: msg.byte_array_value.to_vec(),
                bool_array_value: msg.bool_array_value.to_vec(),
                integer_array_value: msg.integer_array_value.to_vec(),
                double_array_value: msg.double_array_value.to_vec(),
                string_array_value: msg
                    .string_array_value
                    .to_bytes_vec::<crate::RosString>(),
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            msg.type_ = self.type_;
            msg.bool_value = self.bool_value;
            msg.integer_value = self.integer_value;
            msg.double_value = self.double_value;
            msg.string_value.try_assign_bytes((&self.string_value).as_bytes());
            msg.byte_array_value.update(&self.byte_array_value);
            msg.bool_array_value.update(&self.bool_array_value);
            msg.integer_array_value.update(&self.integer_array_value);
            msg.double_array_value.update(&self.double_array_value);
            msg.string_array_value.try_update_bytes(&self.string_array_value);
        }
    }
    impl Default for ParameterValue {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<ParameterValue>::new();
            ParameterValue::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct SetParametersResult {
        pub successful: bool,
        pub reason: crate::RosString,
    }
    unsafe impl WrappedTypesupport for SetParametersResult {
        type CStruct = rcl_interfaces__msg__SetParametersResult;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__SetParametersResult()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__SetParametersResult {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__SetParametersResult__create() }
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__SetParametersResult__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__SetParametersResult) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__SetParametersResult__destroy(msg) };
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__SetParametersResult__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> SetParametersResult {
            SetParametersResult {
                successful: msg.successful,
                reason: crate::RosString::from(msg.reason.as_bytes()),
            }
        }
        fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
            msg.successful = self.successful;
            msg.reason.try_assign_bytes((&self.reason).as_bytes());
        }
    }
    impl Default for SetParametersResult {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<SetParametersResult>::new();
            SetParametersResult::from_native(&msg_native)
        }
    }
}
pub mod srv {
    #[allow(non_snake_case)]
    pub mod DescribeParameters {
        use super::super::super::*;
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Service();
        unsafe impl WrappedServiceTypeSupport for Service {
            type Request = Request;
            type Response = Response;
            fn get_ts() -> &'static rosidl_service_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_service_type_support_handle__rcl_interfaces__srv__DescribeParameters()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub names: Vec<crate::RosString>,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = rcl_interfaces__srv__DescribeParameters_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__DescribeParameters_Request()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__DescribeParameters_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__DescribeParameters_Request__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__DescribeParameters_Request__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__DescribeParameters_Request,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__DescribeParameters_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__DescribeParameters_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    names: msg.names.to_bytes_vec::<crate::RosString>(),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                msg.names.try_update_bytes(&self.names);
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
            pub descriptors: Vec<rcl_interfaces::msg::ParameterDescriptor>,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = rcl_interfaces__srv__DescribeParameters_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__DescribeParameters_Response()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__DescribeParameters_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__DescribeParameters_Response__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__DescribeParameters_Response__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__DescribeParameters_Response,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    rcl_interfaces__srv__DescribeParameters_Response__destroy(msg)
                };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__DescribeParameters_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    descriptors: {
                        let mut temp = Vec::with_capacity(msg.descriptors.size);
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.descriptors.data,
                                msg.descriptors.size,
                            )
                        };
                        for s in slice {
                            temp.push(
                                rcl_interfaces::msg::ParameterDescriptor::from_native(s),
                            );
                        }
                        temp
                    },
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                unsafe {
                    rcl_interfaces__msg__ParameterDescriptor__Sequence__fini(
                        &mut msg.descriptors,
                    );
                    rcl_interfaces__msg__ParameterDescriptor__Sequence__init(
                        &mut msg.descriptors,
                        self.descriptors.len(),
                    );
                    let slice = std::slice::from_raw_parts_mut(
                        msg.descriptors.data,
                        msg.descriptors.size,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.descriptors) {
                        s.copy_to_native(t);
                    }
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
    pub mod GetParameterTypes {
        use super::super::super::*;
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Service();
        unsafe impl WrappedServiceTypeSupport for Service {
            type Request = Request;
            type Response = Response;
            fn get_ts() -> &'static rosidl_service_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_service_type_support_handle__rcl_interfaces__srv__GetParameterTypes()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub names: Vec<crate::RosString>,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = rcl_interfaces__srv__GetParameterTypes_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__GetParameterTypes_Request()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__GetParameterTypes_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetParameterTypes_Request__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetParameterTypes_Request__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__GetParameterTypes_Request,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetParameterTypes_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetParameterTypes_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    names: msg.names.to_bytes_vec::<crate::RosString>(),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                msg.names.try_update_bytes(&self.names);
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
            pub types: Vec<u8>,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = rcl_interfaces__srv__GetParameterTypes_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__GetParameterTypes_Response()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__GetParameterTypes_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetParameterTypes_Response__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetParameterTypes_Response__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__GetParameterTypes_Response,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetParameterTypes_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetParameterTypes_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    types: msg.types.to_vec(),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                msg.types.update(&self.types);
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
    pub mod GetParameters {
        use super::super::super::*;
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Service();
        unsafe impl WrappedServiceTypeSupport for Service {
            type Request = Request;
            type Response = Response;
            fn get_ts() -> &'static rosidl_service_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_service_type_support_handle__rcl_interfaces__srv__GetParameters()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub names: Vec<crate::RosString>,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = rcl_interfaces__srv__GetParameters_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__GetParameters_Request()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__GetParameters_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetParameters_Request__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetParameters_Request__create()
            }
            fn destroy_msg(msg: *mut rcl_interfaces__srv__GetParameters_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetParameters_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetParameters_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    names: msg.names.to_bytes_vec::<crate::RosString>(),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                msg.names.try_update_bytes(&self.names);
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
            pub values: Vec<rcl_interfaces::msg::ParameterValue>,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = rcl_interfaces__srv__GetParameters_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__GetParameters_Response()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__GetParameters_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetParameters_Response__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetParameters_Response__create()
            }
            fn destroy_msg(msg: *mut rcl_interfaces__srv__GetParameters_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetParameters_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetParameters_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    values: {
                        let mut temp = Vec::with_capacity(msg.values.size);
                        let slice = unsafe {
                            std::slice::from_raw_parts(msg.values.data, msg.values.size)
                        };
                        for s in slice {
                            temp.push(
                                rcl_interfaces::msg::ParameterValue::from_native(s),
                            );
                        }
                        temp
                    },
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                unsafe {
                    rcl_interfaces__msg__ParameterValue__Sequence__fini(&mut msg.values);
                    rcl_interfaces__msg__ParameterValue__Sequence__init(
                        &mut msg.values,
                        self.values.len(),
                    );
                    let slice = std::slice::from_raw_parts_mut(
                        msg.values.data,
                        msg.values.size,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.values) {
                        s.copy_to_native(t);
                    }
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
    pub mod ListParameters {
        use super::super::super::*;
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Service();
        unsafe impl WrappedServiceTypeSupport for Service {
            type Request = Request;
            type Response = Response;
            fn get_ts() -> &'static rosidl_service_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_service_type_support_handle__rcl_interfaces__srv__ListParameters()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub prefixes: Vec<crate::RosString>,
            pub depth: u64,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = rcl_interfaces__srv__ListParameters_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__ListParameters_Request()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__ListParameters_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__ListParameters_Request__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__ListParameters_Request__create()
            }
            fn destroy_msg(msg: *mut rcl_interfaces__srv__ListParameters_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__ListParameters_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__ListParameters_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    prefixes: msg.prefixes.to_bytes_vec::<crate::RosString>(),
                    depth: msg.depth,
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                msg.prefixes.try_update_bytes(&self.prefixes);
                msg.depth = self.depth;
            }
        }
        impl Default for Request {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Request>::new();
                Request::from_native(&msg_native)
            }
        }
        #[allow(non_upper_case_globals)]
        impl Request {
            pub const DEPTH_RECURSIVE: _bindgen_ty_79 = rcl_interfaces__srv__ListParameters_Request__DEPTH_RECURSIVE;
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Response {
            pub result: rcl_interfaces::msg::ListParametersResult,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = rcl_interfaces__srv__ListParameters_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__ListParameters_Response()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__ListParameters_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__ListParameters_Response__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__ListParameters_Response__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__ListParameters_Response,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__ListParameters_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__ListParameters_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    result: rcl_interfaces::msg::ListParametersResult::from_native(
                        &msg.result,
                    ),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
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
    #[allow(non_snake_case)]
    pub mod SetParameters {
        use super::super::super::*;
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Service();
        unsafe impl WrappedServiceTypeSupport for Service {
            type Request = Request;
            type Response = Response;
            fn get_ts() -> &'static rosidl_service_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_service_type_support_handle__rcl_interfaces__srv__SetParameters()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub parameters: Vec<rcl_interfaces::msg::Parameter>,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = rcl_interfaces__srv__SetParameters_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__SetParameters_Request()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__SetParameters_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetParameters_Request__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetParameters_Request__create()
            }
            fn destroy_msg(msg: *mut rcl_interfaces__srv__SetParameters_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetParameters_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetParameters_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    parameters: {
                        let mut temp = Vec::with_capacity(msg.parameters.size);
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.parameters.data,
                                msg.parameters.size,
                            )
                        };
                        for s in slice {
                            temp.push(rcl_interfaces::msg::Parameter::from_native(s));
                        }
                        temp
                    },
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                unsafe {
                    rcl_interfaces__msg__Parameter__Sequence__fini(&mut msg.parameters);
                    rcl_interfaces__msg__Parameter__Sequence__init(
                        &mut msg.parameters,
                        self.parameters.len(),
                    );
                    let slice = std::slice::from_raw_parts_mut(
                        msg.parameters.data,
                        msg.parameters.size,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.parameters) {
                        s.copy_to_native(t);
                    }
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
            pub results: Vec<rcl_interfaces::msg::SetParametersResult>,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = rcl_interfaces__srv__SetParameters_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__SetParameters_Response()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__SetParameters_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetParameters_Response__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetParameters_Response__create()
            }
            fn destroy_msg(msg: *mut rcl_interfaces__srv__SetParameters_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetParameters_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetParameters_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    results: {
                        let mut temp = Vec::with_capacity(msg.results.size);
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.results.data,
                                msg.results.size,
                            )
                        };
                        for s in slice {
                            temp.push(
                                rcl_interfaces::msg::SetParametersResult::from_native(s),
                            );
                        }
                        temp
                    },
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                unsafe {
                    rcl_interfaces__msg__SetParametersResult__Sequence__fini(
                        &mut msg.results,
                    );
                    rcl_interfaces__msg__SetParametersResult__Sequence__init(
                        &mut msg.results,
                        self.results.len(),
                    );
                    let slice = std::slice::from_raw_parts_mut(
                        msg.results.data,
                        msg.results.size,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.results) {
                        s.copy_to_native(t);
                    }
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
    pub mod SetParametersAtomically {
        use super::super::super::*;
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct Service();
        unsafe impl WrappedServiceTypeSupport for Service {
            type Request = Request;
            type Response = Response;
            fn get_ts() -> &'static rosidl_service_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_service_type_support_handle__rcl_interfaces__srv__SetParametersAtomically()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub parameters: Vec<rcl_interfaces::msg::Parameter>,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = rcl_interfaces__srv__SetParametersAtomically_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__SetParametersAtomically_Request()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__SetParametersAtomically_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetParametersAtomically_Request__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetParametersAtomically_Request__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__SetParametersAtomically_Request,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    rcl_interfaces__srv__SetParametersAtomically_Request__destroy(msg)
                };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetParametersAtomically_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    parameters: {
                        let mut temp = Vec::with_capacity(msg.parameters.size);
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.parameters.data,
                                msg.parameters.size,
                            )
                        };
                        for s in slice {
                            temp.push(rcl_interfaces::msg::Parameter::from_native(s));
                        }
                        temp
                    },
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
                unsafe {
                    rcl_interfaces__msg__Parameter__Sequence__fini(&mut msg.parameters);
                    rcl_interfaces__msg__Parameter__Sequence__init(
                        &mut msg.parameters,
                        self.parameters.len(),
                    );
                    let slice = std::slice::from_raw_parts_mut(
                        msg.parameters.data,
                        msg.parameters.size,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.parameters) {
                        s.copy_to_native(t);
                    }
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
            pub result: rcl_interfaces::msg::SetParametersResult,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = rcl_interfaces__srv__SetParametersAtomically_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__SetParametersAtomically_Response()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__SetParametersAtomically_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    rcl_interfaces__srv__SetParametersAtomically_Response__create()
                }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetParametersAtomically_Response__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__SetParametersAtomically_Response,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    rcl_interfaces__srv__SetParametersAtomically_Response__destroy(msg)
                };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetParametersAtomically_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    result: rcl_interfaces::msg::SetParametersResult::from_native(
                        &msg.result,
                    ),
                }
            }
            fn copy_to_native(&self, #[allow(unused)] msg: &mut Self::CStruct) {
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
}
