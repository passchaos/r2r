#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]
// Silence "`extern` fn uses type `u128`, which is not FFI-safe"
// As of rustc 1.78, this has been fixed.
// It could be good to still warn if building with an older rust version.
#![allow(improper_ctypes)]
#![allow(improper_ctypes_definitions)]
#![cfg_attr(
    feature = "doc-only",
    allow(unpredictable_function_pointer_comparisons)
)]
include!(concat!(env!("OUT_DIR"), "/rcl_bindings.rs"));

impl Default for rmw_message_info_t {
    fn default() -> Self {
        unsafe { rmw_get_zero_initialized_message_info() }
    }
}

mod strings;

// conversions from/to vectors of built in types

macro_rules! primitive_sequence {
    ($ctype:ident, $element_type:ident) => {
        paste::item! {
            impl [<$ctype __Sequence>] {
                pub fn update(&mut self, values: &[$element_type]) {
                    let _ = self.try_update(values);
                }

                /// Copies values into native storage, reusing initialized capacity.
                /// Existing contents are preserved if allocating a larger sequence fails.
                #[inline]
                pub fn try_update(&mut self, values: &[$element_type]) -> bool {
                    self.try_update_with(values, [<$ctype __Sequence__init>])
                }

                #[inline]
                fn try_update_with(
                    &mut self, values: &[$element_type],
                    initialize: unsafe extern "C" fn(*mut Self, usize) -> bool,
                ) -> bool {
                    if values.len() > self.capacity {
                        let mut replacement = Self {
                            data: std::ptr::null_mut(), size: 0, capacity: 0,
                        };
                        if !unsafe { initialize(&mut replacement, values.len()) } {
                            return false;
                        }
                        unsafe { [<$ctype __Sequence__fini>] (self as *mut _); }
                        *self = replacement;
                    }
                    if values.is_empty() {
                        self.size = 0;
                        return true;
                    }
                    unsafe { std::ptr::copy_nonoverlapping(values.as_ptr(), self.data, values.len()); }
                    self.size = values.len();
                    true
                }

                pub fn to_vec(&self) -> Vec<$element_type> {
                    if self.data.is_null() {
                        return Vec::new();
                    }
                    let mut target = Vec::with_capacity(self.size);
                    unsafe {
                        std::ptr::copy_nonoverlapping(self.data, target.as_mut_ptr(), self.size);
                        target.set_len(self.size);
                    }
                    target
                }
            }
        }
    };
}

primitive_sequence!(rosidl_runtime_c__float32, f32);
primitive_sequence!(rosidl_runtime_c__float64, f64);

#[cfg(any(
    all(target_os = "macos", target_arch = "aarch64"),
    target_arch = "arm",
    target_os = "windows"
))]
primitive_sequence!(rosidl_runtime_c__long_double, f64);

#[cfg(not(any(
    all(target_os = "macos", target_arch = "aarch64"),
    target_arch = "arm",
    target_os = "windows"
)))]
primitive_sequence!(rosidl_runtime_c__long_double, u128);

primitive_sequence!(rosidl_runtime_c__char, i8);
primitive_sequence!(rosidl_runtime_c__wchar, u16);
primitive_sequence!(rosidl_runtime_c__boolean, bool);
primitive_sequence!(rosidl_runtime_c__octet, u8);
primitive_sequence!(rosidl_runtime_c__uint8, u8);
primitive_sequence!(rosidl_runtime_c__int8, i8);
primitive_sequence!(rosidl_runtime_c__uint16, u16);
primitive_sequence!(rosidl_runtime_c__int16, i16);
primitive_sequence!(rosidl_runtime_c__uint32, u32);
primitive_sequence!(rosidl_runtime_c__int32, i32);
primitive_sequence!(rosidl_runtime_c__uint64, u64);
primitive_sequence!(rosidl_runtime_c__int64, i64);

#[cfg(all(test, not(feature = "doc-only")))]
mod sequence_tests;
