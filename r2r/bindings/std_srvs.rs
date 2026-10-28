pub mod srv {
    #[allow(non_snake_case)]
    pub mod Empty {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__std_srvs__srv__Empty()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {}
        unsafe impl WrappedTypesupport for Request {
            type CStruct = std_srvs__srv__Empty_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__std_srvs__srv__Empty_Request()
                }
            }
            fn create_msg() -> *mut std_srvs__srv__Empty_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__Empty_Request__create() }
                #[cfg(feature = "doc-only")] std_srvs__srv__Empty_Request__create()
            }
            fn destroy_msg(msg: *mut std_srvs__srv__Empty_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__Empty_Request__destroy(msg) };
                #[cfg(feature = "doc-only")] std_srvs__srv__Empty_Request__destroy(msg)
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
        pub struct Response {}
        unsafe impl WrappedTypesupport for Response {
            type CStruct = std_srvs__srv__Empty_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__std_srvs__srv__Empty_Response()
                }
            }
            fn create_msg() -> *mut std_srvs__srv__Empty_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__Empty_Response__create() }
                #[cfg(feature = "doc-only")] std_srvs__srv__Empty_Response__create()
            }
            fn destroy_msg(msg: *mut std_srvs__srv__Empty_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__Empty_Response__destroy(msg) };
                #[cfg(feature = "doc-only")] std_srvs__srv__Empty_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {}
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
            pub request: Vec<std_srvs::srv::Empty::Request>,
            pub response: Vec<std_srvs::srv::Empty::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = std_srvs__srv__Empty_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__std_srvs__srv__Empty_Event()
                }
            }
            fn create_msg() -> *mut std_srvs__srv__Empty_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__Empty_Event__create() }
                #[cfg(feature = "doc-only")] std_srvs__srv__Empty_Event__create()
            }
            fn destroy_msg(msg: *mut std_srvs__srv__Empty_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__Empty_Event__destroy(msg) };
                #[cfg(feature = "doc-only")] std_srvs__srv__Empty_Event__destroy(msg)
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
                                temp.push(std_srvs::srv::Empty::Request::from_native(s));
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
                                temp.push(std_srvs::srv::Empty::Response::from_native(s));
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
                        if !std_srvs__srv__Empty_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        std_srvs__srv__Empty_Request__Sequence__fini(&mut msg.request);
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
                        if !std_srvs__srv__Empty_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        std_srvs__srv__Empty_Response__Sequence__fini(&mut msg.response);
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
    pub mod SetBool {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__std_srvs__srv__SetBool()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub data: bool,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = std_srvs__srv__SetBool_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__std_srvs__srv__SetBool_Request()
                }
            }
            fn create_msg() -> *mut std_srvs__srv__SetBool_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__SetBool_Request__create() }
                #[cfg(feature = "doc-only")] std_srvs__srv__SetBool_Request__create()
            }
            fn destroy_msg(msg: *mut std_srvs__srv__SetBool_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__SetBool_Request__destroy(msg) };
                #[cfg(feature = "doc-only")] std_srvs__srv__SetBool_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request { data: msg.data }
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
                msg.data = self.data;
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
            type CStruct = std_srvs__srv__SetBool_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__std_srvs__srv__SetBool_Response()
                }
            }
            fn create_msg() -> *mut std_srvs__srv__SetBool_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__SetBool_Response__create() }
                #[cfg(feature = "doc-only")] std_srvs__srv__SetBool_Response__create()
            }
            fn destroy_msg(msg: *mut std_srvs__srv__SetBool_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__SetBool_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                std_srvs__srv__SetBool_Response__destroy(msg)
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
            pub request: Vec<std_srvs::srv::SetBool::Request>,
            pub response: Vec<std_srvs::srv::SetBool::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = std_srvs__srv__SetBool_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__std_srvs__srv__SetBool_Event()
                }
            }
            fn create_msg() -> *mut std_srvs__srv__SetBool_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__SetBool_Event__create() }
                #[cfg(feature = "doc-only")] std_srvs__srv__SetBool_Event__create()
            }
            fn destroy_msg(msg: *mut std_srvs__srv__SetBool_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__SetBool_Event__destroy(msg) };
                #[cfg(feature = "doc-only")] std_srvs__srv__SetBool_Event__destroy(msg)
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
                                temp.push(std_srvs::srv::SetBool::Request::from_native(s));
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
                                temp.push(std_srvs::srv::SetBool::Response::from_native(s));
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
                        if !std_srvs__srv__SetBool_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        std_srvs__srv__SetBool_Request__Sequence__fini(&mut msg.request);
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
                        if !std_srvs__srv__SetBool_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        std_srvs__srv__SetBool_Response__Sequence__fini(
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
    pub mod Trigger {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__std_srvs__srv__Trigger()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {}
        unsafe impl WrappedTypesupport for Request {
            type CStruct = std_srvs__srv__Trigger_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__std_srvs__srv__Trigger_Request()
                }
            }
            fn create_msg() -> *mut std_srvs__srv__Trigger_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__Trigger_Request__create() }
                #[cfg(feature = "doc-only")] std_srvs__srv__Trigger_Request__create()
            }
            fn destroy_msg(msg: *mut std_srvs__srv__Trigger_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__Trigger_Request__destroy(msg) };
                #[cfg(feature = "doc-only")] std_srvs__srv__Trigger_Request__destroy(msg)
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
            pub success: bool,
            pub message: crate::RosString,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = std_srvs__srv__Trigger_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__std_srvs__srv__Trigger_Response()
                }
            }
            fn create_msg() -> *mut std_srvs__srv__Trigger_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__Trigger_Response__create() }
                #[cfg(feature = "doc-only")] std_srvs__srv__Trigger_Response__create()
            }
            fn destroy_msg(msg: *mut std_srvs__srv__Trigger_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__Trigger_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                std_srvs__srv__Trigger_Response__destroy(msg)
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
            pub request: Vec<std_srvs::srv::Trigger::Request>,
            pub response: Vec<std_srvs::srv::Trigger::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = std_srvs__srv__Trigger_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__std_srvs__srv__Trigger_Event()
                }
            }
            fn create_msg() -> *mut std_srvs__srv__Trigger_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__Trigger_Event__create() }
                #[cfg(feature = "doc-only")] std_srvs__srv__Trigger_Event__create()
            }
            fn destroy_msg(msg: *mut std_srvs__srv__Trigger_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { std_srvs__srv__Trigger_Event__destroy(msg) };
                #[cfg(feature = "doc-only")] std_srvs__srv__Trigger_Event__destroy(msg)
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
                                temp.push(std_srvs::srv::Trigger::Request::from_native(s));
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
                                temp.push(std_srvs::srv::Trigger::Response::from_native(s));
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
                        if !std_srvs__srv__Trigger_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        std_srvs__srv__Trigger_Request__Sequence__fini(&mut msg.request);
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
                        if !std_srvs__srv__Trigger_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        std_srvs__srv__Trigger_Response__Sequence__fini(
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
