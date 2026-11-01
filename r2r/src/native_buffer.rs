//! Native storage recycling shared by action, subscription and batch delivery.

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
            target: LeaseTarget::Single(Arc::clone(&self.slot)),
        })
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

/// Bounded multi-slot recycling for operations that hold several native messages at once,
/// such as a batch of accepted action goals prepared before the server lock is taken.
pub(crate) struct NativePool<N> {
    slots: Arc<Mutex<Vec<N>>>,
    max_slots: usize,
}

impl<N> fmt::Debug for NativePool<N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativePool")
            .field("max_slots", &self.max_slots)
            .finish_non_exhaustive()
    }
}

impl<N> Clone for NativePool<N> {
    fn clone(&self) -> Self {
        Self {
            slots: Arc::clone(&self.slots),
            max_slots: self.max_slots,
        }
    }
}

impl<N> NativePool<N> {
    pub(crate) fn new(max_slots: usize) -> Self {
        assert!(max_slots > 0, "native pool needs at least one slot");
        Self {
            slots: Arc::new(Mutex::new(Vec::new())),
            max_slots,
        }
    }

    pub(crate) fn checkout(&self, create: impl FnOnce() -> Result<N>) -> Result<NativeLease<N>> {
        let cached = { self.slots.lock().pop() };
        let native = match cached {
            Some(native) => native,
            None => create()?,
        };
        Ok(NativeLease {
            native: Some(native),
            target: LeaseTarget::Pool {
                slots: Arc::clone(&self.slots),
                max_slots: self.max_slots,
            },
        })
    }

    #[cfg(test)]
    pub(crate) fn available_len(&self) -> usize {
        self.slots.lock().len()
    }
}

enum LeaseTarget<N> {
    Single(Arc<Mutex<Option<N>>>),
    Pool {
        slots: Arc<Mutex<Vec<N>>>,
        max_slots: usize,
    },
}

pub(crate) struct NativeLease<N> {
    native: Option<N>,
    target: LeaseTarget<N>,
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
        let Some(native) = self.native.take() else {
            return;
        };
        match &self.target {
            LeaseTarget::Single(slot) => {
                // A concurrent return keeps the most recently returned storage. Retire the
                // displaced value after releasing the lock so arbitrary native finalizers can
                // safely reenter.
                let retired = { slot.lock().replace(native) };
                drop(retired);
            }
            LeaseTarget::Pool { slots, max_slots } => {
                let retired = {
                    let mut slots = slots.lock();
                    if slots.len() < *max_slots {
                        slots.push(native);
                        None
                    } else {
                        Some(native)
                    }
                };
                drop(retired);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_buffer_reuses_and_retires_displaced_storage() {
        let buffer = NativeBuffer::default();
        drop(buffer.checkout(|| Ok(1usize)).unwrap());
        assert!(buffer.is_available());
        let lease = buffer.checkout(|| Ok(2usize)).unwrap();
        assert!(!buffer.is_available());
        drop(lease);
        assert!(buffer.is_available());
    }

    #[test]
    fn pool_recycles_up_to_its_bound_and_retires_excess() {
        let pool = NativePool::new(2);
        let first = pool.checkout(|| Ok(1usize)).unwrap();
        let second = pool.checkout(|| Ok(2usize)).unwrap();
        let third = pool.checkout(|| Ok(3usize)).unwrap();
        drop(first);
        drop(second);
        assert_eq!(pool.available_len(), 2);
        drop(third);
        assert_eq!(pool.available_len(), 2);
        drop(pool.checkout(|| Ok(4usize)).unwrap());
        drop(pool.checkout(|| Ok(5usize)).unwrap());
        assert_eq!(pool.available_len(), 2);
    }
}
