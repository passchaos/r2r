//! Runtime-agnostic futures retained by exclusive or bounded reentrant groups.

use super::*;
use futures::task::AtomicWaker;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) struct TaskControl {
    cancelled: AtomicBool,
    finished: AtomicBool,
    wake: AtomicWaker,
}

impl TaskControl {
    fn cancel(&self) -> bool {
        if self.finished.load(Ordering::Acquire) {
            return false;
        }
        let changed = !self.cancelled.swap(true, Ordering::AcqRel);
        if changed {
            self.wake.wake();
        }
        changed
    }
}

/// RAII control for one future retained by a [`CallbackGroup`] or
/// [`ReentrantCallbackGroup`].
///
/// Dropping or cancelling the handle wakes the group and drops the application future on the
/// group worker at its next poll. [`detach`](Self::detach) leaves ownership entirely with the
/// group. Group/executor cancellation pauses the task without dropping it; after
/// [`CallbackExecutor::reset_cancel`] and restart it continues with the same future state.
pub struct CallbackGroupTask {
    control: Arc<TaskControl>,
    cancel_on_drop: bool,
}

impl CallbackGroupTask {
    /// Requests task cancellation. Returns `true` only for the first live request.
    pub fn cancel(&self) -> bool {
        self.control.cancel()
    }

    /// Whether individual task cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.control.cancelled.load(Ordering::Acquire)
    }

    /// Whether the future completed or was retired after individual cancellation.
    pub fn is_finished(&self) -> bool {
        self.control.finished.load(Ordering::Acquire)
    }

    /// Lets the group retain the future without an external RAII cancellation handle.
    pub fn detach(mut self) {
        self.cancel_on_drop = false;
    }
}

impl Drop for CallbackGroupTask {
    fn drop(&mut self) {
        if self.cancel_on_drop {
            self.control.cancel();
        }
    }
}

struct RegisteredTask<F> {
    future: Option<Pin<Box<F>>>,
    control: Arc<TaskControl>,
    group_cancelled: Arc<std::sync::atomic::AtomicBool>,
}

impl<F> Future for RegisteredTask<F>
where
    F: Future<Output = Result<()>>,
{
    type Output = Result<()>;

    fn poll(mut self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Self::Output> {
        self.control.wake.register(context.waker());
        if self.group_cancelled.load(Ordering::Acquire) {
            return Poll::Pending;
        }
        if self.control.cancelled.load(Ordering::Acquire) {
            self.future.take();
            self.control.finished.store(true, Ordering::Release);
            return Poll::Ready(Ok(()));
        }
        let result = self
            .future
            .as_mut()
            .expect("finished callback group task polled again")
            .as_mut()
            .poll(context);
        if result.is_ready() {
            self.future.take();
            self.control.finished.store(true, Ordering::Release);
        }
        result
    }
}

impl<F> Drop for RegisteredTask<F> {
    fn drop(&mut self) {
        self.control.finished.store(true, Ordering::Release);
        self.control.wake.take();
    }
}

type GroupCallbacks = FuturesUnordered<Pin<Box<dyn Future<Output = Result<()>> + Send>>>;

fn register_task<F>(
    tasks: &mut Vec<std::sync::Weak<TaskControl>>, callbacks: &mut GroupCallbacks,
    group_cancelled: Arc<std::sync::atomic::AtomicBool>, future: F,
) -> CallbackGroupTask
where
    F: Future<Output = Result<()>> + Send + 'static,
{
    let control = Arc::new(TaskControl {
        cancelled: AtomicBool::new(false),
        finished: AtomicBool::new(false),
        wake: AtomicWaker::new(),
    });
    tasks.retain(|task| {
        task.upgrade()
            .is_some_and(|task| !task.finished.load(Ordering::Acquire))
    });
    tasks.push(Arc::downgrade(&control));
    callbacks.push(Box::pin(RegisteredTask {
        future: Some(Box::pin(future)),
        control: control.clone(),
        group_cancelled,
    }));
    CallbackGroupTask {
        control,
        cancel_on_drop: true,
    }
}

fn wake_tasks(tasks: &mut Vec<std::sync::Weak<TaskControl>>) {
    tasks.retain(|task| {
        let Some(task) = task.upgrade() else {
            return false;
        };
        task.wake.wake();
        !task.finished.load(Ordering::Acquire)
    });
}

impl CallbackGroup {
    pub(super) fn wake_tasks(&mut self) {
        wake_tasks(&mut self.tasks);
    }

    /// Retains a runtime-agnostic application future in this mutually exclusive group.
    ///
    /// The future is polled with the same scheduler token as subscription, service and timer
    /// handlers, so its synchronous work cannot overlap them. Awaiting an action stream, client
    /// result, external I/O or another Rust future yields the group normally.
    ///
    /// ```compile_fail
    /// use std::rc::Rc;
    /// let mut group = r2r::CallbackGroup::new();
    /// let local = Rc::new(1);
    /// group.spawn_task(async move {
    ///     futures::future::ready(()).await;
    ///     drop(local);
    ///     Ok(())
    /// });
    /// ```
    pub fn spawn_task<F>(&mut self, future: F) -> CallbackGroupTask
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        register_task(&mut self.tasks, &mut self.callbacks, self.cancelled.clone(), future)
    }
}

impl ReentrantCallbackGroup {
    pub(super) fn wake_tasks(&mut self) {
        wake_tasks(&mut self.tasks);
    }

    /// Retains a runtime-agnostic application future in one bounded reentrant lane.
    ///
    /// Registration chooses the lane with the fewest retained callbacks. Each future is polled
    /// serially with the subscription, service, timer and task work in that lane, while different
    /// lanes can make progress concurrently up to [`concurrency`](Self::concurrency). A future is
    /// never polled concurrently and therefore needs `Send`, but not `Sync`.
    ///
    /// Individual task cancellation, executor stop/reset and error propagation follow the same
    /// contract as [`CallbackGroup::spawn_task`].
    ///
    /// ```compile_fail
    /// use std::rc::Rc;
    /// let mut group = r2r::ReentrantCallbackGroup::new(2).unwrap();
    /// let local = Rc::new(1);
    /// group.spawn_task(async move {
    ///     futures::future::ready(()).await;
    ///     drop(local);
    ///     Ok(())
    /// });
    /// ```
    pub fn spawn_task<F>(&mut self, future: F) -> CallbackGroupTask
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        let lane = self
            .lanes
            .iter()
            .enumerate()
            .min_by_key(|(index, lane)| (lane.len(), *index))
            .map(|(index, _)| index)
            .expect("reentrant group always has at least one lane");
        register_task(&mut self.tasks, &mut self.lanes[lane], self.cancelled.clone(), future)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::channel::oneshot;
    use std::sync::atomic::AtomicUsize;

    struct Capture(Arc<AtomicUsize>);
    impl Drop for Capture {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Release);
        }
    }

    fn poll(group: &mut CallbackGroup) -> Poll<Result<()>> {
        let waker = futures::task::noop_waker();
        group.poll_callbacks(&mut TaskContext::from_waker(&waker))
    }

    fn poll_reentrant(group: &mut ReentrantCallbackGroup) -> Poll<Result<()>> {
        let waker = futures::task::noop_waker();
        let mut context = TaskContext::from_waker(&waker);
        let mut pending = false;
        for lane in &mut group.lanes {
            loop {
                match Pin::new(&mut *lane).poll_next(&mut context) {
                    Poll::Pending => {
                        pending = true;
                        break;
                    }
                    Poll::Ready(None) => break,
                    Poll::Ready(Some(Ok(()))) => {}
                    Poll::Ready(Some(Err(error))) => return Poll::Ready(Err(error)),
                }
            }
        }
        if pending {
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }

    #[test]
    fn task_handle_cancels_and_retires_the_future_on_group_poll() {
        let retired = Arc::new(AtomicUsize::new(0));
        let capture = Capture(retired.clone());
        let mut group = CallbackGroup::new();
        let task = group.spawn_task(async move {
            let _capture = capture;
            futures::future::pending::<()>().await;
            Ok(())
        });

        assert!(poll(&mut group).is_pending());
        assert!(task.cancel());
        assert!(!task.cancel());
        assert!(matches!(poll(&mut group), Poll::Ready(Ok(()))));
        assert!(task.is_cancelled());
        assert!(task.is_finished());
        assert_eq!(retired.load(Ordering::Acquire), 1);
    }

    #[test]
    fn detached_task_completes_under_group_ownership() {
        let (complete, completed) = oneshot::channel();
        let retired = Arc::new(AtomicUsize::new(0));
        let capture = Capture(retired.clone());
        let mut group = CallbackGroup::new();
        group
            .spawn_task(async move {
                let _capture = capture;
                completed.await.map_err(|_| Error::RCL_RET_ERROR)?;
                Ok(())
            })
            .detach();

        assert!(poll(&mut group).is_pending());
        complete.send(()).unwrap();
        assert!(matches!(poll(&mut group), Poll::Ready(Ok(()))));
        assert_eq!(retired.load(Ordering::Acquire), 1);
    }

    #[test]
    fn executor_cancellation_pauses_task_without_losing_future_state() {
        let polls = Arc::new(AtomicUsize::new(0));
        let count = polls.clone();
        let mut group = CallbackGroup::new();
        let task = group.spawn_task(futures::future::poll_fn(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
            Poll::<Result<()>>::Pending
        }));

        group.cancelled.store(true, Ordering::Release);
        assert!(poll(&mut group).is_pending());
        assert_eq!(polls.load(Ordering::Relaxed), 0);
        assert!(!task.is_finished());

        group.cancelled.store(false, Ordering::Release);
        group.wake_tasks();
        assert!(poll(&mut group).is_pending());
        assert_eq!(polls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn task_error_is_reported_by_the_group_poll() {
        let mut group = CallbackGroup::new();
        group
            .spawn_task(async { Err(Error::RCL_RET_BAD_ALLOC) })
            .detach();
        assert!(matches!(poll(&mut group), Poll::Ready(Err(Error::RCL_RET_BAD_ALLOC))));
    }

    #[test]
    fn completed_handles_do_not_accumulate_group_wake_bookkeeping() {
        let mut group = CallbackGroup::new();
        let completed = group.spawn_task(async { Ok(()) });
        assert!(matches!(poll(&mut group), Poll::Ready(Ok(()))));
        assert!(completed.is_finished());
        assert_eq!(group.tasks.len(), 1);

        let pending = group.spawn_task(futures::future::pending::<Result<()>>());
        assert_eq!(group.tasks.len(), 1);
        assert!(poll(&mut group).is_pending());
        assert!(pending.cancel());
        assert!(matches!(poll(&mut group), Poll::Ready(Ok(()))));
        assert!(completed.is_finished());
    }

    #[test]
    fn reentrant_tasks_balance_stably_across_bounded_lanes_and_cancel() {
        let mut group = ReentrantCallbackGroup::new(2).unwrap();
        let tasks = (0..5)
            .map(|_| group.spawn_task(futures::future::pending::<Result<()>>()))
            .collect::<Vec<_>>();
        assert_eq!(
            group
                .lanes
                .iter()
                .map(|lane| lane.len())
                .collect::<Vec<_>>(),
            [3, 2]
        );
        assert!(poll_reentrant(&mut group).is_pending());

        for task in &tasks {
            assert!(task.cancel());
        }
        assert!(matches!(poll_reentrant(&mut group), Poll::Ready(Ok(()))));
        assert!(tasks.iter().all(CallbackGroupTask::is_finished));
        assert!(group.is_empty());
    }

    #[test]
    fn reentrant_group_cancellation_pauses_and_reset_wakes_task_state() {
        let polls = Arc::new(AtomicUsize::new(0));
        let count = polls.clone();
        let mut group = ReentrantCallbackGroup::new(2).unwrap();
        let task = group.spawn_task(futures::future::poll_fn(move |_| {
            count.fetch_add(1, Ordering::Relaxed);
            Poll::<Result<()>>::Pending
        }));

        group.cancelled.store(true, Ordering::Release);
        assert!(poll_reentrant(&mut group).is_pending());
        assert_eq!(polls.load(Ordering::Relaxed), 0);
        assert!(!task.is_finished());

        group.cancelled.store(false, Ordering::Release);
        group.wake_tasks();
        assert!(poll_reentrant(&mut group).is_pending());
        assert_eq!(polls.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn reentrant_task_error_is_reported_by_its_lane() {
        let mut group = ReentrantCallbackGroup::new(2).unwrap();
        group
            .spawn_task(async { Err(Error::RCL_RET_BAD_ALLOC) })
            .detach();
        assert!(matches!(poll_reentrant(&mut group), Poll::Ready(Err(Error::RCL_RET_BAD_ALLOC))));
    }
}
