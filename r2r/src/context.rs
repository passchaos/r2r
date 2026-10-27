use parking_lot::{Condvar, Mutex};
use std::{
    ffi::CString,
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{Arc, OnceLock},
};

use crate::{error::*, log_guard, utils::mark_logging_initialized};
use r2r_rcl::*;

/// Options used to initialize the process-wide ROS context.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContextOptions {
    arguments: Option<Vec<String>>,
    domain_id: Option<usize>,
}

impl ContextOptions {
    /// Replaces the process command-line arguments passed to `rcl_init`.
    ///
    /// Include an executable name as the first item when matching normal process argv semantics.
    pub fn arguments<I, S>(mut self, arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.arguments = Some(arguments.into_iter().map(Into::into).collect());
        self
    }

    /// Overrides the ROS domain id for this context.
    pub const fn domain_id(mut self, domain_id: usize) -> Self {
        self.domain_id = Some(domain_id);
        self
    }

    fn resolve(self) -> Result<ContextConfiguration> {
        let arguments = match self.arguments {
            Some(arguments) => arguments,
            None => std::env::args_os()
                .map(|argument| {
                    argument
                        .into_string()
                        .map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)
                })
                .collect::<Result<Vec<_>>>()?,
        };
        Ok(ContextConfiguration {
            arguments,
            domain_id: self.domain_id,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ContextConfiguration {
    arguments: Vec<String>,
    domain_id: Option<usize>,
}

/// A ROS context. Needed to create nodes etc.
#[derive(Debug)]
pub struct Context {
    pub(crate) context_handle: Arc<Mutex<ContextHandle>>,
    shutdown_changed: Arc<Condvar>,
}

// Memory corruption (double free and others) was observed creating multiple
// `Context` objects in a single thread
//
// To reproduce, run the tests from `tokio_testing` or `tokio_test_raw`
// without this OnceLock

static CONTEXT: OnceLock<Mutex<WeakContextHandle>> = OnceLock::new();
static CONTEXT_SHUTDOWN_CHANGED: OnceLock<Arc<Condvar>> = OnceLock::new();

type WeakContextHandle = std::sync::Weak<Mutex<ContextHandle>>;

impl Context {
    /// Create a ROS context.
    pub fn create() -> Result<Context> {
        Self::create_with_options(ContextOptions::default())
    }

    /// Creates a ROS context with explicit process arguments and initialization options.
    pub fn create_with_options(options: ContextOptions) -> Result<Context> {
        let configuration = options.resolve()?;
        let shutdown_changed =
            Arc::clone(CONTEXT_SHUTDOWN_CHANGED.get_or_init(|| Arc::new(Condvar::new())));
        let mut shared = CONTEXT
            .get_or_init(|| Mutex::new(WeakContextHandle::new()))
            .lock();
        if let Some(context_handle) = shared.upgrade() {
            let context = context_handle.lock();
            if !unsafe { rcl_context_is_valid(context.handle.as_ref()) } {
                return Err(Error::RCL_RET_ALREADY_SHUTDOWN);
            }
            if context.configuration != configuration {
                return Err(Error::RCL_RET_ALREADY_INIT);
            }
            drop(context);
            return Ok(Context {
                context_handle,
                shutdown_changed,
            });
        }

        let context_handle = Arc::new(Mutex::new(ContextHandle::create(configuration)?));
        *shared = Arc::downgrade(&context_handle);
        Ok(Context {
            context_handle,
            shutdown_changed,
        })
    }

    /// Check if the ROS context is valid.
    ///
    /// (This is abbreviated to rcl_ok() in the other bindings.)
    pub fn is_valid(&self) -> bool {
        let mut ctx = self.context_handle.lock();
        unsafe { rcl_context_is_valid(ctx.as_mut()) }
    }

    /// Invalidates this context and every ROS entity created from it.
    ///
    /// This wakes waits using the context. Native resources remain allocated until their owning
    /// nodes and the final [`Context`] clone are dropped. Calling this method more than once is
    /// harmless.
    pub fn shutdown(&self) -> Result<()> {
        let result = self.context_handle.lock().request_shutdown();
        self.shutdown_changed.notify_all();
        result
    }

    pub(crate) fn wait_for_shutdown_or_timeout(&self, timeout: std::time::Duration) -> Result<()> {
        let mut context = self.context_handle.lock();
        if !unsafe { rcl_context_is_valid(context.handle.as_ref()) } {
            return Err(Error::RCL_RET_NOT_INIT);
        }
        self.shutdown_changed.wait_while_for(
            &mut context,
            |context| unsafe { rcl_context_is_valid(context.handle.as_ref()) },
            timeout,
        );
        if unsafe { rcl_context_is_valid(context.handle.as_ref()) } {
            Ok(())
        } else {
            Err(Error::RCL_RET_NOT_INIT)
        }
    }
}

impl Clone for Context {
    fn clone(&self) -> Self {
        Self {
            context_handle: Arc::clone(&self.context_handle),
            shutdown_changed: Arc::clone(&self.shutdown_changed),
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
        self.context_handle.lock().finalize();
    }
}

#[derive(Debug)]
pub struct ContextHandle {
    handle: Box<rcl_context_t>,
    initialized: bool,
    configuration: ContextConfiguration,
    shutdown_wakes: Vec<std::sync::Weak<dyn ShutdownWake>>,
}

pub(crate) trait ShutdownWake: Send + Sync {
    fn wake_shutdown(&self);
}

struct InitOptions {
    value: rcl_init_options_t,
    initialized: bool,
}

impl InitOptions {
    fn new(domain_id: Option<usize>) -> Result<Self> {
        let allocator = unsafe { rcutils_get_default_allocator() };
        let mut result = Self {
            value: unsafe { rcl_get_zero_initialized_init_options() },
            initialized: false,
        };
        let ret = unsafe { rcl_init_options_init(&mut result.value, allocator) };
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        result.initialized = true;
        if let Some(domain_id) = domain_id {
            let ret = unsafe { rcl_init_options_set_domain_id(&mut result.value, domain_id) };
            if ret != RCL_RET_OK as i32 {
                return Err(Error::from_rcl_error(ret));
            }
        }
        Ok(result)
    }

    fn fini(&mut self) -> Result<()> {
        if !self.initialized {
            return Ok(());
        }
        self.initialized = false;
        let ret = unsafe { rcl_init_options_fini(&mut self.value) };
        if ret == RCL_RET_OK as i32 {
            Ok(())
        } else {
            Err(Error::from_rcl_error(ret))
        }
    }
}

impl Drop for InitOptions {
    fn drop(&mut self) {
        if let Err(err) = self.fini() {
            log::error!("could not finalize ROS context options: {err}");
        }
    }
}

// SAFETY: all access to a shared ContextHandle is serialized by Context's mutex.
unsafe impl Send for ContextHandle {}

impl ContextHandle {
    fn create(configuration: ContextConfiguration) -> Result<Self> {
        let mut context = unsafe { Box::new(rcl_get_zero_initialized_context()) };
        let args = configuration
            .arguments
            .iter()
            .map(|arg| CString::new(arg.as_str()).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT))
            .collect::<Result<Vec<_>>>()?;
        let c_args = args.iter().map(|arg| arg.as_ptr()).collect::<Vec<_>>();
        let argc = std::os::raw::c_int::try_from(c_args.len())
            .map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let argv = if c_args.is_empty() {
            std::ptr::null()
        } else {
            c_args.as_ptr()
        };

        let mut options = InitOptions::new(configuration.domain_id)?;
        let ret = unsafe { rcl_init(argc, argv, &options.value, context.as_mut()) };
        let fini_result = options.fini();
        if ret != RCL_RET_OK as i32 {
            return Err(Error::from_rcl_error(ret));
        }
        if let Err(err) = fini_result {
            unsafe {
                rcl_shutdown(context.as_mut());
                rcl_context_fini(context.as_mut());
            }
            return Err(err);
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
            let ret = rcl_logging_configure(
                &context.as_ref().global_arguments,
                &rcutils_get_default_allocator(),
            );
            if ret == RCL_RET_OK as i32 {
                mark_logging_initialized();
            }
            ret
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
            configuration,
            shutdown_wakes: Vec::new(),
        })
    }

    fn request_shutdown(&mut self) -> Result<()> {
        if !self.initialized || !unsafe { rcl_context_is_valid(self.handle.as_mut()) } {
            return Ok(());
        }

        // Signal registered waits while native guard handles still belong to a valid context.
        // Registrations are weak; node/waker lifetimes do not form a context reference cycle.
        for wake in self.shutdown_wakes.iter().filter_map(|wake| wake.upgrade()) {
            wake.wake_shutdown();
        }
        let ret = unsafe { rcl_shutdown(self.handle.as_mut()) };
        if ret == RCL_RET_OK as i32 {
            self.shutdown_wakes.clear();
            Ok(())
        } else {
            Err(Error::from_rcl_error(ret))
        }
    }

    pub(crate) fn register_shutdown_wake(&mut self, wake: std::sync::Weak<dyn ShutdownWake>) {
        self.shutdown_wakes.retain(|wake| wake.strong_count() > 0);
        self.shutdown_wakes.push(wake);
    }

    fn finalize(&mut self) {
        if !self.initialized {
            return;
        }

        if let Err(err) = self.request_shutdown() {
            log::error!("could not shut down ROS context: {err}");
        }
        let ret = unsafe { rcl_context_fini(self.handle.as_mut()) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize ROS context: {}", Error::from_rcl_error(ret));
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
        self.finalize();
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

    #[test]
    fn context_cloning_does_not_require_the_global_registry_lock() {
        let context = Context::create().unwrap();
        let registry = CONTEXT.get().unwrap().lock();

        let cloned = context.clone();

        assert!(Arc::ptr_eq(&context.context_handle, &cloned.context_handle));
        drop(cloned);
        drop(registry);
    }
}
