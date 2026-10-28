use super::*;

unsafe extern "C" fn poisoned_allocate(bytes: usize, _: *mut c_void) -> *mut c_void {
    let allocator = unsafe { rcutils_get_default_allocator() };
    let data = unsafe { allocator.allocate.unwrap()(bytes, allocator.state) };
    if !data.is_null() {
        unsafe { data.cast::<u8>().write_bytes(0xab, bytes) };
    }
    data
}
unsafe extern "C" fn poisoned_growth(
    data: *mut c_void, bytes: usize, _: *mut c_void,
) -> *mut c_void {
    if data.is_null() {
        return unsafe { poisoned_allocate(bytes, std::ptr::null_mut()) };
    }
    let old = unsafe { (*data.cast::<AllocationHeader>()).bytes } + HEADER;
    let allocator = unsafe { rcutils_get_default_allocator() };
    let replacement = unsafe { allocator.reallocate.unwrap()(data, bytes, allocator.state) };
    if !replacement.is_null() && bytes > old {
        unsafe {
            replacement
                .cast::<u8>()
                .add(old)
                .write_bytes(0xab, bytes - old)
        };
    }
    replacement
}

#[test]
fn poisoned_initial_storage_and_grown_tails_are_initialized_without_erasing_prefixes() {
    let mut allocator = unsafe { rcutils_get_default_allocator() };
    allocator.allocate = Some(poisoned_allocate);
    allocator.reallocate = Some(poisoned_growth);
    let mut buffer = SerializedMessageBuffer::with_allocator(64, allocator).unwrap();
    assert!(unsafe { std::slice::from_raw_parts(buffer.handle.buffer, 64) }
        .iter()
        .all(|&byte| byte == 0));
    assert_eq!(buffer.handle.buffer as usize % 16, 0);
    unsafe { buffer.handle.buffer.write_bytes(0x5a, 64) };
    for capacity in [128, 32, 96] {
        assert_eq!(
            unsafe { rcutils_uint8_array_resize(&mut buffer.handle, capacity) },
            RCL_RET_OK as i32
        );
        let bytes = unsafe { std::slice::from_raw_parts(buffer.handle.buffer, capacity) };
        let prefix = if capacity == 128 { 64 } else { 32 };
        assert!(bytes[..prefix].iter().all(|&byte| byte == 0x5a));
        assert!(bytes[prefix..].iter().all(|&byte| byte == 0));
        assert_eq!(buffer.capacity(), capacity);
        assert_eq!(buffer.handle.buffer as usize % 16, 0);
    }
}

#[test]
fn zero_allocation_and_size_overflow_follow_the_native_allocator_contract() {
    let mut owner = Box::new(unsafe { rcutils_get_default_allocator() });
    let state = (&mut *owner as *mut rcutils_allocator_t).cast();
    let data = unsafe { zero_allocate(3, 7, state) };
    assert!(!data.is_null());
    assert!(unsafe { std::slice::from_raw_parts(data.cast::<u8>(), 21) }
        .iter()
        .all(|&byte| byte == 0));
    assert!(unsafe { zero_allocate(usize::MAX, 2, state) }.is_null());
    assert!(unsafe { reallocate(data, usize::MAX, state) }.is_null());
    assert!(unsafe { allocate(usize::MAX, state) }.is_null());
    assert!(unsafe { std::slice::from_raw_parts(data.cast::<u8>(), 21) }
        .iter()
        .all(|&byte| byte == 0));
    unsafe { deallocate(data, state) };
    unsafe { deallocate(std::ptr::null_mut(), state) };
}

#[test]
fn failed_growth_is_freed_once_by_rcutils_and_leaves_a_finalizable_buffer() {
    static FREES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    unsafe extern "C" fn fail_growth(_: *mut c_void, _: usize, _: *mut c_void) -> *mut c_void {
        std::ptr::null_mut()
    }
    unsafe extern "C" fn count_free(data: *mut c_void, _: *mut c_void) {
        if !data.is_null() {
            FREES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        let allocator = unsafe { rcutils_get_default_allocator() };
        unsafe { allocator.deallocate.unwrap()(data, allocator.state) };
    }
    let mut allocator = unsafe { rcutils_get_default_allocator() };
    allocator.reallocate = Some(fail_growth);
    allocator.deallocate = Some(count_free);
    FREES.store(0, std::sync::atomic::Ordering::Relaxed);
    let mut buffer = SerializedMessageBuffer::with_allocator(32, allocator).unwrap();
    assert_eq!(
        unsafe { rcutils_uint8_array_resize(&mut buffer.handle, 64) },
        RCL_RET_BAD_ALLOC as i32
    );
    unsafe { rcutils_reset_error() };
    assert!(buffer.handle.buffer.is_null());
    assert_eq!(buffer.capacity(), 0);
    drop(buffer);
    assert_eq!(FREES.load(std::sync::atomic::Ordering::Relaxed), 1);
}

#[test]
fn allocator_state_and_storage_survive_moving_the_buffer_to_another_thread() {
    let buffer = SerializedMessageBuffer::with_capacity(64).unwrap();
    unsafe { buffer.handle.buffer.write_bytes(0x5a, 64) };
    std::thread::spawn(move || {
        let mut buffer = buffer;
        assert_eq!(
            unsafe { rcutils_uint8_array_resize(&mut buffer.handle, 128) },
            RCL_RET_OK as i32
        );
        let bytes = unsafe { std::slice::from_raw_parts(buffer.handle.buffer, 128) };
        assert!(bytes[..64].iter().all(|&byte| byte == 0x5a));
        assert!(bytes[64..].iter().all(|&byte| byte == 0));
    })
    .join()
    .unwrap();
}
