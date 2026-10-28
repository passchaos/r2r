pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Goals {
        pub header: std_msgs::msg::Header,
        pub goals: Vec<geometry_msgs::msg::PoseStamped>,
    }
    unsafe impl WrappedTypesupport for Goals {
        type CStruct = nav_msgs__msg__Goals;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__msg__Goals()
            }
        }
        fn create_msg() -> *mut nav_msgs__msg__Goals {
            #[cfg(not(feature = "doc-only"))] unsafe { nav_msgs__msg__Goals__create() }
            #[cfg(feature = "doc-only")] nav_msgs__msg__Goals__create()
        }
        fn destroy_msg(msg: *mut nav_msgs__msg__Goals) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__Goals__destroy(msg) };
            #[cfg(feature = "doc-only")] nav_msgs__msg__Goals__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Goals {
            Goals {
                header: std_msgs::msg::Header::from_native(&msg.header),
                goals: {
                    let mut temp = Vec::with_capacity(msg.goals.size);
                    if msg.goals.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(msg.goals.data, msg.goals.size)
                        };
                        for s in slice {
                            temp.push(geometry_msgs::msg::PoseStamped::from_native(s));
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
                let length = self.goals.len();
                if length > msg.goals.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !geometry_msgs__msg__PoseStamped__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    geometry_msgs__msg__PoseStamped__Sequence__fini(&mut msg.goals);
                    msg.goals = replacement;
                }
                msg.goals.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(msg.goals.data, length);
                    for (t, s) in slice.iter_mut().zip(&self.goals) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for Goals {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Goals>::new();
            Goals::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct GridCells {
        pub header: std_msgs::msg::Header,
        pub cell_width: f32,
        pub cell_height: f32,
        pub cells: Vec<geometry_msgs::msg::Point>,
    }
    unsafe impl WrappedTypesupport for GridCells {
        type CStruct = nav_msgs__msg__GridCells;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__msg__GridCells()
            }
        }
        fn create_msg() -> *mut nav_msgs__msg__GridCells {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__GridCells__create() }
            #[cfg(feature = "doc-only")] nav_msgs__msg__GridCells__create()
        }
        fn destroy_msg(msg: *mut nav_msgs__msg__GridCells) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__GridCells__destroy(msg) };
            #[cfg(feature = "doc-only")] nav_msgs__msg__GridCells__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> GridCells {
            GridCells {
                header: std_msgs::msg::Header::from_native(&msg.header),
                cell_width: msg.cell_width,
                cell_height: msg.cell_height,
                cells: {
                    let mut temp = Vec::with_capacity(msg.cells.size);
                    if msg.cells.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(msg.cells.data, msg.cells.size)
                        };
                        for s in slice {
                            temp.push(geometry_msgs::msg::Point::from_native(s));
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
            msg.cell_width = self.cell_width;
            msg.cell_height = self.cell_height;
            unsafe {
                let length = self.cells.len();
                if length > msg.cells.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !geometry_msgs__msg__Point__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    geometry_msgs__msg__Point__Sequence__fini(&mut msg.cells);
                    msg.cells = replacement;
                }
                msg.cells.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(msg.cells.data, length);
                    for (t, s) in slice.iter_mut().zip(&self.cells) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for GridCells {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<GridCells>::new();
            GridCells::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct MapMetaData {
        pub map_load_time: builtin_interfaces::msg::Time,
        pub resolution: f32,
        pub width: u32,
        pub height: u32,
        pub origin: geometry_msgs::msg::Pose,
    }
    unsafe impl WrappedTypesupport for MapMetaData {
        type CStruct = nav_msgs__msg__MapMetaData;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__msg__MapMetaData()
            }
        }
        fn create_msg() -> *mut nav_msgs__msg__MapMetaData {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__MapMetaData__create() }
            #[cfg(feature = "doc-only")] nav_msgs__msg__MapMetaData__create()
        }
        fn destroy_msg(msg: *mut nav_msgs__msg__MapMetaData) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__MapMetaData__destroy(msg) };
            #[cfg(feature = "doc-only")] nav_msgs__msg__MapMetaData__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> MapMetaData {
            MapMetaData {
                map_load_time: builtin_interfaces::msg::Time::from_native(
                    &msg.map_load_time,
                ),
                resolution: msg.resolution,
                width: msg.width,
                height: msg.height,
                origin: geometry_msgs::msg::Pose::from_native(&msg.origin),
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
            self.map_load_time.try_copy_to_native(&mut msg.map_load_time)?;
            msg.resolution = self.resolution;
            msg.width = self.width;
            msg.height = self.height;
            self.origin.try_copy_to_native(&mut msg.origin)?;
            Ok(())
        }
    }
    impl Default for MapMetaData {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<MapMetaData>::new();
            MapMetaData::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct OccupancyGrid {
        pub header: std_msgs::msg::Header,
        pub info: nav_msgs::msg::MapMetaData,
        pub data: Vec<i8>,
    }
    unsafe impl WrappedTypesupport for OccupancyGrid {
        type CStruct = nav_msgs__msg__OccupancyGrid;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__msg__OccupancyGrid()
            }
        }
        fn create_msg() -> *mut nav_msgs__msg__OccupancyGrid {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__OccupancyGrid__create() }
            #[cfg(feature = "doc-only")] nav_msgs__msg__OccupancyGrid__create()
        }
        fn destroy_msg(msg: *mut nav_msgs__msg__OccupancyGrid) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__OccupancyGrid__destroy(msg) };
            #[cfg(feature = "doc-only")] nav_msgs__msg__OccupancyGrid__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> OccupancyGrid {
            OccupancyGrid {
                header: std_msgs::msg::Header::from_native(&msg.header),
                info: nav_msgs::msg::MapMetaData::from_native(&msg.info),
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
            self.info.try_copy_to_native(&mut msg.info)?;
            check_rosidl_runtime_ret(msg.data.try_update(&self.data))?;
            Ok(())
        }
    }
    impl Default for OccupancyGrid {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<OccupancyGrid>::new();
            OccupancyGrid::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Odometry {
        pub header: std_msgs::msg::Header,
        pub child_frame_id: crate::RosString,
        pub pose: geometry_msgs::msg::PoseWithCovariance,
        pub twist: geometry_msgs::msg::TwistWithCovariance,
    }
    unsafe impl WrappedTypesupport for Odometry {
        type CStruct = nav_msgs__msg__Odometry;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__msg__Odometry()
            }
        }
        fn create_msg() -> *mut nav_msgs__msg__Odometry {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__Odometry__create() }
            #[cfg(feature = "doc-only")] nav_msgs__msg__Odometry__create()
        }
        fn destroy_msg(msg: *mut nav_msgs__msg__Odometry) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__Odometry__destroy(msg) };
            #[cfg(feature = "doc-only")] nav_msgs__msg__Odometry__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Odometry {
            Odometry {
                header: std_msgs::msg::Header::from_native(&msg.header),
                child_frame_id: crate::RosString::from(msg.child_frame_id.as_bytes()),
                pose: geometry_msgs::msg::PoseWithCovariance::from_native(&msg.pose),
                twist: geometry_msgs::msg::TwistWithCovariance::from_native(&msg.twist),
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
            check_rosidl_runtime_ret(
                msg.child_frame_id.try_assign_bytes(self.child_frame_id.as_bytes()),
            )?;
            self.pose.try_copy_to_native(&mut msg.pose)?;
            self.twist.try_copy_to_native(&mut msg.twist)?;
            Ok(())
        }
    }
    impl Default for Odometry {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Odometry>::new();
            Odometry::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Path {
        pub header: std_msgs::msg::Header,
        pub poses: Vec<geometry_msgs::msg::PoseStamped>,
    }
    unsafe impl WrappedTypesupport for Path {
        type CStruct = nav_msgs__msg__Path;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__msg__Path()
            }
        }
        fn create_msg() -> *mut nav_msgs__msg__Path {
            #[cfg(not(feature = "doc-only"))] unsafe { nav_msgs__msg__Path__create() }
            #[cfg(feature = "doc-only")] nav_msgs__msg__Path__create()
        }
        fn destroy_msg(msg: *mut nav_msgs__msg__Path) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__Path__destroy(msg) };
            #[cfg(feature = "doc-only")] nav_msgs__msg__Path__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Path {
            Path {
                header: std_msgs::msg::Header::from_native(&msg.header),
                poses: {
                    let mut temp = Vec::with_capacity(msg.poses.size);
                    if msg.poses.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(msg.poses.data, msg.poses.size)
                        };
                        for s in slice {
                            temp.push(geometry_msgs::msg::PoseStamped::from_native(s));
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
                let length = self.poses.len();
                if length > msg.poses.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !geometry_msgs__msg__PoseStamped__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    geometry_msgs__msg__PoseStamped__Sequence__fini(&mut msg.poses);
                    msg.poses = replacement;
                }
                msg.poses.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(msg.poses.data, length);
                    for (t, s) in slice.iter_mut().zip(&self.poses) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for Path {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Path>::new();
            Path::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Trajectory {
        pub header: std_msgs::msg::Header,
        pub points: Vec<nav_msgs::msg::TrajectoryPoint>,
    }
    unsafe impl WrappedTypesupport for Trajectory {
        type CStruct = nav_msgs__msg__Trajectory;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__msg__Trajectory()
            }
        }
        fn create_msg() -> *mut nav_msgs__msg__Trajectory {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__Trajectory__create() }
            #[cfg(feature = "doc-only")] nav_msgs__msg__Trajectory__create()
        }
        fn destroy_msg(msg: *mut nav_msgs__msg__Trajectory) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__Trajectory__destroy(msg) };
            #[cfg(feature = "doc-only")] nav_msgs__msg__Trajectory__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Trajectory {
            Trajectory {
                header: std_msgs::msg::Header::from_native(&msg.header),
                points: {
                    let mut temp = Vec::with_capacity(msg.points.size);
                    if msg.points.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(msg.points.data, msg.points.size)
                        };
                        for s in slice {
                            temp.push(nav_msgs::msg::TrajectoryPoint::from_native(s));
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
                let length = self.points.len();
                if length > msg.points.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !nav_msgs__msg__TrajectoryPoint__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    nav_msgs__msg__TrajectoryPoint__Sequence__fini(&mut msg.points);
                    msg.points = replacement;
                }
                msg.points.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(msg.points.data, length);
                    for (t, s) in slice.iter_mut().zip(&self.points) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for Trajectory {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Trajectory>::new();
            Trajectory::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct TrajectoryPoint {
        pub header: std_msgs::msg::Header,
        pub pose: geometry_msgs::msg::Pose,
        pub velocity: geometry_msgs::msg::Twist,
        pub acceleration: geometry_msgs::msg::Accel,
        pub effort: geometry_msgs::msg::Wrench,
    }
    unsafe impl WrappedTypesupport for TrajectoryPoint {
        type CStruct = nav_msgs__msg__TrajectoryPoint;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__msg__TrajectoryPoint()
            }
        }
        fn create_msg() -> *mut nav_msgs__msg__TrajectoryPoint {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__TrajectoryPoint__create() }
            #[cfg(feature = "doc-only")] nav_msgs__msg__TrajectoryPoint__create()
        }
        fn destroy_msg(msg: *mut nav_msgs__msg__TrajectoryPoint) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { nav_msgs__msg__TrajectoryPoint__destroy(msg) };
            #[cfg(feature = "doc-only")] nav_msgs__msg__TrajectoryPoint__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> TrajectoryPoint {
            TrajectoryPoint {
                header: std_msgs::msg::Header::from_native(&msg.header),
                pose: geometry_msgs::msg::Pose::from_native(&msg.pose),
                velocity: geometry_msgs::msg::Twist::from_native(&msg.velocity),
                acceleration: geometry_msgs::msg::Accel::from_native(&msg.acceleration),
                effort: geometry_msgs::msg::Wrench::from_native(&msg.effort),
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
            self.pose.try_copy_to_native(&mut msg.pose)?;
            self.velocity.try_copy_to_native(&mut msg.velocity)?;
            self.acceleration.try_copy_to_native(&mut msg.acceleration)?;
            self.effort.try_copy_to_native(&mut msg.effort)?;
            Ok(())
        }
    }
    impl Default for TrajectoryPoint {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<TrajectoryPoint>::new();
            TrajectoryPoint::from_native(&msg_native)
        }
    }
}
pub mod srv {
    #[allow(non_snake_case)]
    pub mod GetMap {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__nav_msgs__srv__GetMap()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {}
        unsafe impl WrappedTypesupport for Request {
            type CStruct = nav_msgs__srv__GetMap_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__srv__GetMap_Request()
                }
            }
            fn create_msg() -> *mut nav_msgs__srv__GetMap_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__GetMap_Request__create() }
                #[cfg(feature = "doc-only")] nav_msgs__srv__GetMap_Request__create()
            }
            fn destroy_msg(msg: *mut nav_msgs__srv__GetMap_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__GetMap_Request__destroy(msg) };
                #[cfg(feature = "doc-only")] nav_msgs__srv__GetMap_Request__destroy(msg)
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
            pub map: nav_msgs::msg::OccupancyGrid,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = nav_msgs__srv__GetMap_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__srv__GetMap_Response()
                }
            }
            fn create_msg() -> *mut nav_msgs__srv__GetMap_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__GetMap_Response__create() }
                #[cfg(feature = "doc-only")] nav_msgs__srv__GetMap_Response__create()
            }
            fn destroy_msg(msg: *mut nav_msgs__srv__GetMap_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__GetMap_Response__destroy(msg) };
                #[cfg(feature = "doc-only")] nav_msgs__srv__GetMap_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    map: nav_msgs::msg::OccupancyGrid::from_native(&msg.map),
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
            pub request: Vec<nav_msgs::srv::GetMap::Request>,
            pub response: Vec<nav_msgs::srv::GetMap::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = nav_msgs__srv__GetMap_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__srv__GetMap_Event()
                }
            }
            fn create_msg() -> *mut nav_msgs__srv__GetMap_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__GetMap_Event__create() }
                #[cfg(feature = "doc-only")] nav_msgs__srv__GetMap_Event__create()
            }
            fn destroy_msg(msg: *mut nav_msgs__srv__GetMap_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__GetMap_Event__destroy(msg) };
                #[cfg(feature = "doc-only")] nav_msgs__srv__GetMap_Event__destroy(msg)
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
                                temp.push(nav_msgs::srv::GetMap::Request::from_native(s));
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
                                temp.push(nav_msgs::srv::GetMap::Response::from_native(s));
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
                        if !nav_msgs__srv__GetMap_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        nav_msgs__srv__GetMap_Request__Sequence__fini(&mut msg.request);
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
                        if !nav_msgs__srv__GetMap_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        nav_msgs__srv__GetMap_Response__Sequence__fini(
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
    pub mod GetPlan {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__nav_msgs__srv__GetPlan()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub start: geometry_msgs::msg::PoseStamped,
            pub goal: geometry_msgs::msg::PoseStamped,
            pub tolerance: f32,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = nav_msgs__srv__GetPlan_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__srv__GetPlan_Request()
                }
            }
            fn create_msg() -> *mut nav_msgs__srv__GetPlan_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__GetPlan_Request__create() }
                #[cfg(feature = "doc-only")] nav_msgs__srv__GetPlan_Request__create()
            }
            fn destroy_msg(msg: *mut nav_msgs__srv__GetPlan_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__GetPlan_Request__destroy(msg) };
                #[cfg(feature = "doc-only")] nav_msgs__srv__GetPlan_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    start: geometry_msgs::msg::PoseStamped::from_native(&msg.start),
                    goal: geometry_msgs::msg::PoseStamped::from_native(&msg.goal),
                    tolerance: msg.tolerance,
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
                self.start.try_copy_to_native(&mut msg.start)?;
                self.goal.try_copy_to_native(&mut msg.goal)?;
                msg.tolerance = self.tolerance;
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
            pub plan: nav_msgs::msg::Path,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = nav_msgs__srv__GetPlan_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__srv__GetPlan_Response()
                }
            }
            fn create_msg() -> *mut nav_msgs__srv__GetPlan_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__GetPlan_Response__create() }
                #[cfg(feature = "doc-only")] nav_msgs__srv__GetPlan_Response__create()
            }
            fn destroy_msg(msg: *mut nav_msgs__srv__GetPlan_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__GetPlan_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                nav_msgs__srv__GetPlan_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    plan: nav_msgs::msg::Path::from_native(&msg.plan),
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
                self.plan.try_copy_to_native(&mut msg.plan)?;
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
            pub request: Vec<nav_msgs::srv::GetPlan::Request>,
            pub response: Vec<nav_msgs::srv::GetPlan::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = nav_msgs__srv__GetPlan_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__srv__GetPlan_Event()
                }
            }
            fn create_msg() -> *mut nav_msgs__srv__GetPlan_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__GetPlan_Event__create() }
                #[cfg(feature = "doc-only")] nav_msgs__srv__GetPlan_Event__create()
            }
            fn destroy_msg(msg: *mut nav_msgs__srv__GetPlan_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__GetPlan_Event__destroy(msg) };
                #[cfg(feature = "doc-only")] nav_msgs__srv__GetPlan_Event__destroy(msg)
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
                                temp.push(nav_msgs::srv::GetPlan::Request::from_native(s));
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
                                temp.push(nav_msgs::srv::GetPlan::Response::from_native(s));
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
                        if !nav_msgs__srv__GetPlan_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        nav_msgs__srv__GetPlan_Request__Sequence__fini(&mut msg.request);
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
                        if !nav_msgs__srv__GetPlan_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        nav_msgs__srv__GetPlan_Response__Sequence__fini(
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
    pub mod LoadMap {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__nav_msgs__srv__LoadMap()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub map_url: crate::RosString,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = nav_msgs__srv__LoadMap_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__srv__LoadMap_Request()
                }
            }
            fn create_msg() -> *mut nav_msgs__srv__LoadMap_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__LoadMap_Request__create() }
                #[cfg(feature = "doc-only")] nav_msgs__srv__LoadMap_Request__create()
            }
            fn destroy_msg(msg: *mut nav_msgs__srv__LoadMap_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__LoadMap_Request__destroy(msg) };
                #[cfg(feature = "doc-only")] nav_msgs__srv__LoadMap_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    map_url: crate::RosString::from(msg.map_url.as_bytes()),
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
                    msg.map_url.try_assign_bytes(self.map_url.as_bytes()),
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
            pub map: nav_msgs::msg::OccupancyGrid,
            pub result: u8,
        }
        unsafe impl WrappedTypesupport for Response {
            type CStruct = nav_msgs__srv__LoadMap_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__srv__LoadMap_Response()
                }
            }
            fn create_msg() -> *mut nav_msgs__srv__LoadMap_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__LoadMap_Response__create() }
                #[cfg(feature = "doc-only")] nav_msgs__srv__LoadMap_Response__create()
            }
            fn destroy_msg(msg: *mut nav_msgs__srv__LoadMap_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__LoadMap_Response__destroy(msg) };
                #[cfg(feature = "doc-only")]
                nav_msgs__srv__LoadMap_Response__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Response {
                Response {
                    map: nav_msgs::msg::OccupancyGrid::from_native(&msg.map),
                    result: msg.result,
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
                msg.result = self.result;
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
            pub const RESULT_INVALID_MAP_DATA: _bindgen_ty_204 = nav_msgs__srv__LoadMap_Response__RESULT_INVALID_MAP_DATA;
            pub const RESULT_INVALID_MAP_METADATA: _bindgen_ty_205 = nav_msgs__srv__LoadMap_Response__RESULT_INVALID_MAP_METADATA;
            pub const RESULT_MAP_DOES_NOT_EXIST: _bindgen_ty_203 = nav_msgs__srv__LoadMap_Response__RESULT_MAP_DOES_NOT_EXIST;
            pub const RESULT_SUCCESS: _bindgen_ty_202 = nav_msgs__srv__LoadMap_Response__RESULT_SUCCESS;
            pub const RESULT_UNDEFINED_FAILURE: _bindgen_ty_206 = nav_msgs__srv__LoadMap_Response__RESULT_UNDEFINED_FAILURE;
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Event {
            pub info: service_msgs::msg::ServiceEventInfo,
            pub request: Vec<nav_msgs::srv::LoadMap::Request>,
            pub response: Vec<nav_msgs::srv::LoadMap::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = nav_msgs__srv__LoadMap_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__srv__LoadMap_Event()
                }
            }
            fn create_msg() -> *mut nav_msgs__srv__LoadMap_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__LoadMap_Event__create() }
                #[cfg(feature = "doc-only")] nav_msgs__srv__LoadMap_Event__create()
            }
            fn destroy_msg(msg: *mut nav_msgs__srv__LoadMap_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__LoadMap_Event__destroy(msg) };
                #[cfg(feature = "doc-only")] nav_msgs__srv__LoadMap_Event__destroy(msg)
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
                                temp.push(nav_msgs::srv::LoadMap::Request::from_native(s));
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
                                temp.push(nav_msgs::srv::LoadMap::Response::from_native(s));
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
                        if !nav_msgs__srv__LoadMap_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        nav_msgs__srv__LoadMap_Request__Sequence__fini(&mut msg.request);
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
                        if !nav_msgs__srv__LoadMap_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        nav_msgs__srv__LoadMap_Response__Sequence__fini(
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
    pub mod SetMap {
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
                    &*rosidl_typesupport_c__get_service_type_support_handle__nav_msgs__srv__SetMap()
                }
            }
        }
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Request {
            pub map: nav_msgs::msg::OccupancyGrid,
            pub initial_pose: geometry_msgs::msg::PoseWithCovarianceStamped,
        }
        unsafe impl WrappedTypesupport for Request {
            type CStruct = nav_msgs__srv__SetMap_Request;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__srv__SetMap_Request()
                }
            }
            fn create_msg() -> *mut nav_msgs__srv__SetMap_Request {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__SetMap_Request__create() }
                #[cfg(feature = "doc-only")] nav_msgs__srv__SetMap_Request__create()
            }
            fn destroy_msg(msg: *mut nav_msgs__srv__SetMap_Request) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__SetMap_Request__destroy(msg) };
                #[cfg(feature = "doc-only")] nav_msgs__srv__SetMap_Request__destroy(msg)
            }
            fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Request {
                Request {
                    map: nav_msgs::msg::OccupancyGrid::from_native(&msg.map),
                    initial_pose: geometry_msgs::msg::PoseWithCovarianceStamped::from_native(
                        &msg.initial_pose,
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
                self.map.try_copy_to_native(&mut msg.map)?;
                self.initial_pose.try_copy_to_native(&mut msg.initial_pose)?;
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
            type CStruct = nav_msgs__srv__SetMap_Response;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__srv__SetMap_Response()
                }
            }
            fn create_msg() -> *mut nav_msgs__srv__SetMap_Response {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__SetMap_Response__create() }
                #[cfg(feature = "doc-only")] nav_msgs__srv__SetMap_Response__create()
            }
            fn destroy_msg(msg: *mut nav_msgs__srv__SetMap_Response) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__SetMap_Response__destroy(msg) };
                #[cfg(feature = "doc-only")] nav_msgs__srv__SetMap_Response__destroy(msg)
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
            pub request: Vec<nav_msgs::srv::SetMap::Request>,
            pub response: Vec<nav_msgs::srv::SetMap::Response>,
        }
        unsafe impl WrappedTypesupport for Event {
            type CStruct = nav_msgs__srv__SetMap_Event;
            fn get_ts() -> &'static rosidl_message_type_support_t {
                unsafe {
                    &*rosidl_typesupport_c__get_message_type_support_handle__nav_msgs__srv__SetMap_Event()
                }
            }
            fn create_msg() -> *mut nav_msgs__srv__SetMap_Event {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__SetMap_Event__create() }
                #[cfg(feature = "doc-only")] nav_msgs__srv__SetMap_Event__create()
            }
            fn destroy_msg(msg: *mut nav_msgs__srv__SetMap_Event) -> () {
                #[cfg(not(feature = "doc-only"))]
                unsafe { nav_msgs__srv__SetMap_Event__destroy(msg) };
                #[cfg(feature = "doc-only")] nav_msgs__srv__SetMap_Event__destroy(msg)
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
                                temp.push(nav_msgs::srv::SetMap::Request::from_native(s));
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
                                temp.push(nav_msgs::srv::SetMap::Response::from_native(s));
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
                        if !nav_msgs__srv__SetMap_Request__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        nav_msgs__srv__SetMap_Request__Sequence__fini(&mut msg.request);
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
                        if !nav_msgs__srv__SetMap_Response__Sequence__init(
                            &mut replacement,
                            length,
                        ) {
                            return Err(Error::RCL_RET_BAD_ALLOC);
                        }
                        nav_msgs__srv__SetMap_Response__Sequence__fini(
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
