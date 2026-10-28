pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Action {
        pub name: crate::RosString,
        pub send_goal: rosgraph_msgs::msg::Service,
        pub get_result: rosgraph_msgs::msg::Service,
        pub cancel_goal: rosgraph_msgs::msg::Service,
        pub feedback: rosgraph_msgs::msg::Topic,
        pub status: rosgraph_msgs::msg::Topic,
    }
    unsafe impl WrappedTypesupport for Action {
        type CStruct = rosgraph_msgs__msg__Action;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rosgraph_msgs__msg__Action()
            }
        }
        fn create_msg() -> *mut rosgraph_msgs__msg__Action {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__Action__create() }
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__Action__create()
        }
        fn destroy_msg(msg: *mut rosgraph_msgs__msg__Action) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__Action__destroy(msg) };
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__Action__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Action {
            Action {
                name: crate::RosString::from(msg.name.as_bytes()),
                send_goal: rosgraph_msgs::msg::Service::from_native(&msg.send_goal),
                get_result: rosgraph_msgs::msg::Service::from_native(&msg.get_result),
                cancel_goal: rosgraph_msgs::msg::Service::from_native(&msg.cancel_goal),
                feedback: rosgraph_msgs::msg::Topic::from_native(&msg.feedback),
                status: rosgraph_msgs::msg::Topic::from_native(&msg.status),
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
            self.send_goal.try_copy_to_native(&mut msg.send_goal)?;
            self.get_result.try_copy_to_native(&mut msg.get_result)?;
            self.cancel_goal.try_copy_to_native(&mut msg.cancel_goal)?;
            self.feedback.try_copy_to_native(&mut msg.feedback)?;
            self.status.try_copy_to_native(&mut msg.status)?;
            Ok(())
        }
    }
    impl Default for Action {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Action>::new();
            Action::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Clock {
        pub clock: builtin_interfaces::msg::Time,
    }
    unsafe impl WrappedTypesupport for Clock {
        type CStruct = rosgraph_msgs__msg__Clock;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rosgraph_msgs__msg__Clock()
            }
        }
        fn create_msg() -> *mut rosgraph_msgs__msg__Clock {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__Clock__create() }
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__Clock__create()
        }
        fn destroy_msg(msg: *mut rosgraph_msgs__msg__Clock) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__Clock__destroy(msg) };
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__Clock__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Clock {
            Clock {
                clock: builtin_interfaces::msg::Time::from_native(&msg.clock),
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
            self.clock.try_copy_to_native(&mut msg.clock)?;
            Ok(())
        }
    }
    impl Default for Clock {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Clock>::new();
            Clock::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Graph {
        pub nodes: Vec<rosgraph_msgs::msg::Node>,
    }
    unsafe impl WrappedTypesupport for Graph {
        type CStruct = rosgraph_msgs__msg__Graph;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rosgraph_msgs__msg__Graph()
            }
        }
        fn create_msg() -> *mut rosgraph_msgs__msg__Graph {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__Graph__create() }
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__Graph__create()
        }
        fn destroy_msg(msg: *mut rosgraph_msgs__msg__Graph) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__Graph__destroy(msg) };
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__Graph__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Graph {
            Graph {
                nodes: {
                    let mut temp = Vec::with_capacity(msg.nodes.size);
                    if msg.nodes.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(msg.nodes.data, msg.nodes.size)
                        };
                        for s in slice {
                            temp.push(rosgraph_msgs::msg::Node::from_native(s));
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
                let length = self.nodes.len();
                if length > msg.nodes.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rosgraph_msgs__msg__Node__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rosgraph_msgs__msg__Node__Sequence__fini(&mut msg.nodes);
                    msg.nodes = replacement;
                }
                msg.nodes.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(msg.nodes.data, length);
                    for (t, s) in slice.iter_mut().zip(&self.nodes) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for Graph {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Graph>::new();
            Graph::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct InterfaceType {
        pub name: crate::RosString,
        pub hash: rosgraph_msgs::msg::TypeHash,
    }
    unsafe impl WrappedTypesupport for InterfaceType {
        type CStruct = rosgraph_msgs__msg__InterfaceType;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rosgraph_msgs__msg__InterfaceType()
            }
        }
        fn create_msg() -> *mut rosgraph_msgs__msg__InterfaceType {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__InterfaceType__create() }
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__InterfaceType__create()
        }
        fn destroy_msg(msg: *mut rosgraph_msgs__msg__InterfaceType) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__InterfaceType__destroy(msg) };
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__InterfaceType__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> InterfaceType {
            InterfaceType {
                name: crate::RosString::from(msg.name.as_bytes()),
                hash: rosgraph_msgs::msg::TypeHash::from_native(&msg.hash),
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
            self.hash.try_copy_to_native(&mut msg.hash)?;
            Ok(())
        }
    }
    impl Default for InterfaceType {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<InterfaceType>::new();
            InterfaceType::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Node {
        pub name: crate::RosString,
        pub parameters: Vec<rcl_interfaces::msg::ParameterDescriptor>,
        pub parameter_values: Vec<rcl_interfaces::msg::ParameterValue>,
        pub publishers: Vec<rosgraph_msgs::msg::Topic>,
        pub subscriptions: Vec<rosgraph_msgs::msg::Topic>,
        pub service_clients: Vec<rosgraph_msgs::msg::Service>,
        pub service_servers: Vec<rosgraph_msgs::msg::Service>,
        pub action_clients: Vec<rosgraph_msgs::msg::Action>,
        pub action_servers: Vec<rosgraph_msgs::msg::Action>,
    }
    unsafe impl WrappedTypesupport for Node {
        type CStruct = rosgraph_msgs__msg__Node;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rosgraph_msgs__msg__Node()
            }
        }
        fn create_msg() -> *mut rosgraph_msgs__msg__Node {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__Node__create() }
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__Node__create()
        }
        fn destroy_msg(msg: *mut rosgraph_msgs__msg__Node) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__Node__destroy(msg) };
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__Node__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Node {
            Node {
                name: crate::RosString::from(msg.name.as_bytes()),
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
                            temp.push(
                                rcl_interfaces::msg::ParameterDescriptor::from_native(s),
                            );
                        }
                    }
                    temp
                },
                parameter_values: {
                    let mut temp = Vec::with_capacity(msg.parameter_values.size);
                    if msg.parameter_values.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.parameter_values.data,
                                msg.parameter_values.size,
                            )
                        };
                        for s in slice {
                            temp.push(
                                rcl_interfaces::msg::ParameterValue::from_native(s),
                            );
                        }
                    }
                    temp
                },
                publishers: {
                    let mut temp = Vec::with_capacity(msg.publishers.size);
                    if msg.publishers.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.publishers.data,
                                msg.publishers.size,
                            )
                        };
                        for s in slice {
                            temp.push(rosgraph_msgs::msg::Topic::from_native(s));
                        }
                    }
                    temp
                },
                subscriptions: {
                    let mut temp = Vec::with_capacity(msg.subscriptions.size);
                    if msg.subscriptions.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.subscriptions.data,
                                msg.subscriptions.size,
                            )
                        };
                        for s in slice {
                            temp.push(rosgraph_msgs::msg::Topic::from_native(s));
                        }
                    }
                    temp
                },
                service_clients: {
                    let mut temp = Vec::with_capacity(msg.service_clients.size);
                    if msg.service_clients.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.service_clients.data,
                                msg.service_clients.size,
                            )
                        };
                        for s in slice {
                            temp.push(rosgraph_msgs::msg::Service::from_native(s));
                        }
                    }
                    temp
                },
                service_servers: {
                    let mut temp = Vec::with_capacity(msg.service_servers.size);
                    if msg.service_servers.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.service_servers.data,
                                msg.service_servers.size,
                            )
                        };
                        for s in slice {
                            temp.push(rosgraph_msgs::msg::Service::from_native(s));
                        }
                    }
                    temp
                },
                action_clients: {
                    let mut temp = Vec::with_capacity(msg.action_clients.size);
                    if msg.action_clients.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.action_clients.data,
                                msg.action_clients.size,
                            )
                        };
                        for s in slice {
                            temp.push(rosgraph_msgs::msg::Action::from_native(s));
                        }
                    }
                    temp
                },
                action_servers: {
                    let mut temp = Vec::with_capacity(msg.action_servers.size);
                    if msg.action_servers.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.action_servers.data,
                                msg.action_servers.size,
                            )
                        };
                        for s in slice {
                            temp.push(rosgraph_msgs::msg::Action::from_native(s));
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
            unsafe {
                let length = self.parameters.len();
                if length > msg.parameters.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rcl_interfaces__msg__ParameterDescriptor__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rcl_interfaces__msg__ParameterDescriptor__Sequence__fini(
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
            unsafe {
                let length = self.parameter_values.len();
                if length > msg.parameter_values.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rcl_interfaces__msg__ParameterValue__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rcl_interfaces__msg__ParameterValue__Sequence__fini(
                        &mut msg.parameter_values,
                    );
                    msg.parameter_values = replacement;
                }
                msg.parameter_values.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.parameter_values.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.parameter_values) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.publishers.len();
                if length > msg.publishers.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rosgraph_msgs__msg__Topic__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rosgraph_msgs__msg__Topic__Sequence__fini(&mut msg.publishers);
                    msg.publishers = replacement;
                }
                msg.publishers.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.publishers.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.publishers) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.subscriptions.len();
                if length > msg.subscriptions.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rosgraph_msgs__msg__Topic__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rosgraph_msgs__msg__Topic__Sequence__fini(&mut msg.subscriptions);
                    msg.subscriptions = replacement;
                }
                msg.subscriptions.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.subscriptions.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.subscriptions) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.service_clients.len();
                if length > msg.service_clients.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rosgraph_msgs__msg__Service__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rosgraph_msgs__msg__Service__Sequence__fini(
                        &mut msg.service_clients,
                    );
                    msg.service_clients = replacement;
                }
                msg.service_clients.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.service_clients.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.service_clients) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.service_servers.len();
                if length > msg.service_servers.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rosgraph_msgs__msg__Service__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rosgraph_msgs__msg__Service__Sequence__fini(
                        &mut msg.service_servers,
                    );
                    msg.service_servers = replacement;
                }
                msg.service_servers.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.service_servers.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.service_servers) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.action_clients.len();
                if length > msg.action_clients.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rosgraph_msgs__msg__Action__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rosgraph_msgs__msg__Action__Sequence__fini(&mut msg.action_clients);
                    msg.action_clients = replacement;
                }
                msg.action_clients.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.action_clients.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.action_clients) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.action_servers.len();
                if length > msg.action_servers.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !rosgraph_msgs__msg__Action__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    rosgraph_msgs__msg__Action__Sequence__fini(&mut msg.action_servers);
                    msg.action_servers = replacement;
                }
                msg.action_servers.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.action_servers.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.action_servers) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for Node {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Node>::new();
            Node::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct QoSProfile {
        pub depth: u32,
        pub deadline: builtin_interfaces::msg::Duration,
        pub lifespan: builtin_interfaces::msg::Duration,
        pub history: u8,
        pub reliability: u8,
        pub durability: u8,
        pub liveliness: u8,
        pub liveliness_lease_duration: builtin_interfaces::msg::Duration,
    }
    unsafe impl WrappedTypesupport for QoSProfile {
        type CStruct = rosgraph_msgs__msg__QoSProfile;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rosgraph_msgs__msg__QoSProfile()
            }
        }
        fn create_msg() -> *mut rosgraph_msgs__msg__QoSProfile {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__QoSProfile__create() }
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__QoSProfile__create()
        }
        fn destroy_msg(msg: *mut rosgraph_msgs__msg__QoSProfile) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__QoSProfile__destroy(msg) };
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__QoSProfile__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> QoSProfile {
            QoSProfile {
                depth: msg.depth,
                deadline: builtin_interfaces::msg::Duration::from_native(&msg.deadline),
                lifespan: builtin_interfaces::msg::Duration::from_native(&msg.lifespan),
                history: msg.history,
                reliability: msg.reliability,
                durability: msg.durability,
                liveliness: msg.liveliness,
                liveliness_lease_duration: builtin_interfaces::msg::Duration::from_native(
                    &msg.liveliness_lease_duration,
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
            msg.depth = self.depth;
            self.deadline.try_copy_to_native(&mut msg.deadline)?;
            self.lifespan.try_copy_to_native(&mut msg.lifespan)?;
            msg.history = self.history;
            msg.reliability = self.reliability;
            msg.durability = self.durability;
            msg.liveliness = self.liveliness;
            self.liveliness_lease_duration
                .try_copy_to_native(&mut msg.liveliness_lease_duration)?;
            Ok(())
        }
    }
    impl Default for QoSProfile {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<QoSProfile>::new();
            QoSProfile::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl QoSProfile {
        pub const DURABILITY_BEST_AVAILABLE: _bindgen_ty_264 = rosgraph_msgs__msg__QoSProfile__DURABILITY_BEST_AVAILABLE;
        pub const DURABILITY_SYSTEM_DEFAULT: _bindgen_ty_260 = rosgraph_msgs__msg__QoSProfile__DURABILITY_SYSTEM_DEFAULT;
        pub const DURABILITY_TRANSIENT_LOCAL: _bindgen_ty_261 = rosgraph_msgs__msg__QoSProfile__DURABILITY_TRANSIENT_LOCAL;
        pub const DURABILITY_UNKNOWN: _bindgen_ty_263 = rosgraph_msgs__msg__QoSProfile__DURABILITY_UNKNOWN;
        pub const DURABILITY_VOLATILE: _bindgen_ty_262 = rosgraph_msgs__msg__QoSProfile__DURABILITY_VOLATILE;
        pub const HISTORY_KEEP_ALL: _bindgen_ty_253 = rosgraph_msgs__msg__QoSProfile__HISTORY_KEEP_ALL;
        pub const HISTORY_KEEP_LAST: _bindgen_ty_252 = rosgraph_msgs__msg__QoSProfile__HISTORY_KEEP_LAST;
        pub const HISTORY_SYSTEM_DEFAULT: _bindgen_ty_251 = rosgraph_msgs__msg__QoSProfile__HISTORY_SYSTEM_DEFAULT;
        pub const HISTORY_UNKNOWN: _bindgen_ty_254 = rosgraph_msgs__msg__QoSProfile__HISTORY_UNKNOWN;
        pub const LIVELINESS_AUTOMATIC: _bindgen_ty_266 = rosgraph_msgs__msg__QoSProfile__LIVELINESS_AUTOMATIC;
        pub const LIVELINESS_BEST_AVAILABLE: _bindgen_ty_269 = rosgraph_msgs__msg__QoSProfile__LIVELINESS_BEST_AVAILABLE;
        pub const LIVELINESS_MANUAL_BY_TOPIC: _bindgen_ty_267 = rosgraph_msgs__msg__QoSProfile__LIVELINESS_MANUAL_BY_TOPIC;
        pub const LIVELINESS_SYSTEM_DEFAULT: _bindgen_ty_265 = rosgraph_msgs__msg__QoSProfile__LIVELINESS_SYSTEM_DEFAULT;
        pub const LIVELINESS_UNKNOWN: _bindgen_ty_268 = rosgraph_msgs__msg__QoSProfile__LIVELINESS_UNKNOWN;
        pub const RELIABILITY_BEST_AVAILABLE: _bindgen_ty_259 = rosgraph_msgs__msg__QoSProfile__RELIABILITY_BEST_AVAILABLE;
        pub const RELIABILITY_BEST_EFFORT: _bindgen_ty_257 = rosgraph_msgs__msg__QoSProfile__RELIABILITY_BEST_EFFORT;
        pub const RELIABILITY_RELIABLE: _bindgen_ty_256 = rosgraph_msgs__msg__QoSProfile__RELIABILITY_RELIABLE;
        pub const RELIABILITY_SYSTEM_DEFAULT: _bindgen_ty_255 = rosgraph_msgs__msg__QoSProfile__RELIABILITY_SYSTEM_DEFAULT;
        pub const RELIABILITY_UNKNOWN: _bindgen_ty_258 = rosgraph_msgs__msg__QoSProfile__RELIABILITY_UNKNOWN;
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Service {
        pub name: crate::RosString,
        pub request_type: rosgraph_msgs::msg::InterfaceType,
        pub request_qos: rosgraph_msgs::msg::QoSProfile,
        pub response_type: rosgraph_msgs::msg::InterfaceType,
        pub response_qos: rosgraph_msgs::msg::QoSProfile,
    }
    unsafe impl WrappedTypesupport for Service {
        type CStruct = rosgraph_msgs__msg__Service;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rosgraph_msgs__msg__Service()
            }
        }
        fn create_msg() -> *mut rosgraph_msgs__msg__Service {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__Service__create() }
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__Service__create()
        }
        fn destroy_msg(msg: *mut rosgraph_msgs__msg__Service) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__Service__destroy(msg) };
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__Service__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Service {
            Service {
                name: crate::RosString::from(msg.name.as_bytes()),
                request_type: rosgraph_msgs::msg::InterfaceType::from_native(
                    &msg.request_type,
                ),
                request_qos: rosgraph_msgs::msg::QoSProfile::from_native(
                    &msg.request_qos,
                ),
                response_type: rosgraph_msgs::msg::InterfaceType::from_native(
                    &msg.response_type,
                ),
                response_qos: rosgraph_msgs::msg::QoSProfile::from_native(
                    &msg.response_qos,
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
            check_rosidl_runtime_ret(msg.name.try_assign_bytes(self.name.as_bytes()))?;
            self.request_type.try_copy_to_native(&mut msg.request_type)?;
            self.request_qos.try_copy_to_native(&mut msg.request_qos)?;
            self.response_type.try_copy_to_native(&mut msg.response_type)?;
            self.response_qos.try_copy_to_native(&mut msg.response_qos)?;
            Ok(())
        }
    }
    impl Default for Service {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Service>::new();
            Service::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Topic {
        pub name: crate::RosString,
        #[serde(rename = "type")]
        pub type_: rosgraph_msgs::msg::InterfaceType,
        pub qos: rosgraph_msgs::msg::QoSProfile,
    }
    unsafe impl WrappedTypesupport for Topic {
        type CStruct = rosgraph_msgs__msg__Topic;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rosgraph_msgs__msg__Topic()
            }
        }
        fn create_msg() -> *mut rosgraph_msgs__msg__Topic {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__Topic__create() }
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__Topic__create()
        }
        fn destroy_msg(msg: *mut rosgraph_msgs__msg__Topic) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__Topic__destroy(msg) };
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__Topic__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Topic {
            Topic {
                name: crate::RosString::from(msg.name.as_bytes()),
                type_: rosgraph_msgs::msg::InterfaceType::from_native(&msg.type_),
                qos: rosgraph_msgs::msg::QoSProfile::from_native(&msg.qos),
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
            self.qos.try_copy_to_native(&mut msg.qos)?;
            Ok(())
        }
    }
    impl Default for Topic {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Topic>::new();
            Topic::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct TypeHash {
        pub version: u8,
        #[serde(with = "crate::msg_types::fixed_array")]
        pub value: [u8; 32usize],
    }
    unsafe impl WrappedTypesupport for TypeHash {
        type CStruct = rosgraph_msgs__msg__TypeHash;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__rosgraph_msgs__msg__TypeHash()
            }
        }
        fn create_msg() -> *mut rosgraph_msgs__msg__TypeHash {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__TypeHash__create() }
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__TypeHash__create()
        }
        fn destroy_msg(msg: *mut rosgraph_msgs__msg__TypeHash) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { rosgraph_msgs__msg__TypeHash__destroy(msg) };
            #[cfg(feature = "doc-only")] rosgraph_msgs__msg__TypeHash__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> TypeHash {
            TypeHash {
                version: msg.version,
                value: msg.value,
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
            msg.version = self.version;
            msg.value.copy_from_slice(&self.value[..32usize]);
            Ok(())
        }
    }
    impl Default for TypeHash {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<TypeHash>::new();
            TypeHash::from_native(&msg_native)
        }
    }
}
