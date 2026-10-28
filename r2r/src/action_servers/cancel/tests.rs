use super::*;
use std::sync::atomic::AtomicUsize;

struct Count(AtomicUsize);
impl Wake for Count {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

fn request(decisions: Vec<oneshot::Receiver<(uuid::Uuid, bool)>>) -> ActiveCancelRequest {
    ActiveCancelRequest {
        request_id: unsafe { std::mem::zeroed() },
        response: Default::default(),
        pending_decisions: decisions.into(),
        completed_decisions: Vec::new(),
    }
}

#[test]
fn cached_owner_wake_follows_current_binding_and_does_not_retain_queue() {
    let owner = Arc::new(Mutex::new(crate::notifications::NotificationQueue::default()));
    let weak = Arc::downgrade(&owner);
    let cached = current_owner_waker(weak.clone());
    assert_eq!(Arc::strong_count(&owner), 1);
    let first = Arc::new(Count(AtomicUsize::new(0)));
    let second = Arc::new(Count(AtomicUsize::new(0)));
    owner.lock().set_wake(Some(Waker::from(first.clone())));
    cached.wake_by_ref();
    assert_eq!(first.0.load(Ordering::Relaxed), 1);
    owner.lock().set_wake(None);
    cached.wake_by_ref();
    assert_eq!(first.0.load(Ordering::Relaxed), 1);
    owner.lock().set_wake(Some(Waker::from(second.clone())));
    cached.wake_by_ref();
    assert_eq!(first.0.load(Ordering::Relaxed), 1);
    assert_eq!(second.0.load(Ordering::Relaxed), 1);
    drop(owner);
    assert!(weak.upgrade().is_none());
    cached.wake();
    assert_eq!(second.0.load(Ordering::Relaxed), 1);
}

#[test]
fn decision_and_request_batch_boundaries_preserve_all_completed_results() {
    for count in [0_usize, 1, 16, 17, 40, 128] {
        let receivers = (0..count)
            .map(|i| {
                let (sender, receiver) = oneshot::channel();
                sender
                    .send((uuid::Uuid::from_u128(i as u128), i % 2 == 0))
                    .unwrap();
                receiver
            })
            .collect();
        let mut queue = ActiveCancelQueue::default();
        queue.push_back(request(receivers));
        let mut completed = Vec::new();
        for _ in 0..count.max(1).div_ceil(16) {
            let (ready, more) = queue.take_ready();
            completed.extend(ready);
            assert_eq!(more, completed.is_empty());
        }
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].1.len(), count);
        assert!(queue.is_empty());
        let mut queue = ActiveCancelQueue::default();
        for i in 0..count {
            let (sender, receiver) = oneshot::channel();
            sender
                .send((uuid::Uuid::from_u128(i as u128), true))
                .unwrap();
            queue.push_back(request(vec![receiver]));
        }
        let mut total = 0;
        loop {
            let (ready, more) = queue.take_ready();
            assert!(ready.len() <= 16);
            total += ready.len();
            assert_eq!(more, total < count);
            if !more {
                break;
            }
        }
        assert_eq!(total, count);
        assert!(queue.is_empty());
    }
}

#[test]
fn removed_goal_during_detached_full_delivery_retries_after_restoration_unlock() {
    use futures::FutureExt;
    struct Observe {
        actor: Arc<Mutex<CancelSenders>>,
        count: AtomicUsize,
    }
    impl Wake for Observe {
        fn wake(self: Arc<Self>) {
            assert!(
                self.actor.try_lock().is_some(),
                "restoration notification held the server lock"
            );
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }
    let actor = Arc::new(Mutex::new(HashMap::new()));
    let mut streams = Vec::new();
    let mut payloads = VecDeque::new();
    let mut decisions = Vec::new();
    for i in 1..=17 {
        let uuid = uuid::Uuid::from_u128(i);
        let (mut sender, receiver) = mpsc::channel(0);
        let (dummy, _result) = oneshot::channel();
        sender
            .try_send(ActionServerCancelRequest {
                uuid,
                response_sender: dummy,
            })
            .unwrap();
        let (sender_decision, receiver_decision) = oneshot::channel();
        decisions.push(receiver_decision);
        payloads.push_back(PendingCancelDelivery {
            uuid,
            request: ActionServerCancelRequest {
                uuid,
                response_sender: sender_decision,
            },
        });
        actor
            .lock()
            .insert(uuid, Arc::new(Mutex::new(Some(sender))));
        streams.push(receiver);
    }
    let observe = Arc::new(Observe {
        actor: actor.clone(),
        count: AtomicUsize::new(0),
    });
    let bulk = delivery::BulkDeliveries::new(payloads, Waker::from(observe.clone()));
    let mut pending = PendingCancelRequest {
        request_id: unsafe { std::mem::zeroed() },
        response: Default::default(),
        deliveries: VecDeque::new(),
        decisions,
        bulk_deliveries: Some(Box::new(bulk)),
    };
    let (detached, more) = pending
        .bulk_deliveries
        .as_mut()
        .unwrap()
        .take_ready(&actor.lock());
    assert!(more);
    actor.lock().remove(&uuid::Uuid::from_u128(1));
    let stored = Arc::new(Mutex::new(None));
    let capture = stored.clone();
    let owner = actor.clone();
    delivery::deliver(detached, pending, move |mut pending| {
        pending
            .bulk_deliveries
            .as_mut()
            .unwrap()
            .retire_missing_restorations(&owner.lock());
        *capture.lock() = Some(pending);
        Ok(())
    })()
    .unwrap();
    pending = stored.lock().take().unwrap();
    let (detached, more) = pending
        .bulk_deliveries
        .as_mut()
        .unwrap()
        .take_ready(&actor.lock());
    assert_eq!(detached.len(), 1);
    assert!(!more);
    assert_eq!(pending.bulk_deliveries.as_ref().unwrap().remaining, 16);
    assert!(pending.decisions[0].try_recv().is_err());
    assert_eq!(observe.count.load(Ordering::Relaxed), 1);
    streams[0].next().now_or_never().unwrap().unwrap().reject();
}

#[test]
fn large_decisions_register_in_bounded_passes_and_retry_only_ready_indices() {
    let count = Arc::new(Count(AtomicUsize::new(0)));
    let mut queue = ActiveCancelQueue::default();
    queue.set_wake(Some(Waker::from(count.clone())));
    let (mut senders, receivers): (Vec<_>, Vec<_>) = (0..40).map(|_| oneshot::channel()).unzip();
    queue.push_back(request(receivers));
    for more in [true, true, false] {
        let (ready, continuation) = queue.take_ready();
        assert!(ready.is_empty());
        assert_eq!(continuation, more);
    }
    assert_eq!(count.0.load(Ordering::Relaxed), 0);
    for _ in 0..4 {
        assert!(!queue.take_ready().1);
    }
    senders
        .pop()
        .unwrap()
        .send((uuid::Uuid::from_u128(40), true))
        .unwrap();
    assert_eq!(count.0.load(Ordering::Relaxed), 1);
    let (ready, more) = queue.take_ready();
    assert!(ready.is_empty());
    assert!(!more);
    let bulk = queue.bulk.as_ref().unwrap();
    let active = bulk.requests[0].as_ref().unwrap();
    assert_eq!(active.remaining, 39);
    assert_eq!(active.request.completed_decisions, [Ok((uuid::Uuid::from_u128(40), true))]);
    for (i, sender) in senders.into_iter().enumerate() {
        if i % 3 != 0 {
            sender
                .send((uuid::Uuid::from_u128(i as u128 + 1), i % 2 == 0))
                .unwrap();
        }
    }
    let mut completed = Vec::new();
    for more in [true, true, false] {
        let (ready, continuation) = queue.take_ready();
        assert_eq!(continuation, more);
        completed.extend(ready);
    }
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].1.len(), 40);
    assert_eq!(completed[0].1.iter().filter(|r| r.is_err()).count(), 13);
    assert!(queue.is_empty());
    queue.release_idle_bulk();
    assert!(queue.bulk.is_none());
    let (sender, receiver) = oneshot::channel();
    sender.send((uuid::Uuid::nil(), false)).unwrap();
    queue.push_back(request(vec![receiver]));
    assert!(queue.bulk.is_none(), "ordinary request allocated indexed state");
    assert_eq!(queue.take_ready().0.len(), 1);
}

#[test]
fn concurrent_request_slots_are_bounded_reused_and_reject_stale_wakes() {
    let mut queue = ActiveCancelQueue::default();
    let count = Arc::new(Count(AtomicUsize::new(0)));
    queue.set_wake(Some(Waker::from(count.clone())));
    let mut senders = Vec::new();
    for i in 0..40 {
        let (sender, receiver) = oneshot::channel();
        let mut request = request(vec![receiver]);
        request.request_id.sequence_number = i;
        queue.push_back(request);
        senders.push(sender);
    }
    for more in [true, true, false] {
        let (ready, continuation) = queue.take_ready();
        assert!(ready.is_empty());
        assert_eq!(continuation, more);
    }
    let bulk = queue.bulk.as_ref().unwrap();
    let weak = Arc::downgrade(&bulk.ready);
    let stale = bulk.requests[39].as_ref().unwrap().ready.lock().notify();
    senders
        .pop()
        .unwrap()
        .send((uuid::Uuid::from_u128(40), true))
        .unwrap();
    let (done, more) = queue.take_ready();
    assert!(!more);
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].0.request_id.sequence_number, 39);
    let (sender, receiver) = oneshot::channel();
    queue.push_back(request(vec![receiver]));
    assert_eq!(queue.bulk.as_ref().unwrap().requests.len(), 40);
    assert!(queue.take_ready().0.is_empty());
    let before = count.0.load(Ordering::Relaxed);
    stale.wake_by_ref();
    assert_eq!(count.0.load(Ordering::Relaxed), before);
    assert!(!queue.take_ready().1);
    sender.send((uuid::Uuid::from_u128(100), true)).unwrap();
    assert_eq!(queue.take_ready().0.len(), 1);
    drop(queue);
    assert!(weak.upgrade().is_none());
    drop(senders);
    assert_eq!(count.0.load(Ordering::Relaxed), before + 1);
}

#[test]
fn readiness_wakes_unlock_both_queues_and_coalesce_per_registration() {
    struct Observe {
        outer: Weak<Mutex<RequestReadiness>>,
        inner: Weak<Mutex<ReadyQueue>>,
        count: AtomicUsize,
    }
    impl Wake for Observe {
        fn wake(self: Arc<Self>) {
            assert!(
                self.outer
                    .upgrade()
                    .unwrap()
                    .try_lock()
                    .unwrap()
                    .entries
                    .len()
                    == 1
            );
            assert!(self
                .inner
                .upgrade()
                .unwrap()
                .try_lock()
                .unwrap()
                .has_ready());
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }
    let mut queue = ActiveCancelQueue::default();
    let mut senders = Vec::new();
    let mut receivers = Vec::new();
    for _ in 0..17 {
        let (s, r) = oneshot::channel();
        senders.push(s);
        receivers.push(r);
    }
    queue.push_back(request(receivers));
    queue.take_ready();
    queue.take_ready();
    let bulk = queue.bulk.as_ref().unwrap();
    let inner = bulk.requests[0].as_ref().unwrap().ready.clone();
    let observe = Arc::new(Observe {
        outer: Arc::downgrade(&bulk.ready),
        inner: Arc::downgrade(&inner),
        count: AtomicUsize::new(0),
    });
    queue.set_wake(Some(Waker::from(observe.clone())));
    let wake = ReadyQueue::waker(&inner, 16);
    wake.wake_by_ref();
    wake.wake_by_ref();
    assert_eq!(observe.count.load(Ordering::Relaxed), 1);
    drop(queue);
    drop(senders);
}

#[test]
fn forty_full_cancel_streams_park_and_only_recovered_or_removed_goals_retry() {
    use futures::FutureExt;
    let count = Arc::new(Count(AtomicUsize::new(0)));
    let mut senders = HashMap::new();
    let mut receivers = Vec::new();
    let mut deliveries = VecDeque::new();
    let mut decisions = Vec::new();
    for i in 1..=40 {
        let uuid = uuid::Uuid::from_u128(i);
        let (mut sender, receiver) = mpsc::channel(0);
        let (dummy, _dummy_result) = oneshot::channel();
        sender
            .try_send(ActionServerCancelRequest {
                uuid,
                response_sender: dummy,
            })
            .unwrap();
        let (response_sender, response_receiver) = oneshot::channel();
        deliveries.push_back(PendingCancelDelivery {
            uuid,
            request: ActionServerCancelRequest {
                uuid,
                response_sender,
            },
        });
        decisions.push(response_receiver);
        senders.insert(uuid, Arc::new(Mutex::new(Some(sender))));
        receivers.push(receiver);
    }
    let bulk = delivery::BulkDeliveries::new(deliveries, Waker::from(count.clone()));
    let mut pending = PendingCancelRequest {
        request_id: unsafe { std::mem::zeroed() },
        response: Default::default(),
        deliveries: VecDeque::new(),
        decisions,
        bulk_deliveries: Some(Box::new(bulk)),
    };
    for more in [true, true, false] {
        let (detached, continuation) = pending
            .bulk_deliveries
            .as_mut()
            .unwrap()
            .take_ready(&senders);
        assert_eq!(continuation, more);
        let stored = Arc::new(Mutex::new(None));
        let capture = stored.clone();
        delivery::deliver(detached, pending, move |pending| {
            *capture.lock() = Some(pending);
            Ok(())
        })()
        .unwrap();
        pending = stored.lock().take().unwrap();
    }
    let before = count.0.load(Ordering::Relaxed);
    for _ in 0..4 {
        let (detached, more) = pending
            .bulk_deliveries
            .as_mut()
            .unwrap()
            .take_ready(&senders);
        assert!(detached.is_empty());
        assert!(!more);
    }
    assert_eq!(count.0.load(Ordering::Relaxed), before);
    receivers[39]
        .next()
        .now_or_never()
        .unwrap()
        .unwrap()
        .reject();
    assert_eq!(count.0.load(Ordering::Relaxed), before + 1);
    let (detached, more) = pending
        .bulk_deliveries
        .as_mut()
        .unwrap()
        .take_ready(&senders);
    assert_eq!(detached.len(), 1);
    assert!(!more);
    let stored = Arc::new(Mutex::new(None));
    let capture = stored.clone();
    delivery::deliver(detached, pending, move |pending| {
        *capture.lock() = Some(pending);
        Ok(())
    })()
    .unwrap();
    pending = stored.lock().take().unwrap();
    let recovered = receivers[39].next().now_or_never().unwrap().unwrap();
    assert_eq!(recovered.uuid, uuid::Uuid::from_u128(40));
    recovered.accept();
    assert_eq!(pending.bulk_deliveries.as_ref().unwrap().remaining, 39);
    senders.remove(&uuid::Uuid::from_u128(1));
    pending
        .bulk_deliveries
        .as_mut()
        .unwrap()
        .removed_goal(&uuid::Uuid::from_u128(1))
        .unwrap()
        .wake();
    let (detached, more) = pending
        .bulk_deliveries
        .as_mut()
        .unwrap()
        .take_ready(&senders);
    assert!(detached.is_empty());
    assert!(!more);
    assert_eq!(pending.bulk_deliveries.as_ref().unwrap().remaining, 38);
    assert!(pending.decisions[0].try_recv().is_err());
}
