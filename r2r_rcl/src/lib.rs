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
    pub fn try_to_str(&self) -> Result<&str, std::str::Utf8Error> {
        if self.data.is_null() {
            return Ok("");
        }
        let bytes = unsafe { std::slice::from_raw_parts(self.data.cast::<u8>(), self.size) };
        std::str::from_utf8(bytes)
    }

    pub fn to_str(&self) -> &str {
        self.try_to_str().unwrap_or("")
    }

    pub fn assign(&mut self, other: &str) {
        let _ = self.try_assign(other);
    }

    #[inline]
    pub fn try_assign(&mut self, other: &str) -> bool {
        if other.len() < self.capacity && !self.data.is_null() {
            unsafe {
                std::ptr::copy_nonoverlapping(other.as_ptr(), self.data.cast::<u8>(), other.len());
                *self.data.add(other.len()) = 0;
            }
            self.size = other.len();
            return true;
        }
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
    pub fn try_to_string(&self) -> Result<String, widestring::error::Utf16Error> {
        if self.data.is_null() {
            return Ok(String::new());
        }
        let s = unsafe { U16String::from_ptr(self.data, self.size) };
        s.to_string()
    }

    pub fn to_str(&self) -> String {
        self.try_to_string().unwrap_or_default()
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
