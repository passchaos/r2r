use super::{
    future_executor::{self, FutureDriver, FutureGuard, FutureWake},
    spin, *,
};
mod control;
pub use control::ExecutorHandle;

/// Identifies a node owned by an [`Executor`]. IDs remain stable across other node removals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExecutorNodeId(uuid::Uuid);

struct ExecutorNode {
    id: ExecutorNodeId,
    node: Node,
    preparation: Option<spin::SpinPreparation>,
    offsets: spin::EntityOffsets,
}

/// Drives multiple owned ROS nodes through one reusable native wait set.
///
/// Nodes use the same context. Every ready node can progress after the same native wait;
/// idle nodes add no sequential timeout or periodic polling. Rust futures stay on the calling
/// thread and may be non-Send. Independently driven runtimes remain responsible for their I/O.
pub struct Executor {
    control: Option<Arc<control::ExecutorControl>>,
    nodes: Vec<ExecutorNode>,
    indexes: HashMap<ExecutorNodeId, usize>,
    wait_set: WaitSet,
    future_wake_guard: Option<Arc<FutureGuard>>,
    future_wake_state: Option<Arc<FutureWake>>,
    context: Context,
}

// SAFETY: owned Nodes are Send, native wait storage has unique ownership, and no active
// spin can be moved because it holds an exclusive borrow. Wake guards have stable Arc storage.
unsafe impl Send for Executor {}

impl Executor {
    pub fn new(context: Context) -> Self {
        Self {
            control: None,
            nodes: Vec::new(),
            indexes: HashMap::new(),
            wait_set: WaitSet::new(),
            future_wake_guard: None,
            future_wake_state: None,
            context,
        }
    }

    /// Adds an owned node. Node/entity configuration can be changed through [`Self::node_mut`]
    /// between spins. Moving ownership excludes simultaneous waits for the same entities.
    pub fn add_node(&mut self, node: Node) -> Result<ExecutorNodeId> {
        if !Arc::ptr_eq(&node.context.context_handle, &self.context.context_handle) {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        if !self.context.is_valid() {
            return Err(Error::RCL_RET_NOT_INIT);
        }
        self.wait_set.clear()?;
        let id = ExecutorNodeId(uuid::Uuid::new_v4());
        self.indexes.insert(id, self.nodes.len());
        self.nodes.push(ExecutorNode {
            id,
            node,
            preparation: None,
            offsets: Default::default(),
        });
        Ok(id)
    }

    /// Returns an owned node for independent use or destruction.
    pub fn remove_node(&mut self, id: ExecutorNodeId) -> Result<Node> {
        let index = *self.indexes.get(&id).ok_or(Error::RCL_RET_NODE_INVALID)?;
        self.wait_set.clear()?;
        self.indexes.remove(&id);
        let entry = self.nodes.swap_remove(index);
        if let Some(swapped) = self.nodes.get(index) {
            self.indexes.insert(swapped.id, index);
        }
        Ok(entry.node)
    }

    /// Borrows a node for configuration between native waits.
    pub fn node_mut(&mut self, id: ExecutorNodeId) -> Result<&mut Node> {
        let index = *self.indexes.get(&id).ok_or(Error::RCL_RET_NODE_INVALID)?;
        Ok(&mut self.nodes[index].node)
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Returns a weak cancellation handle for independently running work.
    pub fn handle(&mut self) -> ExecutorHandle {
        let control = self
            .control
            .get_or_insert_with(|| Arc::new(control::ExecutorControl::new()));
        if let Some(guard) = &self.future_wake_guard {
            control.attach_guard(guard);
        }
        ExecutorHandle::new(control)
    }

    pub fn is_cancelled(&self) -> bool {
        self.control
            .as_ref()
            .is_some_and(|control| control.is_cancelled())
    }

    /// Clears a cancellation request between runs, retaining native wake/storage caches.
    pub fn reset_cancel(&mut self) {
        if let Some(control) = &self.control {
            control.reset();
        }
    }

    fn check_cancellation(&self) -> Result<()> {
        match &self.control {
            Some(control) => control.check(),
            None => Ok(()),
        }
    }

    /// Drives ROS events until cancellation or context shutdown. Normal stopping returns Ok.
    pub fn spin(&mut self) -> Result<()> {
        while self.context.is_valid() {
            match self.spin_once(Duration::from_nanos(i64::MAX as u64)) {
                Ok(()) => {}
                Err(Error::ExecutorCancelled) => return Ok(()),
                Err(Error::RCL_RET_NOT_INIT) if !self.context.is_valid() => return Ok(()),
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    /// Waits once for all nodes and dispatches ready ROS work.
    /// Pending reliable ACK queries retain their bounded native-query cadence.
    pub fn spin_once(&mut self, timeout: Duration) -> Result<()> {
        duration_to_rcl_nanos(timeout)?;
        self.check_cancellation()?;
        if !self.context.is_valid() {
            return Err(Error::RCL_RET_NOT_INIT);
        }
        let guard = match &self.future_wake_guard {
            Some(guard) => guard.clone(),
            None => {
                let guard = FutureGuard::new(&self.context)?;
                self.cache_future_guard(guard.clone());
                guard
            }
        };
        let active = ActiveWait::new(guard.clone());
        let mut capacities = WaitSetCapacities {
            guard_conditions: 1,
            ..Default::default()
        };
        let mut wait = timeout;
        for entry in &mut self.nodes {
            let preparation = entry.node.prepare_spin(timeout)?;
            add_capacities(&mut capacities, preparation.capacities)?;
            wait = wait.min(preparation.timeout);
            entry.preparation = Some(preparation);
        }
        {
            let mut context = self.context.context_handle.lock();
            self.wait_set.prepare(capacities, context.as_mut())?;
        }
        check_rcl_ret(unsafe {
            rcl_wait_set_add_guard_condition(
                &mut self.wait_set.handle,
                guard.handle(),
                std::ptr::null_mut(),
            )
        })?;
        for entry in &mut self.nodes {
            let (_, entities) = entry.node.wait_entities();
            entry.offsets = entities
                .add_ordinary(&mut self.wait_set.handle, entry.preparation.as_ref().unwrap())?;
        }
        // Ordinary entity spans remain contiguous and disjoint. Native actions store their
        // returned wait-set indices, and are appended after all ordinary node entities.
        for entry in &mut self.nodes {
            let (_, entities) = entry.node.wait_entities();
            entities.add_actions(&mut self.wait_set.handle)?;
        }
        let timeout_nanos = duration_to_rcl_nanos(wait)?;
        self.check_cancellation()?;
        #[cfg(test)]
        guard.before_wait(timeout_nanos);
        let ret = unsafe { rcl_wait(&mut self.wait_set.handle, timeout_nanos) };
        if let Some(wake) = &self.future_wake_state {
            wake.finish_native_wait();
        }
        #[cfg(test)]
        guard.after_wait(ret);
        if !self.context.is_valid() {
            return Err(Error::RCL_RET_NOT_INIT);
        }
        self.check_cancellation()?;
        if ret == RCL_RET_TIMEOUT as i32 {
            return Ok(());
        }
        check_rcl_ret(ret)?;
        let mut error = None;
        for entry in &mut self.nodes {
            if let Some(control) = &self.control {
                control.check()?;
            }
            let (_, mut entities) = entry.node.wait_entities();
            if let Err(failure) = entities.dispatch(&self.wait_set.handle, entry.offsets) {
                error.get_or_insert(failure);
            }
        }
        drop(active);
        match error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Drives a Rust future and all owned nodes, with the default periodic future fallback.
    pub fn spin_until_future_complete<T>(
        &mut self, future: impl Future<Output = Result<T>>, timeout: Duration,
    ) -> Result<T> {
        self.spin_until_future_complete_with_options(
            future,
            timeout,
            FutureExecutionOptions::default(),
        )
    }

    /// Drives a possibly non-Send future through ROS events and Rust notifications.
    pub fn spin_until_future_complete_with_options<T>(
        &mut self, future: impl Future<Output = Result<T>>, timeout: Duration,
        options: FutureExecutionOptions,
    ) -> Result<T> {
        future_executor::drive_future(self, future, timeout, options)
    }
}

impl FutureDriver for Executor {
    fn future_context(&self) -> &Context {
        &self.context
    }
    fn cached_future_guard(&self) -> Option<Arc<FutureGuard>> {
        self.future_wake_guard.clone()
    }
    fn cache_future_guard(&mut self, guard: Arc<FutureGuard>) {
        if let Some(control) = &self.control {
            control.attach_guard(&guard);
        }
        self.future_wake_guard = Some(guard);
    }
    fn set_future_wake(&mut self, wake: Arc<FutureWake>) {
        self.future_wake_state = Some(wake);
    }
    fn spin_future_events(&mut self, timeout: Duration) -> Result<()> {
        self.spin_once(timeout)
    }
    fn check_future_state(&self) -> Result<()> {
        self.check_cancellation()
    }
}

struct ActiveWait {
    guard: Arc<FutureGuard>,
    previous: bool,
}
impl ActiveWait {
    fn new(guard: Arc<FutureGuard>) -> Self {
        let previous = guard.active.swap(true, std::sync::atomic::Ordering::AcqRel);
        Self { guard, previous }
    }
}
impl Drop for ActiveWait {
    fn drop(&mut self) {
        self.guard
            .active
            .store(self.previous, std::sync::atomic::Ordering::Release);
    }
}

fn add_capacities(target: &mut WaitSetCapacities, other: WaitSetCapacities) -> Result<()> {
    for (target, other) in [
        (&mut target.subscriptions, other.subscriptions),
        (&mut target.guard_conditions, other.guard_conditions),
        (&mut target.timers, other.timers),
        (&mut target.clients, other.clients),
        (&mut target.services, other.services),
    ] {
        *target = target
            .checked_add(other)
            .ok_or(Error::RCL_RET_INVALID_ARGUMENT)?;
    }
    Ok(())
}

impl Drop for Executor {
    fn drop(&mut self) {
        self.wait_set.fini();
    }
}

#[cfg(all(test, not(feature = "doc-only")))]
mod tests;
