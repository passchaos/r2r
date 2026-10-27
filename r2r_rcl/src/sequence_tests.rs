use super::*;

#[test]
fn string_capacity_fast_path_matches_native_assignn_oracle() {
    struct Owned(rosidl_runtime_c__String);
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe { rosidl_runtime_c__String__fini(&mut self.0) };
        }
    }
    let empty = || rosidl_runtime_c__String {
        data: std::ptr::null_mut(),
        size: 0,
        capacity: 0,
    };
    let mut candidate = Owned(empty());
    let mut oracle = Owned(empty());
    for text in [
        "",
        "a string with spare room",
        "short",
        "",
        "ハロー\0world",
        "a longer value forcing native allocation",
        "small",
    ] {
        assert!(candidate.0.try_assign(text));
        assert!(unsafe {
            rosidl_runtime_c__String__assignn(&mut oracle.0, text.as_ptr().cast(), text.len())
        });
        assert_eq!(candidate.0.size, oracle.0.size);
        assert!(candidate.0.capacity > candidate.0.size);
        assert_eq!(candidate.0.try_to_str().unwrap(), oracle.0.try_to_str().unwrap());
        assert_eq!(unsafe { *candidate.0.data.add(candidate.0.size) }, 0);
    }
    let pointer = candidate.0.data;
    for text in ["", "a", "b\0c", "ハロー"] {
        assert!(candidate.0.try_assign(text));
        assert_eq!(candidate.0.data, pointer);
        assert_eq!(candidate.0.try_to_str().unwrap(), text);
    }
}

macro_rules! sequence_case {
    ($ctype:ident, $values:expr) => {
        paste::item! {
            #[test]
            fn [<$ctype _retains_capacity_and_roundtrips_updates>]() {
                struct Owned([<$ctype __Sequence>]);
                impl Drop for Owned {
                    fn drop(&mut self) {
                        unsafe { [<$ctype __Sequence__fini>](&mut self.0) };
                    }
                }
                let mut sequence = Owned([<$ctype __Sequence>] {
                    data: std::ptr::null_mut(), size: 0, capacity: 0,
                });
                let values = $values;
                assert!(sequence.0.try_update(&[]));
                assert!(sequence.0.data.is_null());
                assert!(sequence.0.try_update(&values));
                assert_eq!(sequence.0.to_vec(), values);
                let pointer = sequence.0.data;
                let capacity = sequence.0.capacity;
                assert!(sequence.0.try_update(&values[..1]));
                assert_eq!(sequence.0.data, pointer);
                assert_eq!(sequence.0.capacity, capacity);
                assert_eq!(sequence.0.to_vec(), values[..1]);
                assert!(sequence.0.try_update(&[]));
                assert_eq!(sequence.0.data, pointer);
                assert_eq!(sequence.0.capacity, capacity);
                assert_eq!(sequence.0.size, 0);
                assert!(sequence.0.try_update(&values));
                assert_eq!(sequence.0.data, pointer);
                assert_eq!(sequence.0.to_vec(), values);
                let grown = vec![values[0]; 8];
                assert!(sequence.0.try_update(&grown));
                assert_eq!(sequence.0.to_vec(), grown);
                assert!(sequence.0.capacity >= 8);
            }
        }
    };
}

sequence_case!(rosidl_runtime_c__float32, [-0.0_f32, 1.25, f32::INFINITY]);
sequence_case!(rosidl_runtime_c__float64, [-0.0_f64, 1.25, f64::INFINITY]);
#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    target_arch = "arm",
    target_os = "windows"
))]
sequence_case!(rosidl_runtime_c__long_double, [-0.0_f64, 1.25, f64::INFINITY]);
#[cfg(not(any(
    all(target_os = "macos", target_arch = "aarch64"),
    target_arch = "arm",
    target_os = "windows"
)))]
sequence_case!(rosidl_runtime_c__long_double, [0_u128, 1, u128::MAX]);
sequence_case!(rosidl_runtime_c__char, [i8::MIN, 0, i8::MAX]);
sequence_case!(rosidl_runtime_c__wchar, [0_u16, 1, u16::MAX]);
sequence_case!(rosidl_runtime_c__boolean, [true, false, true]);
sequence_case!(rosidl_runtime_c__octet, [0_u8, 1, u8::MAX]);
sequence_case!(rosidl_runtime_c__uint8, [0_u8, 1, u8::MAX]);
sequence_case!(rosidl_runtime_c__int8, [i8::MIN, 0, i8::MAX]);
sequence_case!(rosidl_runtime_c__uint16, [0_u16, 1, u16::MAX]);
sequence_case!(rosidl_runtime_c__int16, [i16::MIN, 0, i16::MAX]);
sequence_case!(rosidl_runtime_c__uint32, [0_u32, 1, u32::MAX]);
sequence_case!(rosidl_runtime_c__int32, [i32::MIN, 0, i32::MAX]);
sequence_case!(rosidl_runtime_c__uint64, [0_u64, 1, u64::MAX]);
sequence_case!(rosidl_runtime_c__int64, [i64::MIN, 0, i64::MAX]);

#[test]
fn failed_primitive_sequence_growth_preserves_existing_values() {
    unsafe extern "C" fn fail_init(_: *mut rosidl_runtime_c__int32__Sequence, _: usize) -> bool {
        false
    }
    let mut sequence = rosidl_runtime_c__int32__Sequence {
        data: std::ptr::null_mut(),
        size: 0,
        capacity: 0,
    };
    assert!(sequence.try_update(&[11, 22, 33]));
    let pointer = sequence.data;
    let capacity = sequence.capacity;
    assert!(!sequence.try_update_with(&[44; 8], fail_init));
    assert_eq!(sequence.to_vec(), [11, 22, 33]);
    assert_eq!(sequence.data, pointer);
    assert_eq!(sequence.capacity, capacity);
    assert!(sequence.try_update(&[55; 2]));
    assert_eq!(sequence.to_vec(), [55; 2]);
    unsafe { rosidl_runtime_c__int32__Sequence__fini(&mut sequence) };
}
