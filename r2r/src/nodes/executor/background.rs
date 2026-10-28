//! Runtime-independent ownership and cancellation for a native executor thread.

use std::{
    fmt,
    future::Future,
    panic::{catch_unwind, AssertUnwindSafe},
    pin::Pin,
    task::{Context, Poll},
    thread::{self, JoinHandle},
};

use futures::channel::oneshot;

use super::super::future_executor::{FutureDriver, FutureGuard};
use super::{Executor, ExecutorHandle};
use crate::{Error, Result};

/// Errors specific to owned background execution.
#[derive(Debug, thiserror::Error)]
pub enum ExecutorTaskError {
    /// A native ROS operation failed.
    #[error(transparent)]
    Native(#[from] Error),
    /// The operating system could not create the native worker.
    #[error("Could not start native executor thread: {reason}")]
    ThreadSpawn { reason: String },
    /// Native dispatch or an inline user callback unwound with a panic.
    #[error("Native executor thread panicked")]
    ThreadPanicked,
}

type TaskResult<T> = std::result::Result<T, ExecutorTaskError>;

fn complete(
    executor: Executor, sender: oneshot::Sender<TaskResult<Executor>>,
    outcome: thread::Result<Result<()>>,
) {
    let error = match outcome {
        Ok(Ok(())) => {
            let _ = sender.send(Ok(executor));
            return;
        }
        Ok(Err(error)) => ExecutorTaskError::Native(error),
        Err(_) => ExecutorTaskError::ThreadPanicked,
    };
    drop(executor);
    let _ = sender.send(Err(error));
}

/// An owned background executor whose completion can be awaited by any runtime.
///
/// One native thread drives every owned node through the executor's shared wait.
/// Streams/futures can be awaited on Tokio, async-std or a local futures executor.
/// [`Self::stop`] requests cancellation and returns the stopped [`Executor`];
/// reset its cancellation before reuse. Dropping a pending task requests a stop
/// and lets the native worker release its owned nodes without blocking the caller.
/// Cancellation is cooperative: the current native dispatch/callback must return.
///
/// The task has exclusive node ownership while running. Configure entities before
/// spawning, or after retrieving the stopped executor. Endpoint handles retain
/// their existing thread-safe ownership rules.
#[must_use = "retain or await the task; dropping it stops the background executor"]
pub struct ExecutorTask {
    completion: oneshot::Receiver<TaskResult<Executor>>,
    control: ExecutorHandle,
    thread: Option<JoinHandle<()>>,
    completed: bool,
}

impl fmt::Debug for ExecutorTask {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ExecutorTask")
            .field("finished", &self.is_finished())
            .field("completed", &self.completed)
            .finish()
    }
}

impl ExecutorTask {
    /// A weak cancellation handle for this background execution.
    pub fn handle(&self) -> ExecutorHandle {
        self.control.clone()
    }

    /// Request a stop without waiting for native dispatch to finish.
    /// An expired executor returns `false`, matching [`ExecutorHandle::cancel`].
    pub fn cancel(&self) -> Result<bool> {
        self.control.cancel()
    }

    /// Whether the native worker has finished, including thread teardown.
    pub fn is_finished(&self) -> bool {
        self.thread.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Stop native execution and asynchronously recover the owned executor.
    /// Cancellation remains set until [`Executor::reset_cancel`].
    pub async fn stop(self) -> std::result::Result<Executor, ExecutorTaskError> {
        self.cancel()?;
        self.await
    }
}

impl Future for ExecutorTask {
    type Output = std::result::Result<Executor, ExecutorTaskError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        match Pin::new(&mut this.completion).poll(context) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(result) => {
                this.completed = true;
                Poll::Ready(result.unwrap_or(Err(ExecutorTaskError::ThreadPanicked)))
            }
        }
    }
}

impl Drop for ExecutorTask {
    fn drop(&mut self) {
        if !self.completed {
            let _ = self.control.cancel();
        }
        // JoinHandle::drop detaches. Native cancellation wakes the shared wait;
        // no runtime worker is synchronously blocked by thread/TLS teardown.
    }
}

impl Executor {
    /// Start one native wait thread and return its runtime-independent completion task.
    ///
    /// Thread creation is fallible. Normal cancellation returns the owned executor;
    /// native errors and callback panics release nodes before reporting failure.
    /// No periodic spin timer or per-node worker is created.
    pub fn spawn(mut self) -> std::result::Result<ExecutorTask, ExecutorTaskError> {
        if !self.context.is_valid() {
            return Err(Error::RCL_RET_NOT_INIT.into());
        }
        let control = self.handle();
        let guard = match &self.future_wake_guard {
            Some(guard) => guard.clone(),
            None => {
                let guard = FutureGuard::new(&self.context)?;
                self.cache_future_guard(guard.clone());
                guard
            }
        };
        let wake = guard.observer_waker();
        for entry in &mut self.nodes {
            entry.node.set_background_wake(Some(wake.clone()));
        }
        let (sender, completion) = oneshot::channel();
        let thread = thread::Builder::new()
            .name("r2r-executor".into())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| self.spin()));
                // A failed send drops returned ownership on this native thread.
                complete(self, sender, result);
            })
            .map_err(|error| ExecutorTaskError::ThreadSpawn {
                reason: error.to_string(),
            })?;
        Ok(ExecutorTask {
            completion,
            control,
            thread: Some(thread),
            completed: false,
        })
    }
}

#[cfg(test)]
mod tests;
