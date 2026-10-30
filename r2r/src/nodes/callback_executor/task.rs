//! Runtime-independent collection and stopped ownership for callback workers.
use super::*;
use futures::channel::oneshot;
use std::thread::{self, JoinHandle};

#[derive(Debug, thiserror::Error)]
/// Preparation, OS thread creation or callback/native worker failure.
pub enum CallbackExecutorTaskError {
    /// Native setup or Node/group membership validation failed.
    #[error("Could not prepare native callback scheduling: {0}")]
    Native(#[from] Error),
    #[error("Could not create callback worker {worker}: {reason}")]
    /// Thread creation failed; already-created workers are canceled cooperatively.
    ThreadSpawn { worker: usize, reason: String },
    #[error("Callback worker {worker} failed: {source}")]
    /// A worker failed. Error completion collects all workers and releases
    /// recovered Node/group ownership; the lowest failing worker index wins.
    Worker {
        worker: usize,
        #[source]
        source: CallbackWorkerError,
    },
}

#[must_use = "retain or await the task; dropping it cancels native and callback work"]
/// Runtime-independent completion of the native job and callback groups.
///
/// Normal stop/context shutdown returns a stopped CallbackExecutor. Dropping
/// a pending task cancels work and detaches threads without joining on the
/// runtime thread. A synchronous handler must return before its worker exits.
pub struct CallbackExecutorTask {
    scheduler: Arc<scheduler::Scheduler>,
    receivers: Vec<oneshot::Receiver<std::result::Result<(), CallbackWorkerError>>>,
    collected: Vec<bool>,
    threads: Vec<JoinHandle<()>>,
    errors: Vec<Option<CallbackWorkerError>>,
    control: CallbackExecutorHandle,
    workers: usize,
    group_indexes: Option<HashMap<CallbackGroupId, usize>>,
    completed: bool,
}

pub(super) fn spawn(
    mut executor: CallbackExecutor,
) -> std::result::Result<CallbackExecutorTask, CallbackExecutorTaskError> {
    let bindings = executor.native.callback_group_bindings();
    if bindings
        .iter()
        .any(|(_, group)| !executor.group_indexes.contains_key(group))
    {
        return Err(Error::RCL_RET_INVALID_ARGUMENT.into());
    }
    let nodes: HashSet<_> = bindings.iter().map(|(node, _)| *node).collect();
    if executor
        .groups
        .iter()
        .flat_map(|group| &group.sources)
        .any(|(node, control)| {
            control
                .upgrade()
                .is_some_and(|control| !control.is_closed())
                && !nodes.contains(node)
        })
    {
        return Err(Error::RCL_RET_INVALID_ARGUMENT.into());
    }
    let control = executor.handle();
    let CallbackExecutor {
        native,
        groups,
        group_indexes,
        workers,
        running,
    } = executor;
    let count = workers.min(1 + groups.iter().filter(|group| !group.is_empty()).count());
    let scheduler = scheduler::Scheduler::new(Some(native), groups, count)?;
    *running.lock() = Arc::downgrade(&scheduler);
    // The weak running link exists before threads start. Recheck persistent
    // native cancellation to close a request racing scheduler construction.
    if control.is_cancelled() {
        scheduler.cancel()?;
    }
    let mut task = CallbackExecutorTask {
        scheduler,
        receivers: Vec::new(),
        collected: vec![false; count],
        threads: Vec::new(),
        errors: (0..count).map(|_| None).collect(),
        control,
        workers,
        group_indexes: Some(group_indexes),
        completed: false,
    };
    for index in 0..count {
        let scheduler = task.scheduler.clone();
        let (sender, receiver) = oneshot::channel();
        let worker = thread::Builder::new()
            .name(format!("r2r-callback-{index}"))
            .spawn(move || {
                let result = scheduler.run(index);
                let _ = sender.send(result);
            })
            .map_err(|error| CallbackExecutorTaskError::ThreadSpawn {
                worker: index,
                reason: error.to_string(),
            })?;
        task.receivers.push(receiver);
        task.threads.push(worker);
    }
    Ok(task)
}
impl CallbackExecutorTask {
    /// Weak native/group cancellation.
    pub fn handle(&self) -> CallbackExecutorHandle {
        self.control.clone()
    }
    /// Number of OS workers actually started, including native waiting.
    pub fn worker_count(&self) -> usize {
        self.threads.len()
    }
    /// Requests a stop without awaiting current dispatch.
    pub fn cancel(&self) -> Result<bool> {
        self.control.cancel()
    }
    /// Whether every native thread has finished, including teardown. This
    /// query does not poll completion or release recovered ownership.
    pub fn is_finished(&self) -> bool {
        self.threads.iter().all(JoinHandle::is_finished)
    }
    /// Cancels and asynchronously recovers Nodes, groups and live handler state.
    /// Cancellation stays set until CallbackExecutor::reset_cancel.
    pub async fn stop(self) -> std::result::Result<CallbackExecutor, CallbackExecutorTaskError> {
        self.cancel()?;
        self.await
    }
}
impl Future for CallbackExecutorTask {
    type Output = std::result::Result<CallbackExecutor, CallbackExecutorTaskError>;
    fn poll(self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let task = self.get_mut();
        assert!(!task.completed, "callback completion was polled twice");
        let mut pending = false;
        for index in 0..task.receivers.len() {
            if task.collected[index] {
                continue;
            }
            match Pin::new(&mut task.receivers[index]).poll(context) {
                Poll::Pending => pending = true,
                Poll::Ready(result) => {
                    task.collected[index] = true;
                    if let Err(source) = result.unwrap_or(Err(CallbackWorkerError::Panic)) {
                        task.errors[index] = Some(source);
                        let _ = task.scheduler.cancel();
                    }
                }
            }
        }
        if pending {
            return Poll::Pending;
        }
        task.completed = true;
        let (native, groups) = task.scheduler.recover();
        if let Some((worker, source)) = task
            .errors
            .iter_mut()
            .enumerate()
            .find_map(|(worker, error)| error.take().map(|error| (worker, error)))
        {
            drop(groups);
            drop(native);
            return Poll::Ready(Err(CallbackExecutorTaskError::Worker { worker, source }));
        }
        Poll::Ready(Ok(CallbackExecutor {
            native: native.expect("native callback ownership missing"),
            groups,
            group_indexes: task.group_indexes.take().unwrap(),
            workers: task.workers,
            running: task.control.running.clone(),
        }))
    }
}
impl Drop for CallbackExecutorTask {
    fn drop(&mut self) {
        if !self.completed {
            let _ = self.scheduler.cancel();
        }
    }
}
