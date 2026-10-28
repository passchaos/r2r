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
        fn copy_to_native(&self, msg: &mut Self::CStruct) {
            self.try_copy_to_native(msg)
                .expect("failed to copy Rust message into native ROS storage");
        }
        fn try_copy_to_native(
            &self,
            #[allow(unused)]
            msg: &mut Self::CStruct,
        ) -> crate::Result<()> {
            msg.from_value = self.from_value;
            msg.to_value = self.to_value;
            msg.step = self.step;
            Ok(())
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
        fn copy_to_native(&self, msg: &mut Self::CStruct) {
            self.try_copy_to_native(msg)
                .expect("failed to copy Rust message into native ROS storage");
        }
        fn try_copy_to_native(
            &self,
            #[allow(unused)]
            msg: &mut Self::CStruct,
        ) -> crate::Result<()> {
            msg.from_value = self.from_value;
            msg.to_value = self.to_value;
            msg.step = self.step;
            Ok(())
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
        fn copy_to_native(&self, msg: &mut Self::CStruct) {
            self.try_copy_to_native(msg)
                .expect("failed to copy Rust message into native ROS storage");
        }
        fn try_copy_to_native(
            &self,
            #[allow(unused)]
            msg: &mut Self::CStruct,
        ) -> crate::Result<()> {
            check_rosidl_runtime_ret(msg.names.try_update_bytes(&self.names))?;
            check_rosidl_runtime_ret(msg.prefixes.try_update_bytes(&self.prefixes))?;
            Ok(())
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
        fn copy_to_native(&self, msg: &mut Self::CStruct) {
            self.try_copy_to_native(msg)
                .expect("failed to copy Rust message into native ROS storage");
        }
        fn try_copy_to_native(
            &self,
            #[allow(unused)]
            msg: &mut Self::CStruct,
        ) -> crate::Result<()> {
            self.stamp.try_copy_to_native(&mut msg.stamp)?;
            msg.level = self.level;
            check_rosidl_runtime_ret(msg.name.try_assign_bytes(self.name.as_bytes()))?;
            check_rosidl_runtime_ret(msg.msg.try_assign_bytes(self.msg.as_bytes()))?;
            check_rosidl_runtime_ret(msg.file.try_assign_bytes(self.file.as_bytes()))?;
            check_rosidl_runtime_ret(
                msg.function.try_assign_bytes(self.function.as_bytes()),
            )?;
            msg.line = self.line;
            Ok(())
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
        pub const DEBUG: _bindgen_ty_211 = rcl_interfaces__msg__Log__DEBUG;
        pub const ERROR: _bindgen_ty_214 = rcl_interfaces__msg__Log__ERROR;
        pub const FATAL: _bindgen_ty_215 = rcl_interfaces__msg__Log__FATAL;
        pub const INFO: _bindgen_ty_212 = rcl_interfaces__msg__Log__INFO;
        pub const WARN: _bindgen_ty_213 = rcl_interfaces__msg__Log__WARN;
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct LoggerLevel {
        pub name: crate::RosString,
        pub level: u32,
    }
    unsafe impl WrappedTypesupport for LoggerLevel {
        type CStruct = rcl_interfaces__msg__LoggerLevel;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__LoggerLevel()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__LoggerLevel {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__LoggerLevel__create() }
            #[cfg(feature = "doc-only")] rcl_interfaces__msg__LoggerLevel__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__LoggerLevel) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__LoggerLevel__destroy(msg) };
            #[cfg(feature = "doc-only")] rcl_interfaces__msg__LoggerLevel__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> LoggerLevel {
            LoggerLevel {
                name: crate::RosString::from(msg.name.as_bytes()),
                level: msg.level,
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
            msg.level = self.level;
            Ok(())
        }
    }
    impl Default for LoggerLevel {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<LoggerLevel>::new();
            LoggerLevel::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl LoggerLevel {
        pub const LOG_LEVEL_DEBUG: _bindgen_ty_217 = rcl_interfaces__msg__LoggerLevel__LOG_LEVEL_DEBUG;
        pub const LOG_LEVEL_ERROR: _bindgen_ty_220 = rcl_interfaces__msg__LoggerLevel__LOG_LEVEL_ERROR;
        pub const LOG_LEVEL_FATAL: _bindgen_ty_221 = rcl_interfaces__msg__LoggerLevel__LOG_LEVEL_FATAL;
        pub const LOG_LEVEL_INFO: _bindgen_ty_218 = rcl_interfaces__msg__LoggerLevel__LOG_LEVEL_INFO;
        pub const LOG_LEVEL_UNKNOWN: _bindgen_ty_216 = rcl_interfaces__msg__LoggerLevel__LOG_LEVEL_UNKNOWN;
        pub const LOG_LEVEL_WARN: _bindgen_ty_219 = rcl_interfaces__msg__LoggerLevel__LOG_LEVEL_WARN;
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
            self.value.try_copy_to_native(&mut msg.value)?;
            Ok(())
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
                    if msg.floating_point_range.data != std::ptr::null_mut() {
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
                    }
                    temp
                },
                integer_range: {
                    let mut temp = Vec::with_capacity(msg.integer_range.size);
                    if msg.integer_range.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.integer_range.data,
                                msg.integer_range.size,
                            )
                        };
                        for s in slice {
                            temp.push(rcl_interfaces::msg::IntegerRange::from_native(s));
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
            check_rosidl_runtime_ret(msg.name.try_assign_bytes(self.name.as_bytes()))?;
            msg.type_ = self.type_;
            check_rosidl_runtime_ret(
                msg.description.try_assign_bytes(self.description.as_bytes()),
            )?;
            check_rosidl_runtime_ret(
                msg
                    .additional_constraints
                    .try_assign_bytes(self.additional_constraints.as_bytes()),
            )?;
            msg.read_only = self.read_only;
            msg.dynamic_typing = self.dynamic_typing;
            if self.floating_point_range.len() > 1usize {
                return Err(Error::RCL_RET_INVALID_ARGUMENT);
            }
            unsafe {
                let length = self.floating_point_range.len();
                if length > msg.floating_point_range.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rcl_interfaces__msg__FloatingPointRange__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rcl_interfaces__msg__FloatingPointRange__Sequence__fini(
                        &mut msg.floating_point_range,
                    );
                    msg.floating_point_range = replacement;
                }
                msg.floating_point_range.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.floating_point_range.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.floating_point_range) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            if self.integer_range.len() > 1usize {
                return Err(Error::RCL_RET_INVALID_ARGUMENT);
            }
            unsafe {
                let length = self.integer_range.len();
                if length > msg.integer_range.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rcl_interfaces__msg__IntegerRange__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rcl_interfaces__msg__IntegerRange__Sequence__fini(
                        &mut msg.integer_range,
                    );
                    msg.integer_range = replacement;
                }
                msg.integer_range.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.integer_range.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.integer_range) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
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
                    if msg.new_parameters.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.new_parameters.data,
                                msg.new_parameters.size,
                            )
                        };
                        for s in slice {
                            temp.push(rcl_interfaces::msg::Parameter::from_native(s));
                        }
                    }
                    temp
                },
                changed_parameters: {
                    let mut temp = Vec::with_capacity(msg.changed_parameters.size);
                    if msg.changed_parameters.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.changed_parameters.data,
                                msg.changed_parameters.size,
                            )
                        };
                        for s in slice {
                            temp.push(rcl_interfaces::msg::Parameter::from_native(s));
                        }
                    }
                    temp
                },
                deleted_parameters: {
                    let mut temp = Vec::with_capacity(msg.deleted_parameters.size);
                    if msg.deleted_parameters.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.deleted_parameters.data,
                                msg.deleted_parameters.size,
                            )
                        };
                        for s in slice {
                            temp.push(rcl_interfaces::msg::Parameter::from_native(s));
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
            self.stamp.try_copy_to_native(&mut msg.stamp)?;
            check_rosidl_runtime_ret(msg.node.try_assign_bytes(self.node.as_bytes()))?;
            unsafe {
                let length = self.new_parameters.len();
                if length > msg.new_parameters.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rcl_interfaces__msg__Parameter__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rcl_interfaces__msg__Parameter__Sequence__fini(
                        &mut msg.new_parameters,
                    );
                    msg.new_parameters = replacement;
                }
                msg.new_parameters.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.new_parameters.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.new_parameters) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.changed_parameters.len();
                if length > msg.changed_parameters.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rcl_interfaces__msg__Parameter__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rcl_interfaces__msg__Parameter__Sequence__fini(
                        &mut msg.changed_parameters,
                    );
                    msg.changed_parameters = replacement;
                }
                msg.changed_parameters.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.changed_parameters.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.changed_parameters) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.deleted_parameters.len();
                if length > msg.deleted_parameters.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rcl_interfaces__msg__Parameter__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rcl_interfaces__msg__Parameter__Sequence__fini(
                        &mut msg.deleted_parameters,
                    );
                    msg.deleted_parameters = replacement;
                }
                msg.deleted_parameters.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.deleted_parameters.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.deleted_parameters) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
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
                    if msg.new_parameters.data != std::ptr::null_mut() {
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
                    }
                    temp
                },
                changed_parameters: {
                    let mut temp = Vec::with_capacity(msg.changed_parameters.size);
                    if msg.changed_parameters.data != std::ptr::null_mut() {
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
                    }
                    temp
                },
                deleted_parameters: {
                    let mut temp = Vec::with_capacity(msg.deleted_parameters.size);
                    if msg.deleted_parameters.data != std::ptr::null_mut() {
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
            unsafe {
                let length = self.new_parameters.len();
                if length > msg.new_parameters.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rcl_interfaces__msg__ParameterDescriptor__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rcl_interfaces__msg__ParameterDescriptor__Sequence__fini(
                        &mut msg.new_parameters,
                    );
                    msg.new_parameters = replacement;
                }
                msg.new_parameters.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.new_parameters.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.new_parameters) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.changed_parameters.len();
                if length > msg.changed_parameters.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rcl_interfaces__msg__ParameterDescriptor__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rcl_interfaces__msg__ParameterDescriptor__Sequence__fini(
                        &mut msg.changed_parameters,
                    );
                    msg.changed_parameters = replacement;
                }
                msg.changed_parameters.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.changed_parameters.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.changed_parameters) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.deleted_parameters.len();
                if length > msg.deleted_parameters.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rcl_interfaces__msg__ParameterDescriptor__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rcl_interfaces__msg__ParameterDescriptor__Sequence__fini(
                        &mut msg.deleted_parameters,
                    );
                    msg.deleted_parameters = replacement;
                }
                msg.deleted_parameters.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.deleted_parameters.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.deleted_parameters) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
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
        fn copy_to_native(&self, msg: &mut Self::CStruct) {
            self.try_copy_to_native(msg)
                .expect("failed to copy Rust message into native ROS storage");
        }
        fn try_copy_to_native(
            &self,
            #[allow(unused)]
            msg: &mut Self::CStruct,
        ) -> crate::Result<()> {
            Ok(())
        }
    }
    impl Default for ParameterType {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<ParameterType>::new();
            ParameterType::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl ParameterType {
        pub const PARAMETER_BOOL: _bindgen_ty_225 = rcl_interfaces__msg__ParameterType__PARAMETER_BOOL;
        pub const PARAMETER_BOOL_ARRAY: _bindgen_ty_230 = rcl_interfaces__msg__ParameterType__PARAMETER_BOOL_ARRAY;
        pub const PARAMETER_BYTE_ARRAY: _bindgen_ty_229 = rcl_interfaces__msg__ParameterType__PARAMETER_BYTE_ARRAY;
        pub const PARAMETER_DOUBLE: _bindgen_ty_227 = rcl_interfaces__msg__ParameterType__PARAMETER_DOUBLE;
        pub const PARAMETER_DOUBLE_ARRAY: _bindgen_ty_232 = rcl_interfaces__msg__ParameterType__PARAMETER_DOUBLE_ARRAY;
        pub const PARAMETER_INTEGER: _bindgen_ty_226 = rcl_interfaces__msg__ParameterType__PARAMETER_INTEGER;
        pub const PARAMETER_INTEGER_ARRAY: _bindgen_ty_231 = rcl_interfaces__msg__ParameterType__PARAMETER_INTEGER_ARRAY;
        pub const PARAMETER_NOT_SET: _bindgen_ty_224 = rcl_interfaces__msg__ParameterType__PARAMETER_NOT_SET;
        pub const PARAMETER_STRING: _bindgen_ty_228 = rcl_interfaces__msg__ParameterType__PARAMETER_STRING;
        pub const PARAMETER_STRING_ARRAY: _bindgen_ty_233 = rcl_interfaces__msg__ParameterType__PARAMETER_STRING_ARRAY;
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
        fn copy_to_native(&self, msg: &mut Self::CStruct) {
            self.try_copy_to_native(msg)
                .expect("failed to copy Rust message into native ROS storage");
        }
        fn try_copy_to_native(
            &self,
            #[allow(unused)]
            msg: &mut Self::CStruct,
        ) -> crate::Result<()> {
            msg.type_ = self.type_;
            msg.bool_value = self.bool_value;
            msg.integer_value = self.integer_value;
            msg.double_value = self.double_value;
            check_rosidl_runtime_ret(
                msg.string_value.try_assign_bytes(self.string_value.as_bytes()),
            )?;
            check_rosidl_runtime_ret(
                msg.byte_array_value.try_update(&self.byte_array_value),
            )?;
            check_rosidl_runtime_ret(
                msg.bool_array_value.try_update(&self.bool_array_value),
            )?;
            check_rosidl_runtime_ret(
                msg.integer_array_value.try_update(&self.integer_array_value),
            )?;
            check_rosidl_runtime_ret(
                msg.double_array_value.try_update(&self.double_array_value),
            )?;
            check_rosidl_runtime_ret(
                msg.string_array_value.try_update_bytes(&self.string_array_value),
            )?;
            Ok(())
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
    pub struct SetLoggerLevelsResult {
        pub successful: bool,
        pub reason: crate::RosString,
    }
    unsafe impl WrappedTypesupport for SetLoggerLevelsResult {
        type CStruct = rcl_interfaces__msg__SetLoggerLevelsResult;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__msg__SetLoggerLevelsResult()
            }
        }
        fn create_msg() -> *mut rcl_interfaces__msg__SetLoggerLevelsResult {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__SetLoggerLevelsResult__create() }
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__SetLoggerLevelsResult__create()
        }
        fn destroy_msg(msg: *mut rcl_interfaces__msg__SetLoggerLevelsResult) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rcl_interfaces__msg__SetLoggerLevelsResult__destroy(msg) };
            #[cfg(feature = "doc-only")]
            rcl_interfaces__msg__SetLoggerLevelsResult__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> SetLoggerLevelsResult {
            SetLoggerLevelsResult {
                successful: msg.successful,
                reason: crate::RosString::from(msg.reason.as_bytes()),
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
                msg.reason.try_assign_bytes(self.reason.as_bytes()),
            )?;
            Ok(())
        }
    }
    impl Default for SetLoggerLevelsResult {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<SetLoggerLevelsResult>::new();
            SetLoggerLevelsResult::from_native(&msg_native)
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
                msg.reason.try_assign_bytes(self.reason.as_bytes()),
            )?;
            Ok(())
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
            fn copy_to_native(&self, msg: &mut Self::CStruct) {
                self.try_copy_to_native(msg)
                    .expect("failed to copy Rust message into native ROS storage");
            }
            fn try_copy_to_native(
                &self,
                #[allow(unused)]
                msg: &mut Self::CStruct,
            ) -> crate::Result<()> {
                check_rosidl_runtime_ret(msg.names.try_update_bytes(&self.names))?;
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
                        if msg.descriptors.data != std::ptr::null_mut() {
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
                unsafe {
                    let length = self.descriptors.len();
                    if length > msg.descriptors.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !rcl_interfaces__msg__ParameterDescriptor__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__msg__ParameterDescriptor__Sequence__fini(
                            &mut msg.descriptors,
                        );
                        msg.descriptors = replacement;
                    }
                    msg.descriptors.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.descriptors.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.descriptors) {
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
            pub request: Vec<rcl_interfaces::srv::DescribeParameters::Request>,
            pub response: Vec<rcl_interfaces::srv::DescribeParameters::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = rcl_interfaces__srv__DescribeParameters_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__DescribeParameters_Event()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__DescribeParameters_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__DescribeParameters_Event__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__DescribeParameters_Event__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__DescribeParameters_Event,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__DescribeParameters_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__DescribeParameters_Event__destroy(msg)
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
                                    rcl_interfaces::srv::DescribeParameters::Request::from_native(
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
                                    rcl_interfaces::srv::DescribeParameters::Response::from_native(
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
                        if !rcl_interfaces__srv__DescribeParameters_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__DescribeParameters_Request__Sequence__fini(
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
                        if !rcl_interfaces__srv__DescribeParameters_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__DescribeParameters_Response__Sequence__fini(
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
    #[allow(non_snake_case)]
    pub mod GetLoggerLevels {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__rcl_interfaces__srv__GetLoggerLevels()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub names: Vec<crate::RosString>,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = rcl_interfaces__srv__GetLoggerLevels_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__GetLoggerLevels_Request()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__GetLoggerLevels_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetLoggerLevels_Request__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetLoggerLevels_Request__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__GetLoggerLevels_Request,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetLoggerLevels_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetLoggerLevels_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    names: msg.names.to_bytes_vec::<crate::RosString>(),
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
                check_rosidl_runtime_ret(msg.names.try_update_bytes(&self.names))?;
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
            pub levels: Vec<rcl_interfaces::msg::LoggerLevel>,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = rcl_interfaces__srv__GetLoggerLevels_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__GetLoggerLevels_Response()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__GetLoggerLevels_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetLoggerLevels_Response__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetLoggerLevels_Response__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__GetLoggerLevels_Response,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetLoggerLevels_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetLoggerLevels_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    levels: {
                        let mut temp = Vec::with_capacity(msg.levels.size);
                        if msg.levels.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(msg.levels.data, msg.levels.size)
                            };
                            for s in slice {
                                temp.push(rcl_interfaces::msg::LoggerLevel::from_native(s));
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
                unsafe {
                    let length = self.levels.len();
                    if length > msg.levels.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !rcl_interfaces__msg__LoggerLevel__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__msg__LoggerLevel__Sequence__fini(
                            &mut msg.levels,
                        );
                        msg.levels = replacement;
                    }
                    msg.levels.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.levels.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.levels) {
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
            pub request: Vec<rcl_interfaces::srv::GetLoggerLevels::Request>,
            pub response: Vec<rcl_interfaces::srv::GetLoggerLevels::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = rcl_interfaces__srv__GetLoggerLevels_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__GetLoggerLevels_Event()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__GetLoggerLevels_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetLoggerLevels_Event__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetLoggerLevels_Event__create()
            }
            fn destroy_msg(msg: *mut rcl_interfaces__srv__GetLoggerLevels_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetLoggerLevels_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetLoggerLevels_Event__destroy(msg)
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
                                    rcl_interfaces::srv::GetLoggerLevels::Request::from_native(
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
                                    rcl_interfaces::srv::GetLoggerLevels::Response::from_native(
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
                        if !rcl_interfaces__srv__GetLoggerLevels_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__GetLoggerLevels_Request__Sequence__fini(
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
                        if !rcl_interfaces__srv__GetLoggerLevels_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__GetLoggerLevels_Response__Sequence__fini(
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
    #[allow(non_snake_case)]
    pub mod GetParameterTypes {
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
            fn copy_to_native(&self, msg: &mut Self::CStruct) {
                self.try_copy_to_native(msg)
                    .expect("failed to copy Rust message into native ROS storage");
            }
            fn try_copy_to_native(
                &self,
                #[allow(unused)]
                msg: &mut Self::CStruct,
            ) -> crate::Result<()> {
                check_rosidl_runtime_ret(msg.names.try_update_bytes(&self.names))?;
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
            fn copy_to_native(&self, msg: &mut Self::CStruct) {
                self.try_copy_to_native(msg)
                    .expect("failed to copy Rust message into native ROS storage");
            }
            fn try_copy_to_native(
                &self,
                #[allow(unused)]
                msg: &mut Self::CStruct,
            ) -> crate::Result<()> {
                check_rosidl_runtime_ret(msg.types.try_update(&self.types))?;
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
            pub request: Vec<rcl_interfaces::srv::GetParameterTypes::Request>,
            pub response: Vec<rcl_interfaces::srv::GetParameterTypes::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = rcl_interfaces__srv__GetParameterTypes_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__GetParameterTypes_Event()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__GetParameterTypes_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetParameterTypes_Event__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetParameterTypes_Event__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__GetParameterTypes_Event,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetParameterTypes_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetParameterTypes_Event__destroy(msg)
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
                                    rcl_interfaces::srv::GetParameterTypes::Request::from_native(
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
                                    rcl_interfaces::srv::GetParameterTypes::Response::from_native(
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
                        if !rcl_interfaces__srv__GetParameterTypes_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__GetParameterTypes_Request__Sequence__fini(
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
                        if !rcl_interfaces__srv__GetParameterTypes_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__GetParameterTypes_Response__Sequence__fini(
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
    #[allow(non_snake_case)]
    pub mod GetParameters {
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
            fn copy_to_native(&self, msg: &mut Self::CStruct) {
                self.try_copy_to_native(msg)
                    .expect("failed to copy Rust message into native ROS storage");
            }
            fn try_copy_to_native(
                &self,
                #[allow(unused)]
                msg: &mut Self::CStruct,
            ) -> crate::Result<()> {
                check_rosidl_runtime_ret(msg.names.try_update_bytes(&self.names))?;
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
                        if msg.values.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(msg.values.data, msg.values.size)
                            };
                            for s in slice {
                                temp.push(
                                    rcl_interfaces::msg::ParameterValue::from_native(s),
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
                unsafe {
                    let length = self.values.len();
                    if length > msg.values.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !rcl_interfaces__msg__ParameterValue__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__msg__ParameterValue__Sequence__fini(
                            &mut msg.values,
                        );
                        msg.values = replacement;
                    }
                    msg.values.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.values.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.values) {
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
            pub request: Vec<rcl_interfaces::srv::GetParameters::Request>,
            pub response: Vec<rcl_interfaces::srv::GetParameters::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = rcl_interfaces__srv__GetParameters_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__GetParameters_Event()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__GetParameters_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetParameters_Event__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetParameters_Event__create()
            }
            fn destroy_msg(msg: *mut rcl_interfaces__srv__GetParameters_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__GetParameters_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__GetParameters_Event__destroy(msg)
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
                                    rcl_interfaces::srv::GetParameters::Request::from_native(s),
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
                                    rcl_interfaces::srv::GetParameters::Response::from_native(s),
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
                        if !rcl_interfaces__srv__GetParameters_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__GetParameters_Request__Sequence__fini(
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
                        if !rcl_interfaces__srv__GetParameters_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__GetParameters_Response__Sequence__fini(
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
    #[allow(non_snake_case)]
    pub mod ListParameters {
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
            fn copy_to_native(&self, msg: &mut Self::CStruct) {
                self.try_copy_to_native(msg)
                    .expect("failed to copy Rust message into native ROS storage");
            }
            fn try_copy_to_native(
                &self,
                #[allow(unused)]
                msg: &mut Self::CStruct,
            ) -> crate::Result<()> {
                check_rosidl_runtime_ret(msg.prefixes.try_update_bytes(&self.prefixes))?;
                msg.depth = self.depth;
                Ok(())
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
            pub const DEPTH_RECURSIVE: _bindgen_ty_242 = rcl_interfaces__srv__ListParameters_Request__DEPTH_RECURSIVE;
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
            fn copy_to_native(&self, msg: &mut Self::CStruct) {
                self.try_copy_to_native(msg)
                    .expect("failed to copy Rust message into native ROS storage");
            }
            fn try_copy_to_native(
                &self,
                #[allow(unused)]
                msg: &mut Self::CStruct,
            ) -> crate::Result<()> {
                self.result.try_copy_to_native(&mut msg.result)?;
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
            pub request: Vec<rcl_interfaces::srv::ListParameters::Request>,
            pub response: Vec<rcl_interfaces::srv::ListParameters::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = rcl_interfaces__srv__ListParameters_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__ListParameters_Event()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__ListParameters_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__ListParameters_Event__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__ListParameters_Event__create()
            }
            fn destroy_msg(msg: *mut rcl_interfaces__srv__ListParameters_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__ListParameters_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__ListParameters_Event__destroy(msg)
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
                                    rcl_interfaces::srv::ListParameters::Request::from_native(s),
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
                                    rcl_interfaces::srv::ListParameters::Response::from_native(
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
                        if !rcl_interfaces__srv__ListParameters_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__ListParameters_Request__Sequence__fini(
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
                        if !rcl_interfaces__srv__ListParameters_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__ListParameters_Response__Sequence__fini(
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
    #[allow(non_snake_case)]
    pub mod SetLoggerLevels {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__rcl_interfaces__srv__SetLoggerLevels()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub levels: Vec<rcl_interfaces::msg::LoggerLevel>,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = rcl_interfaces__srv__SetLoggerLevels_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__SetLoggerLevels_Request()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__SetLoggerLevels_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetLoggerLevels_Request__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetLoggerLevels_Request__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__SetLoggerLevels_Request,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetLoggerLevels_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetLoggerLevels_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    levels: {
                        let mut temp = Vec::with_capacity(msg.levels.size);
                        if msg.levels.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(msg.levels.data, msg.levels.size)
                            };
                            for s in slice {
                                temp.push(rcl_interfaces::msg::LoggerLevel::from_native(s));
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
                unsafe {
                    let length = self.levels.len();
                    if length > msg.levels.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !rcl_interfaces__msg__LoggerLevel__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__msg__LoggerLevel__Sequence__fini(
                            &mut msg.levels,
                        );
                        msg.levels = replacement;
                    }
                    msg.levels.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.levels.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.levels) {
                            s.try_copy_to_native(t)?;
                        }
                    }
                }
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
            pub results: Vec<rcl_interfaces::msg::SetLoggerLevelsResult>,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = rcl_interfaces__srv__SetLoggerLevels_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__SetLoggerLevels_Response()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__SetLoggerLevels_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetLoggerLevels_Response__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetLoggerLevels_Response__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__SetLoggerLevels_Response,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetLoggerLevels_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetLoggerLevels_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    results: {
                        let mut temp = Vec::with_capacity(msg.results.size);
                        if msg.results.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                    msg.results.data,
                                    msg.results.size,
                                )
                            };
                            for s in slice {
                                temp.push(
                                    rcl_interfaces::msg::SetLoggerLevelsResult::from_native(s),
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
                unsafe {
                    let length = self.results.len();
                    if length > msg.results.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !rcl_interfaces__msg__SetLoggerLevelsResult__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__msg__SetLoggerLevelsResult__Sequence__fini(
                            &mut msg.results,
                        );
                        msg.results = replacement;
                    }
                    msg.results.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.results.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.results) {
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
            pub request: Vec<rcl_interfaces::srv::SetLoggerLevels::Request>,
            pub response: Vec<rcl_interfaces::srv::SetLoggerLevels::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = rcl_interfaces__srv__SetLoggerLevels_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__SetLoggerLevels_Event()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__SetLoggerLevels_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetLoggerLevels_Event__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetLoggerLevels_Event__create()
            }
            fn destroy_msg(msg: *mut rcl_interfaces__srv__SetLoggerLevels_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetLoggerLevels_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetLoggerLevels_Event__destroy(msg)
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
                                    rcl_interfaces::srv::SetLoggerLevels::Request::from_native(
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
                                    rcl_interfaces::srv::SetLoggerLevels::Response::from_native(
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
                        if !rcl_interfaces__srv__SetLoggerLevels_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__SetLoggerLevels_Request__Sequence__fini(
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
                        if !rcl_interfaces__srv__SetLoggerLevels_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__SetLoggerLevels_Response__Sequence__fini(
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
    #[allow(non_snake_case)]
    pub mod SetParameters {
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
                        if msg.parameters.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                    msg.parameters.data,
                                    msg.parameters.size,
                                )
                            };
                            for s in slice {
                                temp.push(rcl_interfaces::msg::Parameter::from_native(s));
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
                unsafe {
                    let length = self.parameters.len();
                    if length > msg.parameters.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !rcl_interfaces__msg__Parameter__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__msg__Parameter__Sequence__fini(
                            &mut msg.parameters,
                        );
                        msg.parameters = replacement;
                    }
                    msg.parameters.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.parameters.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.parameters) {
                            s.try_copy_to_native(t)?;
                        }
                    }
                }
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
                        if msg.results.data != std::ptr::null_mut() {
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
                unsafe {
                    let length = self.results.len();
                    if length > msg.results.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !rcl_interfaces__msg__SetParametersResult__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__msg__SetParametersResult__Sequence__fini(
                            &mut msg.results,
                        );
                        msg.results = replacement;
                    }
                    msg.results.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.results.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.results) {
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
            pub request: Vec<rcl_interfaces::srv::SetParameters::Request>,
            pub response: Vec<rcl_interfaces::srv::SetParameters::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = rcl_interfaces__srv__SetParameters_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__SetParameters_Event()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__SetParameters_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetParameters_Event__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetParameters_Event__create()
            }
            fn destroy_msg(msg: *mut rcl_interfaces__srv__SetParameters_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetParameters_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetParameters_Event__destroy(msg)
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
                                    rcl_interfaces::srv::SetParameters::Request::from_native(s),
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
                                    rcl_interfaces::srv::SetParameters::Response::from_native(s),
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
                        if !rcl_interfaces__srv__SetParameters_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__SetParameters_Request__Sequence__fini(
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
                        if !rcl_interfaces__srv__SetParameters_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__SetParameters_Response__Sequence__fini(
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
    #[allow(non_snake_case)]
    pub mod SetParametersAtomically {
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
                        if msg.parameters.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                    msg.parameters.data,
                                    msg.parameters.size,
                                )
                            };
                            for s in slice {
                                temp.push(rcl_interfaces::msg::Parameter::from_native(s));
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
                unsafe {
                    let length = self.parameters.len();
                    if length > msg.parameters.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !rcl_interfaces__msg__Parameter__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__msg__Parameter__Sequence__fini(
                            &mut msg.parameters,
                        );
                        msg.parameters = replacement;
                    }
                    msg.parameters.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.parameters.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.parameters) {
                            s.try_copy_to_native(t)?;
                        }
                    }
                }
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
            fn copy_to_native(&self, msg: &mut Self::CStruct) {
                self.try_copy_to_native(msg)
                    .expect("failed to copy Rust message into native ROS storage");
            }
            fn try_copy_to_native(
                &self,
                #[allow(unused)]
                msg: &mut Self::CStruct,
            ) -> crate::Result<()> {
                self.result.try_copy_to_native(&mut msg.result)?;
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
            pub request: Vec<rcl_interfaces::srv::SetParametersAtomically::Request>,
            pub response: Vec<rcl_interfaces::srv::SetParametersAtomically::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = rcl_interfaces__srv__SetParametersAtomically_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__rcl_interfaces__srv__SetParametersAtomically_Event()
                }
            }
            fn create_msg() -> *mut rcl_interfaces__srv__SetParametersAtomically_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { rcl_interfaces__srv__SetParametersAtomically_Event__create() }
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetParametersAtomically_Event__create()
            }
            fn destroy_msg(
                msg: *mut rcl_interfaces__srv__SetParametersAtomically_Event,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    rcl_interfaces__srv__SetParametersAtomically_Event__destroy(msg)
                };
                #[cfg(feature = "doc-only")]
                rcl_interfaces__srv__SetParametersAtomically_Event__destroy(msg)
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
                                    rcl_interfaces::srv::SetParametersAtomically::Request::from_native(
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
                                    rcl_interfaces::srv::SetParametersAtomically::Response::from_native(
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
                        if !rcl_interfaces__srv__SetParametersAtomically_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__SetParametersAtomically_Request__Sequence__fini(
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
                        if !rcl_interfaces__srv__SetParametersAtomically_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        rcl_interfaces__srv__SetParametersAtomically_Response__Sequence__fini(
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
