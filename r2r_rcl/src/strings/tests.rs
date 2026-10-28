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

#[test]
fn utf16_direct_encoding_matches_native_assignn_and_retains_spare_capacity() {
    struct Owned(rosidl_runtime_c__U16String);
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe { rosidl_runtime_c__U16String__fini(&mut self.0) };
        }
    }
    let empty = || rosidl_runtime_c__U16String {
        data: std::ptr::null_mut(),
        size: 0,
        capacity: 0,
    };
    let mut candidate = Owned(empty());
    let mut oracle = Owned(empty());
    for text in [
        "",
        "ASCII",
        "ハロー\0world",
        "😀🦀",
        "a longer allocation",
        "",
        "x",
    ] {
        let units: Vec<u16> = text.encode_utf16().collect();
        assert!(candidate.0.try_assign(text));
        assert!(unsafe {
            rosidl_runtime_c__U16String__assignn(&mut oracle.0, units.as_ptr(), units.len())
        });
        assert_eq!(candidate.0.size, units.len());
        assert_eq!(candidate.0.to_str(), text);
        assert_eq!(candidate.0.to_str(), oracle.0.to_str());
        assert!(candidate.0.capacity > candidate.0.size);
        assert_eq!(unsafe { *candidate.0.data.add(candidate.0.size) }, 0);
    }
    let pointer = candidate.0.data;
    let capacity = candidate.0.capacity;
    for text in ["", "日本語", "a\0b", "🦀"] {
        assert!(candidate.0.try_assign(text));
        assert_eq!(candidate.0.data, pointer);
        assert_eq!(candidate.0.capacity, capacity);
        assert_eq!(candidate.0.to_str(), text);
    }
    // Exact capacity needs another unit for the terminator, including surrogate pairs.
    let text = "🦀".repeat(capacity);
    assert!(candidate.0.try_assign(&text));
    assert_eq!(candidate.0.size, capacity * 2);
    assert_eq!(candidate.0.to_str(), text);
    assert!(candidate.0.try_assign("x"));
    unsafe { *candidate.0.data = 0xd800 };
    assert!(candidate.0.try_to_string().is_err());
    assert!(candidate.0.try_assign("🦀"));
    assert_eq!(candidate.0.to_str(), "🦀");
}

#[test]
fn failed_utf16_growth_preserves_the_previous_value() {
    unsafe extern "C" fn fail_resize(_: *mut rosidl_runtime_c__U16String, _: usize) -> bool {
        false
    }
    let mut value = rosidl_runtime_c__U16String {
        data: std::ptr::null_mut(),
        size: 0,
        capacity: 0,
    };
    assert!(value.try_assign("🦀"));
    let pointer = value.data;
    let capacity = value.capacity;
    assert!(!value.try_assign_with("a longer value", fail_resize));
    assert_eq!(value.data, pointer);
    assert_eq!(value.capacity, capacity);
    assert_eq!(value.to_str(), "🦀");
    assert!(value.try_assign_with("x", fail_resize));
    assert_eq!(value.to_str(), "x");
    unsafe { rosidl_runtime_c__U16String__fini(&mut value) };
}

macro_rules! string_sequence_cases {
    ($ctype:ident) => {
        paste::item! {
            #[test]
            fn [<$ctype _sequence_reuses_outer_and_hidden_string_storage>]() {
                let mut sequence = [<$ctype __Sequence>] {
                    data: std::ptr::null_mut(), size: 0, capacity: 0,
                };
                assert!(sequence.try_update(&[]));
                assert!(sequence.data.is_null());
                let original: Vec<String> = ["a long string with spare room", "日本語🦀\0value", "third value"]
                    .into_iter().map(String::from).collect();
                assert!(sequence.try_update(&original));
                let outer = sequence.data;
                let buffers: Vec<_> = unsafe {
                    std::slice::from_raw_parts(sequence.data, sequence.capacity)
                }.iter().map(|value| (value.data, value.capacity)).collect();
                for values in [vec!["x".into()], vec![], vec!["".into(), "🦀".into()], original.clone()] {
                    assert!(sequence.try_update(&values));
                    assert_eq!(sequence.to_vec(), values);
                    assert_eq!(sequence.data, outer);
                    assert_eq!(sequence.capacity, 3);
                    for (index, &(data, capacity)) in buffers.iter().enumerate() {
                        let value = unsafe { &*sequence.data.add(index) };
                        assert_eq!((value.data, value.capacity), (data, capacity));
                    }
                }
                let grown = vec!["expanded".into(); 8];
                assert!(sequence.try_update(&grown));
                assert_eq!(sequence.to_vec(), grown);
                for (index, &(data, _)) in buffers.iter().enumerate() {
                    assert_eq!(unsafe { (*sequence.data.add(index)).data }, data);
                }
                assert!(sequence.try_update(&[]));
                // Finalization must visit all initialized capacity slots after logical shrink.
                unsafe { [<$ctype __Sequence__fini>](&mut sequence) };
            }

            #[test]
            fn [<$ctype _sequence_rolls_back_new_slots_after_initialization_failure>]() {
                static CALLS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                unsafe extern "C" fn fail_second_initialize(value: *mut $ctype) -> bool {
                    if CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 1 {
                        false
                    } else {
                        unsafe { [<$ctype __init>](value) }
                    }
                }
                let mut sequence = [<$ctype __Sequence>] {
                    data: std::ptr::null_mut(), size: 0, capacity: 0,
                };
                let values = vec!["first initialized value".into(), "🦀 second value".into()];
                assert!(sequence.try_update(&values));
                let buffers = unsafe { [(*sequence.data).data, (*sequence.data.add(1)).data] };
                let allocator = unsafe { rcutils_get_default_allocator() };
                CALLS.store(0, std::sync::atomic::Ordering::Relaxed);
                assert!(!sequence.try_grow_with(8, fail_second_initialize,
                    allocator.reallocate.unwrap(), allocator.state));
                assert_eq!(CALLS.load(std::sync::atomic::Ordering::Relaxed), 2);
                assert_eq!(sequence.size, 2);
                assert_eq!(sequence.capacity, 2);
                assert_eq!(sequence.to_vec(), values);
                assert_eq!(unsafe { [(*sequence.data).data, (*sequence.data.add(1)).data] }, buffers);
                assert!(sequence.try_update(&vec!["retry".into(); 8]));
                assert_eq!(sequence.to_vec(), vec!["retry"; 8]);
                unsafe { [<$ctype __Sequence__fini>](&mut sequence) };
            }

            #[test]
            fn [<$ctype _sequence_allocation_and_size_overflow_fail_without_losing_values>]() {
                unsafe extern "C" fn fail_reallocate(
                    _: *mut std::os::raw::c_void, _: usize, _: *mut std::os::raw::c_void,
                ) -> *mut std::os::raw::c_void {
                    std::ptr::null_mut()
                }
                let mut sequence = [<$ctype __Sequence>] {
                    data: std::ptr::null_mut(), size: 0, capacity: 0,
                };
                let values = vec!["original".into()];
                assert!(sequence.try_update(&values));
                let pointer = sequence.data;
                for length in [8, usize::MAX] {
                    assert!(!sequence.try_grow_with(length, [<$ctype __init>],
                        fail_reallocate, std::ptr::null_mut()));
                    assert_eq!(sequence.data, pointer);
                    assert_eq!(sequence.to_vec(), values);
                    assert_eq!((sequence.size, sequence.capacity), (1, 1));
                }
                unsafe { [<$ctype __Sequence__fini>](&mut sequence) };
            }
        }
    };
}

string_sequence_cases!(rosidl_runtime_c__String);
string_sequence_cases!(rosidl_runtime_c__U16String);

#[test]
fn raw_string_sequences_preserve_non_utf8_bytes_and_reuse_storage() {
    let mut sequence = rosidl_runtime_c__String__Sequence {
        data: std::ptr::null_mut(),
        size: 0,
        capacity: 0,
    };
    let values = vec![vec![0xff, 0, b'x', 0xfe], vec![0x80, 0x81]];
    assert!(sequence.try_update_bytes(&values));
    let pointer = sequence.data;
    assert_eq!(sequence.to_bytes_vec::<Vec<u8>>(), values);
    assert!(unsafe { (*sequence.data).try_to_str() }.is_err());
    assert!(sequence.try_update_bytes::<Vec<u8>>(&[]));
    assert!(sequence.try_update_bytes(&values));
    assert_eq!(sequence.data, pointer);
    assert_eq!(sequence.to_bytes_vec::<Vec<u8>>(), values);
    unsafe { rosidl_runtime_c__String__Sequence__fini(&mut sequence) };
}

#[test]
fn raw_wide_sequences_preserve_unpaired_surrogates_and_reuse_storage() {
    let mut sequence = rosidl_runtime_c__U16String__Sequence {
        data: std::ptr::null_mut(),
        size: 0,
        capacity: 0,
    };
    let values = vec![vec![0xd800, 0, 0x4e2d], vec![0xdc00]];
    assert!(sequence.try_update_units(&values));
    let pointer = sequence.data;
    assert_eq!(sequence.to_units_vec::<Vec<u16>>(), values);
    assert!(unsafe { (*sequence.data).try_to_string() }.is_err());
    assert!(sequence.try_update_units::<Vec<u16>>(&[]));
    assert!(sequence.try_update_units(&values));
    assert_eq!(sequence.data, pointer);
    assert_eq!(sequence.to_units_vec::<Vec<u16>>(), values);
    unsafe { rosidl_runtime_c__U16String__Sequence__fini(&mut sequence) };
}
