pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct JointTrajectory {
        pub header: std_msgs::msg::Header,
        pub joint_names: Vec<crate::RosString>,
        pub points: Vec<trajectory_msgs::msg::JointTrajectoryPoint>,
    }
    unsafe impl WrappedTypesupport for JointTrajectory {
        type CStruct = trajectory_msgs__msg__JointTrajectory;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__trajectory_msgs__msg__JointTrajectory()
            }
        }
        fn create_msg() -> *mut trajectory_msgs__msg__JointTrajectory {
            #[cfg(not(feature = "doc-only"))]
            unsafe { trajectory_msgs__msg__JointTrajectory__create() }
            #[cfg(feature = "doc-only")] trajectory_msgs__msg__JointTrajectory__create()
        }
        fn destroy_msg(msg: *mut trajectory_msgs__msg__JointTrajectory) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { trajectory_msgs__msg__JointTrajectory__destroy(msg) };
            #[cfg(feature = "doc-only")]
            trajectory_msgs__msg__JointTrajectory__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> JointTrajectory {
            JointTrajectory {
                header: std_msgs::msg::Header::from_native(&msg.header),
                joint_names: msg.joint_names.to_bytes_vec::<crate::RosString>(),
                points: {
                    let mut temp = Vec::with_capacity(msg.points.size);
                    if msg.points.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(msg.points.data, msg.points.size)
                        };
                        for s in slice {
                            temp.push(
                                trajectory_msgs::msg::JointTrajectoryPoint::from_native(s),
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
            check_rosidl_runtime_ret(
                msg.joint_names.try_update_bytes(&self.joint_names),
            )?;
            unsafe {
                let length = self.points.len();
                if length > msg.points.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !trajectory_msgs__msg__JointTrajectoryPoint__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    trajectory_msgs__msg__JointTrajectoryPoint__Sequence__fini(
                        &mut msg.points,
                    );
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
    impl Default for JointTrajectory {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<JointTrajectory>::new();
            JointTrajectory::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct JointTrajectoryPoint {
        pub positions: Vec<f64>,
        pub velocities: Vec<f64>,
        pub accelerations: Vec<f64>,
        pub effort: Vec<f64>,
        pub time_from_start: builtin_interfaces::msg::Duration,
    }
    unsafe impl WrappedTypesupport for JointTrajectoryPoint {
        type CStruct = trajectory_msgs__msg__JointTrajectoryPoint;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__trajectory_msgs__msg__JointTrajectoryPoint()
            }
        }
        fn create_msg() -> *mut trajectory_msgs__msg__JointTrajectoryPoint {
            #[cfg(not(feature = "doc-only"))]
            unsafe { trajectory_msgs__msg__JointTrajectoryPoint__create() }
            #[cfg(feature = "doc-only")]
            trajectory_msgs__msg__JointTrajectoryPoint__create()
        }
        fn destroy_msg(msg: *mut trajectory_msgs__msg__JointTrajectoryPoint) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { trajectory_msgs__msg__JointTrajectoryPoint__destroy(msg) };
            #[cfg(feature = "doc-only")]
            trajectory_msgs__msg__JointTrajectoryPoint__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> JointTrajectoryPoint {
            JointTrajectoryPoint {
                positions: msg.positions.to_vec(),
                velocities: msg.velocities.to_vec(),
                accelerations: msg.accelerations.to_vec(),
                effort: msg.effort.to_vec(),
                time_from_start: builtin_interfaces::msg::Duration::from_native(
                    &msg.time_from_start,
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
            check_rosidl_runtime_ret(msg.positions.try_update(&self.positions))?;
            check_rosidl_runtime_ret(msg.velocities.try_update(&self.velocities))?;
            check_rosidl_runtime_ret(msg.accelerations.try_update(&self.accelerations))?;
            check_rosidl_runtime_ret(msg.effort.try_update(&self.effort))?;
            self.time_from_start.try_copy_to_native(&mut msg.time_from_start)?;
            Ok(())
        }
    }
    impl Default for JointTrajectoryPoint {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<JointTrajectoryPoint>::new();
            JointTrajectoryPoint::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct MultiDOFJointTrajectory {
        pub header: std_msgs::msg::Header,
        pub joint_names: Vec<crate::RosString>,
        pub points: Vec<trajectory_msgs::msg::MultiDOFJointTrajectoryPoint>,
    }
    unsafe impl WrappedTypesupport for MultiDOFJointTrajectory {
        type CStruct = trajectory_msgs__msg__MultiDOFJointTrajectory;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__trajectory_msgs__msg__MultiDOFJointTrajectory()
            }
        }
        fn create_msg() -> *mut trajectory_msgs__msg__MultiDOFJointTrajectory {
            #[cfg(not(feature = "doc-only"))]
            unsafe { trajectory_msgs__msg__MultiDOFJointTrajectory__create() }
            #[cfg(feature = "doc-only")]
            trajectory_msgs__msg__MultiDOFJointTrajectory__create()
        }
        fn destroy_msg(msg: *mut trajectory_msgs__msg__MultiDOFJointTrajectory) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { trajectory_msgs__msg__MultiDOFJointTrajectory__destroy(msg) };
            #[cfg(feature = "doc-only")]
            trajectory_msgs__msg__MultiDOFJointTrajectory__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> MultiDOFJointTrajectory {
            MultiDOFJointTrajectory {
                header: std_msgs::msg::Header::from_native(&msg.header),
                joint_names: msg.joint_names.to_bytes_vec::<crate::RosString>(),
                points: {
                    let mut temp = Vec::with_capacity(msg.points.size);
                    if msg.points.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(msg.points.data, msg.points.size)
                        };
                        for s in slice {
                            temp.push(
                                trajectory_msgs::msg::MultiDOFJointTrajectoryPoint::from_native(
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
            self.header.try_copy_to_native(&mut msg.header)?;
            check_rosidl_runtime_ret(
                msg.joint_names.try_update_bytes(&self.joint_names),
            )?;
            unsafe {
                let length = self.points.len();
                if length > msg.points.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !trajectory_msgs__msg__MultiDOFJointTrajectoryPoint__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    trajectory_msgs__msg__MultiDOFJointTrajectoryPoint__Sequence__fini(
                        &mut msg.points,
                    );
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
    impl Default for MultiDOFJointTrajectory {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<MultiDOFJointTrajectory>::new();
            MultiDOFJointTrajectory::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct MultiDOFJointTrajectoryPoint {
        pub transforms: Vec<geometry_msgs::msg::Transform>,
        pub velocities: Vec<geometry_msgs::msg::Twist>,
        pub accelerations: Vec<geometry_msgs::msg::Twist>,
        pub time_from_start: builtin_interfaces::msg::Duration,
    }
    unsafe impl WrappedTypesupport for MultiDOFJointTrajectoryPoint {
        type CStruct = trajectory_msgs__msg__MultiDOFJointTrajectoryPoint;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__trajectory_msgs__msg__MultiDOFJointTrajectoryPoint()
            }
        }
        fn create_msg() -> *mut trajectory_msgs__msg__MultiDOFJointTrajectoryPoint {
            #[cfg(not(feature = "doc-only"))]
            unsafe { trajectory_msgs__msg__MultiDOFJointTrajectoryPoint__create() }
            #[cfg(feature = "doc-only")]
            trajectory_msgs__msg__MultiDOFJointTrajectoryPoint__create()
        }
        fn destroy_msg(
            msg: *mut trajectory_msgs__msg__MultiDOFJointTrajectoryPoint,
        ) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { trajectory_msgs__msg__MultiDOFJointTrajectoryPoint__destroy(msg) };
            #[cfg(feature = "doc-only")]
            trajectory_msgs__msg__MultiDOFJointTrajectoryPoint__destroy(msg)
        }
        fn from_native(
            #[allow(unused)]
            msg: &Self::CStruct,
        ) -> MultiDOFJointTrajectoryPoint {
            MultiDOFJointTrajectoryPoint {
                transforms: {
                    let mut temp = Vec::with_capacity(msg.transforms.size);
                    if msg.transforms.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.transforms.data,
                                msg.transforms.size,
                            )
                        };
                        for s in slice {
                            temp.push(geometry_msgs::msg::Transform::from_native(s));
                        }
                    }
                    temp
                },
                velocities: {
                    let mut temp = Vec::with_capacity(msg.velocities.size);
                    if msg.velocities.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.velocities.data,
                                msg.velocities.size,
                            )
                        };
                        for s in slice {
                            temp.push(geometry_msgs::msg::Twist::from_native(s));
                        }
                    }
                    temp
                },
                accelerations: {
                    let mut temp = Vec::with_capacity(msg.accelerations.size);
                    if msg.accelerations.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.accelerations.data,
                                msg.accelerations.size,
                            )
                        };
                        for s in slice {
                            temp.push(geometry_msgs::msg::Twist::from_native(s));
                        }
                    }
                    temp
                },
                time_from_start: builtin_interfaces::msg::Duration::from_native(
                    &msg.time_from_start,
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
            unsafe {
                let length = self.transforms.len();
                if length > msg.transforms.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !geometry_msgs__msg__Transform__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    geometry_msgs__msg__Transform__Sequence__fini(&mut msg.transforms);
                    msg.transforms = replacement;
                }
                msg.transforms.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.transforms.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.transforms) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.velocities.len();
                if length > msg.velocities.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !geometry_msgs__msg__Twist__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    geometry_msgs__msg__Twist__Sequence__fini(&mut msg.velocities);
                    msg.velocities = replacement;
                }
                msg.velocities.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.velocities.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.velocities) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.accelerations.len();
                if length > msg.accelerations.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !geometry_msgs__msg__Twist__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    geometry_msgs__msg__Twist__Sequence__fini(&mut msg.accelerations);
                    msg.accelerations = replacement;
                }
                msg.accelerations.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.accelerations.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.accelerations) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            self.time_from_start.try_copy_to_native(&mut msg.time_from_start)?;
            Ok(())
        }
    }
    impl Default for MultiDOFJointTrajectoryPoint {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<MultiDOFJointTrajectoryPoint>::new();
            MultiDOFJointTrajectoryPoint::from_native(&msg_native)
        }
    }
}
