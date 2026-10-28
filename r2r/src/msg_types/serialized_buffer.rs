use super::{serialized_message_bytes, VoidPtr, WrappedNativeMsg, WrappedTypesupport};
use crate::{Error, Result};
use r2r_rcl::*;
use std::ffi::c_void;

// Two pointer-sized words retain the allocator's ordinary malloc alignment on supported
// platforms. Metadata belongs to each block, so callbacks need no shared mutation or lock.
#[repr(C)]
struct AllocationHeader {
    bytes: usize,
    _reserved: usize,
}
const HEADER: usize = 16;
const _: () = assert!(std::mem::size_of::<AllocationHeader>() <= HEADER);

unsafe extern "C" fn allocate(bytes: usize, state: *mut c_void) -> *mut c_void {
    let Some(total) = bytes.checked_add(HEADER) else {
        return std::ptr::null_mut();
    };
    let allocator = unsafe { &*state.cast::<rcutils_allocator_t>() };
    let base = unsafe { allocator.allocate.unwrap()(total, allocator.state) }.cast::<u8>();
    if base.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        base.cast::<AllocationHeader>().write(AllocationHeader {
            bytes,
            _reserved: 0,
        });
        let data = base.add(HEADER);
        data.write_bytes(0, bytes);
        data.cast()
    }
}

unsafe extern "C" fn deallocate(data: *mut c_void, state: *mut c_void) {
    let allocator = unsafe { &*state.cast::<rcutils_allocator_t>() };
    let base = if data.is_null() {
        data
    } else {
        unsafe { data.cast::<u8>().sub(HEADER).cast() }
    };
    unsafe { allocator.deallocate.unwrap()(base, allocator.state) };
}

unsafe extern "C" fn reallocate(
    data: *mut c_void, bytes: usize, state: *mut c_void,
) -> *mut c_void {
    if data.is_null() {
        return unsafe { allocate(bytes, state) };
    }
    let Some(total) = bytes.checked_add(HEADER) else {
        return std::ptr::null_mut();
    };
    let allocator = unsafe { &*state.cast::<rcutils_allocator_t>() };
    let base = unsafe { data.cast::<u8>().sub(HEADER) };
    let old = unsafe { (*base.cast::<AllocationHeader>()).bytes };
    let replacement =
        unsafe { allocator.reallocate.unwrap()(base.cast(), total, allocator.state) }.cast::<u8>();
    if replacement.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        replacement
            .cast::<AllocationHeader>()
            .write(AllocationHeader {
                bytes,
                _reserved: 0,
            });
        let data = replacement.add(HEADER);
        if bytes > old {
            data.add(old).write_bytes(0, bytes - old);
        }
        data.cast()
    }
}

unsafe extern "C" fn zero_allocate(count: usize, bytes: usize, state: *mut c_void) -> *mut c_void {
    let Some(bytes) = count.checked_mul(bytes) else {
        return std::ptr::null_mut();
    };
    unsafe { allocate(bytes, state) }
}

pub(crate) struct SerializedMessageBuffer {
    pub(super) handle: rcl_serialized_message_t,
    _allocator: Box<rcutils_allocator_t>,
}
impl SerializedMessageBuffer {
    pub(crate) fn with_capacity(capacity: usize) -> std::result::Result<Self, i32> {
        Self::with_allocator(capacity, unsafe { rcutils_get_default_allocator() })
    }
    fn with_allocator(
        capacity: usize, allocator: rcutils_allocator_t,
    ) -> std::result::Result<Self, i32> {
        if !unsafe { rcutils_allocator_is_valid(&allocator) } {
            return Err(RCL_RET_INVALID_ARGUMENT as i32);
        }
        let mut owner = Box::new(allocator);
        let wrapped = rcutils_allocator_t {
            allocate: Some(allocate),
            deallocate: Some(deallocate),
            reallocate: Some(reallocate),
            zero_allocate: Some(zero_allocate),
            state: (&mut *owner as *mut rcutils_allocator_t).cast(),
        };
        let mut buffer = unsafe { rcutils_get_zero_initialized_uint8_array() };
        let result = unsafe { rcutils_uint8_array_init(&mut buffer, capacity, &wrapped) };
        if result == RCL_RET_OK as i32 {
            Ok(Self {
                handle: buffer,
                _allocator: owner,
            })
        } else {
            Err(result)
        }
    }
    pub(crate) fn capacity(&self) -> usize {
        self.handle.buffer_capacity
    }
    pub(crate) fn serialize<T: WrappedTypesupport>(
        &mut self, message: &WrappedNativeMsg<T>,
    ) -> Result<&[u8]> {
        let result = unsafe { rmw_serialize(message.void_ptr(), T::get_ts(), &mut self.handle) };
        if result != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(result));
        }
        unsafe { serialized_message_bytes(&self.handle) }
    }
}
// SAFETY: native storage and its stable boxed allocator have unique ownership. The captured
// default allocator is process-lifetime; callbacks mutate only metadata in their own block.
unsafe impl Send for SerializedMessageBuffer {}
impl Drop for SerializedMessageBuffer {
    fn drop(&mut self) {
        let ret = unsafe { rcutils_uint8_array_fini(&mut self.handle) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize serialized message buffer: {ret}");
        }
    }
}

#[cfg(all(test, not(feature = "doc-only")))]
mod tests;
