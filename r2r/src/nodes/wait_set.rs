//! Native rcl wait set lifecycle and per-kind entity capacities.

use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::nodes) struct WaitSetCapacities {
    pub(in crate::nodes) subscriptions: usize,
    pub(in crate::nodes) guard_conditions: usize,
    pub(in crate::nodes) timers: usize,
    pub(in crate::nodes) clients: usize,
    pub(in crate::nodes) services: usize,
}

impl WaitSetCapacities {
    pub(in crate::nodes) fn add(&mut self, other: Self) {
        self.subscriptions += other.subscriptions;
        self.guard_conditions += other.guard_conditions;
        self.timers += other.timers;
        self.clients += other.clients;
        self.services += other.services;
    }

    pub(in crate::nodes) fn subtract(&mut self, other: Self) {
        self.subscriptions -= other.subscriptions;
        self.guard_conditions -= other.guard_conditions;
        self.timers -= other.timers;
        self.clients -= other.clients;
        self.services -= other.services;
    }
}

pub(in crate::nodes) fn action_client_wait_set_capacities(
    handle: &rcl_action_client_t,
) -> Result<WaitSetCapacities> {
    let mut capacities = WaitSetCapacities::default();
    action_client_get_num_waits(
        handle,
        &mut capacities.subscriptions,
        &mut capacities.guard_conditions,
        &mut capacities.timers,
        &mut capacities.clients,
        &mut capacities.services,
    )?;
    Ok(capacities)
}

pub(in crate::nodes) fn action_server_wait_set_capacities(
    handle: &rcl_action_server_t,
) -> Result<WaitSetCapacities> {
    let mut capacities = WaitSetCapacities::default();
    action_server_get_num_waits(
        handle,
        &mut capacities.subscriptions,
        &mut capacities.guard_conditions,
        &mut capacities.timers,
        &mut capacities.clients,
        &mut capacities.services,
    )?;
    Ok(capacities)
}

pub(in crate::nodes) struct WaitSet {
    pub(in crate::nodes) handle: rcl_wait_set_t,
    pub(in crate::nodes) capacities: Option<WaitSetCapacities>,
}

impl WaitSet {
    pub(in crate::nodes) fn new() -> Self {
        Self {
            handle: unsafe { rcl_get_zero_initialized_wait_set() },
            capacities: None,
        }
    }

    pub(in crate::nodes) fn clear(&mut self) -> Result<()> {
        if self.capacities.is_some() {
            check_rcl_ret(unsafe { rcl_wait_set_clear(&mut self.handle) })?;
        }
        Ok(())
    }

    pub(in crate::nodes) fn prepare(
        &mut self, capacities: WaitSetCapacities, context: &mut rcl_context_t,
    ) -> Result<()> {
        let ret = match self.capacities {
            None => unsafe {
                rcl_wait_set_init(
                    &mut self.handle,
                    capacities.subscriptions,
                    capacities.guard_conditions,
                    capacities.timers,
                    capacities.clients,
                    capacities.services,
                    0,
                    context,
                    rcutils_get_default_allocator(),
                )
            },
            Some(previous) if previous != capacities => unsafe {
                rcl_wait_set_resize(
                    &mut self.handle,
                    capacities.subscriptions,
                    capacities.guard_conditions,
                    capacities.timers,
                    capacities.clients,
                    capacities.services,
                    0,
                )
            },
            Some(_) => unsafe { rcl_wait_set_clear(&mut self.handle) },
        };
        check_rcl_ret(ret)?;
        self.capacities = Some(capacities);
        Ok(())
    }

    pub(in crate::nodes) fn fini(&mut self) {
        if self.capacities.take().is_some() {
            let ret = unsafe { rcl_wait_set_fini(&mut self.handle) };
            if ret != RCL_RET_OK as i32 {
                log::error!("could not finalize wait set: {}", ret);
            }
        }
    }
}

impl Drop for WaitSet {
    fn drop(&mut self) {
        self.fini();
    }
}
