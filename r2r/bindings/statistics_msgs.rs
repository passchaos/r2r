pub mod msg {
    use super::super::*;
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct MetricsMessage {
        pub measurement_source_name: crate::RosString,
        pub metrics_source: crate::RosString,
        pub unit: crate::RosString,
        pub window_start: builtin_interfaces::msg::Time,
        pub window_stop: builtin_interfaces::msg::Time,
        pub statistics: Vec<statistics_msgs::msg::StatisticDataPoint>,
    }
    unsafe impl WrappedTypesupport for MetricsMessage {
        type CStruct = statistics_msgs__msg__MetricsMessage;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__statistics_msgs__msg__MetricsMessage()
            }
        }
        fn create_msg() -> *mut statistics_msgs__msg__MetricsMessage {
            #[cfg(not(feature = "doc-only"))]
            unsafe { statistics_msgs__msg__MetricsMessage__create() }
            #[cfg(feature = "doc-only")] statistics_msgs__msg__MetricsMessage__create()
        }
        fn destroy_msg(msg: *mut statistics_msgs__msg__MetricsMessage) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { statistics_msgs__msg__MetricsMessage__destroy(msg) };
            #[cfg(feature = "doc-only")]
            statistics_msgs__msg__MetricsMessage__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> MetricsMessage {
            MetricsMessage {
                measurement_source_name: crate::RosString::from(
                    msg.measurement_source_name.as_bytes(),
                ),
                metrics_source: crate::RosString::from(msg.metrics_source.as_bytes()),
                unit: crate::RosString::from(msg.unit.as_bytes()),
                window_start: builtin_interfaces::msg::Time::from_native(
                    &msg.window_start,
                ),
                window_stop: builtin_interfaces::msg::Time::from_native(
                    &msg.window_stop,
                ),
                statistics: {
                    let mut temp = Vec::with_capacity(msg.statistics.size);
                    if msg.statistics.data != std::ptr::null_mut() {
                        let slice = unsafe {
                            std::slice::from_raw_parts(
                                msg.statistics.data,
                                msg.statistics.size,
                            )
                        };
                        for s in slice {
                            temp.push(
                                statistics_msgs::msg::StatisticDataPoint::from_native(s),
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
            check_rosidl_runtime_ret(
                msg
                    .measurement_source_name
                    .try_assign_bytes(self.measurement_source_name.as_bytes()),
            )?;
            check_rosidl_runtime_ret(
                msg.metrics_source.try_assign_bytes(self.metrics_source.as_bytes()),
            )?;
            check_rosidl_runtime_ret(msg.unit.try_assign_bytes(self.unit.as_bytes()))?;
            self.window_start.try_copy_to_native(&mut msg.window_start)?;
            self.window_stop.try_copy_to_native(&mut msg.window_stop)?;
            unsafe {
                let length = self.statistics.len();
                if length > msg.statistics.capacity {
                    let mut replacement = std::mem::zeroed();
                    if !statistics_msgs__msg__StatisticDataPoint__Sequence__init(
                        &mut replacement,
                        length,
                    ) {
                        return Err(Error::RCL_RET_BAD_ALLOC);
                    }
                    statistics_msgs__msg__StatisticDataPoint__Sequence__fini(
                        &mut msg.statistics,
                    );
                    msg.statistics = replacement;
                }
                msg.statistics.size = length;
                if length != 0 {
                    let slice = std::slice::from_raw_parts_mut(
                        msg.statistics.data,
                        length,
                    );
                    for (t, s) in slice.iter_mut().zip(&self.statistics) {
                        s.try_copy_to_native(t)?;
                    }
                }
            }
            Ok(())
        }
    }
    impl Default for MetricsMessage {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<MetricsMessage>::new();
            MetricsMessage::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct StatisticDataPoint {
        pub data_type: u8,
        pub data: f64,
    }
    unsafe impl WrappedTypesupport for StatisticDataPoint {
        type CStruct = statistics_msgs__msg__StatisticDataPoint;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__statistics_msgs__msg__StatisticDataPoint()
            }
        }
        fn create_msg() -> *mut statistics_msgs__msg__StatisticDataPoint {
            #[cfg(not(feature = "doc-only"))]
            unsafe { statistics_msgs__msg__StatisticDataPoint__create() }
            #[cfg(feature = "doc-only")]
            statistics_msgs__msg__StatisticDataPoint__create()
        }
        fn destroy_msg(msg: *mut statistics_msgs__msg__StatisticDataPoint) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { statistics_msgs__msg__StatisticDataPoint__destroy(msg) };
            #[cfg(feature = "doc-only")]
            statistics_msgs__msg__StatisticDataPoint__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> StatisticDataPoint {
            StatisticDataPoint {
                data_type: msg.data_type,
                data: msg.data,
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
            msg.data_type = self.data_type;
            msg.data = self.data;
            Ok(())
        }
    }
    impl Default for StatisticDataPoint {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<StatisticDataPoint>::new();
            StatisticDataPoint::from_native(&msg_native)
        }
    }
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default)]
    pub struct StatisticDataType {}
    unsafe impl WrappedTypesupport for StatisticDataType {
        type CStruct = statistics_msgs__msg__StatisticDataType;
        fn get_ts() -> &'static rosidl_message_type_support_t {
            unsafe {
                &*rosidl_typesupport_c__get_message_type_support_handle__statistics_msgs__msg__StatisticDataType()
            }
        }
        fn create_msg() -> *mut statistics_msgs__msg__StatisticDataType {
            #[cfg(not(feature = "doc-only"))]
            unsafe { statistics_msgs__msg__StatisticDataType__create() }
            #[cfg(feature = "doc-only")]
            statistics_msgs__msg__StatisticDataType__create()
        }
        fn destroy_msg(msg: *mut statistics_msgs__msg__StatisticDataType) -> () {
            #[cfg(not(feature = "doc-only"))]
            unsafe { statistics_msgs__msg__StatisticDataType__destroy(msg) };
            #[cfg(feature = "doc-only")]
            statistics_msgs__msg__StatisticDataType__destroy(msg)
        }
        fn from_native(#[allow(unused)] msg: &Self::CStruct) -> StatisticDataType {
            StatisticDataType {}
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
    impl Default for StatisticDataType {
        fn default() -> Self {
            let msg_native = WrappedNativeMsg::<StatisticDataType>::new();
            StatisticDataType::from_native(&msg_native)
        }
    }
    #[allow(non_upper_case_globals)]
    impl StatisticDataType {
        pub const STATISTICS_DATA_TYPE_AVERAGE: _bindgen_ty_330 = statistics_msgs__msg__StatisticDataType__STATISTICS_DATA_TYPE_AVERAGE;
        pub const STATISTICS_DATA_TYPE_MAXIMUM: _bindgen_ty_332 = statistics_msgs__msg__StatisticDataType__STATISTICS_DATA_TYPE_MAXIMUM;
        pub const STATISTICS_DATA_TYPE_MINIMUM: _bindgen_ty_331 = statistics_msgs__msg__StatisticDataType__STATISTICS_DATA_TYPE_MINIMUM;
        pub const STATISTICS_DATA_TYPE_SAMPLE_COUNT: _bindgen_ty_334 = statistics_msgs__msg__StatisticDataType__STATISTICS_DATA_TYPE_SAMPLE_COUNT;
        pub const STATISTICS_DATA_TYPE_STDDEV: _bindgen_ty_333 = statistics_msgs__msg__StatisticDataType__STATISTICS_DATA_TYPE_STDDEV;
        pub const STATISTICS_DATA_TYPE_UNINITIALIZED: _bindgen_ty_329 = statistics_msgs__msg__StatisticDataType__STATISTICS_DATA_TYPE_UNINITIALIZED;
    }
}
