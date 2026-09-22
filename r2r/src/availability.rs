use futures::{
    channel::oneshot,
    task::{waker_ref, ArcWake},
};
use parking_lot::Mutex;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    task::{Context, Poll},
};

const MAX_CANCELED_AVAILABILITY_WAITERS_PER_SPIN: usize = 16;

struct AvailabilityCancellationWake {
    canceled_waiters: Arc<Mutex<VecDeque<u64>>>,
    token: u64,
}

impl ArcWake for AvailabilityCancellationWake {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        arc_self.canceled_waiters.lock().push_back(arc_self.token);
    }
}

pub(crate) struct Waiters<T> {
    senders: BTreeMap<u64, oneshot::Sender<T>>,
    canceled_waiters: Arc<Mutex<VecDeque<u64>>>,
    next_token: u64,
}

impl<T> Default for Waiters<T> {
    fn default() -> Self {
        Self {
            senders: BTreeMap::new(),
            canceled_waiters: Arc::new(Mutex::new(VecDeque::new())),
            next_token: 0,
        }
    }
}

impl<T> Waiters<T> {
    pub(crate) fn register(&mut self, mut sender: oneshot::Sender<T>) {
        let token = loop {
            let token = self.next_token;
            self.next_token = self.next_token.wrapping_add(1);
            if !self.senders.contains_key(&token) {
                break token;
            }
        };
        let wake = Arc::new(AvailabilityCancellationWake {
            canceled_waiters: Arc::clone(&self.canceled_waiters),
            token,
        });
        let waker = waker_ref(&wake);
        let mut context = Context::from_waker(&waker);
        if matches!(sender.poll_canceled(&mut context), Poll::Pending) {
            self.senders.insert(token, sender);
        }
    }

    pub(crate) fn prune_canceled(&mut self) {
        let candidates = {
            let mut canceled_waiters = self.canceled_waiters.lock();
            let count = canceled_waiters
                .len()
                .min(MAX_CANCELED_AVAILABILITY_WAITERS_PER_SPIN);
            canceled_waiters.drain(..count).collect::<Vec<_>>()
        };
        for token in candidates {
            if self
                .senders
                .get(&token)
                .is_some_and(oneshot::Sender::is_canceled)
            {
                self.senders.remove(&token);
            }
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.senders.is_empty()
    }

    pub(crate) fn poll_boundary(&mut self) -> Option<u64> {
        self.prune_canceled();
        self.senders.last_key_value().map(|(token, _)| *token)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.senders.len()
    }

    pub(crate) fn take_all(&mut self) -> Vec<oneshot::Sender<T>> {
        std::mem::take(&mut self.senders).into_values().collect()
    }

    pub(crate) fn take_through(&mut self, boundary: u64) -> Vec<oneshot::Sender<T>> {
        let tokens = self
            .senders
            .range(..=boundary)
            .map(|(token, _)| *token)
            .collect::<Vec<_>>();
        tokens
            .into_iter()
            .filter_map(|token| self.senders.remove(&token))
            .collect()
    }
}

pub(crate) type AvailabilityWaiters = Waiters<()>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canceled_waiters_are_removed_by_token_and_live_waiters_are_preserved() {
        let mut waiters = AvailabilityWaiters::default();
        let (canceled_sender, canceled_receiver) = oneshot::channel();
        let (live_sender, live_receiver) = oneshot::channel();
        waiters.register(canceled_sender);
        waiters.register(live_sender);

        drop(canceled_receiver);
        waiters.prune_canceled();

        assert_eq!(waiters.senders.len(), 1);
        let senders = waiters.take_all();
        assert_eq!(senders.len(), 1);
        senders.into_iter().next().unwrap().send(()).unwrap();
        assert_eq!(futures::executor::block_on(live_receiver), Ok(()));
    }

    #[test]
    fn canceled_waiter_cleanup_is_bounded() {
        let mut waiters = AvailabilityWaiters::default();
        for _ in 0..20 {
            let (sender, receiver) = oneshot::channel();
            waiters.register(sender);
            drop(receiver);
        }

        waiters.prune_canceled();

        assert_eq!(waiters.senders.len(), 4);
        assert_eq!(waiters.canceled_waiters.lock().len(), 4);
    }

    #[test]
    fn already_canceled_waiter_is_not_registered() {
        let mut waiters = AvailabilityWaiters::default();
        let (sender, receiver) = oneshot::channel();
        drop(receiver);

        waiters.register(sender);

        assert!(waiters.is_empty());
        assert!(waiters.canceled_waiters.lock().is_empty());
    }

    #[test]
    fn waiter_tokens_preserve_registration_order() {
        let mut waiters = AvailabilityWaiters::default();
        let mut receivers = Vec::new();
        for _ in 0..3 {
            let (sender, receiver) = oneshot::channel();
            waiters.register(sender);
            receivers.push(receiver);
        }

        assert_eq!(waiters.senders.keys().copied().collect::<Vec<_>>(), [0, 1, 2]);
        assert_eq!(waiters.take_all().len(), 3);
        drop(receivers);
    }

    #[test]
    fn poll_boundary_excludes_waiters_registered_during_poll() {
        let mut waiters = Waiters::<()>::default();
        let (first, _first_receiver) = oneshot::channel();
        waiters.register(first);
        let boundary = waiters.poll_boundary().unwrap();
        let (later, mut later_receiver) = oneshot::channel();
        waiters.register(later);

        assert_eq!(waiters.take_through(boundary).len(), 1);
        assert_eq!(waiters.len(), 1);
        waiters.take_all().pop().unwrap().send(()).unwrap();
        assert_eq!(futures::executor::block_on(&mut later_receiver), Ok(()));
    }
}
