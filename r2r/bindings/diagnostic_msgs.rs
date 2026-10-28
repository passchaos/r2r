pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct DiagnosticArray {
        pub header: std_msgs::msg::Header,
        pub status: Vec<diagnostic_msgs::msg::DiagnosticStatus>,
    }
    unsafe impl WrappedTypesupport for DiagnosticArray {
        type CStruct = diagnostic_msgs__msg__DiagnosticArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__diagnostic_msgs__msg__DiagnosticArray()
            }
        }
        fn create_msg() -> *mut diagnostic_msgs__msg__DiagnosticArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { diagnostic_msgs__msg__DiagnosticArray__create() }
            #[cfg(feature = "doc-only")] diagnostic_msgs__msg__DiagnosticArray__create()
        }
        fn destroy_msg(msg: *mut diagnostic_msgs__msg__DiagnosticArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { diagnostic_msgs__msg__DiagnosticArray__destroy(msg) };
            #[cfg(feature = "doc-only")]
            diagnostic_msgs__msg__DiagnosticArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> DiagnosticArray {
            DiagnosticArray {
                header: std_msgs::msg::Header::from_native(&msg.header),
                status: {
                    let mut temp = Vec::with_capacity(msg.status.size);
                    if msg.status.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(msg.status.data, msg.status.size)
                        };
                        for s in slice {
                            temp.push(
                                diagnostic_msgs::msg::DiagnosticStatus::from_native(s),
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
            self.header.try_copy_to_native(&mut msg.header)?;
            unsafe {
                let length = self.status.len();
                if length > msg.status.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !diagnostic_msgs__msg__DiagnosticStatus__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    diagnostic_msgs__msg__DiagnosticStatus__Sequence__fini(
                        &mut msg.status,
                    );
                    msg.status = replacement;
                }
                msg.status.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(msg.status.data, length);
                    for (t, s) in slice.iter_mut().zip(&self.status) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for DiagnosticArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<DiagnosticArray>::new();
            DiagnosticArray::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct DiagnosticStatus {
        pub level: u8,
        pub name: crate::RosString,
        pub message: crate::RosString,
        pub hardware_id: crate::RosString,
        pub values: Vec<diagnostic_msgs::msg::KeyValue>,
    }
    unsafe impl WrappedTypesupport for DiagnosticStatus {
        type CStruct = diagnostic_msgs__msg__DiagnosticStatus;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__diagnostic_msgs__msg__DiagnosticStatus()
            }
        }
        fn create_msg() -> *mut diagnostic_msgs__msg__DiagnosticStatus {
            #[cfg(not(feature = "doc-only"))]
            unsafe { diagnostic_msgs__msg__DiagnosticStatus__create() }
            #[cfg(feature = "doc-only")] diagnostic_msgs__msg__DiagnosticStatus__create()
        }
        fn destroy_msg(msg: *mut diagnostic_msgs__msg__DiagnosticStatus) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { diagnostic_msgs__msg__DiagnosticStatus__destroy(msg) };
            #[cfg(feature = "doc-only")]
            diagnostic_msgs__msg__DiagnosticStatus__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> DiagnosticStatus {
            DiagnosticStatus {
                level: msg.level,
                name: crate::RosString::from(msg.name.as_bytes()),
                message: crate::RosString::from(msg.message.as_bytes()),
                hardware_id: crate::RosString::from(msg.hardware_id.as_bytes()),
                values: {
                    let mut temp = Vec::with_capacity(msg.values.size);
                    if msg.values.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(msg.values.data, msg.values.size)
                        };
                        for s in slice {
                            temp.push(diagnostic_msgs::msg::KeyValue::from_native(s));
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
            msg.level = self.level;
            check_rosidl_runtime_ret(msg.name.try_assign_bytes(self.name.as_bytes()))?;
            check_rosidl_runtime_ret(
                msg.message.try_assign_bytes(self.message.as_bytes()),
            )?;
            check_rosidl_runtime_ret(
                msg.hardware_id.try_assign_bytes(self.hardware_id.as_bytes()),
            )?;
            unsafe {
                let length = self.values.len();
                if length > msg.values.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !diagnostic_msgs__msg__KeyValue__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    diagnostic_msgs__msg__KeyValue__Sequence__fini(&mut msg.values);
                    msg.values = replacement;
                }
                msg.values.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(msg.values.data, length);
                    for (t, s) in slice.iter_mut().zip(&self.values) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for DiagnosticStatus {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<DiagnosticStatus>::new();
            DiagnosticStatus::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl DiagnosticStatus {
        pub const ERROR: _bindgen_ty_111 = diagnostic_msgs__msg__DiagnosticStatus__ERROR;
        pub const OK: _bindgen_ty_109 = diagnostic_msgs__msg__DiagnosticStatus__OK;
        pub const STALE: _bindgen_ty_112 = diagnostic_msgs__msg__DiagnosticStatus__STALE;
        pub const WARN: _bindgen_ty_110 = diagnostic_msgs__msg__DiagnosticStatus__WARN;
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct KeyValue {
        pub key: crate::RosString,
        pub value: crate::RosString,
    }
    unsafe impl WrappedTypesupport for KeyValue {
        type CStruct = diagnostic_msgs__msg__KeyValue;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__diagnostic_msgs__msg__KeyValue()
            }
        }
        fn create_msg() -> *mut diagnostic_msgs__msg__KeyValue {
            #[cfg(not(feature = "doc-only"))]
            unsafe { diagnostic_msgs__msg__KeyValue__create() }
            #[cfg(feature = "doc-only")] diagnostic_msgs__msg__KeyValue__create()
        }
        fn destroy_msg(msg: *mut diagnostic_msgs__msg__KeyValue) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { diagnostic_msgs__msg__KeyValue__destroy(msg) };
            #[cfg(feature = "doc-only")] diagnostic_msgs__msg__KeyValue__destroy(msg)
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
}
pub mod srv {
    #[allow(non_snake_case)]
    pub mod AddDiagnostics {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__diagnostic_msgs__srv__AddDiagnostics()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub load_namespace: crate::RosString,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = diagnostic_msgs__srv__AddDiagnostics_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__diagnostic_msgs__srv__AddDiagnostics_Request()
                }
            }
            fn create_msg() -> *mut diagnostic_msgs__srv__AddDiagnostics_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { diagnostic_msgs__srv__AddDiagnostics_Request__create() }
                #[cfg(feature = "doc-only")]
                diagnostic_msgs__srv__AddDiagnostics_Request__create()
            }
            fn destroy_msg(
                msg: *mut diagnostic_msgs__srv__AddDiagnostics_Request,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { diagnostic_msgs__srv__AddDiagnostics_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                diagnostic_msgs__srv__AddDiagnostics_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    load_namespace: crate::RosString::from(msg.load_namespace.as_bytes()),
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
                    msg.load_namespace.try_assign_bytes(self.load_namespace.as_bytes()),
                )?;
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
            pub success: bool,
            pub message: crate::RosString,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = diagnostic_msgs__srv__AddDiagnostics_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__diagnostic_msgs__srv__AddDiagnostics_Response()
                }
            }
            fn create_msg() -> *mut diagnostic_msgs__srv__AddDiagnostics_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { diagnostic_msgs__srv__AddDiagnostics_Response__create() }
                #[cfg(feature = "doc-only")]
                diagnostic_msgs__srv__AddDiagnostics_Response__create()
            }
            fn destroy_msg(
                msg: *mut diagnostic_msgs__srv__AddDiagnostics_Response,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { diagnostic_msgs__srv__AddDiagnostics_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                diagnostic_msgs__srv__AddDiagnostics_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    success: msg.success,
                    message: crate::RosString::from(msg.message.as_bytes()),
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
                msg.success = self.success;
                check_rosidl_runtime_ret(
                    msg.message.try_assign_bytes(self.message.as_bytes()),
                )?;
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
            pub request: Vec<diagnostic_msgs::srv::AddDiagnostics::Request>,
            pub response: Vec<diagnostic_msgs::srv::AddDiagnostics::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = diagnostic_msgs__srv__AddDiagnostics_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__diagnostic_msgs__srv__AddDiagnostics_Event()
                }
            }
            fn create_msg() -> *mut diagnostic_msgs__srv__AddDiagnostics_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { diagnostic_msgs__srv__AddDiagnostics_Event__create() }
                #[cfg(feature = "doc-only")]
                diagnostic_msgs__srv__AddDiagnostics_Event__create()
            }
            fn destroy_msg(msg: *mut diagnostic_msgs__srv__AddDiagnostics_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { diagnostic_msgs__srv__AddDiagnostics_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                diagnostic_msgs__srv__AddDiagnostics_Event__destroy(msg)
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
                                    diagnostic_msgs::srv::AddDiagnostics::Request::from_native(
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
                                    diagnostic_msgs::srv::AddDiagnostics::Response::from_native(
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
                        if !diagnostic_msgs__srv__AddDiagnostics_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        diagnostic_msgs__srv__AddDiagnostics_Request__Sequence__fini(
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
                        if !diagnostic_msgs__srv__AddDiagnostics_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        diagnostic_msgs__srv__AddDiagnostics_Response__Sequence__fini(
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
    pub mod SelfTest {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__diagnostic_msgs__srv__SelfTest()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {}
        unsafe impl WrappedTypesupport for Request {
            type CStruct = diagnostic_msgs__srv__SelfTest_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__diagnostic_msgs__srv__SelfTest_Request()
                }
            }
            fn create_msg() -> *mut diagnostic_msgs__srv__SelfTest_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { diagnostic_msgs__srv__SelfTest_Request__create() }
                #[cfg(feature = "doc-only")]
                diagnostic_msgs__srv__SelfTest_Request__create()
            }
            fn destroy_msg(msg: *mut diagnostic_msgs__srv__SelfTest_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { diagnostic_msgs__srv__SelfTest_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                diagnostic_msgs__srv__SelfTest_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {}
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
        impl Default for Request {
            fn default() -> Self {
                let msg_native = WrappedNativeMsg::<Request>::new();
                Request::from_native(&msg_native)
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Response {
            pub id: crate::RosString,
            pub passed: u8,
            pub status: Vec<diagnostic_msgs::msg::DiagnosticStatus>,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = diagnostic_msgs__srv__SelfTest_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__diagnostic_msgs__srv__SelfTest_Response()
                }
            }
            fn create_msg() -> *mut diagnostic_msgs__srv__SelfTest_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { diagnostic_msgs__srv__SelfTest_Response__create() }
                #[cfg(feature = "doc-only")]
                diagnostic_msgs__srv__SelfTest_Response__create()
            }
            fn destroy_msg(msg: *mut diagnostic_msgs__srv__SelfTest_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { diagnostic_msgs__srv__SelfTest_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                diagnostic_msgs__srv__SelfTest_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    id: crate::RosString::from(msg.id.as_bytes()),
                    passed: msg.passed,
                    status: {
                        let mut temp = Vec::with_capacity(msg.status.size);
                        if msg.status.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(msg.status.data, msg.status.size)
                            };
                            for s in slice {
                                temp.push(
                                    diagnostic_msgs::msg::DiagnosticStatus::from_native(s),
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
                check_rosidl_runtime_ret(msg.id.try_assign_bytes(self.id.as_bytes()))?;
                msg.passed = self.passed;
                unsafe {
                    let length = self.status.len();
                    if length > msg.status.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !diagnostic_msgs__msg__DiagnosticStatus__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        diagnostic_msgs__msg__DiagnosticStatus__Sequence__fini(
                            &mut msg.status,
                        );
                        msg.status = replacement;
                    }
                    msg.status.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.status.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.status) {
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
            pub request: Vec<diagnostic_msgs::srv::SelfTest::Request>,
            pub response: Vec<diagnostic_msgs::srv::SelfTest::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = diagnostic_msgs__srv__SelfTest_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__diagnostic_msgs__srv__SelfTest_Event()
                }
            }
            fn create_msg() -> *mut diagnostic_msgs__srv__SelfTest_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { diagnostic_msgs__srv__SelfTest_Event__create() }
                #[cfg(feature = "doc-only")]
                diagnostic_msgs__srv__SelfTest_Event__create()
            }
            fn destroy_msg(msg: *mut diagnostic_msgs__srv__SelfTest_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { diagnostic_msgs__srv__SelfTest_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                diagnostic_msgs__srv__SelfTest_Event__destroy(msg)
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
                                    diagnostic_msgs::srv::SelfTest::Request::from_native(s),
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
                                    diagnostic_msgs::srv::SelfTest::Response::from_native(s),
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
                        if !diagnostic_msgs__srv__SelfTest_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        diagnostic_msgs__srv__SelfTest_Request__Sequence__fini(
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
                        if !diagnostic_msgs__srv__SelfTest_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        diagnostic_msgs__srv__SelfTest_Response__Sequence__fini(
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
