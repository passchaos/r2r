//! Invalidate native wait caches when an event-bearing wait set changes membership.
//!
//! Some RMW wait caches keep event status conditions attached after the RCL event list changes.
//! Rebuild only at that layout boundary. Stable observer layouts retain native wait storage.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Entry {
    Subscription(*const rcl_subscription_t),
    Guard(*const rcl_guard_condition_t),
    Timer(*const rcl_timer_t),
    Client(*const rcl_client_t),
    Service(*const rcl_service_t),
    Event(*const rcl_event_t, *mut std::ffi::c_void, rmw_event_type_t),
}

#[derive(Default)]
pub(super) struct EventLayout {
    previous: Vec<Entry>,
    current: Vec<Entry>,
}

impl EventLayout {
    fn collect(&mut self, handle: &rcl_wait_set_t) -> Result<()> {
        self.current.clear();
        macro_rules! append {
            ($ptr:expr, $len:expr, $kind:ident) => {
                if $len != 0 {
                    // Called only after all entities have been added and before rcl_wait.
                    for &pointer in unsafe { std::slice::from_raw_parts($ptr, $len) } {
                        self.current.push(Entry::$kind(pointer));
                    }
                }
            };
        }
        append!(handle.subscriptions, handle.size_of_subscriptions, Subscription);
        append!(handle.guard_conditions, handle.size_of_guard_conditions, Guard);
        append!(handle.timers, handle.size_of_timers, Timer);
        append!(handle.clients, handle.size_of_clients, Client);
        append!(handle.services, handle.size_of_services, Service);
        if handle.size_of_events != 0 {
            for &pointer in
                unsafe { std::slice::from_raw_parts(handle.events, handle.size_of_events) }
            {
                let native = unsafe { rcl_event_get_rmw_handle(pointer).as_ref() }
                    .ok_or(Error::RCL_RET_EVENT_INVALID)?;
                // Same-sized event replacement can reuse the allocation address. Native endpoint
                // identity and event kind distinguish that semantic membership change.
                self.current
                    .push(Entry::Event(pointer, native.data, native.event_type));
            }
        }
        Ok(())
    }
    fn restore(&self, handle: &mut rcl_wait_set_t) -> Result<()> {
        for entry in &self.current {
            let ret = unsafe {
                match *entry {
                    Entry::Subscription(pointer) => {
                        rcl_wait_set_add_subscription(handle, pointer, std::ptr::null_mut())
                    }
                    Entry::Guard(pointer) => {
                        rcl_wait_set_add_guard_condition(handle, pointer, std::ptr::null_mut())
                    }
                    Entry::Timer(pointer) => {
                        rcl_wait_set_add_timer(handle, pointer, std::ptr::null_mut())
                    }
                    Entry::Client(pointer) => {
                        rcl_wait_set_add_client(handle, pointer, std::ptr::null_mut())
                    }
                    Entry::Service(pointer) => {
                        rcl_wait_set_add_service(handle, pointer, std::ptr::null_mut())
                    }
                    Entry::Event(pointer, _, _) => {
                        rcl_wait_set_add_event(handle, pointer, std::ptr::null_mut())
                    }
                }
            };
            check_rcl_ret(ret)?;
        }
        Ok(())
    }
}

impl WaitSet {
    pub(in crate::nodes) fn refresh_event_layout(&mut self, context: &Context) -> Result<()> {
        if self.handle.size_of_events == 0 && self.event_layout.is_none() {
            return Ok(());
        }
        let mut layout = self.event_layout.take().unwrap_or_default();
        layout.collect(&self.handle)?;
        if !layout.previous.is_empty() && layout.previous != layout.current {
            #[cfg(test)]
            {
                self.event_layout_rebuilds += 1;
            }
            let capacities = self.capacities.ok_or(Error::RCL_RET_WAIT_SET_INVALID)?;
            let mut context = context.context_handle.lock();
            self.fini();
            self.prepare(capacities, context.as_mut())?;
            // Action readiness indices are numeric positions in their per-kind arrays. Restore
            // those arrays in the same order, preserving every ordinary/action offset.
            layout.restore(&mut self.handle)?;
        }
        if self.handle.size_of_events != 0 {
            std::mem::swap(&mut layout.previous, &mut layout.current);
            self.event_layout = Some(layout);
        }
        Ok(())
    }
}
