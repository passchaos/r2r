//! Optional synchronous timer callbacks with stable native storage and owner-thread cleanup.

use super::*;
use std::pin::Pin;

mod api;
mod handle;
pub(crate) mod native;
pub use handle::{CallbackTimer, CallbackTimerOptions};
use native::NativeTimer;

#[derive(Default)]
pub(super) struct TimerCallbacks {
    timers: Vec<Pin<Box<dyn NativeTimer>>>,
    indexes: HashMap<uuid::Uuid, usize>,
    canceled: Arc<Mutex<crate::notifications::NotificationQueue<uuid::Uuid>>>,
}

impl TimerCallbacks {
    pub(super) fn len(&self) -> usize {
        self.timers.len()
    }
    pub(super) fn set_wake(&self, wake: Option<std::task::Waker>) {
        self.canceled.lock().set_wake(wake);
    }
    fn insert(&mut self, timer: Pin<Box<dyn NativeTimer>>) -> CallbackTimer {
        let token = CallbackTimer::new(
            timer.id(),
            timer.alive(),
            timer.clock_type(),
            self.canceled.clone(),
        );
        self.indexes.insert(timer.id(), self.timers.len());
        self.timers.push(timer);
        token
    }
    fn get(&self, id: uuid::Uuid) -> Result<&dyn NativeTimer> {
        self.indexes
            .get(&id)
            .and_then(|index| self.timers.get(*index))
            .map(|timer| timer.as_ref().get_ref())
            .ok_or(Error::RCL_RET_TIMER_INVALID)
    }
    fn get_mut(&mut self, id: uuid::Uuid) -> Result<Pin<&mut (dyn NativeTimer + 'static)>> {
        self.indexes
            .get(&id)
            .and_then(|index| self.timers.get_mut(*index))
            .map(|timer| timer.as_mut())
            .ok_or(Error::RCL_RET_TIMER_INVALID)
    }
    fn remove(&mut self, id: uuid::Uuid) -> bool {
        let Some(index) = self.indexes.remove(&id) else {
            return false;
        };
        let mut timer = self.timers.swap_remove(index);
        if let Some(moved) = self.timers.get(index) {
            self.indexes.insert(moved.id(), index);
        }
        timer.as_mut().destroy();
        // Native teardown and captured state destruction never hold the notification lock.
        drop(timer);
        true
    }
    pub(super) fn prepare(&mut self) -> usize {
        loop {
            let id = self.canceled.lock().pop_front();
            let Some(id) = id else {
                break;
            };
            self.remove(id);
        }
        self.timers.len()
    }
    pub(super) fn add_to_wait_set(&self, wait_set: &mut rcl_wait_set_t) -> Result<usize> {
        let mut first = 0;
        for (position, timer) in self.timers.iter().enumerate() {
            let mut index = 0;
            check_rcl_ret(unsafe { rcl_wait_set_add_timer(wait_set, timer.handle(), &mut index) })?;
            if position == 0 {
                first = index;
            }
        }
        Ok(first)
    }
    pub(super) fn dispatch(&mut self, wait_set: &rcl_wait_set_t, first: usize) -> Result<()> {
        if self.timers.is_empty() {
            return Ok(());
        }
        if wait_set.timers.is_null()
            || first
                .checked_add(self.timers.len())
                .is_none_or(|end| end > wait_set.size_of_timers)
        {
            return Err(Error::RCL_RET_WAIT_SET_INVALID);
        }
        let ready =
            unsafe { std::slice::from_raw_parts(wait_set.timers.add(first), self.timers.len()) };
        let mut error = None;
        for (timer, pointer) in self.timers.iter_mut().zip(ready) {
            if pointer.is_null() {
                continue;
            }
            if *pointer != timer.handle() {
                return Err(Error::RCL_RET_WAIT_SET_INVALID);
            }
            if let Err(failure) = timer.as_mut().call() {
                error.get_or_insert(failure);
            }
        }
        error.map_or(Ok(()), Err)
    }
    pub(super) fn destroy_all(&mut self) {
        for timer in &mut self.timers {
            timer.as_mut().destroy();
        }
    }
}

#[cfg(all(test, not(feature = "doc-only")))]
mod tests;
