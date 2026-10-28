pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Mesh {
        pub triangles: Vec<shape_msgs::msg::MeshTriangle>,
        pub vertices: Vec<geometry_msgs::msg::Point>,
    }
    unsafe impl WrappedTypesupport for Mesh {
        type CStruct = shape_msgs__msg__Mesh;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__shape_msgs__msg__Mesh()
            }
        }
        fn create_msg() -> *mut shape_msgs__msg__Mesh {
            #[cfg(not(feature = "doc-only"))] unsafe { shape_msgs__msg__Mesh__create() }
            #[cfg(feature = "doc-only")] shape_msgs__msg__Mesh__create()
        }
        fn destroy_msg(msg: *mut shape_msgs__msg__Mesh) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { shape_msgs__msg__Mesh__destroy(msg) };
            #[cfg(feature = "doc-only")] shape_msgs__msg__Mesh__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Mesh {
            Mesh {
                triangles: {
                    let mut temp = Vec::with_capacity(msg.triangles.size);
                    if msg.triangles.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.triangles.data,
                                msg.triangles.size,
                            )
                        };
                        for s in slice {
                            temp.push(shape_msgs::msg::MeshTriangle::from_native(s));
                        }
                    }
                    temp
                },
                vertices: {
                    let mut temp = Vec::with_capacity(msg.vertices.size);
                    if msg.vertices.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.vertices.data,
                                msg.vertices.size,
                            )
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
            unsafe {
                let length = self.triangles.len();
                if length > msg.triangles.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !shape_msgs__msg__MeshTriangle__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    shape_msgs__msg__MeshTriangle__Sequence__fini(&mut msg.triangles);
                    msg.triangles = replacement;
                }
                msg.triangles.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.triangles.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.triangles) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            unsafe {
                let length = self.vertices.len();
                if length > msg.vertices.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !geometry_msgs__msg__Point__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    geometry_msgs__msg__Point__Sequence__fini(&mut msg.vertices);
                    msg.vertices = replacement;
                }
                msg.vertices.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.vertices.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.vertices) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for Mesh {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Mesh>::new();
            Mesh::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct MeshTriangle {
        #[serde(with = "crate::msg_types::fixed_array")]
        pub vertex_indices: [u32; 3usize],
    }
    unsafe impl WrappedTypesupport for MeshTriangle {
        type CStruct = shape_msgs__msg__MeshTriangle;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__shape_msgs__msg__MeshTriangle()
            }
        }
        fn create_msg() -> *mut shape_msgs__msg__MeshTriangle {
            #[cfg(not(feature = "doc-only"))]
            unsafe { shape_msgs__msg__MeshTriangle__create() }
            #[cfg(feature = "doc-only")] shape_msgs__msg__MeshTriangle__create()
        }
        fn destroy_msg(msg: *mut shape_msgs__msg__MeshTriangle) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { shape_msgs__msg__MeshTriangle__destroy(msg) };
            #[cfg(feature = "doc-only")] shape_msgs__msg__MeshTriangle__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> MeshTriangle {
            MeshTriangle {
                vertex_indices: msg.vertex_indices,
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
            msg.vertex_indices.copy_from_slice(&self.vertex_indices[..3usize]);
            Ok(())
        }
    }
    impl Default for MeshTriangle {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<MeshTriangle>::new();
            MeshTriangle::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct Plane {
        #[serde(with = "crate::msg_types::fixed_array")]
        pub coef: [f64; 4usize],
    }
    unsafe impl WrappedTypesupport for Plane {
        type CStruct = shape_msgs__msg__Plane;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__shape_msgs__msg__Plane()
            }
        }
        fn create_msg() -> *mut shape_msgs__msg__Plane {
            #[cfg(not(feature = "doc-only"))] unsafe { shape_msgs__msg__Plane__create() }
            #[cfg(feature = "doc-only")] shape_msgs__msg__Plane__create()
        }
        fn destroy_msg(msg: *mut shape_msgs__msg__Plane) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { shape_msgs__msg__Plane__destroy(msg) };
            #[cfg(feature = "doc-only")] shape_msgs__msg__Plane__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> Plane {
            Plane { coef: msg.coef }
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
            msg.coef.copy_from_slice(&self.coef[..4usize]);
            Ok(())
        }
    }
    impl Default for Plane {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<Plane>::new();
            Plane::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct SolidPrimitive {
        #[serde(rename = "type")]
        pub type_: u8,
        pub dimensions: Vec<f64>,
        pub polygon: geometry_msgs::msg::Polygon,
    }
    unsafe impl WrappedTypesupport for SolidPrimitive {
        type CStruct = shape_msgs__msg__SolidPrimitive;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__shape_msgs__msg__SolidPrimitive()
            }
        }
        fn create_msg() -> *mut shape_msgs__msg__SolidPrimitive {
            #[cfg(not(feature = "doc-only"))]
            unsafe { shape_msgs__msg__SolidPrimitive__create() }
            #[cfg(feature = "doc-only")] shape_msgs__msg__SolidPrimitive__create()
        }
        fn destroy_msg(msg: *mut shape_msgs__msg__SolidPrimitive) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { shape_msgs__msg__SolidPrimitive__destroy(msg) };
            #[cfg(feature = "doc-only")] shape_msgs__msg__SolidPrimitive__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> SolidPrimitive {
            SolidPrimitive {
                type_: msg.type_,
                dimensions: msg.dimensions.to_vec(),
                polygon: geometry_msgs::msg::Polygon::from_native(&msg.polygon),
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
            if self.dimensions.len() > 3usize {
                return Err(Error::RCL_RET_INVALID_ARGUMENT);
            }
            check_rosidl_runtime_ret(msg.dimensions.try_update(&self.dimensions))?;
            self.polygon.try_copy_to_native(&mut msg.polygon)?;
            Ok(())
        }
    }
    impl Default for SolidPrimitive {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<SolidPrimitive>::new();
            SolidPrimitive::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl SolidPrimitive {
        pub const BOX: _bindgen_ty_314 = shape_msgs__msg__SolidPrimitive__BOX;
        pub const BOX_X: _bindgen_ty_319 = shape_msgs__msg__SolidPrimitive__BOX_X;
        pub const BOX_Y: _bindgen_ty_320 = shape_msgs__msg__SolidPrimitive__BOX_Y;
        pub const BOX_Z: _bindgen_ty_321 = shape_msgs__msg__SolidPrimitive__BOX_Z;
        pub const CONE: _bindgen_ty_317 = shape_msgs__msg__SolidPrimitive__CONE;
        pub const CONE_HEIGHT: _bindgen_ty_325 = shape_msgs__msg__SolidPrimitive__CONE_HEIGHT;
        pub const CONE_RADIUS: _bindgen_ty_326 = shape_msgs__msg__SolidPrimitive__CONE_RADIUS;
        pub const CYLINDER: _bindgen_ty_316 = shape_msgs__msg__SolidPrimitive__CYLINDER;
        pub const CYLINDER_HEIGHT: _bindgen_ty_323 = shape_msgs__msg__SolidPrimitive__CYLINDER_HEIGHT;
        pub const CYLINDER_RADIUS: _bindgen_ty_324 = shape_msgs__msg__SolidPrimitive__CYLINDER_RADIUS;
        pub const PRISM: _bindgen_ty_318 = shape_msgs__msg__SolidPrimitive__PRISM;
        pub const PRISM_HEIGHT: _bindgen_ty_327 = shape_msgs__msg__SolidPrimitive__PRISM_HEIGHT;
        pub const SPHERE: _bindgen_ty_315 = shape_msgs__msg__SolidPrimitive__SPHERE;
        pub const SPHERE_RADIUS: _bindgen_ty_322 = shape_msgs__msg__SolidPrimitive__SPHERE_RADIUS;
    }
}
