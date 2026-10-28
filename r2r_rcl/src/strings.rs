use super::*;

// special treatment to convert to/from rust strings.
// ros strings are owned by ros, assignment is a copy
impl rosidl_runtime_c__String {
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        if self.data.is_null() {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(self.data.cast::<u8>(), self.size) }
    }
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
        self.try_assign_bytes(other.as_bytes())
    }
    #[inline]
    pub fn try_assign_bytes(&mut self, other: &[u8]) -> bool {
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

use widestring::U16Str;
impl rosidl_runtime_c__U16String {
    #[inline]
    pub fn as_units(&self) -> &[u16] {
        if self.data.is_null() {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(self.data, self.size) }
    }
    #[inline]
    pub fn try_assign_units(&mut self, units: &[u16]) -> bool {
        if (units.len() >= self.capacity || self.data.is_null())
            && !unsafe { rosidl_runtime_c__U16String__resize(self, units.len()) }
        {
            return false;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(units.as_ptr(), self.data, units.len());
            *self.data.add(units.len()) = 0;
        }
        self.size = units.len();
        true
    }
    pub fn try_to_string(&self) -> Result<String, widestring::error::Utf16Error> {
        if self.data.is_null() {
            return Ok(String::new());
        }
        let s = unsafe { U16Str::from_ptr(self.data, self.size) };
        s.to_string()
    }

    pub fn to_str(&self) -> String {
        self.try_to_string().unwrap_or_default()
    }

    pub fn assign(&mut self, other: &str) {
        let _ = self.try_assign(other);
    }

    pub fn try_assign(&mut self, other: &str) -> bool {
        self.try_assign_with(other, rosidl_runtime_c__U16String__resize)
    }

    fn try_assign_with(
        &mut self, other: &str, resize: unsafe extern "C" fn(*mut Self, usize) -> bool,
    ) -> bool {
        let length = other.encode_utf16().count();
        if (length >= self.capacity || self.data.is_null()) && !unsafe { resize(self, length) } {
            return false;
        }
        // Encode directly into native storage; no temporary UTF-16 allocation or copy.
        for (index, unit) in other.encode_utf16().enumerate() {
            unsafe { *self.data.add(index) = unit as uint_least16_t };
        }
        unsafe { *self.data.add(length) = 0 };
        self.size = length;
        true
    }
}

mod sequences;

#[cfg(all(test, not(feature = "doc-only")))]
mod tests;
