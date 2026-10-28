#[repr(C)]
#[derive(Default)]
pub(crate) struct NativeStats {
    pub(crate) malloc_calls: u64,
    pub(crate) calloc_calls: u64,
    pub(crate) realloc_calls: u64,
    pub(crate) free_calls: u64,
    pub(crate) null_free_calls: u64,
    pub(crate) aligned_calls: u64,
    pub(crate) failed_allocations: u64,
    pub(crate) requested_bytes: u64,
    pub(crate) size_overflows: u64,
    pub(crate) counter_overflows: u64,
    pub(crate) user_cpu_ns: u64,
    pub(crate) system_cpu_ns: u64,
    pub(crate) voluntary_context_switches: u64,
    pub(crate) involuntary_context_switches: u64,
    pub(crate) process_max_rss_kib: u64,
}
pub(crate) struct Probe {
    pub(crate) begin: unsafe extern "C" fn() -> i32,
    pub(crate) end: unsafe extern "C" fn(*mut NativeStats) -> i32,
}
impl Probe {
    pub(crate) fn load() -> Result<Option<Self>, Box<dyn std::error::Error>> {
        #[cfg(target_os = "linux")]
        {
            #[link(name = "dl")]
            extern "C" {
                fn dlsym(
                    handle: *mut std::ffi::c_void, name: *const std::ffi::c_char,
                ) -> *mut std::ffi::c_void;
            }
            unsafe {
                let version = dlsym(std::ptr::null_mut(), c"r2r_bench_probe_version".as_ptr());
                if version.is_null() {
                    return Ok(None);
                }
                let version: unsafe extern "C" fn() -> i32 = std::mem::transmute(version);
                let begin = dlsym(std::ptr::null_mut(), c"r2r_bench_probe_begin".as_ptr());
                let end = dlsym(std::ptr::null_mut(), c"r2r_bench_probe_end".as_ptr());
                if version() != 1
                    || begin.is_null()
                    || end.is_null()
                    || std::mem::size_of::<NativeStats>() != 120
                {
                    return Err("incompatible benchmark probe ABI".into());
                }
                Ok(Some(Self {
                    begin: std::mem::transmute::<
                        *mut std::ffi::c_void,
                        unsafe extern "C" fn() -> i32,
                    >(begin),
                    end: std::mem::transmute::<
                        *mut std::ffi::c_void,
                        unsafe extern "C" fn(*mut NativeStats) -> i32,
                    >(end),
                }))
            }
        }
        #[cfg(not(target_os = "linux"))]
        Ok(None)
    }
}
