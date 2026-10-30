//! Owned callback groups, bounded native input transfer and worker scheduling.

use super::*;
use futures::{stream::FuturesUnordered, Stream};
use std::{
    future::Future,
    pin::Pin,
    task::{Context as TaskContext, Poll},
};

mod input;
mod scheduler;
pub(in crate::nodes) mod sources;
mod task;
pub(crate) use input::Producer;
pub use scheduler::Failure as CallbackWorkerError;
pub use sources::{
    CallbackQueueOptions, GroupedService, GroupedSubscription, GroupedSubscriptionOptions,
    GroupedTimer,
};
pub use task::{CallbackExecutorTask, CallbackExecutorTaskError};

/// Drives owned Nodes and mutually exclusive callback groups on a fixed worker budget.
///
/// The budget includes the native wait job. One worker exclusively owns the
/// Executor/Nodes during each native wait and dispatch; ready group jobs can run
/// on other workers, and the native job can move between workers. A group keeps
/// its handlers serial while different groups can progress concurrently.
/// Streams and application futures stay on their chosen Rust runtime.
///
/// Configure entities and groups before spawning. Stop returns Node IDs, group
/// IDs and handler state for reuse; reset cancellation before the next run.
/// Native or callback failure cancels peers and releases ownership before
/// returning an error. Cancellation is cooperative for a handler already running.
/// Reentrant groups and live configuration changes are separate capabilities.
///
/// ```no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let context = r2r::Context::create()?;
/// let mut node = r2r::Node::create(context.clone(), "grouped", "")?;
/// let mut group = r2r::CallbackGroup::new();
/// let timer = node.create_wall_timer_callback_in_group(
///     &mut group, std::time::Duration::from_millis(10), || Ok(()),
/// )?;
/// let mut executor = r2r::CallbackExecutor::new(context, 2)?;
/// let node_id = executor.add_node(node)?;
/// let group_id = executor.add_group(group)?;
/// let task = executor.spawn()?;
/// let mut executor = task.stop().await?;
/// executor.reset_cancel();
/// let (node, group) = executor.node_and_group_mut(node_id, group_id)?;
/// # let _ = (node, group, timer);
/// # Ok(())
/// # }
/// ```
pub struct CallbackExecutor {
    native: Executor,
    groups: Vec<CallbackGroup>,
    group_indexes: HashMap<CallbackGroupId, usize>,
    workers: usize,
    running: Arc<Mutex<std::sync::Weak<scheduler::Scheduler>>>,
}

#[derive(Clone)]
/// Weak cancellation of the native wait and every callback group.
///
/// Clones do not keep Nodes, group handlers or the Context alive. Requests
/// persist through stopped reuse until the owner resets cancellation.
pub struct CallbackExecutorHandle {
    native: ExecutorHandle,
    groups: Arc<[std::sync::Weak<std::sync::atomic::AtomicBool>]>,
    running: Arc<Mutex<std::sync::Weak<scheduler::Scheduler>>>,
}
impl CallbackExecutorHandle {
    /// Requests cooperative cancellation. An expired executor returns false.
    pub fn cancel(&self) -> Result<bool> {
        for flag in self.groups.iter().filter_map(std::sync::Weak::upgrade) {
            flag.store(true, std::sync::atomic::Ordering::Release);
        }
        let result = self.native.cancel();
        let running = self.running.lock().upgrade();
        if let Some(running) = running {
            running.cancel()?;
        }
        result
    }
    /// Whether the live native executor has a pending request; expired handles
    /// return false.
    pub fn is_cancelled(&self) -> bool {
        self.native.is_cancelled()
    }
}

impl CallbackExecutor {
    /// Creates a total native/callback worker budget. Zero workers and a
    /// shut-down Context fail before any worker starts.
    pub fn new(context: Context, workers: usize) -> Result<Self> {
        if workers == 0 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        if !context.is_valid() {
            return Err(Error::RCL_RET_NOT_INIT);
        }
        Ok(Self {
            native: Executor::new(context),
            groups: Vec::new(),
            group_indexes: HashMap::new(),
            workers,
            running: Arc::new(Mutex::new(std::sync::Weak::new())),
        })
    }
    /// Configured total worker budget, including the native wait job.
    pub fn worker_count(&self) -> usize {
        self.workers
    }
    /// Transfers a Node from the same Context into the native owner.
    /// Its stable ID survives stop/restart and other Node removals.
    pub fn add_node(&mut self, node: Node) -> Result<ExecutorNodeId> {
        self.native.add_node(node)
    }
    /// Borrows a Node while stopped. Missing/foreign IDs report node-invalid.
    pub fn node_mut(&mut self, id: ExecutorNodeId) -> Result<&mut Node> {
        self.native.node_mut(id)
    }
    /// Clears inactive native wait references and returns Node ownership.
    /// Remaining live group sources must belong to Nodes added before spawning.
    pub fn remove_node(&mut self, id: ExecutorNodeId) -> Result<Node> {
        self.native.remove_node(id)
    }
    /// Number of owned Nodes.
    pub fn len(&self) -> usize {
        self.native.len()
    }
    /// Whether there are no owned Nodes.
    pub fn is_empty(&self) -> bool {
        self.native.is_empty()
    }
    /// Transfers configured handlers into a mutually exclusive scheduling group.
    /// A group is owned by this executor until stopped ownership is recovered.
    pub fn add_group(&mut self, group: CallbackGroup) -> Result<CallbackGroupId> {
        let id = group.id();
        if self.group_indexes.contains_key(&id) {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        self.group_indexes.insert(id, self.groups.len());
        self.groups.push(group);
        Ok(id)
    }
    /// Borrows a stopped group for additional entity registration.
    pub fn group_mut(&mut self, id: CallbackGroupId) -> Result<&mut CallbackGroup> {
        let index = *self
            .group_indexes
            .get(&id)
            .ok_or(Error::RCL_RET_INVALID_ARGUMENT)?;
        Ok(&mut self.groups[index])
    }
    /// Borrows disjoint Node/group configuration together, avoiding split-borrow
    /// workarounds when adding entities after stop.
    pub fn node_and_group_mut(
        &mut self, node: ExecutorNodeId, group: CallbackGroupId,
    ) -> Result<(&mut Node, &mut CallbackGroup)> {
        let index = *self
            .group_indexes
            .get(&group)
            .ok_or(Error::RCL_RET_INVALID_ARGUMENT)?;
        Ok((self.native.node_mut(node)?, &mut self.groups[index]))
    }
    /// Creates weak controls that remain usable across stop/restart.
    pub fn handle(&mut self) -> CallbackExecutorHandle {
        CallbackExecutorHandle {
            native: self.native.handle(),
            groups: self
                .groups
                .iter()
                .map(|group| Arc::downgrade(&group.cancelled))
                .collect::<Vec<_>>()
                .into(),
            running: self.running.clone(),
        }
    }
    /// Whether cancellation remains pending on the stopped native owner.
    pub fn is_cancelled(&self) -> bool {
        self.native.is_cancelled()
    }
    /// Resets native/group requests before reuse. Coordinate this with external
    /// cancellation; an already-issued native wake can be spurious.
    pub fn reset_cancel(&mut self) {
        self.native.reset_cancel();
        for group in &self.groups {
            group
                .cancelled
                .store(false, std::sync::atomic::Ordering::Release);
        }
    }
    /// Starts at most the configured budget and validates every live source's
    /// group/Node ownership. Native errors and user panics stop peers directly.
    ///
    /// ```compile_fail
    /// # fn example(mut executor: r2r::CallbackExecutor, id: r2r::ExecutorNodeId) {
    /// let task = executor.spawn().unwrap();
    /// executor.node_mut(id).unwrap(); // execution exclusively owns the Node
    /// # }
    /// ```
    pub fn spawn(self) -> std::result::Result<CallbackExecutorTask, CallbackExecutorTaskError> {
        task::spawn(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Stable identity of one callback group across ownership recovery.
pub struct CallbackGroupId(uuid::Uuid);

/// Configuration and retained callback state for one mutually exclusive group.
/// Register entities before transferring the group into a callback executor.
///
/// A group serializes all its handlers, even when they come from different
/// Nodes. Different groups can execute concurrently within the total budget.
/// User handlers must be Send; the executor moves exclusive group ownership.
pub struct CallbackGroup {
    id: CallbackGroupId,
    cancelled: Arc<std::sync::atomic::AtomicBool>,
    sources: Vec<(uuid::Uuid, std::sync::Weak<dyn input::Control>)>,
    callbacks: FuturesUnordered<Pin<Box<dyn Future<Output = Result<()>> + Send>>>,
}

impl Default for CallbackGroup {
    fn default() -> Self {
        Self::new()
    }
}

impl CallbackGroup {
    /// Creates an empty mutually exclusive group without starting a thread.
    pub fn new() -> Self {
        Self {
            id: CallbackGroupId(uuid::Uuid::new_v4()),
            cancelled: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            sources: Vec::new(),
            callbacks: FuturesUnordered::new(),
        }
    }
    /// This group's stable identity.
    pub fn id(&self) -> CallbackGroupId {
        self.id
    }
    /// Number of retained handler registrations. Closed sources retire when
    /// polled; normal stop retains live registrations and handler state.
    pub fn len(&self) -> usize {
        self.callbacks.len()
    }
    /// Whether no handler registrations remain.
    pub fn is_empty(&self) -> bool {
        self.callbacks.is_empty()
    }
    #[cfg(test)]
    pub(crate) fn input<T, F>(&mut self, capacity: usize, callback: F) -> Result<Producer<T>>
    where
        T: Send + 'static,
        F: FnMut(T) -> Result<()> + Send + 'static,
    {
        let (producer, consumer) = self.prepare_input(capacity, callback)?;
        self.commit_input(consumer);
        Ok(producer)
    }
    fn prepare_input<T, F>(
        &self, capacity: usize, callback: F,
    ) -> Result<(Producer<T>, input::Consumer<T, F>)> {
        input::channel(capacity, self.cancelled.clone(), callback)
    }
    fn commit_input<T, F>(&mut self, consumer: input::Consumer<T, F>)
    where
        T: Send + 'static,
        F: FnMut(T) -> Result<()> + Send + 'static,
    {
        self.callbacks.push(Box::pin(consumer));
    }
    fn poll_callbacks(&mut self, context: &mut TaskContext<'_>) -> Poll<Result<()>> {
        loop {
            match Pin::new(&mut self.callbacks).poll_next(context) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Ready(Some(Ok(()))) => {}
                Poll::Ready(Some(Err(error))) => return Poll::Ready(Err(error)),
            }
        }
    }
}

#[cfg(test)]
mod tests;
