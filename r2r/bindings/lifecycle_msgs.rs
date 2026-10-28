pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct State {
        pub id: u8,
        pub label: crate::RosString,
    }
    unsafe impl WrappedTypesupport for State {
        type CStruct = lifecycle_msgs__msg__State;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__msg__State()
            }
        }
        fn create_msg() -> *mut lifecycle_msgs__msg__State {
            #[cfg(not(feature = "doc-only"))]
            unsafe { lifecycle_msgs__msg__State__create() }
            #[cfg(feature = "doc-only")] lifecycle_msgs__msg__State__create()
        }
        fn destroy_msg(msg: *mut lifecycle_msgs__msg__State) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { lifecycle_msgs__msg__State__destroy(msg) };
            #[cfg(feature = "doc-only")] lifecycle_msgs__msg__State__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> State {
            State {
                id: msg.id,
                label: crate::RosString::from(msg.label.as_bytes()),
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
            msg.id = self.id;
            check_rosidl_runtime_ret(msg.label.try_assign_bytes(self.label.as_bytes()))?;
            Ok(())
        }
    }
    impl Default for State {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<State>::new();
            State::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl State {
        pub const PRIMARY_STATE_ACTIVE: _bindgen_ty_130 = lifecycle_msgs__msg__State__PRIMARY_STATE_ACTIVE;
        pub const PRIMARY_STATE_FINALIZED: _bindgen_ty_131 = lifecycle_msgs__msg__State__PRIMARY_STATE_FINALIZED;
        pub const PRIMARY_STATE_INACTIVE: _bindgen_ty_129 = lifecycle_msgs__msg__State__PRIMARY_STATE_INACTIVE;
        pub const PRIMARY_STATE_UNCONFIGURED: _bindgen_ty_128 = lifecycle_msgs__msg__State__PRIMARY_STATE_UNCONFIGURED;
        pub const PRIMARY_STATE_UNKNOWN: _bindgen_ty_127 = lifecycle_msgs__msg__State__PRIMARY_STATE_UNKNOWN;
        pub const TRANSITION_STATE_ACTIVATING: _bindgen_ty_135 = lifecycle_msgs__msg__State__TRANSITION_STATE_ACTIVATING;
        pub const TRANSITION_STATE_CLEANINGUP: _bindgen_ty_133 = lifecycle_msgs__msg__State__TRANSITION_STATE_CLEANINGUP;
        pub const TRANSITION_STATE_CONFIGURING: _bindgen_ty_132 = lifecycle_msgs__msg__State__TRANSITION_STATE_CONFIGURING;
        pub const TRANSITION_STATE_DEACTIVATING: _bindgen_ty_136 = lifecycle_msgs__msg__State__TRANSITION_STATE_DEACTIVATING;
        pub const TRANSITION_STATE_ERRORPROCESSING: _bindgen_ty_137 = lifecycle_msgs__msg__State__TRANSITION_STATE_ERRORPROCESSING;
        pub const TRANSITION_STATE_SHUTTINGDOWN: _bindgen_ty_134 = lifecycle_msgs__msg__State__TRANSITION_STATE_SHUTTINGDOWN;
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Transition {
        pub id: u8,
        pub label: crate::RosString,
    }
    unsafe impl WrappedTypesupport for Transition {
        type CStruct = lifecycle_msgs__msg__Transition;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__msg__Transition()
            }
        }
        fn create_msg() -> *mut lifecycle_msgs__msg__Transition {
            #[cfg(not(feature = "doc-only"))]
            unsafe { lifecycle_msgs__msg__Transition__create() }
            #[cfg(feature = "doc-only")] lifecycle_msgs__msg__Transition__create()
        }
        fn destroy_msg(msg: *mut lifecycle_msgs__msg__Transition) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { lifecycle_msgs__msg__Transition__destroy(msg) };
            #[cfg(feature = "doc-only")] lifecycle_msgs__msg__Transition__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Transition {
            Transition {
                id: msg.id,
                label: crate::RosString::from(msg.label.as_bytes()),
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
            msg.id = self.id;
            check_rosidl_runtime_ret(msg.label.try_assign_bytes(self.label.as_bytes()))?;
            Ok(())
        }
    }
    impl Default for Transition {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Transition>::new();
            Transition::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl Transition {
        pub const TRANSITION_ACTIVATE: _bindgen_ty_141 = lifecycle_msgs__msg__Transition__TRANSITION_ACTIVATE;
        pub const TRANSITION_ACTIVE_SHUTDOWN: _bindgen_ty_145 = lifecycle_msgs__msg__Transition__TRANSITION_ACTIVE_SHUTDOWN;
        pub const TRANSITION_CALLBACK_ERROR: _bindgen_ty_167 = lifecycle_msgs__msg__Transition__TRANSITION_CALLBACK_ERROR;
        pub const TRANSITION_CALLBACK_FAILURE: _bindgen_ty_166 = lifecycle_msgs__msg__Transition__TRANSITION_CALLBACK_FAILURE;
        pub const TRANSITION_CALLBACK_SUCCESS: _bindgen_ty_165 = lifecycle_msgs__msg__Transition__TRANSITION_CALLBACK_SUCCESS;
        pub const TRANSITION_CLEANUP: _bindgen_ty_140 = lifecycle_msgs__msg__Transition__TRANSITION_CLEANUP;
        pub const TRANSITION_CONFIGURE: _bindgen_ty_139 = lifecycle_msgs__msg__Transition__TRANSITION_CONFIGURE;
        pub const TRANSITION_CREATE: _bindgen_ty_138 = lifecycle_msgs__msg__Transition__TRANSITION_CREATE;
        pub const TRANSITION_DEACTIVATE: _bindgen_ty_142 = lifecycle_msgs__msg__Transition__TRANSITION_DEACTIVATE;
        pub const TRANSITION_DESTROY: _bindgen_ty_146 = lifecycle_msgs__msg__Transition__TRANSITION_DESTROY;
        pub const TRANSITION_INACTIVE_SHUTDOWN: _bindgen_ty_144 = lifecycle_msgs__msg__Transition__TRANSITION_INACTIVE_SHUTDOWN;
        pub const TRANSITION_ON_ACTIVATE_ERROR: _bindgen_ty_155 = lifecycle_msgs__msg__Transition__TRANSITION_ON_ACTIVATE_ERROR;
        pub const TRANSITION_ON_ACTIVATE_FAILURE: _bindgen_ty_154 = lifecycle_msgs__msg__Transition__TRANSITION_ON_ACTIVATE_FAILURE;
        pub const TRANSITION_ON_ACTIVATE_SUCCESS: _bindgen_ty_153 = lifecycle_msgs__msg__Transition__TRANSITION_ON_ACTIVATE_SUCCESS;
        pub const TRANSITION_ON_CLEANUP_ERROR: _bindgen_ty_152 = lifecycle_msgs__msg__Transition__TRANSITION_ON_CLEANUP_ERROR;
        pub const TRANSITION_ON_CLEANUP_FAILURE: _bindgen_ty_151 = lifecycle_msgs__msg__Transition__TRANSITION_ON_CLEANUP_FAILURE;
        pub const TRANSITION_ON_CLEANUP_SUCCESS: _bindgen_ty_150 = lifecycle_msgs__msg__Transition__TRANSITION_ON_CLEANUP_SUCCESS;
        pub const TRANSITION_ON_CONFIGURE_ERROR: _bindgen_ty_149 = lifecycle_msgs__msg__Transition__TRANSITION_ON_CONFIGURE_ERROR;
        pub const TRANSITION_ON_CONFIGURE_FAILURE: _bindgen_ty_148 = lifecycle_msgs__msg__Transition__TRANSITION_ON_CONFIGURE_FAILURE;
        pub const TRANSITION_ON_CONFIGURE_SUCCESS: _bindgen_ty_147 = lifecycle_msgs__msg__Transition__TRANSITION_ON_CONFIGURE_SUCCESS;
        pub const TRANSITION_ON_DEACTIVATE_ERROR: _bindgen_ty_158 = lifecycle_msgs__msg__Transition__TRANSITION_ON_DEACTIVATE_ERROR;
        pub const TRANSITION_ON_DEACTIVATE_FAILURE: _bindgen_ty_157 = lifecycle_msgs__msg__Transition__TRANSITION_ON_DEACTIVATE_FAILURE;
        pub const TRANSITION_ON_DEACTIVATE_SUCCESS: _bindgen_ty_156 = lifecycle_msgs__msg__Transition__TRANSITION_ON_DEACTIVATE_SUCCESS;
        pub const TRANSITION_ON_ERROR_ERROR: _bindgen_ty_164 = lifecycle_msgs__msg__Transition__TRANSITION_ON_ERROR_ERROR;
        pub const TRANSITION_ON_ERROR_FAILURE: _bindgen_ty_163 = lifecycle_msgs__msg__Transition__TRANSITION_ON_ERROR_FAILURE;
        pub const TRANSITION_ON_ERROR_SUCCESS: _bindgen_ty_162 = lifecycle_msgs__msg__Transition__TRANSITION_ON_ERROR_SUCCESS;
        pub const TRANSITION_ON_SHUTDOWN_ERROR: _bindgen_ty_161 = lifecycle_msgs__msg__Transition__TRANSITION_ON_SHUTDOWN_ERROR;
        pub const TRANSITION_ON_SHUTDOWN_FAILURE: _bindgen_ty_160 = lifecycle_msgs__msg__Transition__TRANSITION_ON_SHUTDOWN_FAILURE;
        pub const TRANSITION_ON_SHUTDOWN_SUCCESS: _bindgen_ty_159 = lifecycle_msgs__msg__Transition__TRANSITION_ON_SHUTDOWN_SUCCESS;
        pub const TRANSITION_UNCONFIGURED_SHUTDOWN: _bindgen_ty_143 = lifecycle_msgs__msg__Transition__TRANSITION_UNCONFIGURED_SHUTDOWN;
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct TransitionDescription {
        pub transition: lifecycle_msgs::msg::Transition,
        pub start_state: lifecycle_msgs::msg::State,
        pub goal_state: lifecycle_msgs::msg::State,
    }
    unsafe impl WrappedTypesupport for TransitionDescription {
        type CStruct = lifecycle_msgs__msg__TransitionDescription;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__msg__TransitionDescription()
            }
        }
        fn create_msg() -> *mut lifecycle_msgs__msg__TransitionDescription {
            #[cfg(not(feature = "doc-only"))]
            unsafe { lifecycle_msgs__msg__TransitionDescription__create() }
            #[cfg(feature = "doc-only")]
            lifecycle_msgs__msg__TransitionDescription__create()
        }
        fn destroy_msg(msg: *mut lifecycle_msgs__msg__TransitionDescription) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { lifecycle_msgs__msg__TransitionDescription__destroy(msg) };
            #[cfg(feature = "doc-only")]
            lifecycle_msgs__msg__TransitionDescription__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> TransitionDescription {
            TransitionDescription {
                transition: lifecycle_msgs::msg::Transition::from_native(
                    &msg.transition,
                ),
                start_state: lifecycle_msgs::msg::State::from_native(&msg.start_state),
                goal_state: lifecycle_msgs::msg::State::from_native(&msg.goal_state),
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
            self.transition.try_copy_to_native(&mut msg.transition)?;
            self.start_state.try_copy_to_native(&mut msg.start_state)?;
            self.goal_state.try_copy_to_native(&mut msg.goal_state)?;
            Ok(())
        }
    }
    impl Default for TransitionDescription {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<TransitionDescription>::new();
            TransitionDescription::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct TransitionEvent {
        pub timestamp: u64,
        pub transition: lifecycle_msgs::msg::Transition,
        pub start_state: lifecycle_msgs::msg::State,
        pub goal_state: lifecycle_msgs::msg::State,
    }
    unsafe impl WrappedTypesupport for TransitionEvent {
        type CStruct = lifecycle_msgs__msg__TransitionEvent;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__msg__TransitionEvent()
            }
        }
        fn create_msg() -> *mut lifecycle_msgs__msg__TransitionEvent {
            #[cfg(not(feature = "doc-only"))]
            unsafe { lifecycle_msgs__msg__TransitionEvent__create() }
            #[cfg(feature = "doc-only")] lifecycle_msgs__msg__TransitionEvent__create()
        }
        fn destroy_msg(msg: *mut lifecycle_msgs__msg__TransitionEvent) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { lifecycle_msgs__msg__TransitionEvent__destroy(msg) };
            #[cfg(feature = "doc-only")]
            lifecycle_msgs__msg__TransitionEvent__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> TransitionEvent {
            TransitionEvent {
                timestamp: msg.timestamp,
                transition: lifecycle_msgs::msg::Transition::from_native(
                    &msg.transition,
                ),
                start_state: lifecycle_msgs::msg::State::from_native(&msg.start_state),
                goal_state: lifecycle_msgs::msg::State::from_native(&msg.goal_state),
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
            msg.timestamp = self.timestamp;
            self.transition.try_copy_to_native(&mut msg.transition)?;
            self.start_state.try_copy_to_native(&mut msg.start_state)?;
            self.goal_state.try_copy_to_native(&mut msg.goal_state)?;
            Ok(())
        }
    }
    impl Default for TransitionEvent {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<TransitionEvent>::new();
            TransitionEvent::from_native(&msg_native)
        }
    }
}
pub mod srv {
    #[allow(non_snake_case)]
    pub mod ChangeState {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__lifecycle_msgs__srv__ChangeState()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub transition: lifecycle_msgs::msg::Transition,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = lifecycle_msgs__srv__ChangeState_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__srv__ChangeState_Request()
                }
            }
            fn create_msg() -> *mut lifecycle_msgs__srv__ChangeState_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__ChangeState_Request__create() }
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__ChangeState_Request__create()
            }
            fn destroy_msg(msg: *mut lifecycle_msgs__srv__ChangeState_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__ChangeState_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__ChangeState_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    transition: lifecycle_msgs::msg::Transition::from_native(
                        &msg.transition,
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
                self.transition.try_copy_to_native(&mut msg.transition)?;
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
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = lifecycle_msgs__srv__ChangeState_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__srv__ChangeState_Response()
                }
            }
            fn create_msg() -> *mut lifecycle_msgs__srv__ChangeState_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__ChangeState_Response__create() }
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__ChangeState_Response__create()
            }
            fn destroy_msg(msg: *mut lifecycle_msgs__srv__ChangeState_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__ChangeState_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__ChangeState_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response { success: msg.success }
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
            pub request: Vec<lifecycle_msgs::srv::ChangeState::Request>,
            pub response: Vec<lifecycle_msgs::srv::ChangeState::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = lifecycle_msgs__srv__ChangeState_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__srv__ChangeState_Event()
                }
            }
            fn create_msg() -> *mut lifecycle_msgs__srv__ChangeState_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__ChangeState_Event__create() }
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__ChangeState_Event__create()
            }
            fn destroy_msg(msg: *mut lifecycle_msgs__srv__ChangeState_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__ChangeState_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__ChangeState_Event__destroy(msg)
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
                                    lifecycle_msgs::srv::ChangeState::Request::from_native(s),
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
                                    lifecycle_msgs::srv::ChangeState::Response::from_native(s),
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
                        if !lifecycle_msgs__srv__ChangeState_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        lifecycle_msgs__srv__ChangeState_Request__Sequence__fini(
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
                        if !lifecycle_msgs__srv__ChangeState_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        lifecycle_msgs__srv__ChangeState_Response__Sequence__fini(
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
    pub mod GetAvailableStates {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__lifecycle_msgs__srv__GetAvailableStates()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {}
        unsafe impl WrappedTypesupport for Request {
            type CStruct = lifecycle_msgs__srv__GetAvailableStates_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__srv__GetAvailableStates_Request()
                }
            }
            fn create_msg() -> *mut lifecycle_msgs__srv__GetAvailableStates_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetAvailableStates_Request__create() }
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetAvailableStates_Request__create()
            }
            fn destroy_msg(
                msg: *mut lifecycle_msgs__srv__GetAvailableStates_Request,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetAvailableStates_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetAvailableStates_Request__destroy(msg)
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
            pub available_states: Vec<lifecycle_msgs::msg::State>,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = lifecycle_msgs__srv__GetAvailableStates_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__srv__GetAvailableStates_Response()
                }
            }
            fn create_msg() -> *mut lifecycle_msgs__srv__GetAvailableStates_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetAvailableStates_Response__create() }
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetAvailableStates_Response__create()
            }
            fn destroy_msg(
                msg: *mut lifecycle_msgs__srv__GetAvailableStates_Response,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    lifecycle_msgs__srv__GetAvailableStates_Response__destroy(msg)
                };
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetAvailableStates_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    available_states: {
                        let mut temp = Vec::with_capacity(msg.available_states.size);
                        if msg.available_states.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                    msg.available_states.data,
                                    msg.available_states.size,
                                )
                            };
                            for s in slice {
                                temp.push(lifecycle_msgs::msg::State::from_native(s));
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
                    let length = self.available_states.len();
                    if length > msg.available_states.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !lifecycle_msgs__msg__State__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        lifecycle_msgs__msg__State__Sequence__fini(
                            &mut msg.available_states,
                        );
                        msg.available_states = replacement;
                    }
                    msg.available_states.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.available_states.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.available_states) {
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
            pub request: Vec<lifecycle_msgs::srv::GetAvailableStates::Request>,
            pub response: Vec<lifecycle_msgs::srv::GetAvailableStates::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = lifecycle_msgs__srv__GetAvailableStates_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__srv__GetAvailableStates_Event()
                }
            }
            fn create_msg() -> *mut lifecycle_msgs__srv__GetAvailableStates_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetAvailableStates_Event__create() }
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetAvailableStates_Event__create()
            }
            fn destroy_msg(
                msg: *mut lifecycle_msgs__srv__GetAvailableStates_Event,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetAvailableStates_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetAvailableStates_Event__destroy(msg)
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
                                    lifecycle_msgs::srv::GetAvailableStates::Request::from_native(
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
                                    lifecycle_msgs::srv::GetAvailableStates::Response::from_native(
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
                        if !lifecycle_msgs__srv__GetAvailableStates_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        lifecycle_msgs__srv__GetAvailableStates_Request__Sequence__fini(
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
                        if !lifecycle_msgs__srv__GetAvailableStates_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        lifecycle_msgs__srv__GetAvailableStates_Response__Sequence__fini(
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
    pub mod GetAvailableTransitions {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__lifecycle_msgs__srv__GetAvailableTransitions()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {}
        unsafe impl WrappedTypesupport for Request {
            type CStruct = lifecycle_msgs__srv__GetAvailableTransitions_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__srv__GetAvailableTransitions_Request()
                }
            }
            fn create_msg() -> *mut lifecycle_msgs__srv__GetAvailableTransitions_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetAvailableTransitions_Request__create() }
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetAvailableTransitions_Request__create()
            }
            fn destroy_msg(
                msg: *mut lifecycle_msgs__srv__GetAvailableTransitions_Request,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    lifecycle_msgs__srv__GetAvailableTransitions_Request__destroy(msg)
                };
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetAvailableTransitions_Request__destroy(msg)
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
            pub available_transitions: Vec<lifecycle_msgs::msg::TransitionDescription>,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = lifecycle_msgs__srv__GetAvailableTransitions_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__srv__GetAvailableTransitions_Response()
                }
            }
            fn create_msg() -> *mut lifecycle_msgs__srv__GetAvailableTransitions_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    lifecycle_msgs__srv__GetAvailableTransitions_Response__create()
                }
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetAvailableTransitions_Response__create()
            }
            fn destroy_msg(
                msg: *mut lifecycle_msgs__srv__GetAvailableTransitions_Response,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    lifecycle_msgs__srv__GetAvailableTransitions_Response__destroy(msg)
                };
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetAvailableTransitions_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    available_transitions: {
                        let mut temp = Vec::with_capacity(
                            msg.available_transitions.size,
                        );
                        if msg.available_transitions.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                    msg.available_transitions.data,
                                    msg.available_transitions.size,
                                )
                            };
                            for s in slice {
                                temp.push(
                                    lifecycle_msgs::msg::TransitionDescription::from_native(s),
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
                    let length = self.available_transitions.len();
                    if length > msg.available_transitions.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !lifecycle_msgs__msg__TransitionDescription__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        lifecycle_msgs__msg__TransitionDescription__Sequence__fini(
                            &mut msg.available_transitions,
                        );
                        msg.available_transitions = replacement;
                    }
                    msg.available_transitions.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.available_transitions.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.available_transitions) {
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
            pub request: Vec<lifecycle_msgs::srv::GetAvailableTransitions::Request>,
            pub response: Vec<lifecycle_msgs::srv::GetAvailableTransitions::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = lifecycle_msgs__srv__GetAvailableTransitions_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__srv__GetAvailableTransitions_Event()
                }
            }
            fn create_msg() -> *mut lifecycle_msgs__srv__GetAvailableTransitions_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetAvailableTransitions_Event__create() }
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetAvailableTransitions_Event__create()
            }
            fn destroy_msg(
                msg: *mut lifecycle_msgs__srv__GetAvailableTransitions_Event,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    lifecycle_msgs__srv__GetAvailableTransitions_Event__destroy(msg)
                };
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetAvailableTransitions_Event__destroy(msg)
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
                                    lifecycle_msgs::srv::GetAvailableTransitions::Request::from_native(
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
                                    lifecycle_msgs::srv::GetAvailableTransitions::Response::from_native(
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
                        if !lifecycle_msgs__srv__GetAvailableTransitions_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        lifecycle_msgs__srv__GetAvailableTransitions_Request__Sequence__fini(
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
                        if !lifecycle_msgs__srv__GetAvailableTransitions_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        lifecycle_msgs__srv__GetAvailableTransitions_Response__Sequence__fini(
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
    pub mod GetState {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__lifecycle_msgs__srv__GetState()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {}
        unsafe impl WrappedTypesupport for Request {
            type CStruct = lifecycle_msgs__srv__GetState_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__srv__GetState_Request()
                }
            }
            fn create_msg() -> *mut lifecycle_msgs__srv__GetState_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetState_Request__create() }
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetState_Request__create()
            }
            fn destroy_msg(msg: *mut lifecycle_msgs__srv__GetState_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetState_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetState_Request__destroy(msg)
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
            pub current_state: lifecycle_msgs::msg::State,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = lifecycle_msgs__srv__GetState_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__srv__GetState_Response()
                }
            }
            fn create_msg() -> *mut lifecycle_msgs__srv__GetState_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetState_Response__create() }
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetState_Response__create()
            }
            fn destroy_msg(msg: *mut lifecycle_msgs__srv__GetState_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetState_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetState_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    current_state: lifecycle_msgs::msg::State::from_native(
                        &msg.current_state,
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
                self.current_state.try_copy_to_native(&mut msg.current_state)?;
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
            pub request: Vec<lifecycle_msgs::srv::GetState::Request>,
            pub response: Vec<lifecycle_msgs::srv::GetState::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = lifecycle_msgs__srv__GetState_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__lifecycle_msgs__srv__GetState_Event()
                }
            }
            fn create_msg() -> *mut lifecycle_msgs__srv__GetState_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetState_Event__create() }
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetState_Event__create()
            }
            fn destroy_msg(msg: *mut lifecycle_msgs__srv__GetState_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { lifecycle_msgs__srv__GetState_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                lifecycle_msgs__srv__GetState_Event__destroy(msg)
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
                                    lifecycle_msgs::srv::GetState::Request::from_native(s),
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
                                    lifecycle_msgs::srv::GetState::Response::from_native(s),
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
                        if !lifecycle_msgs__srv__GetState_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        lifecycle_msgs__srv__GetState_Request__Sequence__fini(
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
                        if !lifecycle_msgs__srv__GetState_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        lifecycle_msgs__srv__GetState_Response__Sequence__fini(
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
