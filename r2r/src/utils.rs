use parking_lot::{Mutex, MutexGuard};
use r2r_rcl::*;

use crate::{Error, Result};
use std::{
    ffi::CString,
    sync::atomic::{AtomicBool, Ordering},
};

use lazy_static::lazy_static;

lazy_static! {
    static ref LOG_GUARD: Mutex<()> = Mutex::new(());
}

pub(crate) fn log_guard() -> MutexGuard<'static, ()> {
    LOG_GUARD.lock()
}

pub(crate) fn mark_logging_initialized() {
    IS_INIT.store(true, Ordering::Relaxed);
}

// for some hidden reason g_rcutils_logging_initialized
// is not found on windows even when rcutils.lib is linked
// as a work around using onwned is_init
static IS_INIT: AtomicBool = AtomicBool::new(false);

fn ensure_logging_initialized() -> Result<()> {
    let is_init = if cfg!(target_os = "windows") {
        IS_INIT.load(Ordering::Relaxed)
    } else {
        unsafe { g_rcutils_logging_initialized }
    };
    if is_init {
        return Ok(());
    }

    let ret = unsafe { rcutils_logging_initialize() };
    if ret != RCUTILS_RET_OK as i32 {
        return Err(rcutils_logging_error(ret));
    }
    IS_INIT.store(true, Ordering::Relaxed);
    Ok(())
}

fn rcutils_logging_error(ret: rcutils_ret_t) -> Error {
    unsafe { rcutils_reset_error() };
    match ret as u32 {
        RCUTILS_RET_INVALID_ARGUMENT => Error::RCL_RET_INVALID_ARGUMENT,
        RCUTILS_RET_BAD_ALLOC => Error::RCL_RET_BAD_ALLOC,
        _ => Error::RcutilsLoggingError(ret),
    }
}

fn logger_name(name: &str) -> Result<CString> {
    CString::new(name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)
}

/// Don't call this directly, use the logging macros instead.
#[doc(hidden)]
pub fn log(msg: &str, logger_name: &str, file: &str, line: u32, severity: LogSeverity) {
    let _guard = log_guard();
    if let Err(err) = ensure_logging_initialized() {
        log::error!("could not create logging system: {err}");
        return;
    }
    // currently not possible to get function name in rust.
    // see https://github.com/rust-lang/rfcs/pull/2818
    let function = CString::default();
    let file = cstring_without_nul(file);
    let location = rcutils_log_location_t {
        function_name: function.as_ptr(),
        file_name: file.as_ptr(),
        line_number: line as usize,
    };
    let format = CString::new("%s").expect("static format contains no NUL");
    let logger_name = cstring_without_nul(logger_name);
    let message = cstring_without_nul(msg);
    let severity = severity.to_native();
    unsafe {
        rcutils_log(
            &location,
            severity as i32,
            logger_name.as_ptr(),
            format.as_ptr(),
            message.as_ptr(),
        );
    }
}

fn cstring_without_nul(value: &str) -> CString {
    let mut bytes = value.as_bytes().to_vec();
    bytes.retain(|byte| *byte != 0);
    // Removing all interior NUL bytes makes this infallible.
    CString::new(bytes).expect("interior NUL bytes were removed")
}

/// Logging severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LogSeverity {
    Unset,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

impl LogSeverity {
    fn to_native(self) -> RCUTILS_LOG_SEVERITY {
        use RCUTILS_LOG_SEVERITY::*;
        match self {
            LogSeverity::Unset => RCUTILS_LOG_SEVERITY_UNSET,
            LogSeverity::Debug => RCUTILS_LOG_SEVERITY_DEBUG,
            LogSeverity::Info => RCUTILS_LOG_SEVERITY_INFO,
            LogSeverity::Warn => RCUTILS_LOG_SEVERITY_WARN,
            LogSeverity::Error => RCUTILS_LOG_SEVERITY_ERROR,
            LogSeverity::Fatal => RCUTILS_LOG_SEVERITY_FATAL,
        }
    }

    fn from_native(level: i32) -> Result<Self> {
        match level {
            value if value == RCUTILS_LOG_SEVERITY::RCUTILS_LOG_SEVERITY_UNSET as i32 => {
                Ok(Self::Unset)
            }
            value if value == RCUTILS_LOG_SEVERITY::RCUTILS_LOG_SEVERITY_DEBUG as i32 => {
                Ok(Self::Debug)
            }
            value if value == RCUTILS_LOG_SEVERITY::RCUTILS_LOG_SEVERITY_INFO as i32 => {
                Ok(Self::Info)
            }
            value if value == RCUTILS_LOG_SEVERITY::RCUTILS_LOG_SEVERITY_WARN as i32 => {
                Ok(Self::Warn)
            }
            value if value == RCUTILS_LOG_SEVERITY::RCUTILS_LOG_SEVERITY_ERROR as i32 => {
                Ok(Self::Error)
            }
            value if value == RCUTILS_LOG_SEVERITY::RCUTILS_LOG_SEVERITY_FATAL as i32 => {
                Ok(Self::Fatal)
            }
            _ => Err(Error::RcutilsLoggingError(level)),
        }
    }
}

/// Sets the explicit severity level for a named logger.
///
/// [`LogSeverity::Unset`] restores inheritance from the nearest configured ancestor.
pub fn set_logger_level(name: &str, severity: LogSeverity) -> Result<()> {
    let name = logger_name(name)?;
    let _guard = log_guard();
    ensure_logging_initialized()?;
    let ret =
        unsafe { rcutils_logging_set_logger_level(name.as_ptr(), severity.to_native() as i32) };
    if ret == RCUTILS_RET_OK as i32 {
        Ok(())
    } else {
        Err(rcutils_logging_error(ret))
    }
}

/// Returns the explicitly configured severity for a named logger.
pub fn logger_level(name: &str) -> Result<LogSeverity> {
    let name = logger_name(name)?;
    let _guard = log_guard();
    ensure_logging_initialized()?;
    let level = unsafe { rcutils_logging_get_logger_level(name.as_ptr()) };
    if level < 0 {
        unsafe { rcutils_reset_error() };
        return Err(Error::RcutilsLoggingError(level));
    }
    LogSeverity::from_native(level)
}

/// Returns a logger's effective severity after applying ancestor and default levels.
pub fn logger_effective_level(name: &str) -> Result<LogSeverity> {
    let name = logger_name(name)?;
    let _guard = log_guard();
    ensure_logging_initialized()?;
    let level = unsafe { rcutils_logging_get_logger_effective_level(name.as_ptr()) };
    if level < 0 {
        unsafe { rcutils_reset_error() };
        return Err(Error::RcutilsLoggingError(level));
    }
    LogSeverity::from_native(level)
}

/// Returns whether a named logger will emit a message at `severity`.
pub fn logger_is_enabled(name: &str, severity: LogSeverity) -> Result<bool> {
    let name = logger_name(name)?;
    let _guard = log_guard();
    ensure_logging_initialized()?;
    Ok(
        unsafe {
            rcutils_logging_logger_is_enabled_for(name.as_ptr(), severity.to_native() as i32)
        },
    )
}

// A helper macro to log the message.
#[doc(hidden)]
#[macro_export]
macro_rules! __impl_log {
    ($logger_name:expr, $msg:expr, $file:expr, $line:expr, $severity:expr) => {{
        let logger_name = $logger_name;
        let severity = $severity;
        if $crate::logger_is_enabled(logger_name, severity).unwrap_or(true) {
            $crate::log(&std::fmt::format($msg), logger_name, $file, $line, severity);
        }
    }};
}

/// Debug log message.
#[macro_export]
macro_rules! log_debug {
    ($logger_name:expr, $($args:tt)*) => {{
        $crate::__impl_log!($logger_name, format_args!($($args)*),
                            file!(), line!(), $crate::LogSeverity::Debug)
    }}
}

/// Info log message.
#[macro_export]
macro_rules! log_info {
    ($logger_name:expr, $($args:tt)*) => {{
        $crate::__impl_log!($logger_name, format_args!($($args)*),
                            file!(), line!(), $crate::LogSeverity::Info)
    }}
}

/// Warning log message.
#[macro_export]
macro_rules! log_warn {
    ($logger_name:expr, $($args:tt)*) => {{
        $crate::__impl_log!($logger_name, format_args!($($args)*),
                            file!(), line!(), $crate::LogSeverity::Warn)
    }}
}

/// Error log message.
#[macro_export]
macro_rules! log_error {
    ($logger_name:expr, $($args:tt)*) => {{
        $crate::__impl_log!($logger_name, format_args!($($args)*),
                            file!(), line!(), $crate::LogSeverity::Error)
    }}
}

/// Fatal log message.
#[macro_export]
macro_rules! log_fatal {
    ($logger_name:expr, $($args:tt)*) => {{
        $crate::__impl_log!($logger_name, format_args!($($args)*),
                            file!(), line!(), $crate::LogSeverity::Fatal)
    }}
}

/// Causes compile time error if `use_sim_time` is unsupported.
#[cfg(r2r__rosgraph_msgs__msg__Clock)]
#[macro_export]
macro_rules! assert_compiled_with_use_sim_time_support {
    () => {};
}

/// Causes compile time error if `use_sim_time` is unsupported.
#[cfg(not(r2r__rosgraph_msgs__msg__Clock))]
#[macro_export]
macro_rules! assert_compiled_with_use_sim_time_support {
    () => {
        compile_error!("assert_compiled_with_use_sim_time_support failed: 'rosgraph_msgs' dependency is missing!");
    };
}

#[test]
fn test_log() {
    log_debug!("log_test", "debug msg");
    log_info!("log_test", "info msg");
    log_warn!("log_test", "warn msg");
    log_error!("log_test", "error msg");
    log_fatal!("log_test", "fatal msg");
}

#[test]
fn log_fields_with_nul_bytes_do_not_panic() {
    log("message\0suffix", "logger\0suffix", "file\0suffix", 1, LogSeverity::Info);
}

#[test]
fn logger_levels_inherit_and_disabled_macros_do_not_evaluate_arguments() {
    let parent = format!("r2r_test_{}", uuid::Uuid::new_v4().simple());
    let child = format!("{parent}.child");
    set_logger_level(&parent, LogSeverity::Warn).unwrap();
    assert_eq!(logger_level(&child).unwrap(), LogSeverity::Unset);
    assert_eq!(logger_effective_level(&child).unwrap(), LogSeverity::Warn);
    assert!(!logger_is_enabled(&child, LogSeverity::Info).unwrap());
    assert!(logger_is_enabled(&child, LogSeverity::Error).unwrap());

    let evaluated = AtomicBool::new(false);
    log_debug!(&child, "{}", {
        evaluated.store(true, Ordering::Relaxed);
        "unused"
    });
    assert!(!evaluated.load(Ordering::Relaxed));

    set_logger_level(&child, LogSeverity::Debug).unwrap();
    assert_eq!(logger_level(&child).unwrap(), LogSeverity::Debug);
    assert_eq!(logger_effective_level(&child).unwrap(), LogSeverity::Debug);
    assert!(logger_is_enabled(&child, LogSeverity::Debug).unwrap());

    set_logger_level(&child, LogSeverity::Unset).unwrap();
    set_logger_level(&parent, LogSeverity::Unset).unwrap();
    assert!(matches!(
        set_logger_level("invalid\0logger", LogSeverity::Info),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
}
