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
                    &*rosidl_typesupport_c__get_action_type_support_handle__example_interfaces__action__Fibonacci()
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
            fn native_feedback_uuid(
                msg: &<FeedbackMessage as WrappedTypesupport>::CStruct,
            ) -> Option<[u8; 16]> {
                Some(msg.goal_id.uuid)
            }
            fn supports_native_feedback_delivery() -> bool {
                true
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
            type CStruct = example_interfaces__action__Fibonacci_Goal;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__action__Fibonacci_Goal()
                }
            }
            fn create_msg() -> *mut example_interfaces__action__Fibonacci_Goal {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__action__Fibonacci_Goal__create() }
                #[cfg(feature = "doc-only")]
                example_interfaces__action__Fibonacci_Goal__create()
            }
            fn destroy_msg(msg: *mut example_interfaces__action__Fibonacci_Goal) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__action__Fibonacci_Goal__destroy(msg) };
                #[cfg(feature = "doc-only")]
                example_interfaces__action__Fibonacci_Goal__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Goal {
                Goal { order: msg.order }
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
                msg.order = self.order;
                Ok(())
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
            type CStruct = example_interfaces__action__Fibonacci_Result;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__action__Fibonacci_Result()
                }
            }
            fn create_msg() -> *mut example_interfaces__action__Fibonacci_Result {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__action__Fibonacci_Result__create() }
                #[cfg(feature = "doc-only")]
                example_interfaces__action__Fibonacci_Result__create()
            }
            fn destroy_msg(
                msg: *mut example_interfaces__action__Fibonacci_Result,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__action__Fibonacci_Result__destroy(msg) };
                #[cfg(feature = "doc-only")]
                example_interfaces__action__Fibonacci_Result__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Result {
                Result {
                    sequence: msg.sequence.to_vec(),
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
                check_rosidl_runtime_ret(msg.sequence.try_update(&self.sequence))?;
                Ok(())
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
            type CStruct = example_interfaces__action__Fibonacci_Feedback;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__action__Fibonacci_Feedback()
                }
            }
            fn create_msg() -> *mut example_interfaces__action__Fibonacci_Feedback {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__action__Fibonacci_Feedback__create() }
                #[cfg(feature = "doc-only")]
                example_interfaces__action__Fibonacci_Feedback__create()
            }
            fn destroy_msg(
                msg: *mut example_interfaces__action__Fibonacci_Feedback,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__action__Fibonacci_Feedback__destroy(msg) };
                #[cfg(feature = "doc-only")]
                example_interfaces__action__Fibonacci_Feedback__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Feedback {
                Feedback {
                    sequence: msg.sequence.to_vec(),
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
                check_rosidl_runtime_ret(msg.sequence.try_update(&self.sequence))?;
                Ok(())
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
                        &*rosidl_typesupport_c__get_service_type_support_handle__example_interfaces__action__Fibonacci_SendGoal()
                    }
                }
            }
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            #[serde(default)]
            pub struct Request {
                pub goal_id: unique_identifier_msgs::msg::UUID,
                pub goal: example_interfaces::action::Fibonacci::Goal,
            }
            unsafe impl WrappedTypesupport for Request {
                type CStruct = example_interfaces__action__Fibonacci_SendGoal_Request;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__action__Fibonacci_SendGoal_Request()
                    }
                }
                fn create_msg() -> *mut example_interfaces__action__Fibonacci_SendGoal_Request {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        example_interfaces__action__Fibonacci_SendGoal_Request__create()
                    }
                    #[cfg(feature = "doc-only")]
                    example_interfaces__action__Fibonacci_SendGoal_Request__create()
                }
                fn destroy_msg(
                    msg: *mut example_interfaces__action__Fibonacci_SendGoal_Request,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        example_interfaces__action__Fibonacci_SendGoal_Request__destroy(
                            msg,
                        )
                    };
                    #[cfg(feature = "doc-only")]
                    example_interfaces__action__Fibonacci_SendGoal_Request__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                    Request {
                        goal_id: unique_identifier_msgs::msg::UUID::from_native(
                            &msg.goal_id,
                        ),
                        goal: example_interfaces::action::Fibonacci::Goal::from_native(
                            &msg.goal,
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
                    self.goal_id.try_copy_to_native(&mut msg.goal_id)?;
                    self.goal.try_copy_to_native(&mut msg.goal)?;
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
                pub accepted: bool,
                pub stamp: builtin_interfaces::msg::Time,
            }
            unsafe impl WrappedTypesupport for Response {
                type CStruct = example_interfaces__action__Fibonacci_SendGoal_Response;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__action__Fibonacci_SendGoal_Response()
                    }
                }
                fn create_msg() -> *mut example_interfaces__action__Fibonacci_SendGoal_Response {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        example_interfaces__action__Fibonacci_SendGoal_Response__create()
                    }
                    #[cfg(feature = "doc-only")]
                    example_interfaces__action__Fibonacci_SendGoal_Response__create()
                }
                fn destroy_msg(
                    msg: *mut example_interfaces__action__Fibonacci_SendGoal_Response,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        example_interfaces__action__Fibonacci_SendGoal_Response__destroy(
                            msg,
                        )
                    };
                    #[cfg(feature = "doc-only")]
                    example_interfaces__action__Fibonacci_SendGoal_Response__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                    Response {
                        accepted: msg.accepted,
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
                    msg.accepted = self.accepted;
                    self.stamp.try_copy_to_native(&mut msg.stamp)?;
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
                    example_interfaces::action::Fibonacci::SendGoal::Request,
                >,
                pub response: Vec<
                    example_interfaces::action::Fibonacci::SendGoal::Response,
                >,
            }
            unsafe impl WrappedTypesupport for Event {
                type CStruct = example_interfaces__action__Fibonacci_SendGoal_Event;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__action__Fibonacci_SendGoal_Event()
                    }
                }
                fn create_msg() -> *mut example_interfaces__action__Fibonacci_SendGoal_Event {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        example_interfaces__action__Fibonacci_SendGoal_Event__create()
                    }
                    #[cfg(feature = "doc-only")]
                    example_interfaces__action__Fibonacci_SendGoal_Event__create()
                }
                fn destroy_msg(
                    msg: *mut example_interfaces__action__Fibonacci_SendGoal_Event,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        example_interfaces__action__Fibonacci_SendGoal_Event__destroy(
                            msg,
                        )
                    };
                    #[cfg(feature = "doc-only")]
                    example_interfaces__action__Fibonacci_SendGoal_Event__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Event {
                    Event {
                        info: service_msgs::msg::ServiceEventInfo::from_native(
                            &msg.info,
                        ),
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
                                        example_interfaces::action::Fibonacci::SendGoal::Request::from_native(
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
                                        example_interfaces::action::Fibonacci::SendGoal::Response::from_native(
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
                            if !example_interfaces__action__Fibonacci_SendGoal_Request__Sequence__init(
                                &mut replacement,
                                length,
                            ) {
                                return Err(Error::RCL_RET_BAD_ALLOC);
                            }
                            example_interfaces__action__Fibonacci_SendGoal_Request__Sequence__fini(
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
                            if !example_interfaces__action__Fibonacci_SendGoal_Response__Sequence__init(
                                &mut replacement,
                                length,
                            ) {
                                return Err(Error::RCL_RET_BAD_ALLOC);
                            }
                            example_interfaces__action__Fibonacci_SendGoal_Response__Sequence__fini(
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
        pub mod GetResult {
            use super::super::super::super::*;
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
                        &*rosidl_typesupport_c__get_service_type_support_handle__example_interfaces__action__Fibonacci_GetResult()
                    }
                }
            }
            #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
            #[serde(default)]
            pub struct Request {
                pub goal_id: unique_identifier_msgs::msg::UUID,
            }
            unsafe impl WrappedTypesupport for Request {
                type CStruct = example_interfaces__action__Fibonacci_GetResult_Request;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__action__Fibonacci_GetResult_Request()
                    }
                }
                fn create_msg() -> *mut example_interfaces__action__Fibonacci_GetResult_Request {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        example_interfaces__action__Fibonacci_GetResult_Request__create()
                    }
                    #[cfg(feature = "doc-only")]
                    example_interfaces__action__Fibonacci_GetResult_Request__create()
                }
                fn destroy_msg(
                    msg: *mut example_interfaces__action__Fibonacci_GetResult_Request,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        example_interfaces__action__Fibonacci_GetResult_Request__destroy(
                            msg,
                        )
                    };
                    #[cfg(feature = "doc-only")]
                    example_interfaces__action__Fibonacci_GetResult_Request__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                    Request {
                        goal_id: unique_identifier_msgs::msg::UUID::from_native(
                            &msg.goal_id,
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
                    self.goal_id.try_copy_to_native(&mut msg.goal_id)?;
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
                pub status: i8,
                pub result: example_interfaces::action::Fibonacci::Result,
            }
            unsafe impl WrappedTypesupport for Response {
                type CStruct = example_interfaces__action__Fibonacci_GetResult_Response;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__action__Fibonacci_GetResult_Response()
                    }
                }
                fn create_msg() -> *mut example_interfaces__action__Fibonacci_GetResult_Response {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        example_interfaces__action__Fibonacci_GetResult_Response__create()
                    }
                    #[cfg(feature = "doc-only")]
                    example_interfaces__action__Fibonacci_GetResult_Response__create()
                }
                fn destroy_msg(
                    msg: *mut example_interfaces__action__Fibonacci_GetResult_Response,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        example_interfaces__action__Fibonacci_GetResult_Response__destroy(
                            msg,
                        )
                    };
                    #[cfg(feature = "doc-only")]
                    example_interfaces__action__Fibonacci_GetResult_Response__destroy(
                        msg,
                    )
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                    Response {
                        status: msg.status,
                        result: example_interfaces::action::Fibonacci::Result::from_native(
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
                    msg.status = self.status;
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
                pub request: Vec<
                    example_interfaces::action::Fibonacci::GetResult::Request,
                >,
                pub response: Vec<
                    example_interfaces::action::Fibonacci::GetResult::Response,
                >,
            }
            unsafe impl WrappedTypesupport for Event {
                type CStruct = example_interfaces__action__Fibonacci_GetResult_Event;
                fn get_ts() -> &'static rosidl_message_type_support_t {
                    unsafe {
                        &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__action__Fibonacci_GetResult_Event()
                    }
                }
                fn create_msg() -> *mut example_interfaces__action__Fibonacci_GetResult_Event {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        example_interfaces__action__Fibonacci_GetResult_Event__create()
                    }
                    #[cfg(feature = "doc-only")]
                    example_interfaces__action__Fibonacci_GetResult_Event__create()
                }
                fn destroy_msg(
                    msg: *mut example_interfaces__action__Fibonacci_GetResult_Event,
                ) -> () {
                    #[cfg(not(feature = "doc-only"))]
                    unsafe {
                        example_interfaces__action__Fibonacci_GetResult_Event__destroy(
                            msg,
                        )
                    };
                    #[cfg(feature = "doc-only")]
                    example_interfaces__action__Fibonacci_GetResult_Event__destroy(msg)
                }
                fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Event {
                    Event {
                        info: service_msgs::msg::ServiceEventInfo::from_native(
                            &msg.info,
                        ),
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
                                        example_interfaces::action::Fibonacci::GetResult::Request::from_native(
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
                                        example_interfaces::action::Fibonacci::GetResult::Response::from_native(
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
                            if !example_interfaces__action__Fibonacci_GetResult_Request__Sequence__init(
                                &mut replacement,
                                length,
                            ) {
                                return Err(Error::RCL_RET_BAD_ALLOC);
                            }
                            example_interfaces__action__Fibonacci_GetResult_Request__Sequence__fini(
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
                            if !example_interfaces__action__Fibonacci_GetResult_Response__Sequence__init(
                                &mut replacement,
                                length,
                            ) {
                                return Err(Error::RCL_RET_BAD_ALLOC);
                            }
                            example_interfaces__action__Fibonacci_GetResult_Response__Sequence__fini(
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
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct FeedbackMessage {
            pub goal_id: unique_identifier_msgs::msg::UUID,
            pub feedback: example_interfaces::action::Fibonacci::Feedback,
        }
        unsafe impl WrappedTypesupport for FeedbackMessage {
            type CStruct = example_interfaces__action__Fibonacci_FeedbackMessage;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__action__Fibonacci_FeedbackMessage()
                }
            }
            fn create_msg() -> *mut example_interfaces__action__Fibonacci_FeedbackMessage {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    example_interfaces__action__Fibonacci_FeedbackMessage__create()
                }
                #[cfg(feature = "doc-only")]
                example_interfaces__action__Fibonacci_FeedbackMessage__create()
            }
            fn destroy_msg(
                msg: *mut example_interfaces__action__Fibonacci_FeedbackMessage,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe {
                    example_interfaces__action__Fibonacci_FeedbackMessage__destroy(msg)
                };
                #[cfg(feature = "doc-only")]
                example_interfaces__action__Fibonacci_FeedbackMessage__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> FeedbackMessage {
                FeedbackMessage {
                    goal_id: unique_identifier_msgs::msg::UUID::from_native(
                        &msg.goal_id,
                    ),
                    feedback: example_interfaces::action::Fibonacci::Feedback::from_native(
                        &msg.feedback,
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
                self.goal_id.try_copy_to_native(&mut msg.goal_id)?;
                self.feedback.try_copy_to_native(&mut msg.feedback)?;
                Ok(())
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
    pub struct Bool {
        pub data: bool,
    }
    unsafe impl WrappedTypesupport for Bool {
        type CStruct = example_interfaces__msg__Bool;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Bool()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Bool {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Bool__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__Bool__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Bool) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Bool__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__Bool__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Bool {
            Bool { data: msg.data }
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
    impl Default for Bool {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Bool>::new();
            Bool::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Byte {
        pub data: u8,
    }
    unsafe impl WrappedTypesupport for Byte {
        type CStruct = example_interfaces__msg__Byte;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Byte()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Byte {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Byte__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__Byte__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Byte) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Byte__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__Byte__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Byte {
            Byte { data: msg.data }
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
    impl Default for Byte {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Byte>::new();
            Byte::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ByteMultiArray {
        pub layout: example_interfaces::msg::MultiArrayLayout,
        pub data: Vec<u8>,
    }
    unsafe impl WrappedTypesupport for ByteMultiArray {
        type CStruct = example_interfaces__msg__ByteMultiArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__ByteMultiArray()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__ByteMultiArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__ByteMultiArray__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__ByteMultiArray__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__ByteMultiArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__ByteMultiArray__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__ByteMultiArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> ByteMultiArray {
            ByteMultiArray {
                layout: example_interfaces::msg::MultiArrayLayout::from_native(
                    &msg.layout,
                ),
                data: msg.data.to_vec(),
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
            self.layout.try_copy_to_native(&mut msg.layout)?;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for ByteMultiArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<ByteMultiArray>::new();
            ByteMultiArray::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Char {
        pub data: u8,
    }
    unsafe impl WrappedTypesupport for Char {
        type CStruct = example_interfaces__msg__Char;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Char()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Char {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Char__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__Char__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Char) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Char__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__Char__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Char {
            Char { data: msg.data }
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
    impl Default for Char {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Char>::new();
            Char::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Empty {}
    unsafe impl WrappedTypesupport for Empty {
        type CStruct = example_interfaces__msg__Empty;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Empty()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Empty {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Empty__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__Empty__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Empty) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Empty__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__Empty__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Empty {
            Empty {}
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
    impl Default for Empty {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Empty>::new();
            Empty::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Float32 {
        pub data: f32,
    }
    unsafe impl WrappedTypesupport for Float32 {
        type CStruct = example_interfaces__msg__Float32;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Float32()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Float32 {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Float32__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__Float32__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Float32) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Float32__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__Float32__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Float32 {
            Float32 { data: msg.data }
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
    impl Default for Float32 {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Float32>::new();
            Float32::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Float32MultiArray {
        pub layout: example_interfaces::msg::MultiArrayLayout,
        pub data: Vec<f32>,
    }
    unsafe impl WrappedTypesupport for Float32MultiArray {
        type CStruct = example_interfaces__msg__Float32MultiArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Float32MultiArray()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Float32MultiArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Float32MultiArray__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__Float32MultiArray__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Float32MultiArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Float32MultiArray__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__Float32MultiArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Float32MultiArray {
            Float32MultiArray {
                layout: example_interfaces::msg::MultiArrayLayout::from_native(
                    &msg.layout,
                ),
                data: msg.data.to_vec(),
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
            self.layout.try_copy_to_native(&mut msg.layout)?;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for Float32MultiArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Float32MultiArray>::new();
            Float32MultiArray::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Float64 {
        pub data: f64,
    }
    unsafe impl WrappedTypesupport for Float64 {
        type CStruct = example_interfaces__msg__Float64;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Float64()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Float64 {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Float64__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__Float64__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Float64) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Float64__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__Float64__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Float64 {
            Float64 { data: msg.data }
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
    impl Default for Float64 {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Float64>::new();
            Float64::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Float64MultiArray {
        pub layout: example_interfaces::msg::MultiArrayLayout,
        pub data: Vec<f64>,
    }
    unsafe impl WrappedTypesupport for Float64MultiArray {
        type CStruct = example_interfaces__msg__Float64MultiArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Float64MultiArray()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Float64MultiArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Float64MultiArray__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__Float64MultiArray__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Float64MultiArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Float64MultiArray__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__Float64MultiArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Float64MultiArray {
            Float64MultiArray {
                layout: example_interfaces::msg::MultiArrayLayout::from_native(
                    &msg.layout,
                ),
                data: msg.data.to_vec(),
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
            self.layout.try_copy_to_native(&mut msg.layout)?;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for Float64MultiArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Float64MultiArray>::new();
            Float64MultiArray::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Int16 {
        pub data: i16,
    }
    unsafe impl WrappedTypesupport for Int16 {
        type CStruct = example_interfaces__msg__Int16;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Int16()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Int16 {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int16__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__Int16__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Int16) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int16__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__Int16__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Int16 {
            Int16 { data: msg.data }
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
    impl Default for Int16 {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Int16>::new();
            Int16::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Int16MultiArray {
        pub layout: example_interfaces::msg::MultiArrayLayout,
        pub data: Vec<i16>,
    }
    unsafe impl WrappedTypesupport for Int16MultiArray {
        type CStruct = example_interfaces__msg__Int16MultiArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Int16MultiArray()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Int16MultiArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int16MultiArray__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__Int16MultiArray__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Int16MultiArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int16MultiArray__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__Int16MultiArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Int16MultiArray {
            Int16MultiArray {
                layout: example_interfaces::msg::MultiArrayLayout::from_native(
                    &msg.layout,
                ),
                data: msg.data.to_vec(),
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
            self.layout.try_copy_to_native(&mut msg.layout)?;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for Int16MultiArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Int16MultiArray>::new();
            Int16MultiArray::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Int32 {
        pub data: i32,
    }
    unsafe impl WrappedTypesupport for Int32 {
        type CStruct = example_interfaces__msg__Int32;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Int32()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Int32 {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int32__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__Int32__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Int32) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int32__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__Int32__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Int32 {
            Int32 { data: msg.data }
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
    impl Default for Int32 {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Int32>::new();
            Int32::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Int32MultiArray {
        pub layout: example_interfaces::msg::MultiArrayLayout,
        pub data: Vec<i32>,
    }
    unsafe impl WrappedTypesupport for Int32MultiArray {
        type CStruct = example_interfaces__msg__Int32MultiArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Int32MultiArray()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Int32MultiArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int32MultiArray__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__Int32MultiArray__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Int32MultiArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int32MultiArray__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__Int32MultiArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Int32MultiArray {
            Int32MultiArray {
                layout: example_interfaces::msg::MultiArrayLayout::from_native(
                    &msg.layout,
                ),
                data: msg.data.to_vec(),
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
            self.layout.try_copy_to_native(&mut msg.layout)?;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for Int32MultiArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Int32MultiArray>::new();
            Int32MultiArray::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Int64 {
        pub data: i64,
    }
    unsafe impl WrappedTypesupport for Int64 {
        type CStruct = example_interfaces__msg__Int64;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Int64()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Int64 {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int64__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__Int64__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Int64) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int64__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__Int64__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Int64 {
            Int64 { data: msg.data }
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
    impl Default for Int64 {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Int64>::new();
            Int64::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Int64MultiArray {
        pub layout: example_interfaces::msg::MultiArrayLayout,
        pub data: Vec<i64>,
    }
    unsafe impl WrappedTypesupport for Int64MultiArray {
        type CStruct = example_interfaces__msg__Int64MultiArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Int64MultiArray()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Int64MultiArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int64MultiArray__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__Int64MultiArray__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Int64MultiArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int64MultiArray__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__Int64MultiArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Int64MultiArray {
            Int64MultiArray {
                layout: example_interfaces::msg::MultiArrayLayout::from_native(
                    &msg.layout,
                ),
                data: msg.data.to_vec(),
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
            self.layout.try_copy_to_native(&mut msg.layout)?;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for Int64MultiArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Int64MultiArray>::new();
            Int64MultiArray::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Int8 {
        pub data: i8,
    }
    unsafe impl WrappedTypesupport for Int8 {
        type CStruct = example_interfaces__msg__Int8;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Int8()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Int8 {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int8__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__Int8__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Int8) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int8__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__Int8__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Int8 {
            Int8 { data: msg.data }
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
    impl Default for Int8 {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Int8>::new();
            Int8::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Int8MultiArray {
        pub layout: example_interfaces::msg::MultiArrayLayout,
        pub data: Vec<i8>,
    }
    unsafe impl WrappedTypesupport for Int8MultiArray {
        type CStruct = example_interfaces__msg__Int8MultiArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__Int8MultiArray()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__Int8MultiArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int8MultiArray__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__Int8MultiArray__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__Int8MultiArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__Int8MultiArray__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__Int8MultiArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Int8MultiArray {
            Int8MultiArray {
                layout: example_interfaces::msg::MultiArrayLayout::from_native(
                    &msg.layout,
                ),
                data: msg.data.to_vec(),
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
            self.layout.try_copy_to_native(&mut msg.layout)?;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for Int8MultiArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Int8MultiArray>::new();
            Int8MultiArray::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct MultiArrayDimension {
        pub label: crate::RosString,
        pub size: u32,
        pub stride: u32,
    }
    unsafe impl WrappedTypesupport for MultiArrayDimension {
        type CStruct = example_interfaces__msg__MultiArrayDimension;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__MultiArrayDimension()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__MultiArrayDimension {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__MultiArrayDimension__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__MultiArrayDimension__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__MultiArrayDimension) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__MultiArrayDimension__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__MultiArrayDimension__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> MultiArrayDimension {
            MultiArrayDimension {
                label: crate::RosString::from(msg.label.as_bytes()),
                size: msg.size,
                stride: msg.stride,
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
            check_rosidl_runtime_ret(msg.label.try_assign_bytes(self.label.as_bytes()))?;
            msg.size = self.size;
            msg.stride = self.stride;
            Ok(())
        }
    }
    impl Default for MultiArrayDimension {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<MultiArrayDimension>::new();
            MultiArrayDimension::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct MultiArrayLayout {
        pub dim: Vec<example_interfaces::msg::MultiArrayDimension>,
        pub data_offset: u32,
    }
    unsafe impl WrappedTypesupport for MultiArrayLayout {
        type CStruct = example_interfaces__msg__MultiArrayLayout;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__MultiArrayLayout()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__MultiArrayLayout {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__MultiArrayLayout__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__MultiArrayLayout__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__MultiArrayLayout) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__MultiArrayLayout__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__MultiArrayLayout__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> MultiArrayLayout {
            MultiArrayLayout {
                dim: {
                    let mut temp = Vec::with_capacity(msg.dim.size);
                    if msg.dim.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(msg.dim.data, msg.dim.size)
                        };
                        for s in slice {
                            temp.push(
                                example_interfaces::msg::MultiArrayDimension::from_native(s),
                            );
                        }
                    }
                    temp
                },
                data_offset: msg.data_offset,
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
                let length = self.dim.len();
                if length > msg.dim.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !example_interfaces__msg__MultiArrayDimension__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    example_interfaces__msg__MultiArrayDimension__Sequence__fini(
                        &mut msg.dim,
                    );
                    msg.dim = replacement;
                }
                msg.dim.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(msg.dim.data, length);
                    for (t, s) in slice.iter_mut().zip(&self.dim) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            msg.data_offset = self.data_offset;
            Ok(())
        }
    }
    impl Default for MultiArrayLayout {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<MultiArrayLayout>::new();
            MultiArrayLayout::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct String {
        pub data: crate::RosString,
    }
    unsafe impl WrappedTypesupport for String {
        type CStruct = example_interfaces__msg__String;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__String()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__String {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__String__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__String__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__String) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__String__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__String__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> String {
            String {
                data: crate::RosString::from(msg.data.as_bytes()),
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
            check_rosidl_runtime_ret(msg.data.try_assign_bytes(self.data.as_bytes()))?;
            Ok(())
        }
    }
    impl Default for String {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<String>::new();
            String::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct UInt16 {
        pub data: u16,
    }
    unsafe impl WrappedTypesupport for UInt16 {
        type CStruct = example_interfaces__msg__UInt16;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__UInt16()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__UInt16 {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt16__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__UInt16__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__UInt16) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt16__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__UInt16__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> UInt16 {
            UInt16 { data: msg.data }
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
    impl Default for UInt16 {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<UInt16>::new();
            UInt16::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct UInt16MultiArray {
        pub layout: example_interfaces::msg::MultiArrayLayout,
        pub data: Vec<u16>,
    }
    unsafe impl WrappedTypesupport for UInt16MultiArray {
        type CStruct = example_interfaces__msg__UInt16MultiArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__UInt16MultiArray()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__UInt16MultiArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt16MultiArray__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__UInt16MultiArray__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__UInt16MultiArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt16MultiArray__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__UInt16MultiArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> UInt16MultiArray {
            UInt16MultiArray {
                layout: example_interfaces::msg::MultiArrayLayout::from_native(
                    &msg.layout,
                ),
                data: msg.data.to_vec(),
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
            self.layout.try_copy_to_native(&mut msg.layout)?;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for UInt16MultiArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<UInt16MultiArray>::new();
            UInt16MultiArray::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct UInt32 {
        pub data: u32,
    }
    unsafe impl WrappedTypesupport for UInt32 {
        type CStruct = example_interfaces__msg__UInt32;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__UInt32()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__UInt32 {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt32__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__UInt32__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__UInt32) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt32__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__UInt32__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> UInt32 {
            UInt32 { data: msg.data }
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
    impl Default for UInt32 {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<UInt32>::new();
            UInt32::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct UInt32MultiArray {
        pub layout: example_interfaces::msg::MultiArrayLayout,
        pub data: Vec<u32>,
    }
    unsafe impl WrappedTypesupport for UInt32MultiArray {
        type CStruct = example_interfaces__msg__UInt32MultiArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__UInt32MultiArray()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__UInt32MultiArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt32MultiArray__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__UInt32MultiArray__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__UInt32MultiArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt32MultiArray__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__UInt32MultiArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> UInt32MultiArray {
            UInt32MultiArray {
                layout: example_interfaces::msg::MultiArrayLayout::from_native(
                    &msg.layout,
                ),
                data: msg.data.to_vec(),
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
            self.layout.try_copy_to_native(&mut msg.layout)?;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for UInt32MultiArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<UInt32MultiArray>::new();
            UInt32MultiArray::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct UInt64 {
        pub data: u64,
    }
    unsafe impl WrappedTypesupport for UInt64 {
        type CStruct = example_interfaces__msg__UInt64;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__UInt64()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__UInt64 {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt64__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__UInt64__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__UInt64) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt64__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__UInt64__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> UInt64 {
            UInt64 { data: msg.data }
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
    impl Default for UInt64 {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<UInt64>::new();
            UInt64::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct UInt64MultiArray {
        pub layout: example_interfaces::msg::MultiArrayLayout,
        pub data: Vec<u64>,
    }
    unsafe impl WrappedTypesupport for UInt64MultiArray {
        type CStruct = example_interfaces__msg__UInt64MultiArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__UInt64MultiArray()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__UInt64MultiArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt64MultiArray__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__UInt64MultiArray__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__UInt64MultiArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt64MultiArray__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__UInt64MultiArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> UInt64MultiArray {
            UInt64MultiArray {
                layout: example_interfaces::msg::MultiArrayLayout::from_native(
                    &msg.layout,
                ),
                data: msg.data.to_vec(),
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
            self.layout.try_copy_to_native(&mut msg.layout)?;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for UInt64MultiArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<UInt64MultiArray>::new();
            UInt64MultiArray::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct UInt8 {
        pub data: u8,
    }
    unsafe impl WrappedTypesupport for UInt8 {
        type CStruct = example_interfaces__msg__UInt8;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__UInt8()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__UInt8 {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt8__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__UInt8__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__UInt8) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt8__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__UInt8__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> UInt8 {
            UInt8 { data: msg.data }
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
    impl Default for UInt8 {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<UInt8>::new();
            UInt8::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct UInt8MultiArray {
        pub layout: example_interfaces::msg::MultiArrayLayout,
        pub data: Vec<u8>,
    }
    unsafe impl WrappedTypesupport for UInt8MultiArray {
        type CStruct = example_interfaces__msg__UInt8MultiArray;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__UInt8MultiArray()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__UInt8MultiArray {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt8MultiArray__create() }
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__UInt8MultiArray__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__UInt8MultiArray) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__UInt8MultiArray__destroy(msg) };
            #[cfg(feature = "doc-only")]
            example_interfaces__msg__UInt8MultiArray__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> UInt8MultiArray {
            UInt8MultiArray {
                layout: example_interfaces::msg::MultiArrayLayout::from_native(
                    &msg.layout,
                ),
                data: msg.data.to_vec(),
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
            self.layout.try_copy_to_native(&mut msg.layout)?;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for UInt8MultiArray {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<UInt8MultiArray>::new();
            UInt8MultiArray::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct WString {
        pub data: crate::RosWString,
    }
    unsafe impl WrappedTypesupport for WString {
        type CStruct = example_interfaces__msg__WString;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__msg__WString()
            }
        }
        fn create_msg() -> *mut example_interfaces__msg__WString {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__WString__create() }
            #[cfg(feature = "doc-only")] example_interfaces__msg__WString__create()
        }
        fn destroy_msg(msg: *mut example_interfaces__msg__WString) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { example_interfaces__msg__WString__destroy(msg) };
            #[cfg(feature = "doc-only")] example_interfaces__msg__WString__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> WString {
            WString {
                data: crate::RosWString::from(msg.data.as_units()),
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
            check_rosidl_runtime_ret(msg.data.try_assign_units(self.data.as_units()))?;
            Ok(())
        }
    }
    impl Default for WString {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<WString>::new();
            WString::from_native(&msg_native)
        }
    }
}
pub mod srv {
    #[allow(non_snake_case)]
    pub mod AddTwoInts {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__example_interfaces__srv__AddTwoInts()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub a: i64,
            pub b: i64,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = example_interfaces__srv__AddTwoInts_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__srv__AddTwoInts_Request()
                }
            }
            fn create_msg() -> *mut example_interfaces__srv__AddTwoInts_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__AddTwoInts_Request__create() }
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__AddTwoInts_Request__create()
            }
            fn destroy_msg(msg: *mut example_interfaces__srv__AddTwoInts_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__AddTwoInts_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__AddTwoInts_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request { a: msg.a, b: msg.b }
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
                msg.a = self.a;
                msg.b = self.b;
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
            pub sum: i64,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = example_interfaces__srv__AddTwoInts_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__srv__AddTwoInts_Response()
                }
            }
            fn create_msg() -> *mut example_interfaces__srv__AddTwoInts_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__AddTwoInts_Response__create() }
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__AddTwoInts_Response__create()
            }
            fn destroy_msg(
                msg: *mut example_interfaces__srv__AddTwoInts_Response,
            ) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__AddTwoInts_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__AddTwoInts_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response { sum: msg.sum }
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
                msg.sum = self.sum;
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
            pub request: Vec<example_interfaces::srv::AddTwoInts::Request>,
            pub response: Vec<example_interfaces::srv::AddTwoInts::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = example_interfaces__srv__AddTwoInts_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__srv__AddTwoInts_Event()
                }
            }
            fn create_msg() -> *mut example_interfaces__srv__AddTwoInts_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__AddTwoInts_Event__create() }
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__AddTwoInts_Event__create()
            }
            fn destroy_msg(msg: *mut example_interfaces__srv__AddTwoInts_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__AddTwoInts_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__AddTwoInts_Event__destroy(msg)
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
                                    example_interfaces::srv::AddTwoInts::Request::from_native(s),
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
                                    example_interfaces::srv::AddTwoInts::Response::from_native(
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
                        if !example_interfaces__srv__AddTwoInts_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        example_interfaces__srv__AddTwoInts_Request__Sequence__fini(
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
                        if !example_interfaces__srv__AddTwoInts_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        example_interfaces__srv__AddTwoInts_Response__Sequence__fini(
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__example_interfaces__srv__SetBool()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub data: bool,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = example_interfaces__srv__SetBool_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__srv__SetBool_Request()
                }
            }
            fn create_msg() -> *mut example_interfaces__srv__SetBool_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__SetBool_Request__create() }
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__SetBool_Request__create()
            }
            fn destroy_msg(msg: *mut example_interfaces__srv__SetBool_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__SetBool_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__SetBool_Request__destroy(msg)
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
            type CStruct = example_interfaces__srv__SetBool_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__srv__SetBool_Response()
                }
            }
            fn create_msg() -> *mut example_interfaces__srv__SetBool_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__SetBool_Response__create() }
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__SetBool_Response__create()
            }
            fn destroy_msg(msg: *mut example_interfaces__srv__SetBool_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__SetBool_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__SetBool_Response__destroy(msg)
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
            pub request: Vec<example_interfaces::srv::SetBool::Request>,
            pub response: Vec<example_interfaces::srv::SetBool::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = example_interfaces__srv__SetBool_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__srv__SetBool_Event()
                }
            }
            fn create_msg() -> *mut example_interfaces__srv__SetBool_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__SetBool_Event__create() }
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__SetBool_Event__create()
            }
            fn destroy_msg(msg: *mut example_interfaces__srv__SetBool_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__SetBool_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__SetBool_Event__destroy(msg)
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
                                    example_interfaces::srv::SetBool::Request::from_native(s),
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
                                    example_interfaces::srv::SetBool::Response::from_native(s),
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
                        if !example_interfaces__srv__SetBool_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        example_interfaces__srv__SetBool_Request__Sequence__fini(
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
                        if !example_interfaces__srv__SetBool_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        example_interfaces__srv__SetBool_Response__Sequence__fini(
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__example_interfaces__srv__Trigger()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {}
        unsafe impl WrappedTypesupport for Request {
            type CStruct = example_interfaces__srv__Trigger_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__srv__Trigger_Request()
                }
            }
            fn create_msg() -> *mut example_interfaces__srv__Trigger_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__Trigger_Request__create() }
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__Trigger_Request__create()
            }
            fn destroy_msg(msg: *mut example_interfaces__srv__Trigger_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__Trigger_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__Trigger_Request__destroy(msg)
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
            type CStruct = example_interfaces__srv__Trigger_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__srv__Trigger_Response()
                }
            }
            fn create_msg() -> *mut example_interfaces__srv__Trigger_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__Trigger_Response__create() }
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__Trigger_Response__create()
            }
            fn destroy_msg(msg: *mut example_interfaces__srv__Trigger_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__Trigger_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__Trigger_Response__destroy(msg)
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
            pub request: Vec<example_interfaces::srv::Trigger::Request>,
            pub response: Vec<example_interfaces::srv::Trigger::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = example_interfaces__srv__Trigger_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__example_interfaces__srv__Trigger_Event()
                }
            }
            fn create_msg() -> *mut example_interfaces__srv__Trigger_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__Trigger_Event__create() }
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__Trigger_Event__create()
            }
            fn destroy_msg(msg: *mut example_interfaces__srv__Trigger_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { example_interfaces__srv__Trigger_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                example_interfaces__srv__Trigger_Event__destroy(msg)
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
                                    example_interfaces::srv::Trigger::Request::from_native(s),
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
                                    example_interfaces::srv::Trigger::Response::from_native(s),
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
                        if !example_interfaces__srv__Trigger_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        example_interfaces__srv__Trigger_Request__Sequence__fini(
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
                        if !example_interfaces__srv__Trigger_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        example_interfaces__srv__Trigger_Response__Sequence__fini(
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
