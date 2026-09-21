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

// special treatment to convert to/from rust strings.
// ros strings are owned by ros, assignment is a copy
impl rosidl_runtime_c__String {
    pub fn to_str(&self) -> &str {
        if self.data.is_null() {
            return "";
        }
        let bytes = unsafe { std::slice::from_raw_parts(self.data.cast::<u8>(), self.size) };
        std::str::from_utf8(bytes).unwrap_or("")
    }

    pub fn assign(&mut self, other: &str) {
        let _ = self.try_assign(other);
    }

    pub fn try_assign(&mut self, other: &str) -> bool {
        unsafe {
            rosidl_runtime_c__String__assignn(
                self as *mut _,
                other.as_ptr().cast::<std::os::raw::c_char>(),
                other.len(),
            )
        }
    }
}

use widestring::U16String;
impl rosidl_runtime_c__U16String {
    pub fn to_str(&self) -> String {
        let s = unsafe { U16String::from_ptr(self.data, self.size) };
        s.to_string_lossy()
    }

    pub fn assign(&mut self, other: &str) {
        let _ = self.try_assign(other);
    }

    pub fn try_assign(&mut self, other: &str) -> bool {
        let wstr = U16String::from_str(other);
        let to_send_ptr = wstr.as_ptr() as *const uint_least16_t;
        unsafe { rosidl_runtime_c__U16String__assignn(self as *mut _, to_send_ptr, wstr.len()) }
    }
}

impl rosidl_runtime_c__U16String__Sequence {
    pub fn update(&mut self, values: &[String]) {
        let _ = self.try_update(values);
    }

    pub fn try_update(&mut self, values: &[String]) -> bool {
        unsafe {
            rosidl_runtime_c__U16String__Sequence__fini(self as *mut _);
        }
        if !unsafe { rosidl_runtime_c__U16String__Sequence__init(self as *mut _, values.len()) } {
            return false;
        }
        if values.is_empty() {
            return true;
        }
        let strs = unsafe { std::slice::from_raw_parts_mut(self.data, values.len()) };
        for (target, source) in strs.iter_mut().zip(values) {
            if !target.try_assign(source) {
                return false;
            }
        }
        true
    }

    pub fn to_vec(&self) -> Vec<String> {
        if self.data.is_null() {
            return Vec::new();
        }
        let mut target = Vec::with_capacity(self.size);
        let strs = unsafe { std::slice::from_raw_parts(self.data, self.size) };
        for s in strs {
            target.push(s.to_str().to_owned());
        }
        target
    }
}

impl rosidl_runtime_c__String__Sequence {
    pub fn update(&mut self, values: &[String]) {
        let _ = self.try_update(values);
    }

    pub fn try_update(&mut self, values: &[String]) -> bool {
        unsafe {
            rosidl_runtime_c__String__Sequence__fini(self as *mut _);
        }
        if !unsafe { rosidl_runtime_c__String__Sequence__init(self as *mut _, values.len()) } {
            return false;
        }
        if values.is_empty() {
            return true;
        }
        let strs = unsafe { std::slice::from_raw_parts_mut(self.data, values.len()) };
        for (target, source) in strs.iter_mut().zip(values) {
            if !target.try_assign(source) {
                return false;
            }
        }
        true
    }

    pub fn to_vec(&self) -> Vec<String> {
        if self.data.is_null() {
            return Vec::new();
        }
        let mut target = Vec::with_capacity(self.size);
        let strs = unsafe { std::slice::from_raw_parts(self.data, self.size) };
        for s in strs {
            target.push(s.to_str().to_owned());
        }
        target
    }
}

// conversions from/to vectors of built in types

macro_rules! primitive_sequence {
    ($ctype:ident, $element_type:ident) => {
        paste::item! {
            impl [<$ctype __Sequence>] {
                pub fn update(&mut self, values: &[$element_type]) {
                    let _ = self.try_update(values);
                }

                pub fn try_update(&mut self, values: &[$element_type]) -> bool {
                    unsafe { [<$ctype __Sequence__fini>] (self as *mut _); }
                    if !unsafe { [<$ctype __Sequence__init>] (self as *mut _, values.len()) } {
                        return false;
                    }
                    if values.is_empty() {
                        return true;
                    }
                    unsafe { std::ptr::copy_nonoverlapping(values.as_ptr(), self.data, values.len()); }
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
