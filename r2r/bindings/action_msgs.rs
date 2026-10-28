pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct GoalInfo {
        pub goal_id: unique_identifier_msgs::msg::UUID,
        pub stamp: builtin_interfaces::msg::Time,
    }
    unsafe impl WrappedTypesupport for GoalInfo {
        type CStruct = action_msgs__msg__GoalInfo;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__action_msgs__msg__GoalInfo()
            }
        }
        fn create_msg() -> *mut action_msgs__msg__GoalInfo {
            #[cfg(not(feature = "doc-only"))]
            unsafe { action_msgs__msg__GoalInfo__create() }
            #[cfg(feature = "doc-only")] action_msgs__msg__GoalInfo__create()
        }
        fn destroy_msg(msg: *mut action_msgs__msg__GoalInfo) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { action_msgs__msg__GoalInfo__destroy(msg) };
            #[cfg(feature = "doc-only")] action_msgs__msg__GoalInfo__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> GoalInfo {
            GoalInfo {
                goal_id: unique_identifier_msgs::msg::UUID::from_native(&msg.goal_id),
                stamp: builtin_interfaces::msg::Time::from_native(&msg.stamp),
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
            self.goal_id.try_copy_to_native(&mut msg.goal_id)?;
            self.stamp.try_copy_to_native(&mut msg.stamp)?;
            Ok(())
        }
    }
    impl Default for GoalInfo {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<GoalInfo>::new();
            GoalInfo::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct GoalStatus {
        pub goal_info: action_msgs::msg::GoalInfo,
        pub status: i8,
    }
    unsafe impl WrappedTypesupport for GoalStatus {
        type CStruct = action_msgs__msg__GoalStatus;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__action_msgs__msg__GoalStatus()
            }
        }
        fn create_msg() -> *mut action_msgs__msg__GoalStatus {
            #[cfg(not(feature = "doc-only"))]
            unsafe { action_msgs__msg__GoalStatus__create() }
            #[cfg(feature = "doc-only")] action_msgs__msg__GoalStatus__create()
        }
        fn destroy_msg(msg: *mut action_msgs__msg__GoalStatus) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { action_msgs__msg__GoalStatus__destroy(msg) };
            #[cfg(feature = "doc-only")] action_msgs__msg__GoalStatus__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> GoalStatus {
            GoalStatus {
                goal_info: action_msgs::msg::GoalInfo::from_native(&msg.goal_info),
                status: msg.status,
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
            self.goal_info.try_copy_to_native(&mut msg.goal_info)?;
            msg.status = self.status;
            Ok(())
        }
    }
    impl Default for GoalStatus {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<GoalStatus>::new();
            GoalStatus::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl GoalStatus {
        pub const STATUS_ABORTED: _bindgen_ty_98 = action_msgs__msg__GoalStatus__STATUS_ABORTED;
        pub const STATUS_ACCEPTED: _bindgen_ty_93 = action_msgs__msg__GoalStatus__STATUS_ACCEPTED;
        pub const STATUS_CANCELED: _bindgen_ty_97 = action_msgs__msg__GoalStatus__STATUS_CANCELED;
        pub const STATUS_CANCELING: _bindgen_ty_95 = action_msgs__msg__GoalStatus__STATUS_CANCELING;
        pub const STATUS_EXECUTING: _bindgen_ty_94 = action_msgs__msg__GoalStatus__STATUS_EXECUTING;
        pub const STATUS_SUCCEEDED: _bindgen_ty_96 = action_msgs__msg__GoalStatus__STATUS_SUCCEEDED;
        pub const STATUS_UNKNOWN: _bindgen_ty_92 = action_msgs__msg__GoalStatus__STATUS_UNKNOWN;
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct GoalStatusArray {
        pub status_list: Vec<action_msgs::msg::GoalStatus>,
    }
    unsafe impl WrappedTypesupport for GoalStatusArray {
        type CStruct = action_msgs__msg__GoalStatusArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__action_msgs__msg__GoalStatusArray()
            }
        }
        fn create_msg() -> *mut action_msgs__msg__GoalStatusArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { action_msgs__msg__GoalStatusArray__create() }
            #[cfg(feature = "doc-only")] action_msgs__msg__GoalStatusArray__create()
        }
        fn destroy_msg(msg: *mut action_msgs__msg__GoalStatusArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { action_msgs__msg__GoalStatusArray__destroy(msg) };
            #[cfg(feature = "doc-only")] action_msgs__msg__GoalStatusArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> GoalStatusArray {
            GoalStatusArray {
                status_list: {
                    let mut temp = Vec::with_capacity(msg.status_list.size);
                    if msg.status_list.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.status_list.data,
                                msg.status_list.size,
                            )
                        };
                        for s in slice {
                            temp.push(action_msgs::msg::GoalStatus::from_native(s));
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
                let length = self.status_list.len();
                if length > msg.status_list.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !action_msgs__msg__GoalStatus__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    action_msgs__msg__GoalStatus__Sequence__fini(&mut msg.status_list);
                    msg.status_list = replacement;
                }
                msg.status_list.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.status_list.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.status_list) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for GoalStatusArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<GoalStatusArray>::new();
            GoalStatusArray::from_native(&msg_native)
        }
    }
}
pub mod srv {
    #[allow(non_snake_case)]
    pub mod CancelGoal {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__action_msgs__srv__CancelGoal()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub goal_info: action_msgs::msg::GoalInfo,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = action_msgs__srv__CancelGoal_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__action_msgs__srv__CancelGoal_Request()
                }
            }
            fn create_msg() -> *mut action_msgs__srv__CancelGoal_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { action_msgs__srv__CancelGoal_Request__create() }
                #[cfg(feature = "doc-only")]
                action_msgs__srv__CancelGoal_Request__create()
            }
            fn destroy_msg(msg: *mut action_msgs__srv__CancelGoal_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { action_msgs__srv__CancelGoal_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                action_msgs__srv__CancelGoal_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    goal_info: action_msgs::msg::GoalInfo::from_native(&msg.goal_info),
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
                self.goal_info.try_copy_to_native(&mut msg.goal_info)?;
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
            pub return_code: i8,
            pub goals_canceling: Vec<action_msgs::msg::GoalInfo>,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = action_msgs__srv__CancelGoal_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__action_msgs__srv__CancelGoal_Response()
                }
            }
            fn create_msg() -> *mut action_msgs__srv__CancelGoal_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { action_msgs__srv__CancelGoal_Response__create() }
                #[cfg(feature = "doc-only")]
                action_msgs__srv__CancelGoal_Response__create()
            }
            fn destroy_msg(msg: *mut action_msgs__srv__CancelGoal_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { action_msgs__srv__CancelGoal_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                action_msgs__srv__CancelGoal_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    return_code: msg.return_code,
                    goals_canceling: {
                        let mut temp = Vec::with_capacity(msg.goals_canceling.size);
                        if msg.goals_canceling.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                    msg.goals_canceling.data,
                                    msg.goals_canceling.size,
                                )
                            };
                            for s in slice {
                                temp.push(action_msgs::msg::GoalInfo::from_native(s));
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
                msg.return_code = self.return_code;
                unsafe {
                    let length = self.goals_canceling.len();
                    if length > msg.goals_canceling.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !action_msgs__msg__GoalInfo__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        action_msgs__msg__GoalInfo__Sequence__fini(
                            &mut msg.goals_canceling,
                        );
                        msg.goals_canceling = replacement;
                    }
                    msg.goals_canceling.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.goals_canceling.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.goals_canceling) {
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
        #[allow(non_upper_case_globals)]
        impl Response {
            pub const ERROR_GOAL_TERMINATED: _bindgen_ty_102 = action_msgs__srv__CancelGoal_Response__ERROR_GOAL_TERMINATED;
            pub const ERROR_NONE: _bindgen_ty_99 = action_msgs__srv__CancelGoal_Response__ERROR_NONE;
            pub const ERROR_REJECTED: _bindgen_ty_100 = action_msgs__srv__CancelGoal_Response__ERROR_REJECTED;
            pub const ERROR_UNKNOWN_GOAL_ID: _bindgen_ty_101 = action_msgs__srv__CancelGoal_Response__ERROR_UNKNOWN_GOAL_ID;
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Event {
            pub info: service_msgs::msg::ServiceEventInfo,
            pub request: Vec<action_msgs::srv::CancelGoal::Request>,
            pub response: Vec<action_msgs::srv::CancelGoal::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = action_msgs__srv__CancelGoal_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__action_msgs__srv__CancelGoal_Event()
                }
            }
            fn create_msg() -> *mut action_msgs__srv__CancelGoal_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { action_msgs__srv__CancelGoal_Event__create() }
                #[cfg(feature = "doc-only")] action_msgs__srv__CancelGoal_Event__create()
            }
            fn destroy_msg(msg: *mut action_msgs__srv__CancelGoal_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { action_msgs__srv__CancelGoal_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                action_msgs__srv__CancelGoal_Event__destroy(msg)
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
                                    action_msgs::srv::CancelGoal::Request::from_native(s),
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
                                    action_msgs::srv::CancelGoal::Response::from_native(s),
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
                        if !action_msgs__srv__CancelGoal_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        action_msgs__srv__CancelGoal_Request__Sequence__fini(
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
                        if !action_msgs__srv__CancelGoal_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        action_msgs__srv__CancelGoal_Response__Sequence__fini(
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
