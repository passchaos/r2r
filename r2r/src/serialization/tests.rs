use std::cell::Cell;

use serde::{Deserialize, Serialize};

use super::*;
use crate::{
    std_msgs::msg::{Int32MultiArray, MultiArrayDimension, MultiArrayLayout},
    unique_identifier_msgs::msg::UUID,
};

fn array(size: usize) -> Int32MultiArray {
    Int32MultiArray {
        layout: MultiArrayLayout {
            dim: vec![MultiArrayDimension {
                // Including the terminator this label is 24 bytes, avoiding unspecified
                // CDR alignment padding in byte-for-byte cross-buffer oracle comparisons.
                label: "samples_ハロー_axis!".into(),
                size: size as u32,
                stride: size as u32,
            }],
            data_offset: 3,
        },
        data: (0..size).map(|index| index as i32 - 42).collect(),
    }
}

#[test]
fn serialization_reuses_capacity_and_matches_full_native_wire_oracle() {
    let mut serializer = MessageSerializer::<Int32MultiArray>::with_capacity(4096).unwrap();
    assert!(serializer.capacity() >= 4096);
    let native_pointer = serializer.native.msg;
    let mut data_pointer = None;
    let mut dimension_pointer = None;
    let mut retained = Vec::new();
    for size in [64, 1, 0, 128, 32] {
        let original = array(size);
        let oracle = original.to_serialized_bytes().unwrap();
        let bytes = serializer.serialize(&original).unwrap();
        assert_eq!(bytes, oracle);
        assert_eq!(Int32MultiArray::from_serialized_bytes(bytes).unwrap(), original);
        let byte_count = bytes.len();
        retained.push((bytes.to_vec(), original));
        assert_eq!(serializer.native.msg, native_pointer);
        if let Some((pointer, capacity)) = data_pointer {
            if size <= capacity {
                assert_eq!(serializer.native.data.data, pointer);
            }
        }
        data_pointer = Some((serializer.native.data.data, serializer.native.data.capacity));
        if let Some(pointer) = dimension_pointer {
            assert_eq!(serializer.native.layout.dim.data, pointer);
        } else {
            dimension_pointer = Some(serializer.native.layout.dim.data);
        }
        assert!(serializer.capacity() >= byte_count);
    }
    let original = array(4096);
    let bytes = serializer.serialize(&original).unwrap();
    assert_eq!(Int32MultiArray::from_serialized_bytes(bytes).unwrap(), original);
    assert!(serializer.capacity() > 4096);
    drop(serializer);
    for (bytes, original) in retained {
        assert_eq!(Int32MultiArray::from_serialized_bytes(&bytes).unwrap(), original);
    }
}

#[test]
fn invalid_fixed_array_json_is_rejected_before_native_serialization() {
    let mut serializer = MessageSerializer::<UUID>::new().unwrap();
    let valid = UUID { uuid: [7; 16] };
    for count in [0, 15, 17] {
        let value = serde_json::json!({"uuid":vec![9;count]});
        assert!(serde_json::from_value::<UUID>(value).is_err());
        assert_eq!(serializer.serialize(&valid).unwrap(), valid.to_serialized_bytes().unwrap());
    }
}

#[test]
fn embedded_nul_string_wire_bytes_match_existing_rmw_codec_semantics() {
    let mut original = array(32);
    original.layout.dim[0].label = "samples\0axis".into();
    let oracle = original.to_serialized_bytes().unwrap();
    let mut serializer = MessageSerializer::<Int32MultiArray>::new().unwrap();
    let bytes = serializer.serialize(&original).unwrap();
    assert_eq!(bytes.len(), oracle.len());
    // Native conversion retains the embedded NUL. The selected RMW determines its ROS-string
    // wire treatment. Compare decoded values because native CDR padding bytes are unspecified.
    assert_eq!(Int32MultiArray::from_native(&serializer.native), original);
    assert_eq!(
        Int32MultiArray::from_serialized_bytes(serializer.serialize(&original).unwrap()).unwrap(),
        Int32MultiArray::from_serialized_bytes(&oracle).unwrap(),
    );
}

#[test]
fn serializer_moves_between_threads_and_separate_buffers_are_independent() {
    fn assert_send<T: Send>() {}
    assert_send::<MessageSerializer<Int32MultiArray>>();
    let mut first = MessageSerializer::<Int32MultiArray>::new().unwrap();
    let mut second = MessageSerializer::<Int32MultiArray>::new().unwrap();
    let a = array(32);
    let b = array(64);
    let saved = first.serialize(&a).unwrap().to_vec();
    assert_eq!(second.serialize(&b).unwrap(), b.to_serialized_bytes().unwrap());
    let returned = std::thread::spawn(move || {
        assert_eq!(first.serialize(&a).unwrap(), saved);
        first
    })
    .join()
    .unwrap();
    assert!(returned.capacity() > 0);
}

thread_local! {
    static CREATED: Cell<usize> = const { Cell::new(0) };
    static DESTROYED: Cell<usize> = const { Cell::new(0) };
    static FAIL_CREATE: Cell<bool> = const { Cell::new(false) };
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct Tracked(Int32MultiArray);

// The wrapper delegates all native operations to the exact Int32MultiArray C representation.
unsafe impl WrappedTypesupport for Tracked {
    type CStruct = <Int32MultiArray as WrappedTypesupport>::CStruct;
    fn get_ts() -> &'static r2r_rcl::rosidl_message_type_support_t {
        Int32MultiArray::get_ts()
    }
    fn create_msg() -> *mut Self::CStruct {
        if FAIL_CREATE.with(Cell::get) {
            return std::ptr::null_mut();
        }
        CREATED.with(|count| count.set(count.get() + 1));
        Int32MultiArray::create_msg()
    }
    fn destroy_msg(message: *mut Self::CStruct) {
        DESTROYED.with(|count| count.set(count.get() + 1));
        Int32MultiArray::destroy_msg(message);
    }
    fn from_native(message: &Self::CStruct) -> Self {
        Self(Int32MultiArray::from_native(message))
    }
    fn copy_to_native(&self, message: &mut Self::CStruct) {
        self.0.copy_to_native(message);
    }
    fn try_copy_to_native(&self, message: &mut Self::CStruct) -> Result<()> {
        self.0.try_copy_to_native(message)
    }
}

#[test]
fn serializer_creates_one_native_container_and_releases_it_once() {
    CREATED.with(|count| count.set(0));
    DESTROYED.with(|count| count.set(0));
    let mut serializer = MessageSerializer::<Tracked>::new().unwrap();
    let message = Tracked(array(32));
    for _ in 0..10 {
        assert!(!serializer.serialize(&message).unwrap().is_empty());
    }
    assert_eq!(CREATED.with(Cell::get), 1);
    assert_eq!(DESTROYED.with(Cell::get), 0);
    drop(serializer);
    assert_eq!(DESTROYED.with(Cell::get), 1);
}

#[test]
fn failed_native_creation_is_propagated_without_serializing() {
    FAIL_CREATE.with(|flag| flag.set(true));
    let result = MessageSerializer::<Tracked>::new();
    FAIL_CREATE.with(|flag| flag.set(false));
    assert!(matches!(result, Err(Error::RCL_RET_BAD_ALLOC)));
    assert!(MessageSerializer::<Tracked>::new().is_ok());
}
