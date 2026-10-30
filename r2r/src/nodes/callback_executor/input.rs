//! Preallocated owned-input ring; worker execution and retirement hold no queue lock.

use super::*;
use crossbeam_queue::ArrayQueue;
use futures::task::AtomicWaker;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use std::task::Waker;

struct Shared<T> {
    queue: ArrayQueue<T>,
    gate: AtomicU8,
    consumer_wake: ConsumerWakes,
    consumers: AtomicUsize,
    producer_waiting: AtomicBool,
    native_wake: Mutex<Option<Waker>>,
}

enum ConsumerWakes {
    Single(AtomicWaker),
    Multiple(Box<[AtomicWaker]>),
}
impl ConsumerWakes {
    fn at(&self, index: usize) -> &AtomicWaker {
        match self {
            Self::Single(wake) => {
                debug_assert_eq!(index, 0);
                wake
            }
            Self::Multiple(wakes) => &wakes[index],
        }
    }
    fn wake(&self) {
        match self {
            Self::Single(wake) => wake.wake(),
            Self::Multiple(wakes) => {
                for wake in wakes {
                    wake.wake();
                }
            }
        }
    }
}

pub(in crate::nodes) trait Control: Send + Sync {
    fn is_closed(&self) -> bool;
    fn close(&self);
    fn set_native_wake(&self, wake: Option<Waker>);
}

impl<T: Send> Control for Shared<T> {
    fn is_closed(&self) -> bool {
        self.gate.load(Ordering::Acquire) & CLOSED != 0
    }
    fn close(&self) {
        self.gate.fetch_or(CLOSED, Ordering::AcqRel);
        self.consumer_wake.wake();
        self.wake_native();
    }
    fn set_native_wake(&self, wake: Option<Waker>) {
        *self.native_wake.lock() = wake;
    }
}

const PUBLISHING: u8 = 1;
const CLOSED: u8 = 2;

struct Publication<'a, T>(&'a Shared<T>);
impl<T> Drop for Publication<'_, T> {
    fn drop(&mut self) {
        match self
            .0
            .gate
            .compare_exchange(PUBLISHING, 0, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => {}
            Err(state) => {
                assert_eq!(state, PUBLISHING | CLOSED);
                self.0.gate.store(CLOSED, Ordering::Release);
                // Close raced an admitted producer. The producer owns final
                // retirement so its late publication cannot outlive the receiver.
                while let Some(input) = self.0.queue.pop() {
                    drop(input);
                }
            }
        }
    }
}

pub(crate) struct Producer<T> {
    shared: Arc<Shared<T>>,
}

pub(super) struct Consumer<T, F> {
    shared: Arc<Shared<T>>,
    cancelled: Arc<AtomicBool>,
    callback: F,
    index: usize,
}

pub(super) type PreparedInputs<T, F> = (Producer<T>, Vec<Consumer<T, F>>);

// No field is structurally pinned or exposed through a pin projection. The
// callback is always invoked through FnMut, which requires an ordinary mutable borrow.
impl<T, F> Unpin for Consumer<T, F> {}

pub(super) fn channel<T, F>(
    capacity: usize, cancelled: Arc<AtomicBool>, callback: F,
) -> Result<(Producer<T>, Consumer<T, F>)> {
    if capacity == 0 || capacity >= usize::MAX >> 2 {
        return Err(Error::RCL_RET_INVALID_ARGUMENT);
    }
    let shared = shared(capacity, 1)?;
    Ok((
        Producer {
            shared: shared.clone(),
        },
        Consumer {
            shared,
            cancelled,
            callback,
            index: 0,
        },
    ))
}

fn shared<T>(capacity: usize, consumers: usize) -> Result<Arc<Shared<T>>> {
    if capacity == 0 || capacity >= usize::MAX >> 2 || consumers == 0 {
        return Err(Error::RCL_RET_INVALID_ARGUMENT);
    }
    Ok(Arc::new(Shared {
        queue: ArrayQueue::new(capacity),
        gate: AtomicU8::new(0),
        consumer_wake: if consumers == 1 {
            ConsumerWakes::Single(AtomicWaker::new())
        } else {
            (0..consumers)
                .map(|_| AtomicWaker::new())
                .collect::<Vec<_>>()
                .into_boxed_slice()
                .into()
        },
        consumers: AtomicUsize::new(consumers),
        producer_waiting: AtomicBool::new(false),
        native_wake: Mutex::new(None),
    }))
}

impl From<Box<[AtomicWaker]>> for ConsumerWakes {
    fn from(wakes: Box<[AtomicWaker]>) -> Self {
        Self::Multiple(wakes)
    }
}

/// Receivers share input ownership, never callback mutable state. Each receiver
/// gets one independently constructed FnMut adapter around a shared Fn handler.
pub(super) fn channel_many<T, F>(
    capacity: usize, cancelled: Arc<AtomicBool>, callbacks: Vec<F>,
) -> Result<PreparedInputs<T, F>> {
    let shared = shared(capacity, callbacks.len())?;
    let consumers = callbacks
        .into_iter()
        .enumerate()
        .map(|(index, callback)| Consumer {
            shared: shared.clone(),
            cancelled: cancelled.clone(),
            callback,
            index,
        })
        .collect();
    Ok((Producer { shared }, consumers))
}

impl<T> Shared<T> {
    fn wake_native(&self) {
        let wake = self.native_wake.lock().clone();
        if let Some(wake) = wake {
            wake.wake();
        }
    }
    fn capacity_freed(&self) {
        if self.producer_waiting.swap(false, Ordering::AcqRel) {
            self.wake_native();
        }
    }
}

impl<T> Producer<T> {
    pub(in crate::nodes) fn control(&self) -> Arc<dyn Control>
    where
        T: Send + 'static,
    {
        self.shared.clone()
    }
    #[cfg(test)]
    pub(crate) fn set_native_wake(&self, wake: Option<Waker>) {
        *self.shared.native_wake.lock() = wake;
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.shared.gate.load(Ordering::Acquire) & CLOSED != 0
    }
    /// Register the capacity waiter before rechecking fullness, avoiding a lost
    /// resume when the consumer frees a slot concurrently with native preparation.
    pub(crate) fn is_ready(&self) -> bool {
        if self.is_closed() {
            return false;
        }
        if !self.shared.queue.is_full() {
            return true;
        }
        self.shared.producer_waiting.store(true, Ordering::Release);
        !self.shared.queue.is_full()
    }
    /// A single native producer checks readiness before taking. Rejected owned
    /// input is returned intact; it is never silently dropped under backpressure.
    pub(crate) fn push(&self, input: T) -> std::result::Result<(), T> {
        if self
            .shared
            .gate
            .compare_exchange(0, PUBLISHING, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(input);
        }
        let publication = Publication(&self.shared);
        self.shared.queue.push(input)?;
        drop(publication);
        self.shared.consumer_wake.wake();
        Ok(())
    }
    pub(crate) fn close(&self) {
        self.shared.gate.fetch_or(CLOSED, Ordering::AcqRel);
        self.shared.consumer_wake.wake();
    }
}

impl<T> Drop for Producer<T> {
    fn drop(&mut self) {
        self.close();
    }
}

impl<T, F> Future for Consumer<T, F>
where
    F: FnMut(T) -> Result<()>,
{
    type Output = Result<()>;
    fn poll(self: Pin<&mut Self>, context: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let consumer = self.get_mut();
        consumer
            .shared
            .consumer_wake
            .at(consumer.index)
            .register(context.waker());
        if consumer.cancelled.load(Ordering::Acquire) {
            return Poll::Pending;
        }
        if consumer.shared.gate.load(Ordering::Acquire) & CLOSED != 0 {
            while let Some(input) = consumer.shared.queue.pop() {
                drop(input);
            }
            return Poll::Ready(Ok(()));
        }
        let Some(input) = consumer.shared.queue.pop() else {
            return Poll::Pending;
        };
        consumer.shared.capacity_freed();
        // One input per poll bounds dispatch and gives cancellation/group fairness
        // a scheduling point even with an already-full stream.
        (consumer.callback)(input)?;
        context.waker().wake_by_ref();
        Poll::Pending
    }
}

impl<T, F> Drop for Consumer<T, F> {
    fn drop(&mut self) {
        consumer_wake_release(&self.shared, self.index);
        if self.shared.consumers.fetch_sub(1, Ordering::AcqRel) != 1 {
            return;
        }
        self.shared.gate.fetch_or(CLOSED, Ordering::AcqRel);
        while let Some(input) = self.shared.queue.pop() {
            drop(input);
        }
        self.shared.wake_native();
    }
}

fn consumer_wake_release<T>(shared: &Shared<T>, index: usize) {
    shared.consumer_wake.at(index).take();
}
