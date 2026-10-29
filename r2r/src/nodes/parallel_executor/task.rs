use super::*;
use std::{
    future::Future,
    pin::Pin,
    task::{Context as TaskContext, Poll},
};

/// Startup, dispatch or cancellation errors from bounded native execution.
#[derive(Debug, thiserror::Error)]
pub enum ParallelExecutorTaskError {
    /// One worker failed. After running-worker failures, peers are canceled and
    /// awaited before completion; the lowest failing worker index is reported.
    #[error("Parallel native worker {worker} failed: {source}")]
    Worker {
        /// The configured zero-based group index, including skipped empty groups.
        worker: usize,
        /// The native operation, thread startup or callback panic error.
        #[source]
        source: ExecutorTaskError,
    },
    /// Explicit cancellation could not trigger a worker's native guard.
    #[error("Parallel executor cancellation failed: {0}")]
    Cancellation(#[source] Error),
}

/// Runtime-independent completion of all native worker groups.
///
/// Awaiting normal cancellation or Context shutdown returns the configured
/// [`ParallelExecutor`] with stable Node IDs and placement. Worker failure
/// cancels peers directly from the failing native thread, even if this task is
/// never polled. Completion waits for every worker and releases all recovered
/// Nodes before returning an error. Cancellation is cooperative: a synchronous
/// callback must return before its worker can finish.
///
/// Dropping a pending task requests cancellation and detaches the native threads
/// without blocking the caller. Application streams/futures remain on their
/// chosen runtime and can keep non-Send application state there.
#[must_use = "retain or await the task; dropping it cancels every native worker"]
pub struct ParallelExecutorTask {
    tasks: Vec<Option<ExecutorTask>>,
    collected: Vec<bool>,
    stopped: Vec<Option<Executor>>,
    errors: Vec<Option<ParallelExecutorTaskError>>,
    control: ParallelExecutorHandle,
    placements: Option<HashMap<ExecutorNodeId, usize>>,
    next_worker: usize,
    complete: bool,
}

pub(super) fn spawn(
    executor: ParallelExecutor,
) -> std::result::Result<ParallelExecutorTask, ParallelExecutorTaskError> {
    let ParallelExecutor {
        mut workers,
        placements,
        next_worker,
    } = executor;
    let count = workers.len();
    let empty = placements.is_empty();
    let controls = workers
        .iter_mut()
        .map(Executor::handle)
        .collect::<Vec<_>>()
        .into();
    let mut task = ParallelExecutorTask {
        tasks: (0..count).map(|_| None).collect(),
        collected: vec![false; count],
        stopped: (0..count).map(|_| None).collect(),
        errors: (0..count).map(|_| None).collect(),
        control: ParallelExecutorHandle { workers: controls },
        placements: Some(placements),
        next_worker,
        complete: false,
    };
    for (index, worker) in workers.into_iter().enumerate() {
        if worker.is_empty() && !(empty && index == 0) {
            task.stopped[index] = Some(worker);
            task.collected[index] = true;
            continue;
        }
        let peers = task.control.clone();
        match worker.spawn_with_failure_notify(move || {
            let _ = peers.cancel();
        }) {
            Ok(worker) => task.tasks[index] = Some(worker),
            Err(source) => {
                let _ = task.control.cancel();
                return Err(ParallelExecutorTaskError::Worker {
                    worker: index,
                    source,
                });
            }
        }
    }
    Ok(task)
}

impl ParallelExecutorTask {
    /// The number of native workers launched, excluding skipped empty groups.
    pub fn worker_count(&self) -> usize {
        self.tasks.iter().filter(|task| task.is_some()).count()
    }
    /// A weak aggregate cancellation handle.
    pub fn handle(&self) -> ParallelExecutorHandle {
        self.control.clone()
    }
    /// Requests cancellation without awaiting callback/worker completion.
    pub fn cancel(&self) -> Result<bool> {
        self.control.cancel()
    }
    /// Whether all launched native threads have finished, including teardown.
    /// This query does not poll completion or release returned ownership.
    pub fn is_finished(&self) -> bool {
        self.tasks
            .iter()
            .all(|task| task.as_ref().is_none_or(ExecutorTask::is_finished))
    }
    /// Requests cancellation and asynchronously recovers every Node group.
    /// Cancellation remains set until [`ParallelExecutor::reset_cancel`].
    pub async fn stop(self) -> std::result::Result<ParallelExecutor, ParallelExecutorTaskError> {
        self.cancel()
            .map_err(ParallelExecutorTaskError::Cancellation)?;
        self.await
    }
}

impl Future for ParallelExecutorTask {
    type Output = std::result::Result<ParallelExecutor, ParallelExecutorTaskError>;
    fn poll(self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let task = self.get_mut();
        assert!(!task.complete, "parallel completion was polled twice");
        let mut failure = false;
        let mut pending = false;
        for index in 0..task.tasks.len() {
            if task.collected[index] {
                continue;
            }
            let worker = task.tasks[index].as_mut().expect("worker task missing");
            match Pin::new(worker).poll(context) {
                Poll::Pending => pending = true,
                Poll::Ready(Ok(worker)) => {
                    task.stopped[index] = Some(worker);
                    task.collected[index] = true;
                }
                Poll::Ready(Err(source)) => {
                    task.errors[index] = Some(ParallelExecutorTaskError::Worker {
                        worker: index,
                        source,
                    });
                    task.collected[index] = true;
                    failure = true;
                }
            }
        }
        if failure {
            let _ = task.control.cancel();
        }
        if pending {
            return Poll::Pending;
        }
        task.complete = true;
        if let Some(error) = task.errors.iter_mut().find_map(Option::take) {
            task.stopped.clear();
            return Poll::Ready(Err(error));
        }
        let workers = std::mem::take(&mut task.stopped)
            .into_iter()
            .map(|worker| worker.expect("stopped worker missing"))
            .collect();
        Poll::Ready(Ok(ParallelExecutor {
            workers,
            placements: task.placements.take().unwrap(),
            next_worker: task.next_worker,
        }))
    }
}
impl Drop for ParallelExecutorTask {
    fn drop(&mut self) {
        if !self.complete {
            let _ = self.control.cancel();
        }
    }
}
