use super::*;

struct Fixture<T> {
    queue: Arc<Mutex<PendingFeedbackQueue>>,
    channel: FeedbackSender<T>,
    receiver: mpsc::Receiver<T>,
    channels: HashMap<uuid::Uuid, FeedbackSender<T>>,
}

fn coalescing<T>(capacity: usize) -> Fixture<T> {
    let active = Arc::new(Mutex::new(crate::notifications::NotificationQueue::default()));
    let notifier = Arc::new(ActionClientMaintenanceNotifier::new(active));
    notifier.set_identity(42);
    let queue = Arc::new(Mutex::new(PendingFeedbackQueue::with_overflow_policy(
        notifier,
        FeedbackOverflowPolicy::Coalesce,
    )));
    let (sender, receiver) = mpsc::channel(capacity);
    let channel = Arc::new(Mutex::new(FeedbackChannel::with_overflow_policy(
        sender,
        FeedbackOverflowPolicy::Coalesce,
    )));
    let channels = HashMap::from([(uuid::Uuid::from_u128(1), channel.clone())]);
    Fixture {
        queue,
        channel,
        receiver,
        channels,
    }
}

fn arrive<T: Send + 'static>(
    queue: &Arc<Mutex<PendingFeedbackQueue>>, channel: &FeedbackSender<T>, value: T,
) {
    if let Some(completion) = feedback_completion_with_queue(
        channel.clone(),
        value,
        Some((uuid::Uuid::from_u128(1), queue.clone())),
    ) {
        (completion.notify)().unwrap();
    }
}

#[test]
fn overflow_coalesces_only_pending_samples_and_releases_the_latest_on_capacity() {
    for capacity in [0, 1, 8] {
        let Fixture {
            queue,
            channel,
            mut receiver,
            channels,
        } = coalescing(capacity);
        // futures::mpsc retains the configured capacity plus its sender's reserved slot.
        for value in 0..=capacity as u32 {
            arrive(&queue, &channel, value);
        }
        arrive(&queue, &channel, 100);
        assert!(matches!(
            take_ready_pending_feedback(&queue, &channels),
            PendingFeedbackAction::Continue
        ));
        assert_eq!(queue.lock().blocked_count(), 1);
        for value in 101..=4096 {
            arrive(&queue, &channel, value);
        }
        assert_eq!(
            channel
                .lock()
                .pending
                .as_ref()
                .and_then(PendingFeedback::ready_ref),
            Some(&4096)
        );
        assert!(queue.lock().goals.is_empty(), "replacement requeued a still-blocked sender");
        assert_eq!(queue.lock().blocked_count(), 1);
        for value in 0..=capacity as u32 {
            assert_eq!(receiver.next().now_or_never(), Some(Some(value)));
        }
        let PendingFeedbackAction::Deliver(completion) =
            take_ready_pending_feedback(&queue, &channels)
        else {
            panic!("latest sample did not become writable");
        };
        (completion.notify)().unwrap();
        assert_eq!(receiver.next().now_or_never(), Some(Some(4096)));
        assert!(channel.lock().pending.is_none());
        assert!(queue.lock().is_empty());
    }
}

#[test]
fn overflow_detached_sender_retains_the_newest_arrival_on_success_and_failure() {
    for already_full in [false, true] {
        let Fixture {
            queue,
            channel,
            mut receiver,
            channels,
        } = coalescing(0);
        if already_full {
            arrive(&queue, &channel, 0);
        }
        let sender = channel.lock().sender.take().unwrap();
        let first = feedback::detached_feedback_completion(
            channel.clone(),
            sender,
            1,
            Some((uuid::Uuid::from_u128(1), queue.clone())),
        );
        arrive(&queue, &channel, 2);
        arrive(&queue, &channel, 3);
        (first.notify)().unwrap();
        assert_eq!(
            channel
                .lock()
                .pending
                .as_ref()
                .and_then(PendingFeedback::ready_ref),
            Some(&3),
            "older detached payload replaced the newest arrival"
        );
        assert_eq!(receiver.next().now_or_never(), Some(Some(if already_full { 0 } else { 1 })));
        let PendingFeedbackAction::Deliver(latest) = take_ready_pending_feedback(&queue, &channels)
        else {
            panic!("reentrant latest sample was lost");
        };
        (latest.notify)().unwrap();
        assert_eq!(receiver.next().now_or_never(), Some(Some(3)));
        assert!(queue.lock().is_empty());
    }
}

#[test]
fn overflow_closed_goal_cannot_be_resurrected_by_an_arrival_or_cached_capacity_wake() {
    let Fixture {
        queue,
        channel,
        mut receiver,
        channels,
    } = coalescing(0);
    arrive(&queue, &channel, 0);
    arrive(&queue, &channel, 1);
    let _ = take_ready_pending_feedback(&queue, &channels);
    assert_eq!(queue.lock().blocked_count(), 1);
    (close_feedback_completion(channel.clone()).notify)().unwrap();
    arrive(&queue, &channel, 2);
    assert!(channel.lock().pending.is_none());
    assert_eq!(receiver.next().now_or_never(), Some(Some(0)));
    assert!(matches!(
        take_ready_pending_feedback(&queue, &channels),
        PendingFeedbackAction::None
    ));
    assert!(queue.lock().is_empty());
    assert_eq!(queue.lock().overflow_policy(), FeedbackOverflowPolicy::Coalesce);
    let weak = Arc::downgrade(&queue);
    drop((queue, channel, channels));
    assert!(weak.upgrade().is_none());
}

#[test]
fn overflow_payload_retirement_never_holds_channel_or_readiness_locks() {
    struct Payload {
        value: u32,
        channel: Weak<Mutex<FeedbackChannel<Payload>>>,
        queue: Weak<Mutex<PendingFeedbackQueue>>,
        drops: Arc<AtomicUsize>,
        owner: Weak<Mutex<()>>,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            if let Some(owner) = self.owner.upgrade() {
                assert!(owner.try_lock().is_some(), "payload destruction held the client lock");
            }
            if let Some(channel) = self.channel.upgrade() {
                assert!(channel.try_lock().is_some(), "payload destruction held the channel lock");
            }
            if let Some(queue) = self.queue.upgrade() {
                assert!(queue.try_lock().is_some(), "payload destruction held the retry lock");
            }
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }
    let Fixture {
        queue,
        channel,
        mut receiver,
        channels: _channels,
    } = coalescing(0);
    let drops = Arc::new(AtomicUsize::new(0));
    let owner = Arc::new(Mutex::new(()));
    let payload = |value| Payload {
        value,
        channel: Arc::downgrade(&channel),
        queue: Arc::downgrade(&queue),
        drops: drops.clone(),
        owner: Arc::downgrade(&owner),
    };
    let deliver = |value| {
        let completion = {
            let _locked = owner.lock();
            feedback_completion_with_queue(
                channel.clone(),
                payload(value),
                Some((uuid::Uuid::from_u128(1), queue.clone())),
            )
        }
        .unwrap();
        (completion.notify)().unwrap();
    };
    deliver(0);
    let sender = channel.lock().sender.take().unwrap();
    let first = feedback::detached_feedback_completion(
        channel.clone(),
        sender,
        payload(1),
        Some((uuid::Uuid::from_u128(1), queue.clone())),
    );
    deliver(2);
    deliver(3);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
    (first.notify)().unwrap();
    assert_eq!(drops.load(Ordering::Relaxed), 2);
    assert_eq!(
        channel
            .lock()
            .pending
            .as_ref()
            .unwrap()
            .ready_ref()
            .unwrap()
            .value,
        3
    );
    (close_feedback_completion(channel.clone()).notify)().unwrap();
    deliver(4);
    assert_eq!(drops.load(Ordering::Relaxed), 4);
    drop(receiver.next().now_or_never().unwrap().unwrap());
    assert_eq!(drops.load(Ordering::Relaxed), 5);
}

#[test]
fn deferred_decode_failure_restores_the_sender_for_later_feedback() {
    let Fixture {
        queue,
        channel,
        mut receiver,
        channels,
    } = coalescing::<u32>(0);
    defer_feedback_with_queue(
        channel.clone(),
        || Err(Error::RCL_RET_BAD_ALLOC),
        uuid::Uuid::from_u128(1),
        queue.clone(),
    )
    .unwrap();
    assert!(channel
        .lock()
        .pending
        .as_ref()
        .is_some_and(PendingFeedback::is_deferred));
    let PendingFeedbackAction::Deliver(deliver) = take_ready_pending_feedback(&queue, &channels)
    else {
        panic!("deferred error was not scheduled on a writable sender");
    };
    assert!(matches!((deliver.notify)(), Err(Error::RCL_RET_BAD_ALLOC)));
    assert!(channel.lock().sender.is_some());
    assert!(channel.lock().pending.is_none());
    assert!(queue.lock().is_empty());

    arrive(&queue, &channel, 42);
    assert_eq!(receiver.next().now_or_never(), Some(Some(42)));
}

#[test]
fn deferred_decode_panic_restores_the_sender_during_unwind() {
    let Fixture {
        queue,
        channel,
        receiver: _,
        channels,
    } = coalescing::<u32>(0);
    defer_feedback_with_queue(
        channel.clone(),
        || -> Result<u32> { panic!("injected deferred decoder panic") },
        uuid::Uuid::from_u128(1),
        queue.clone(),
    )
    .unwrap();
    let PendingFeedbackAction::Deliver(deliver) = take_ready_pending_feedback(&queue, &channels)
    else {
        panic!("deferred panic was not scheduled on a writable sender");
    };
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = (deliver.notify)();
    }))
    .is_err());
    assert!(channel.lock().sender.is_some());
    assert!(channel.lock().pending.is_none());
    assert!(queue.lock().is_empty());
}

#[test]
fn deferred_replacement_and_close_retire_captures_after_unlock() {
    struct Resource {
        channel: Weak<Mutex<FeedbackChannel<u32>>>,
        queue: Weak<Mutex<PendingFeedbackQueue>>,
        drops: Arc<AtomicUsize>,
    }
    impl Drop for Resource {
        fn drop(&mut self) {
            assert!(self.channel.upgrade().unwrap().try_lock().is_some());
            assert!(self.queue.upgrade().unwrap().try_lock().is_some());
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }
    let Fixture {
        queue,
        channel,
        receiver: _,
        channels: _,
    } = coalescing::<u32>(0);
    let drops = Arc::new(AtomicUsize::new(0));
    let resource = || Resource {
        channel: Arc::downgrade(&channel),
        queue: Arc::downgrade(&queue),
        drops: drops.clone(),
    };
    for value in [1, 2] {
        let retained = resource();
        defer_feedback_with_queue(
            channel.clone(),
            move || {
                drop(retained);
                Ok(value)
            },
            uuid::Uuid::from_u128(1),
            queue.clone(),
        )
        .unwrap();
    }
    assert_eq!(drops.load(Ordering::Relaxed), 1);
    (close_feedback_completion(channel.clone()).notify)().unwrap();
    assert_eq!(drops.load(Ordering::Relaxed), 2);
    assert!(channel.lock().pending.is_none());
}

#[test]
fn deferred_feedback_bounds_channel_growth_and_preserves_queue_header() {
    #[allow(dead_code)]
    struct LegacyChannel<T> {
        sender: Option<mpsc::Sender<T>>,
        pending: Option<T>,
        closing: bool,
    }
    #[allow(dead_code)]
    struct LegacyQueue {
        goals: VecDeque<uuid::Uuid>,
        queued: HashSet<uuid::Uuid>,
        notifier: Option<Arc<ActionClientMaintenanceNotifier>>,
        blocked: usize,
    }
    use std::mem::size_of;
    assert_eq!(size_of::<PendingFeedbackQueue>(), size_of::<LegacyQueue>());
    for (current, legacy) in [
        (size_of::<FeedbackChannel<u32>>(), size_of::<LegacyChannel<u32>>()),
        (size_of::<FeedbackChannel<Vec<i32>>>(), size_of::<LegacyChannel<Vec<i32>>>()),
        (
            size_of::<FeedbackChannel<Result<serde_json::Value>>>(),
            size_of::<LegacyChannel<Result<serde_json::Value>>>(),
        ),
    ] {
        assert!(current >= legacy);
        assert!(
            current <= legacy + 2 * size_of::<usize>(),
            "deferred state added more than one fat pointer: {legacy} -> {current}"
        );
    }
}

#[test]
#[ignore = "manual optimized feedback delivery stage measurement"]
fn benchmark_feedback_overflow_delivery_stage() {
    const ITERATIONS: u32 = 2_000_000;
    let (sender, mut receiver) = mpsc::channel(0);
    let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
    for value in 0..1000 {
        (feedback_completion(channel.clone(), value).unwrap().notify)().unwrap();
        assert_eq!(receiver.next().now_or_never(), Some(Some(value)));
    }
    let begin = std::time::Instant::now();
    let mut sum = 0_u64;
    for value in 0..ITERATIONS {
        (feedback_completion(channel.clone(), value).unwrap().notify)().unwrap();
        sum += std::hint::black_box(receiver.next().now_or_never().unwrap().unwrap()) as u64;
    }
    let default_ns = begin.elapsed().as_nanos();
    assert_eq!(sum, ITERATIONS as u64 * (ITERATIONS as u64 - 1) / 2);
    let Fixture {
        queue,
        channel,
        mut receiver,
        channels,
    } = coalescing(0);
    arrive(&queue, &channel, 0);
    arrive(&queue, &channel, 1);
    let _ = take_ready_pending_feedback(&queue, &channels);
    let begin = std::time::Instant::now();
    for value in 2..=ITERATIONS {
        arrive(&queue, &channel, std::hint::black_box(value));
    }
    let coalesce_ns = begin.elapsed().as_nanos();
    assert_eq!(
        channel
            .lock()
            .pending
            .as_ref()
            .and_then(PendingFeedback::ready_ref),
        Some(&ITERATIONS)
    );
    assert_eq!(receiver.next().now_or_never(), Some(Some(0)));
    let PendingFeedbackAction::Deliver(deliver) = take_ready_pending_feedback(&queue, &channels)
    else {
        panic!("last sample missing");
    };
    (deliver.notify)().unwrap();
    assert_eq!(receiver.next().now_or_never(), Some(Some(ITERATIONS)));
    assert!(queue.lock().is_empty());
    println!(
        "{}",
        serde_json::json!({"boundary":"feedback completion only; u32 payloads; no ROS ingress or application transport", "iterations":ITERATIONS,"default_ns_per_call":default_ns as f64 / ITERATIONS as f64, "coalesce_ns_per_call":coalesce_ns as f64 / (ITERATIONS-1) as f64,"channel_bytes":std::mem::size_of::<FeedbackChannel<u32>>(),"queue_bytes":std::mem::size_of::<PendingFeedbackQueue>()})
    );
}
