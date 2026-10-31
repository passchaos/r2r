//! Interface libraries are opened on first use and retained for native metadata lifetime.
use libloading::Library;
use std::{
    collections::HashMap,
    ffi::CStr,
    fmt,
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Copy)]
pub enum Kind {
    Generator,
    Typesupport,
    Introspection,
}
impl Kind {
    fn suffix(self) -> &'static str {
        match self {
            Self::Generator => "rosidl_generator_c",
            Self::Typesupport => "rosidl_typesupport_c",
            Self::Introspection => "rosidl_typesupport_introspection_c",
        }
    }
}
#[derive(Debug)]
pub struct LoadError {
    pub library: String,
    pub symbol: Option<String>,
    pub detail: String,
}
impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(symbol) = &self.symbol {
            write!(f, "{}::{symbol}: {}", self.library, self.detail)
        } else {
            write!(f, "{}: {}", self.library, self.detail)
        }
    }
}
impl std::error::Error for LoadError {}

#[derive(Default)]
struct Libraries {
    opened: Mutex<HashMap<String, &'static Library>>,
}
impl Libraries {
    fn get(&self, name: &str) -> Result<&'static Library, LoadError> {
        if let Some(library) = self.opened.lock().unwrap().get(name).copied() {
            return Ok(library);
        }
        // Open outside our mutex: native constructors may invoke other loaders.
        // A duplicate open only adds a loader reference; the winner remains retained.
        let opened = unsafe { Library::new(name) }.map_err(|error| LoadError {
            library: name.into(),
            symbol: None,
            detail: error.to_string(),
        })?;
        let mut cache = self.opened.lock().unwrap();
        if let Some(library) = cache.get(name).copied() {
            drop(cache);
            drop(opened);
            return Ok(library);
        }
        let retained = Box::leak(Box::new(opened));
        cache.insert(name.into(), retained);
        Ok(retained)
    }
}
static LIBRARIES: OnceLock<Libraries> = OnceLock::new();

/// Resolves a generated interface function. Successful handles remain process-owned.
///
/// # Safety
/// F must be the exact native function-pointer type of symbol in this library.
#[cold]
#[inline(never)]
pub unsafe fn symbol<F: Copy>(package: &str, kind: Kind, name: &CStr) -> Result<F, LoadError> {
    let stem = format!("{package}__{}", kind.suffix());
    let filename = if cfg!(target_os = "windows") {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    };
    let library = LIBRARIES.get_or_init(Libraries::default).get(&filename)?;
    let value =
        unsafe { library.get::<F>(name.to_bytes_with_nul()) }.map_err(|error| LoadError {
            library: filename,
            symbol: Some(name.to_string_lossy().into_owned()),
            detail: error.to_string(),
        })?;
    Ok(*value)
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::{path::PathBuf, process::Command, sync::Arc};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "r2r-library-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        fn library(&self) -> PathBuf {
            self.0.join("interface.so")
        }
        fn compile(&self) {
            let source = self.0.join("interface.c");
            std::fs::write(&source,"#include <stdint.h>\n#include <stdlib.h>\nstatic uint64_t descriptor=73;\nconst uint64_t *support(void){return &descriptor;}\nuint64_t *create(void){uint64_t *p=malloc(sizeof(uint64_t));if(p)*p=91;return p;}\nvoid destroy(uint64_t *p){free(p);}\n").unwrap();
            assert!(Command::new("cc")
                .args(["-shared", "-fPIC", "-O2", "-Wall", "-Werror"])
                .arg(source)
                .arg("-o")
                .arg(self.library())
                .status()
                .unwrap()
                .success());
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn native_library_failed_open_is_retryable_and_cached_handle_keeps_metadata_alive() {
        let fixture = Fixture::new();
        let name = fixture.library().to_str().unwrap().to_owned();
        let descriptor;
        {
            let libraries = Libraries::default();
            assert!(libraries.get(&name).is_err());
            fixture.compile();
            let first = libraries.get(&name).unwrap();
            let second = libraries.get(&name).unwrap();
            assert!(std::ptr::eq(first, second));
            let operation =
                unsafe { first.get::<unsafe extern "C" fn() -> *const u64>(b"support\0") }.unwrap();
            descriptor = unsafe { operation() };
            assert!(unsafe { first.get::<unsafe extern "C" fn()>(b"missing_symbol\0") }.is_err());
        }
        drop(fixture);
        assert_eq!(unsafe { *descriptor }, 73, "metadata was unloaded when the resolver retired");
    }
    #[test]
    fn native_library_concurrent_lookup_and_native_allocation_preserve_shared_lifetime() {
        let fixture = Fixture::new();
        fixture.compile();
        let name = fixture.library().to_str().unwrap().to_owned();
        let libraries = Arc::new(Libraries::default());
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let libraries = libraries.clone();
                let name = name.clone();
                std::thread::spawn(move || {
                    let library = libraries.get(&name).unwrap();
                    let create =
                        unsafe { library.get::<unsafe extern "C" fn() -> *mut u64>(b"create\0") }
                            .unwrap();
                    let destroy =
                        unsafe { library.get::<unsafe extern "C" fn(*mut u64)>(b"destroy\0") }
                            .unwrap();
                    for _ in 0..100 {
                        let value = unsafe { create() };
                        assert!(!value.is_null());
                        assert_eq!(unsafe { *value }, 91);
                        unsafe { destroy(value) };
                    }
                    library as *const Library as usize
                })
            })
            .collect();
        let handles: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert!(handles.iter().all(|handle| *handle == handles[0]));
        assert_eq!(libraries.opened.lock().unwrap().len(), 1);
    }
}
