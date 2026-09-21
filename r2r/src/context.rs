use parking_lot::Mutex;
use std::{
    ffi::CString,
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{Arc, OnceLock},
};

use crate::{error::*, log_guard};
use r2r_rcl::*;

/// A ROS context. Needed to create nodes etc.
#[derive(Debug)]
pub struct Context {
    pub(crate) context_handle: Arc<Mutex<ContextHandle>>,
}

// Memory corruption (double free and others) was observed creating multiple
// `Context` objects in a single thread
//
// To reproduce, run the tests from `tokio_testing` or `tokio_test_raw`
// without this OnceLock

static CONTEXT: OnceLock<Mutex<WeakContextHandle>> = OnceLock::new();

type WeakContextHandle = std::sync::Weak<Mutex<ContextHandle>>;

impl Context {
    /// Create a ROS context.
    pub fn create() -> Result<Context> {
        let mut shared = CONTEXT
            .get_or_init(|| Mutex::new(WeakContextHandle::new()))
            .lock();
        if let Some(context_handle) = shared.upgrade() {
            return Ok(Context { context_handle });
        }

        let context_handle = Arc::new(Mutex::new(ContextHandle::create()?));
        *shared = Arc::downgrade(&context_handle);
        Ok(Context { context_handle })
    }

    /// Check if the ROS context is valid.
    ///
    /// (This is abbreviated to rcl_ok() in the other bindings.)
    pub fn is_valid(&self) -> bool {
        let mut ctx = self.context_handle.lock();
        unsafe { rcl_context_is_valid(ctx.as_mut()) }
    }
}

impl Clone for Context {
    fn clone(&self) -> Self {
        let _shared = CONTEXT.get().unwrap().lock();
        Self {
            context_handle: Arc::clone(&self.context_handle),
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if Arc::strong_count(&self.context_handle) != 1 {
            return;
        }

        let mut shared = CONTEXT.get().unwrap().lock();
        // A concurrent clone may have been made before the global lock was acquired.
        if Arc::strong_count(&self.context_handle) != 1 {
            return;
        }

        *shared = WeakContextHandle::new();
        self.context_handle.lock().shutdown();
    }
}

#[derive(Debug)]
pub struct ContextHandle {
    handle: Box<rcl_context_t>,
    initialized: bool,
}

// SAFETY: all access to a shared ContextHandle is serialized by Context's mutex.
unsafe impl Send for ContextHandle {}

impl ContextHandle {
    fn create() -> Result<Self> {
        let mut context = unsafe { Box::new(rcl_get_zero_initialized_context()) };
        let args = std::env::args()
            .map(|arg| CString::new(arg).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT))
            .collect::<Result<Vec<_>>>()?;
        let c_args = args.iter().map(|arg| arg.as_ptr()).collect::<Vec<_>>();

        let allocator = unsafe { rcutils_get_default_allocator() };
        let mut options = unsafe { rcl_get_zero_initialized_init_options() };
        let ret = unsafe { rcl_init_options_init(&mut options, allocator) };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }

        let ret = unsafe {
            rcl_init(
                c_args.len() as std::os::raw::c_int,
                c_args.as_ptr(),
                &options,
                context.as_mut(),
            )
        };
        let fini_ret = unsafe { rcl_init_options_fini(&mut options) };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        if fini_ret != RCL_RET_OK as i32 {
            unsafe {
                rcl_shutdown(context.as_mut());
                rcl_context_fini(context.as_mut());
            }
            return Err(Error::from_rcl_error(fini_ret));
        }
        if !unsafe { rcl_context_is_valid(context.as_mut()) } {
            unsafe {
                rcl_shutdown(context.as_mut());
                rcl_context_fini(context.as_mut());
            }
            return Err(Error::RCL_RET_ERROR);
        }

        let logging_ret = unsafe {
            let _guard = log_guard();
            rcl_logging_configure(
                &context.as_ref().global_arguments,
                &rcutils_get_default_allocator(),
            )
        };
        if logging_ret != RCL_RET_OK as i32 {
            unsafe {
                rcl_shutdown(context.as_mut());
                rcl_context_fini(context.as_mut());
            }
            return Err(Error::from_rcl_error(logging_ret));
        }

        Ok(Self {
            handle: context,
            initialized: true,
        })
    }

    fn shutdown(&mut self) {
        if !self.initialized {
            return;
        }

        unsafe {
            rcl_shutdown(self.handle.as_mut());
            rcl_context_fini(self.handle.as_mut());
        }
        self.initialized = false;
    }
}

impl Deref for ContextHandle {
    type Target = Box<rcl_context_t>;

    fn deref(&self) -> &Box<rcl_context_t> {
        &self.handle
    }
}

impl DerefMut for ContextHandle {
    fn deref_mut(&mut self) -> &mut Box<rcl_context_t> {
        &mut self.handle
    }
}

impl Drop for ContextHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_drop() {
        {
            let ctx = Context::create().unwrap();
            assert!(ctx.is_valid());
        }
        {
            let ctx = Context::create().unwrap();
            assert!(ctx.is_valid());
        }
    }
}
