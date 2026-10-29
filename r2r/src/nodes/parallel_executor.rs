//! Bounded worker groups; each group exclusively owns Nodes and a native wait set.
use super::*;
mod task;
pub use task::{ParallelExecutorTask, ParallelExecutorTaskError};

/// Executes independent Node groups on a fixed native worker budget.
///
/// Each worker exclusively owns its Nodes and a reusable RCL wait set. Nodes in
/// the same group retain serial dispatch; Nodes in other groups can progress
/// during a long synchronous callback. Configure groups before [`Self::spawn`],
/// and recover ownership with [`ParallelExecutorTask::stop`] before changing them.
/// No per-Node or per-spin thread is created. Same-Node callback-group concurrency
/// and work stealing are not provided by this executor.
///
/// ```no_run
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let context = r2r::Context::create()?;
/// let slow = r2r::Node::create(context.clone(), "slow", "")?;
/// let control = r2r::Node::create(context.clone(), "control", "")?;
/// let mut executor = r2r::ParallelExecutor::new(context, 2)?;
/// let slow_id = executor.add_node_to_worker(slow, 0)?;
/// executor.add_node_to_worker(control, 1)?;
/// let task = executor.spawn()?;
/// // The runtime can await endpoint futures while native workers dispatch ROS.
/// let mut executor = task.stop().await?;
/// executor.reset_cancel();
/// let slow = executor.remove_node(slow_id)?;
/// # drop(slow);
/// # Ok(())
/// # }
/// ```
pub struct ParallelExecutor {
    workers: Vec<Executor>,
    placements: HashMap<ExecutorNodeId, usize>,
    next_worker: usize,
}

/// Clonable, thread-safe cancellation for every configured worker.
///
/// Worker references are weak: clones share metadata without retaining Nodes,
/// native workers or their Context. Requests persist until the stopped executor
/// calls [`ParallelExecutor::reset_cancel`].
#[derive(Clone, Debug)]
pub struct ParallelExecutorHandle {
    workers: Arc<[ExecutorHandle]>,
}
impl ParallelExecutorHandle {
    /// Requests cancellation of all workers, including workers not yet started.
    /// Returns `true` if any configured worker still exists. Every worker is
    /// attempted even if a native wake fails; the first wake error is returned.
    pub fn cancel(&self) -> Result<bool> {
        let mut live = false;
        let mut error = None;
        for worker in self.workers.iter() {
            match worker.cancel() {
                Ok(active) => live |= active,
                Err(failure) => {
                    error.get_or_insert(failure);
                }
            }
        }
        error.map_or(Ok(live), Err)
    }
    /// Whether any live worker has a pending cancellation request.
    /// Expired workers report `false`.
    pub fn is_cancelled(&self) -> bool {
        self.workers.iter().any(ExecutorHandle::is_cancelled)
    }
}

impl ParallelExecutor {
    /// Creates a fixed worker budget in one valid Context.
    /// Zero workers reports `RCL_RET_INVALID_ARGUMENT`; a shut-down Context
    /// reports `RCL_RET_NOT_INIT`. Threads are created only by [`Self::spawn`].
    pub fn new(context: Context, workers: usize) -> Result<Self> {
        if workers == 0 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        if !context.is_valid() {
            return Err(Error::RCL_RET_NOT_INIT);
        }
        Ok(Self {
            workers: (0..workers)
                .map(|_| Executor::new(context.clone()))
                .collect(),
            placements: HashMap::new(),
            next_worker: 0,
        })
    }
    /// The configured worker budget, including empty groups.
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }
    /// The total number of owned Nodes across all workers.
    pub fn len(&self) -> usize {
        self.placements.len()
    }
    /// Whether every worker group is empty.
    pub fn is_empty(&self) -> bool {
        self.placements.is_empty()
    }
    /// Moves a Node into the next group in round-robin order.
    /// Nodes must share this executor's Context. IDs remain stable after removals.
    ///
    /// ```compile_fail
    /// # fn example(context: r2r::Context, mut node: r2r::Node) {
    /// let mut executor = r2r::ParallelExecutor::new(context, 2).unwrap();
    /// executor.add_node(node).unwrap();
    /// node.spin_once(std::time::Duration::ZERO); // ownership was transferred
    /// # }
    /// ```
    pub fn add_node(&mut self, node: Node) -> Result<ExecutorNodeId> {
        let worker = self.next_worker;
        let id = self.add_node_to_worker(node, worker)?;
        self.next_worker = (worker + 1) % self.workers.len();
        Ok(id)
    }
    /// Moves a Node into a zero-based worker group without advancing round-robin
    /// placement. An out-of-range index reports `RCL_RET_INVALID_ARGUMENT`.
    pub fn add_node_to_worker(&mut self, node: Node, worker: usize) -> Result<ExecutorNodeId> {
        let executor = self
            .workers
            .get_mut(worker)
            .ok_or(Error::RCL_RET_INVALID_ARGUMENT)?;
        let id = executor.add_node(node)?;
        self.placements.insert(id, worker);
        Ok(id)
    }
    /// Borrows an owned Node for configuration while execution is stopped.
    /// Removed or foreign IDs report `RCL_RET_NODE_INVALID`.
    pub fn node_mut(&mut self, id: ExecutorNodeId) -> Result<&mut Node> {
        let worker = *self
            .placements
            .get(&id)
            .ok_or(Error::RCL_RET_NODE_INVALID)?;
        self.workers[worker].node_mut(id)
    }
    /// Clears inactive native wait references and returns Node ownership.
    /// Removed or foreign IDs report `RCL_RET_NODE_INVALID`.
    pub fn remove_node(&mut self, id: ExecutorNodeId) -> Result<Node> {
        let worker = *self
            .placements
            .get(&id)
            .ok_or(Error::RCL_RET_NODE_INVALID)?;
        let node = self.workers[worker].remove_node(id)?;
        self.placements.remove(&id);
        Ok(node)
    }
    /// Creates a weak cancellation handle for every configured group.
    pub fn handle(&mut self) -> ParallelExecutorHandle {
        ParallelExecutorHandle {
            workers: self
                .workers
                .iter_mut()
                .map(Executor::handle)
                .collect::<Vec<_>>()
                .into(),
        }
    }
    /// Whether any group has a pending cancellation request.
    pub fn is_cancelled(&self) -> bool {
        self.workers.iter().any(Executor::is_cancelled)
    }
    /// Resets all cancellation requests before reuse. Coordinate external
    /// cancellation with reset; an already-issued native wake can be spurious.
    pub fn reset_cancel(&mut self) {
        for worker in &mut self.workers {
            worker.reset_cancel();
        }
    }
    /// Starts one native worker per nonempty group. A completely empty executor
    /// starts one worker to observe cancellation and Context shutdown.
    ///
    /// Thread/startup failure cancels already-started peers; their cleanup remains
    /// cooperative. A running callback must return before its worker can stop.
    ///
    /// ```compile_fail
    /// # fn example(mut executor: r2r::ParallelExecutor, id: r2r::ExecutorNodeId) {
    /// let task = executor.spawn().unwrap();
    /// executor.node_mut(id).unwrap(); // task exclusively owns the Nodes
    /// # }
    /// ```
    pub fn spawn(self) -> std::result::Result<ParallelExecutorTask, ParallelExecutorTaskError> {
        task::spawn(self)
    }
}

#[cfg(all(test, not(feature = "doc-only")))]
mod tests;
