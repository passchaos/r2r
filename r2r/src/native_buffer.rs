//! One-slot native storage recycling shared by action and subscription delivery.

use crate::{msg_types::VoidPtr, Result};
use parking_lot::Mutex;
use std::{
    fmt,
    ops::{Deref, DerefMut},
    sync::Arc,
};

pub(crate) struct NativeBuffer<N> {
    pub(crate) slot: Arc<Mutex<Option<N>>>,
}

impl<N> fmt::Debug for NativeBuffer<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeBuffer")
            .finish_non_exhaustive()
    }
}

impl<N> Clone for NativeBuffer<N> {
    fn clone(&self) -> Self {
        Self {
            slot: Arc::clone(&self.slot),
        }
    }
}

impl<N> Default for NativeBuffer<N> {
    fn default() -> Self {
        Self {
            slot: Arc::new(Mutex::new(None)),
        }
    }
}

impl<N> NativeBuffer<N> {
    pub(crate) fn checkout(&self, create: impl FnOnce() -> Result<N>) -> Result<NativeLease<N>> {
        let cached = { self.slot.lock().take() };
        let native = match cached {
            Some(native) => native,
            None => create()?,
        };
        Ok(NativeLease {
            native: Some(native),
            buffer: self.clone(),
        })
    }

    fn recycle(&self, native: N) {
        // A concurrent return keeps the most recently returned storage. Retire the displaced
        // value after releasing the lock so arbitrary native finalizers can safely reenter.
        let retired = { self.slot.lock().replace(native) };
        drop(retired);
    }

    #[cfg(test)]
    pub(crate) fn is_available(&self) -> bool {
        self.slot.lock().is_some()
    }

    #[cfg(test)]
    pub(crate) fn inspect_available<R>(&self, inspect: impl FnOnce(&N) -> R) -> Option<R> {
        self.slot.lock().as_ref().map(inspect)
    }
}

pub(crate) struct NativeLease<N> {
    native: Option<N>,
    buffer: NativeBuffer<N>,
}

impl<N> NativeLease<N> {
    /// Drops this value instead of returning it to the cache.
    pub(crate) fn retire(mut self) {
        drop(self.native.take());
    }
}

impl<N> Deref for NativeLease<N> {
    type Target = N;

    fn deref(&self) -> &Self::Target {
        self.native.as_ref().unwrap()
    }
}

impl<N> DerefMut for NativeLease<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.native.as_mut().unwrap()
    }
}

impl<N> VoidPtr for NativeLease<N>
where
    N: VoidPtr,
{
    fn void_ptr(&self) -> *const std::os::raw::c_void {
        self.deref().void_ptr()
    }

    fn void_ptr_mut(&mut self) -> *mut std::os::raw::c_void {
        self.deref_mut().void_ptr_mut()
    }
}

impl<N> Drop for NativeLease<N> {
    fn drop(&mut self) {
        if let Some(native) = self.native.take() {
            self.buffer.recycle(native);
        }
    }
}
