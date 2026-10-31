use super::*;
use std::sync::atomic::AtomicBool;

type Callbacks = FuturesUnordered<Pin<Box<dyn Future<Output = Result<()>> + Send>>>;

/// Bounded concurrent invocation of shared Fn + Send + Sync handlers.
/// Native ownership remains exclusive; inputs and per-lane reply buffers are owned.
///
/// The limit applies across all this group's sources. Actual parallelism also
/// depends on the executor's total worker budget, which includes native waiting.
/// Inputs can complete out of order. Shared handlers must synchronize their own
/// application state. Handler bodies hold no executor/native operation lock.
///
/// ```no_run
/// # fn install(node: &mut r2r::Node) -> r2r::Result<r2r::ReentrantCallbackGroup> {
/// let mut group = r2r::ReentrantCallbackGroup::new(4)?;
/// let subscription = node.subscribe_reentrant_callback::<r2r::std_msgs::msg::UInt64, _>(
///     &mut group, "input", r2r::QosProfile::default(), |message, _| {
///         println!("{}", message.data);
///         Ok(())
///     },
/// )?;
/// # let _ = subscription;
/// # Ok(group)
/// # }
/// ```
///
/// Mutable unsynchronized handler captures cannot enter a reentrant group:
///
/// ```compile_fail
/// fn install(node: &mut r2r::Node, group: &mut r2r::ReentrantCallbackGroup) {
///     let mut count = 0;
///     node.subscribe_reentrant_callback::<r2r::std_msgs::msg::UInt64, _>(
///         group, "input", r2r::QosProfile::default(), move |_, _| {
///             count += 1;
///             Ok(())
///         },
///     ).unwrap();
/// }
/// ```
pub struct ReentrantCallbackGroup {
    pub(super) id: CallbackGroupId,
    pub(super) cancelled: Arc<AtomicBool>,
    pub(super) sources: Vec<(uuid::Uuid, std::sync::Weak<dyn input::Control>)>,
    pub(super) tasks: Vec<std::sync::Weak<task_registration::TaskControl>>,
    pub(super) lanes: Vec<Callbacks>,
}
impl ReentrantCallbackGroup {
    /// Creates a maximum invocation concurrency without spawning threads.
    pub fn new(concurrency: usize) -> Result<Self> {
        if concurrency == 0 || concurrency >= usize::MAX >> 2 {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        Ok(Self {
            id: CallbackGroupId(uuid::Uuid::new_v4()),
            cancelled: Arc::new(AtomicBool::new(false)),
            sources: Vec::new(),
            tasks: Vec::new(),
            lanes: (0..concurrency).map(|_| Callbacks::new()).collect(),
        })
    }
    /// Stable identity retained after stop and restart.
    pub fn id(&self) -> CallbackGroupId {
        self.id
    }
    /// Maximum simultaneous invocations across all sources in this group.
    pub fn concurrency(&self) -> usize {
        self.lanes.len()
    }
    /// Whether every invocation lane has retired its source registrations.
    pub fn is_empty(&self) -> bool {
        self.lanes.iter().all(Callbacks::is_empty)
    }
    pub(super) fn prepare_inputs<T, F>(
        &self, capacity: usize, callbacks: Vec<F>,
    ) -> Result<input::PreparedInputs<T, F>> {
        if callbacks.len() != self.lanes.len() {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        input::channel_many(capacity, self.cancelled.clone(), callbacks)
    }
    pub(super) fn commit_inputs<T, F>(&mut self, consumers: Vec<input::Consumer<T, F>>)
    where
        T: Send + 'static,
        F: FnMut(T) -> Result<()> + Send + 'static,
    {
        assert_eq!(consumers.len(), self.lanes.len());
        for (lane, consumer) in self.lanes.iter_mut().zip(consumers) {
            lane.push(Box::pin(consumer));
        }
    }
}

pub(super) enum OwnedGroup {
    Exclusive(CallbackGroup),
    Reentrant(ReentrantCallbackGroup),
}
impl From<CallbackGroup> for OwnedGroup {
    fn from(group: CallbackGroup) -> Self {
        Self::Exclusive(group)
    }
}
impl From<ReentrantCallbackGroup> for OwnedGroup {
    fn from(group: ReentrantCallbackGroup) -> Self {
        Self::Reentrant(group)
    }
}
impl OwnedGroup {
    pub(super) fn id(&self) -> CallbackGroupId {
        match self {
            Self::Exclusive(group) => group.id(),
            Self::Reentrant(group) => group.id(),
        }
    }
    pub(super) fn cancelled(&self) -> &Arc<AtomicBool> {
        match self {
            Self::Exclusive(group) => &group.cancelled,
            Self::Reentrant(group) => &group.cancelled,
        }
    }
    pub(super) fn wake_tasks(&mut self) {
        match self {
            Self::Exclusive(group) => group.wake_tasks(),
            Self::Reentrant(group) => group.wake_tasks(),
        }
    }
    pub(super) fn sources(&self) -> &[(uuid::Uuid, std::sync::Weak<dyn input::Control>)] {
        match self {
            Self::Exclusive(group) => &group.sources,
            Self::Reentrant(group) => &group.sources,
        }
    }
    pub(super) fn units(&self) -> usize {
        match self {
            Self::Exclusive(group) => usize::from(!group.is_empty()),
            Self::Reentrant(group) => group.lanes.iter().filter(|lane| !lane.is_empty()).count(),
        }
    }
    pub(super) fn exclusive_mut(&mut self) -> Option<&mut CallbackGroup> {
        match self {
            Self::Exclusive(group) => Some(group),
            _ => None,
        }
    }
    pub(super) fn reentrant_mut(&mut self) -> Option<&mut ReentrantCallbackGroup> {
        match self {
            Self::Reentrant(group) => Some(group),
            _ => None,
        }
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Exclusive(group) => group.len(),
            Self::Reentrant(group) => group.lanes.iter().map(Callbacks::len).sum(),
        }
    }
}

pub(super) struct Lane {
    pub(super) group: usize,
    pub(super) index: usize,
    pub(super) id: CallbackGroupId,
    pub(super) callbacks: Callbacks,
}
impl Lane {
    pub(super) fn poll(&mut self, context: &mut TaskContext<'_>) -> Poll<Result<()>> {
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
