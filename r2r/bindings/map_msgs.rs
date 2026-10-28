pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct OccupancyGridUpdate {
        pub header: std_msgs::msg::Header,
        pub x: i32,
        pub y: i32,
        pub width: u32,
        pub height: u32,
        pub data: Vec<i8>,
    }
    unsafe impl WrappedTypesupport for OccupancyGridUpdate {
        type CStruct = map_msgs__msg__OccupancyGridUpdate;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__msg__OccupancyGridUpdate()
            }
        }
        fn create_msg() -> *mut map_msgs__msg__OccupancyGridUpdate {
            #[cfg(not(feature = "doc-only"))]
            unsafe { map_msgs__msg__OccupancyGridUpdate__create() }
            #[cfg(feature = "doc-only")] map_msgs__msg__OccupancyGridUpdate__create()
        }
        fn destroy_msg(msg: *mut map_msgs__msg__OccupancyGridUpdate) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { map_msgs__msg__OccupancyGridUpdate__destroy(msg) };
            #[cfg(feature = "doc-only")] map_msgs__msg__OccupancyGridUpdate__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> OccupancyGridUpdate {
            OccupancyGridUpdate {
                header: std_msgs::msg::Header::from_native(&msg.header),
                x: msg.x,
                y: msg.y,
                width: msg.width,
                height: msg.height,
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
            self.header.try_copy_to_native(&mut msg.header)?;
            msg.x = self.x;
            msg.y = self.y;
            msg.width = self.width;
            msg.height = self.height;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for OccupancyGridUpdate {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<OccupancyGridUpdate>::new();
            OccupancyGridUpdate::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct PointCloud2Update {
        pub header: std_msgs::msg::Header,
        #[serde(rename = "type")]
        pub type_: u32,
        pub points: sensor_msgs::msg::PointCloud2,
    }
    unsafe impl WrappedTypesupport for PointCloud2Update {
        type CStruct = map_msgs__msg__PointCloud2Update;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__msg__PointCloud2Update()
            }
        }
        fn create_msg() -> *mut map_msgs__msg__PointCloud2Update {
            #[cfg(not(feature = "doc-only"))]
            unsafe { map_msgs__msg__PointCloud2Update__create() }
            #[cfg(feature = "doc-only")] map_msgs__msg__PointCloud2Update__create()
        }
        fn destroy_msg(msg: *mut map_msgs__msg__PointCloud2Update) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { map_msgs__msg__PointCloud2Update__destroy(msg) };
            #[cfg(feature = "doc-only")] map_msgs__msg__PointCloud2Update__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> PointCloud2Update {
            PointCloud2Update {
                header: std_msgs::msg::Header::from_native(&msg.header),
                type_: msg.type_,
                points: sensor_msgs::msg::PointCloud2::from_native(&msg.points),
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
            msg.type_ = self.type_;
            self.points.try_copy_to_native(&mut msg.points)?;
            Ok(())
        }
    }
    impl Default for PointCloud2Update {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<PointCloud2Update>::new();
            PointCloud2Update::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl PointCloud2Update {
        pub const ADD: _bindgen_ty_176 = map_msgs__msg__PointCloud2Update__ADD;
        pub const DELETE: _bindgen_ty_177 = map_msgs__msg__PointCloud2Update__DELETE;
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ProjectedMap {
        pub map: nav_msgs::msg::OccupancyGrid,
        pub min_z: f64,
        pub max_z: f64,
    }
    unsafe impl WrappedTypesupport for ProjectedMap {
        type CStruct = map_msgs__msg__ProjectedMap;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__msg__ProjectedMap()
            }
        }
        fn create_msg() -> *mut map_msgs__msg__ProjectedMap {
            #[cfg(not(feature = "doc-only"))]
            unsafe { map_msgs__msg__ProjectedMap__create() }
            #[cfg(feature = "doc-only")] map_msgs__msg__ProjectedMap__create()
        }
        fn destroy_msg(msg: *mut map_msgs__msg__ProjectedMap) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { map_msgs__msg__ProjectedMap__destroy(msg) };
            #[cfg(feature = "doc-only")] map_msgs__msg__ProjectedMap__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> ProjectedMap {
            ProjectedMap {
                map: nav_msgs::msg::OccupancyGrid::from_native(&msg.map),
                min_z: msg.min_z,
                max_z: msg.max_z,
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
            self.map.try_copy_to_native(&mut msg.map)?;
            msg.min_z = self.min_z;
            msg.max_z = self.max_z;
            Ok(())
        }
    }
    impl Default for ProjectedMap {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<ProjectedMap>::new();
            ProjectedMap::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct ProjectedMapInfo {
        pub frame_id: crate::RosString,
        pub x: f64,
        pub y: f64,
        pub width: f64,
        pub height: f64,
        pub min_z: f64,
        pub max_z: f64,
    }
    unsafe impl WrappedTypesupport for ProjectedMapInfo {
        type CStruct = map_msgs__msg__ProjectedMapInfo;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__msg__ProjectedMapInfo()
            }
        }
        fn create_msg() -> *mut map_msgs__msg__ProjectedMapInfo {
            #[cfg(not(feature = "doc-only"))]
            unsafe { map_msgs__msg__ProjectedMapInfo__create() }
            #[cfg(feature = "doc-only")] map_msgs__msg__ProjectedMapInfo__create()
        }
        fn destroy_msg(msg: *mut map_msgs__msg__ProjectedMapInfo) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { map_msgs__msg__ProjectedMapInfo__destroy(msg) };
            #[cfg(feature = "doc-only")] map_msgs__msg__ProjectedMapInfo__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> ProjectedMapInfo {
            ProjectedMapInfo {
                frame_id: crate::RosString::from(msg.frame_id.as_bytes()),
                x: msg.x,
                y: msg.y,
                width: msg.width,
                height: msg.height,
                min_z: msg.min_z,
                max_z: msg.max_z,
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
                msg.frame_id.try_assign_bytes(self.frame_id.as_bytes()),
            )?;
            msg.x = self.x;
            msg.y = self.y;
            msg.width = self.width;
            msg.height = self.height;
            msg.min_z = self.min_z;
            msg.max_z = self.max_z;
            Ok(())
        }
    }
    impl Default for ProjectedMapInfo {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<ProjectedMapInfo>::new();
            ProjectedMapInfo::from_native(&msg_native)
        }
    }
}
pub mod srv {
    #[allow(non_snake_case)]
    pub mod GetMapROI {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__map_msgs__srv__GetMapROI()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub x: f64,
            pub y: f64,
            pub l_x: f64,
            pub l_y: f64,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = map_msgs__srv__GetMapROI_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__GetMapROI_Request()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__GetMapROI_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetMapROI_Request__create() }
                #[cfg(feature = "doc-only")] map_msgs__srv__GetMapROI_Request__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__GetMapROI_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetMapROI_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__GetMapROI_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    x: msg.x,
                    y: msg.y,
                    l_x: msg.l_x,
                    l_y: msg.l_y,
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
                msg.x = self.x;
                msg.y = self.y;
                msg.l_x = self.l_x;
                msg.l_y = self.l_y;
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
            pub sub_map: nav_msgs::msg::OccupancyGrid,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = map_msgs__srv__GetMapROI_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__GetMapROI_Response()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__GetMapROI_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetMapROI_Response__create() }
                #[cfg(feature = "doc-only")] map_msgs__srv__GetMapROI_Response__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__GetMapROI_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetMapROI_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__GetMapROI_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    sub_map: nav_msgs::msg::OccupancyGrid::from_native(&msg.sub_map),
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
                self.sub_map.try_copy_to_native(&mut msg.sub_map)?;
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
            pub request: Vec<map_msgs::srv::GetMapROI::Request>,
            pub response: Vec<map_msgs::srv::GetMapROI::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = map_msgs__srv__GetMapROI_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__GetMapROI_Event()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__GetMapROI_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetMapROI_Event__create() }
                #[cfg(feature = "doc-only")] map_msgs__srv__GetMapROI_Event__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__GetMapROI_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetMapROI_Event__destroy(msg) };
                #[cfg(feature = "doc-only")] map_msgs__srv__GetMapROI_Event__destroy(msg)
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
                                    map_msgs::srv::GetMapROI::Request::from_native(s),
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
                                    map_msgs::srv::GetMapROI::Response::from_native(s),
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
                        if !map_msgs__srv__GetMapROI_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__srv__GetMapROI_Request__Sequence__fini(
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
                        if !map_msgs__srv__GetMapROI_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__srv__GetMapROI_Response__Sequence__fini(
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
    pub mod GetPointMap {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__map_msgs__srv__GetPointMap()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {}
        unsafe impl WrappedTypesupport for Request {
            type CStruct = map_msgs__srv__GetPointMap_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__GetPointMap_Request()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__GetPointMap_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetPointMap_Request__create() }
                #[cfg(feature = "doc-only")] map_msgs__srv__GetPointMap_Request__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__GetPointMap_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetPointMap_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__GetPointMap_Request__destroy(msg)
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
            pub map: sensor_msgs::msg::PointCloud2,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = map_msgs__srv__GetPointMap_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__GetPointMap_Response()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__GetPointMap_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetPointMap_Response__create() }
                #[cfg(feature = "doc-only")]
                map_msgs__srv__GetPointMap_Response__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__GetPointMap_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetPointMap_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__GetPointMap_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    map: sensor_msgs::msg::PointCloud2::from_native(&msg.map),
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
                self.map.try_copy_to_native(&mut msg.map)?;
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
            pub request: Vec<map_msgs::srv::GetPointMap::Request>,
            pub response: Vec<map_msgs::srv::GetPointMap::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = map_msgs__srv__GetPointMap_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__GetPointMap_Event()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__GetPointMap_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetPointMap_Event__create() }
                #[cfg(feature = "doc-only")] map_msgs__srv__GetPointMap_Event__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__GetPointMap_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetPointMap_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__GetPointMap_Event__destroy(msg)
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
                                    map_msgs::srv::GetPointMap::Request::from_native(s),
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
                                    map_msgs::srv::GetPointMap::Response::from_native(s),
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
                        if !map_msgs__srv__GetPointMap_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__srv__GetPointMap_Request__Sequence__fini(
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
                        if !map_msgs__srv__GetPointMap_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__srv__GetPointMap_Response__Sequence__fini(
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
    pub mod GetPointMapROI {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__map_msgs__srv__GetPointMapROI()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub x: f64,
            pub y: f64,
            pub z: f64,
            pub r: f64,
            pub l_x: f64,
            pub l_y: f64,
            pub l_z: f64,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = map_msgs__srv__GetPointMapROI_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__GetPointMapROI_Request()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__GetPointMapROI_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetPointMapROI_Request__create() }
                #[cfg(feature = "doc-only")]
                map_msgs__srv__GetPointMapROI_Request__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__GetPointMapROI_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetPointMapROI_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__GetPointMapROI_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    x: msg.x,
                    y: msg.y,
                    z: msg.z,
                    r: msg.r,
                    l_x: msg.l_x,
                    l_y: msg.l_y,
                    l_z: msg.l_z,
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
                msg.x = self.x;
                msg.y = self.y;
                msg.z = self.z;
                msg.r = self.r;
                msg.l_x = self.l_x;
                msg.l_y = self.l_y;
                msg.l_z = self.l_z;
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
            pub sub_map: sensor_msgs::msg::PointCloud2,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = map_msgs__srv__GetPointMapROI_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__GetPointMapROI_Response()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__GetPointMapROI_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetPointMapROI_Response__create() }
                #[cfg(feature = "doc-only")]
                map_msgs__srv__GetPointMapROI_Response__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__GetPointMapROI_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetPointMapROI_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__GetPointMapROI_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    sub_map: sensor_msgs::msg::PointCloud2::from_native(&msg.sub_map),
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
                self.sub_map.try_copy_to_native(&mut msg.sub_map)?;
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
            pub request: Vec<map_msgs::srv::GetPointMapROI::Request>,
            pub response: Vec<map_msgs::srv::GetPointMapROI::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = map_msgs__srv__GetPointMapROI_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__GetPointMapROI_Event()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__GetPointMapROI_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetPointMapROI_Event__create() }
                #[cfg(feature = "doc-only")]
                map_msgs__srv__GetPointMapROI_Event__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__GetPointMapROI_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__GetPointMapROI_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__GetPointMapROI_Event__destroy(msg)
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
                                    map_msgs::srv::GetPointMapROI::Request::from_native(s),
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
                                    map_msgs::srv::GetPointMapROI::Response::from_native(s),
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
                        if !map_msgs__srv__GetPointMapROI_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__srv__GetPointMapROI_Request__Sequence__fini(
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
                        if !map_msgs__srv__GetPointMapROI_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__srv__GetPointMapROI_Response__Sequence__fini(
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
    pub mod ProjectedMapsInfo {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__map_msgs__srv__ProjectedMapsInfo()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub projected_maps_info: Vec<map_msgs::msg::ProjectedMapInfo>,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = map_msgs__srv__ProjectedMapsInfo_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__ProjectedMapsInfo_Request()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__ProjectedMapsInfo_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__ProjectedMapsInfo_Request__create() }
                #[cfg(feature = "doc-only")]
                map_msgs__srv__ProjectedMapsInfo_Request__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__ProjectedMapsInfo_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__ProjectedMapsInfo_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__ProjectedMapsInfo_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    projected_maps_info: {
                        let mut temp = Vec::with_capacity(msg.projected_maps_info.size);
                        if msg.projected_maps_info.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                    msg.projected_maps_info.data,
                                    msg.projected_maps_info.size,
                                )
                            };
                            for s in slice {
                                temp.push(map_msgs::msg::ProjectedMapInfo::from_native(s));
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
                    let length = self.projected_maps_info.len();
                    if length > msg.projected_maps_info.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !map_msgs__msg__ProjectedMapInfo__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__msg__ProjectedMapInfo__Sequence__fini(
                            &mut msg.projected_maps_info,
                        );
                        msg.projected_maps_info = replacement;
                    }
                    msg.projected_maps_info.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.projected_maps_info.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.projected_maps_info) {
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
        pub struct Response {}
        unsafe impl WrappedTypesupport for Response {
            type CStruct = map_msgs__srv__ProjectedMapsInfo_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__ProjectedMapsInfo_Response()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__ProjectedMapsInfo_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__ProjectedMapsInfo_Response__create() }
                #[cfg(feature = "doc-only")]
                map_msgs__srv__ProjectedMapsInfo_Response__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__ProjectedMapsInfo_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__ProjectedMapsInfo_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__ProjectedMapsInfo_Response__destroy(msg)
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
            pub request: Vec<map_msgs::srv::ProjectedMapsInfo::Request>,
            pub response: Vec<map_msgs::srv::ProjectedMapsInfo::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = map_msgs__srv__ProjectedMapsInfo_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__ProjectedMapsInfo_Event()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__ProjectedMapsInfo_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__ProjectedMapsInfo_Event__create() }
                #[cfg(feature = "doc-only")]
                map_msgs__srv__ProjectedMapsInfo_Event__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__ProjectedMapsInfo_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__ProjectedMapsInfo_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__ProjectedMapsInfo_Event__destroy(msg)
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
                                    map_msgs::srv::ProjectedMapsInfo::Request::from_native(s),
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
                                    map_msgs::srv::ProjectedMapsInfo::Response::from_native(s),
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
                        if !map_msgs__srv__ProjectedMapsInfo_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__srv__ProjectedMapsInfo_Request__Sequence__fini(
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
                        if !map_msgs__srv__ProjectedMapsInfo_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__srv__ProjectedMapsInfo_Response__Sequence__fini(
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
    pub mod SaveMap {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__map_msgs__srv__SaveMap()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub filename: std_msgs::msg::String,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = map_msgs__srv__SaveMap_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__SaveMap_Request()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__SaveMap_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__SaveMap_Request__create() }
                #[cfg(feature = "doc-only")] map_msgs__srv__SaveMap_Request__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__SaveMap_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__SaveMap_Request__destroy(msg) };
                #[cfg(feature = "doc-only")] map_msgs__srv__SaveMap_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    filename: std_msgs::msg::String::from_native(&msg.filename),
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
                self.filename.try_copy_to_native(&mut msg.filename)?;
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
            type CStruct = map_msgs__srv__SaveMap_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__SaveMap_Response()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__SaveMap_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__SaveMap_Response__create() }
                #[cfg(feature = "doc-only")] map_msgs__srv__SaveMap_Response__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__SaveMap_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__SaveMap_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__SaveMap_Response__destroy(msg)
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
            pub request: Vec<map_msgs::srv::SaveMap::Request>,
            pub response: Vec<map_msgs::srv::SaveMap::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = map_msgs__srv__SaveMap_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__SaveMap_Event()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__SaveMap_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__SaveMap_Event__create() }
                #[cfg(feature = "doc-only")] map_msgs__srv__SaveMap_Event__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__SaveMap_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__SaveMap_Event__destroy(msg) };
                #[cfg(feature = "doc-only")] map_msgs__srv__SaveMap_Event__destroy(msg)
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
                                temp.push(map_msgs::srv::SaveMap::Request::from_native(s));
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
                                temp.push(map_msgs::srv::SaveMap::Response::from_native(s));
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
                        if !map_msgs__srv__SaveMap_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__srv__SaveMap_Request__Sequence__fini(&mut msg.request);
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
                        if !map_msgs__srv__SaveMap_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__srv__SaveMap_Response__Sequence__fini(
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
    pub mod SetMapProjections {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__map_msgs__srv__SetMapProjections()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {}
        unsafe impl WrappedTypesupport for Request {
            type CStruct = map_msgs__srv__SetMapProjections_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__SetMapProjections_Request()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__SetMapProjections_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__SetMapProjections_Request__create() }
                #[cfg(feature = "doc-only")]
                map_msgs__srv__SetMapProjections_Request__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__SetMapProjections_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__SetMapProjections_Request__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__SetMapProjections_Request__destroy(msg)
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
            pub projected_maps_info: Vec<map_msgs::msg::ProjectedMapInfo>,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = map_msgs__srv__SetMapProjections_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__SetMapProjections_Response()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__SetMapProjections_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__SetMapProjections_Response__create() }
                #[cfg(feature = "doc-only")]
                map_msgs__srv__SetMapProjections_Response__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__SetMapProjections_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__SetMapProjections_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__SetMapProjections_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    projected_maps_info: {
                        let mut temp = Vec::with_capacity(msg.projected_maps_info.size);
                        if msg.projected_maps_info.data != std::ptr::null_mut() {
                            let slice = unsafe {
                                std::slice::from_raw_parts(
                                    msg.projected_maps_info.data,
                                    msg.projected_maps_info.size,
                                )
                            };
                            for s in slice {
                                temp.push(map_msgs::msg::ProjectedMapInfo::from_native(s));
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
                    let length = self.projected_maps_info.len();
                    if length > msg.projected_maps_info.capacity {
                        let mut replacement = std::mem::zeroed();
                        if !map_msgs__msg__ProjectedMapInfo__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__msg__ProjectedMapInfo__Sequence__fini(
                            &mut msg.projected_maps_info,
                        );
                        msg.projected_maps_info = replacement;
                    }
                    msg.projected_maps_info.size = length;
                    if length != 0 {
                        let slice = std::slice::from_raw_parts_mut(
                            msg.projected_maps_info.data,
                            length,
                        );
                        for (t, s) in slice.iter_mut().zip(&self.projected_maps_info) {
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
            pub request: Vec<map_msgs::srv::SetMapProjections::Request>,
            pub response: Vec<map_msgs::srv::SetMapProjections::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = map_msgs__srv__SetMapProjections_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__map_msgs__srv__SetMapProjections_Event()
                }
            }
            fn create_msg() -> *mut map_msgs__srv__SetMapProjections_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__SetMapProjections_Event__create() }
                #[cfg(feature = "doc-only")]
                map_msgs__srv__SetMapProjections_Event__create()
            }
            fn destroy_msg(msg: *mut map_msgs__srv__SetMapProjections_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { map_msgs__srv__SetMapProjections_Event__destroy(msg) };
                #[cfg(feature = "doc-only")]
                map_msgs__srv__SetMapProjections_Event__destroy(msg)
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
                                    map_msgs::srv::SetMapProjections::Request::from_native(s),
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
                                    map_msgs::srv::SetMapProjections::Response::from_native(s),
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
                        if !map_msgs__srv__SetMapProjections_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__srv__SetMapProjections_Request__Sequence__fini(
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
                        if !map_msgs__srv__SetMapProjections_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        map_msgs__srv__SetMapProjections_Response__Sequence__fini(
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
