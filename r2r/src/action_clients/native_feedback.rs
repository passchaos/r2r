//! Route known native identity before decoding its payload.

use super::*;
use std::ops::{Deref, DerefMut};

#[cfg(test)]
use super::feedback_routing::allocation_probe;

pub(crate) struct NativeFeedbackBuffer<N> {
    slot: Arc<Mutex<Option<N>>>,
}

impl<N> Clone for NativeFeedbackBuffer<N> {
    fn clone(&self) -> Self {
        Self {
            slot: Arc::clone(&self.slot),
        }
    }
}

impl<N> Default for NativeFeedbackBuffer<N> {
    fn default() -> Self {
        Self {
            slot: Arc::new(Mutex::new(None)),
        }
    }
}

impl<N> NativeFeedbackBuffer<N> {
    pub(crate) fn checkout(
        &self, create: impl FnOnce() -> Result<N>,
    ) -> Result<NativeFeedbackLease<N>> {
        let cached = { self.slot.lock().take() };
        let native = match cached {
            Some(native) => native,
            None => create()?,
        };
        Ok(NativeFeedbackLease {
            native: Some(native),
            buffer: self.clone(),
        })
    }

    fn recycle(&self, native: N) {
        // A concurrent return keeps the most recently used storage. Retire the displaced
        // value only after releasing the pool lock so native finalizers can reenter safely.
        let retired = { self.slot.lock().replace(native) };
        drop(retired);
    }

    #[cfg(test)]
    fn is_available(&self) -> bool {
        self.slot.lock().is_some()
    }

    #[cfg(test)]
    pub(crate) fn inspect_available<R>(&self, inspect: impl FnOnce(&N) -> R) -> Option<R> {
        self.slot.lock().as_ref().map(inspect)
    }
}

pub(crate) struct NativeFeedbackLease<N> {
    native: Option<N>,
    buffer: NativeFeedbackBuffer<N>,
}

impl<N> Deref for NativeFeedbackLease<N> {
    type Target = N;

    fn deref(&self) -> &Self::Target {
        self.native.as_ref().unwrap()
    }
}

impl<N> DerefMut for NativeFeedbackLease<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.native.as_mut().unwrap()
    }
}

impl<N> Drop for NativeFeedbackLease<N> {
    fn drop(&mut self) {
        if let Some(native) = self.native.take() {
            self.buffer.recycle(native);
        }
    }
}

pub(crate) fn completion<N: Send + 'static, T: Send + 'static>(
    native: NativeFeedbackLease<N>, channel: Option<FeedbackSender<T>>,
    pending: Arc<Mutex<PendingFeedbackQueue>>, uuid: uuid::Uuid,
    decode: impl FnOnce(&N) -> Result<(uuid::Uuid, T)> + Send + 'static,
) -> ActionClientCompletion {
    let Some(channel) = channel else {
        return ActionClientCompletion {
            notify: Box::new(move || {
                drop(native);
                drop(decode);
                drop(pending);
                Ok(true)
            }),
            cleanup_goal_on_failure: None,
        };
    };
    ActionClientCompletion {
        notify: Box::new(move || {
            let (closed, overflow_policy) = {
                let channel = channel.lock();
                (channel.is_closed(), channel.overflow_policy)
            };
            if closed {
                drop(native);
                drop(decode);
                return Ok(true);
            }
            if overflow_policy == FeedbackOverflowPolicy::Coalesce {
                return defer_feedback_with_queue(
                    channel,
                    move || {
                        let (decoded_uuid, payload) = decode(&native)?;
                        if decoded_uuid != uuid {
                            return Err(Error::RCL_RET_ACTION_CLIENT_INVALID);
                        }
                        Ok(payload)
                    },
                    uuid,
                    pending,
                );
            }
            let (decoded_uuid, payload) = decode(&native)?;
            if decoded_uuid != uuid {
                return Err(Error::RCL_RET_ACTION_CLIENT_INVALID);
            }
            if let Some(deliver) =
                feedback_completion_with_queue(channel, payload, Some((uuid, pending)))
            {
                (deliver.notify)()
            } else {
                Ok(true)
            }
        }),
        cleanup_goal_on_failure: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{FutureExt, StreamExt};

    fn leased<N>(native: N) -> (NativeFeedbackBuffer<N>, NativeFeedbackLease<N>) {
        let buffer = NativeFeedbackBuffer::default();
        let lease = buffer.checkout(|| Ok(native)).unwrap();
        (buffer, lease)
    }

    #[test]
    fn native_feedback_lease_reuses_storage_and_recycles_after_factory_failure() {
        let buffer = NativeFeedbackBuffer::default();
        let creates = AtomicUsize::new(0);
        let create_buffer = buffer.clone();
        let first = buffer
            .checkout(|| {
                assert!(create_buffer.slot.try_lock().is_some());
                creates.fetch_add(1, Ordering::Relaxed);
                Ok(Vec::<u8>::with_capacity(32))
            })
            .unwrap();
        let allocation = first.as_ptr();
        drop(first);
        let second = buffer
            .checkout(|| -> Result<Vec<u8>> { panic!("reusable storage was not checked out") })
            .unwrap();
        assert_eq!(second.as_ptr(), allocation);
        assert_eq!(second.capacity(), 32);
        assert_eq!(creates.load(Ordering::Relaxed), 1);
        drop(second);
        assert!(buffer.is_available());

        let empty = NativeFeedbackBuffer::<Vec<u8>>::default();
        assert!(matches!(
            empty.checkout(|| Err(Error::RCL_RET_BAD_ALLOC)),
            Err(Error::RCL_RET_BAD_ALLOC)
        ));
        assert!(!empty.is_available());
    }

    #[test]
    fn native_feedback_buffer_retires_displaced_storage_after_unlock() {
        struct Resource {
            owner: Weak<Mutex<Option<Resource>>>,
            drops: Arc<AtomicUsize>,
        }
        impl Drop for Resource {
            fn drop(&mut self) {
                assert!(self.owner.upgrade().unwrap().try_lock().is_some());
                self.drops.fetch_add(1, Ordering::Relaxed);
            }
        }
        let slot = Arc::new(Mutex::new(None));
        let buffer = NativeFeedbackBuffer { slot: slot.clone() };
        let drops = Arc::new(AtomicUsize::new(0));
        let resource = || Resource {
            owner: Arc::downgrade(&slot),
            drops: drops.clone(),
        };
        let first = buffer.checkout(|| Ok(resource())).unwrap();
        let second = buffer.checkout(|| Ok(resource())).unwrap();
        drop(first);
        drop(second);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        let remaining = { slot.lock().take() };
        drop(remaining);
        assert_eq!(drops.load(Ordering::Relaxed), 2);
        drop(buffer);
        drop(slot);
    }

    #[test]
    fn coalesced_native_feedback_decodes_only_the_latest_when_capacity_recovers() {
        let id = uuid::Uuid::from_u128(1);
        let (sender, mut receiver) = mpsc::channel(0);
        let channel = Arc::new(Mutex::new(FeedbackChannel::with_overflow_policy(
            sender,
            FeedbackOverflowPolicy::Coalesce,
        )));
        let queue = Arc::new(Mutex::new(PendingFeedbackQueue::default()));
        (feedback_completion_with_queue(channel.clone(), vec![0_i32], Some((id, queue.clone())))
            .unwrap()
            .notify)()
        .unwrap();

        let buffer = NativeFeedbackBuffer::default();
        let decodes = Arc::new(AtomicUsize::new(0));
        for value in [1_i32, 2] {
            let native = buffer.checkout(|| Ok(vec![value; 4096])).unwrap();
            let counter = decodes.clone();
            let finish =
                completion(native, Some(channel.clone()), queue.clone(), id, move |native| {
                    counter.fetch_add(1, Ordering::Relaxed);
                    Ok((id, native.clone()))
                });
            (finish.notify)().unwrap();
        }
        assert_eq!(decodes.load(Ordering::Relaxed), 0);
        assert!(channel
            .lock()
            .pending
            .as_ref()
            .is_some_and(PendingFeedback::is_deferred));
        assert!(buffer.is_available(), "superseded native lease was not recycled");
        assert_eq!(receiver.next().now_or_never(), Some(Some(vec![0])));

        let channels = HashMap::from([(id, channel.clone())]);
        let (action, _) = take_ready_pending_feedback_with_progress(&queue, &channels);
        let PendingFeedbackAction::Deliver(deliver) = action else {
            panic!("latest deferred feedback was not ready after capacity recovery");
        };
        (deliver.notify)().unwrap();
        assert_eq!(decodes.load(Ordering::Relaxed), 1);
        let delivered = receiver.next().now_or_never().unwrap().unwrap();
        assert_eq!(delivered.len(), 4096);
        assert!(delivered.iter().all(|value| *value == 2));
        assert!(channel.lock().pending.is_none());
        assert!(queue.lock().is_empty());
        assert!(buffer.is_available());
    }

    #[cfg(all(target_os = "linux", r2r__example_interfaces__action__Fibonacci))]
    #[test]
    #[ignore = "manual native identity delivery allocation stage; LD_PRELOAD probe required"]
    fn benchmark_native_feedback_eliminates_erased_payload_allocation() {
        use crate::example_interfaces::action::Fibonacci;
        const ITERATIONS: usize = 100_000;
        let probe = allocation_probe::Probe::load()
            .unwrap()
            .expect("LD_PRELOAD allocation probe required");
        let id = uuid::Uuid::from_u128(1);
        for observed in [false, true] {
            let mut counts = Vec::new();
            for direct in [false, true] {
                let (sender, mut receiver) = mpsc::channel(0);
                let channel =
                    Arc::new(Mutex::new(FeedbackChannel::<Fibonacci::Feedback>::new(sender)));
                let pending = Arc::new(Mutex::new(PendingFeedbackQueue::default()));
                let native = NativeFeedbackBuffer::default();
                let channels = if observed {
                    HashMap::from([(id, channel.clone())])
                } else {
                    HashMap::new()
                };
                let client =
                    super::super::feedback_routing_tests::typed_state(channels, pending.clone());
                let decodes = Arc::new(AtomicUsize::new(0));
                let mut deliver = || {
                    let counter = decodes.clone();
                    let value = Fibonacci::Feedback::default();
                    let value = native.checkout(|| Ok(value)).unwrap();
                    let operation = if direct {
                        completion(
                            value,
                            observed.then(|| channel.clone()),
                            pending.clone(),
                            id,
                            move |value| {
                                counter.fetch_add(1, Ordering::Relaxed);
                                Ok((id, value.clone()))
                            },
                        )
                    } else {
                        action_feedback_decode_completion(client.clone(), value, move |value| {
                            counter.fetch_add(1, Ordering::Relaxed);
                            Ok((id, Box::new((*value).clone())))
                        })
                    };
                    (operation.notify)().unwrap();
                    if observed {
                        assert!(receiver.next().now_or_never().unwrap().is_some());
                    }
                };
                for _ in 0..1000 {
                    deliver();
                }
                decodes.store(0, Ordering::Relaxed);
                let mut stats = allocation_probe::NativeStats::default();
                assert_eq!(unsafe { (probe.begin)() }, 0);
                let begin = std::time::Instant::now();
                for _ in 0..ITERATIONS {
                    deliver();
                }
                let elapsed_ns = begin.elapsed().as_nanos();
                assert_eq!(unsafe { (probe.end)(&mut stats) }, 0);
                assert_eq!(
                    decodes.load(Ordering::Relaxed),
                    if direct && !observed { 0 } else { ITERATIONS }
                );
                let allocations = stats.malloc_calls
                    + stats.calloc_calls
                    + stats.realloc_calls
                    + stats.aligned_calls;
                counts.push(allocations);
                assert_eq!(
                    stats.failed_allocations + stats.counter_overflows + stats.size_overflows,
                    0
                );
                println!(
                    "{}",
                    serde_json::json!({"boundary":"native identity completion versus erased decode/route; empty Fibonacci payload; no ROS transport", "observed":observed, "direct":direct, "iterations":ITERATIONS, "elapsed_ns":elapsed_ns,"allocations":allocations,"requested_bytes":stats.requested_bytes,"decodes":decodes.load(Ordering::Relaxed)})
                );
            }
            assert_eq!(
                counts[0] - counts[1],
                ITERATIONS as u64,
                "one erased-payload allocation was not eliminated per message"
            );
        }
    }

    #[cfg(all(target_os = "linux", r2r__example_interfaces__action__Fibonacci))]
    #[test]
    #[ignore = "manual native feedback outer-storage reuse allocation stage; LD_PRELOAD probe required"]
    fn benchmark_native_feedback_buffer_reuses_outer_allocation() {
        use crate::example_interfaces::action::Fibonacci;
        const ITERATIONS: usize = 100_000;
        let probe = allocation_probe::Probe::load()
            .unwrap()
            .expect("LD_PRELOAD allocation probe required");
        let id = uuid::Uuid::from_u128(1);
        for reuse in [false, true] {
            let buffer = NativeFeedbackBuffer::default();
            let mut stats = allocation_probe::NativeStats::default();
            for _ in 0..1000 {
                let native = buffer
                    .checkout(WrappedNativeMsg::<Fibonacci::FeedbackMessage>::try_new)
                    .unwrap();
                let operation = completion(
                    native,
                    None::<FeedbackSender<Fibonacci::Feedback>>,
                    Arc::new(Mutex::new(PendingFeedbackQueue::default())),
                    id,
                    |_| panic!("unobserved feedback must not decode"),
                );
                (operation.notify)().unwrap();
                if !reuse {
                    let retired = { buffer.slot.lock().take() };
                    drop(retired);
                }
            }
            assert_eq!(unsafe { (probe.begin)() }, 0);
            let begin = std::time::Instant::now();
            for _ in 0..ITERATIONS {
                let native = buffer
                    .checkout(WrappedNativeMsg::<Fibonacci::FeedbackMessage>::try_new)
                    .unwrap();
                let operation = completion(
                    native,
                    None::<FeedbackSender<Fibonacci::Feedback>>,
                    Arc::new(Mutex::new(PendingFeedbackQueue::default())),
                    id,
                    |_| panic!("unobserved feedback must not decode"),
                );
                (operation.notify)().unwrap();
                if !reuse {
                    let retired = { buffer.slot.lock().take() };
                    drop(retired);
                }
            }
            let elapsed_ns = begin.elapsed().as_nanos();
            assert_eq!(unsafe { (probe.end)(&mut stats) }, 0);
            let allocations =
                stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls;
            assert_eq!(
                stats.failed_allocations + stats.counter_overflows + stats.size_overflows,
                0
            );
            println!(
                "{}",
                serde_json::json!({"boundary":"native feedback outer message create/destroy versus one-slot reuse; empty Fibonacci feedback; no RCL/DDS", "reuse":reuse, "iterations":ITERATIONS, "elapsed_ns":elapsed_ns,"allocations":allocations,"frees":stats.free_calls,"requested_bytes":stats.requested_bytes})
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "manual eager versus deferred coalesced large-feedback stage; LD_PRELOAD probe required"]
    fn benchmark_deferred_coalescing_skips_superseded_large_payload_decodes() {
        const ITERATIONS: usize = 10_000;
        const ELEMENTS: usize = 4096;
        let probe = allocation_probe::Probe::load()
            .unwrap()
            .expect("LD_PRELOAD allocation probe required");
        let id = uuid::Uuid::from_u128(1);
        for deferred in [false, true] {
            let (sender, mut receiver) = mpsc::channel(0);
            let channel = Arc::new(Mutex::new(FeedbackChannel::with_overflow_policy(
                sender,
                FeedbackOverflowPolicy::Coalesce,
            )));
            let queue = Arc::new(Mutex::new(PendingFeedbackQueue::default()));
            (feedback_completion_with_queue(
                channel.clone(),
                vec![-1_i32; ELEMENTS],
                Some((id, queue.clone())),
            )
            .unwrap()
            .notify)()
            .unwrap();
            let buffer = NativeFeedbackBuffer::default();
            let decodes = Arc::new(AtomicUsize::new(0));
            let arrival = |value: i32| {
                let mut native = buffer.checkout(|| Ok(vec![0_i32; ELEMENTS])).unwrap();
                native.fill(std::hint::black_box(value));
                let counter = decodes.clone();
                let delivery_channel = channel.clone();
                let delivery_queue = queue.clone();
                let finish = if deferred {
                    completion(native, Some(delivery_channel), delivery_queue, id, move |native| {
                        counter.fetch_add(1, Ordering::Relaxed);
                        Ok((id, native.clone()))
                    })
                } else {
                    ActionClientCompletion {
                        notify: Box::new(move || {
                            counter.fetch_add(1, Ordering::Relaxed);
                            let payload = native.to_vec();
                            let deliver = feedback_completion_with_queue(
                                delivery_channel,
                                payload,
                                Some((id, delivery_queue)),
                            )
                            .unwrap();
                            (deliver.notify)()
                        }),
                        cleanup_goal_on_failure: None,
                    }
                };
                (finish.notify)().unwrap();
            };
            for value in 0..100 {
                arrival(value);
            }
            decodes.store(0, Ordering::Relaxed);
            let mut stats = allocation_probe::NativeStats::default();
            assert_eq!(unsafe { (probe.begin)() }, 0);
            let begin = std::time::Instant::now();
            for value in 0..ITERATIONS {
                arrival(value as i32);
            }
            let elapsed_ns = begin.elapsed().as_nanos();
            assert_eq!(unsafe { (probe.end)(&mut stats) }, 0);
            let measured_decodes = decodes.load(Ordering::Relaxed);
            assert_eq!(measured_decodes, if deferred { 0 } else { ITERATIONS });
            let allocations =
                stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls;
            assert_eq!(
                stats.failed_allocations + stats.counter_overflows + stats.size_overflows,
                0
            );
            println!(
                "{}",
                serde_json::json!({"boundary":"full-channel coalesced 4096-i32 native arrivals; eager owned clone versus deferred latest-only decode; no RCL/DDS", "deferred":deferred, "iterations":ITERATIONS, "elements":ELEMENTS, "elapsed_ns":elapsed_ns,"allocations":allocations,"frees":stats.free_calls,"requested_bytes":stats.requested_bytes,"arrival_decodes":measured_decodes})
            );

            assert!(receiver.next().now_or_never().unwrap().is_some());
            let channels = HashMap::from([(id, channel.clone())]);
            let (action, _) = take_ready_pending_feedback_with_progress(&queue, &channels);
            let PendingFeedbackAction::Deliver(deliver) = action else {
                panic!("pending latest feedback did not become ready");
            };
            (deliver.notify)().unwrap();
            let latest = receiver.next().now_or_never().unwrap().unwrap();
            assert_eq!(latest.len(), ELEMENTS);
            assert!(latest.iter().all(|value| *value == ITERATIONS as i32 - 1));
            assert_eq!(decodes.load(Ordering::Relaxed), if deferred { 1 } else { ITERATIONS });
        }
    }

    #[cfg(r2r__example_interfaces__action__Fibonacci)]
    #[test]
    fn generated_native_uuid_matches_payload_decode_for_typed_and_untyped_messages() {
        use crate::example_interfaces::action::Fibonacci;
        let support =
            UntypedActionSupport::new_from("example_interfaces/action/Fibonacci").unwrap();
        for length in [0, 1, 32, 65536] {
            for uuid in [
                [0; 16],
                [255; 16],
                [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
            ] {
                let message = <Fibonacci::Action as WrappedActionTypeSupport>::make_feedback_msg(
                    unique_identifier_msgs::msg::UUID { uuid },
                    Fibonacci::Feedback {
                        sequence: (0..length).collect(),
                    },
                );
                let native = WrappedNativeMsg::try_from(&message).unwrap();
                assert_eq!(
                    <Fibonacci::Action as WrappedActionTypeSupport>::native_feedback_uuid(&native),
                    Some(uuid)
                );
                let decoded = Fibonacci::FeedbackMessage::from_native(&native);
                assert_eq!(decoded, message);
                let mut untyped = (support.make_feedback_msg)().unwrap();
                message
                    .try_copy_to_native(unsafe {
                        &mut *(untyped.void_ptr_mut()
                            as *mut <Fibonacci::FeedbackMessage as WrappedTypesupport>::CStruct)
                    })
                    .unwrap();
                assert_eq!((support.native_feedback_uuid)(&untyped), Some(uuid));
                let (id, decoded) = (support.destructure_feedback_msg)(&untyped);
                assert_eq!(id.uuid, uuid);
                assert_eq!(
                    serde_json::from_value::<Fibonacci::Feedback>(decoded.unwrap()).unwrap(),
                    message.feedback
                );
            }
        }
    }

    #[test]
    fn unobserved_native_feedback_is_not_decoded_and_resources_drop_after_unlock() {
        struct Resource {
            owner: Weak<Mutex<()>>,
            drops: Arc<AtomicUsize>,
        }
        impl Drop for Resource {
            fn drop(&mut self) {
                assert!(self.owner.upgrade().unwrap().try_lock().is_some());
                self.drops.fetch_add(1, Ordering::Relaxed);
            }
        }
        let owner = Arc::new(Mutex::new(()));
        let drops = Arc::new(AtomicUsize::new(0));
        let decodes = Arc::new(AtomicUsize::new(0));
        let native = Resource {
            owner: Arc::downgrade(&owner),
            drops: drops.clone(),
        };
        let decoder_resource = Resource {
            owner: Arc::downgrade(&owner),
            drops: drops.clone(),
        };
        let counter = decodes.clone();
        let (buffer, native) = leased(native);
        let finish = {
            let _locked = owner.lock();
            completion(
                native,
                None::<FeedbackSender<u32>>,
                Arc::new(Mutex::new(PendingFeedbackQueue::default())),
                uuid::Uuid::nil(),
                move |native| {
                    counter.fetch_add(1, Ordering::Relaxed);
                    let _ = native;
                    drop(decoder_resource);
                    Ok((uuid::Uuid::nil(), 1))
                },
            )
        };
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        (finish.notify)().unwrap();
        assert_eq!(decodes.load(Ordering::Relaxed), 0);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert!(buffer.is_available());
        drop(buffer);
        assert_eq!(drops.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn observed_native_feedback_decodes_once_and_delivers_without_erased_payload() {
        let id = uuid::Uuid::from_u128(1);
        let (sender, mut receiver) = mpsc::channel(0);
        let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
        let queue = Arc::new(Mutex::new(PendingFeedbackQueue::default()));
        let decodes = Arc::new(AtomicUsize::new(0));
        let counter = decodes.clone();
        let (buffer, native) = leased(42_u32);
        let finish = completion(native, Some(channel), queue, id, move |value| {
            counter.fetch_add(1, Ordering::Relaxed);
            Ok((id, *value))
        });
        assert_eq!(decodes.load(Ordering::Relaxed), 0);
        (finish.notify)().unwrap();
        assert_eq!(receiver.next().now_or_never(), Some(Some(42)));
        assert_eq!(decodes.load(Ordering::Relaxed), 1);
        assert!(buffer.is_available());
    }

    #[test]
    fn closing_during_decode_or_before_completion_cannot_restore_a_feedback_sender() {
        for before in [false, true] {
            let (sender, mut receiver) = mpsc::channel(0);
            let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
            let close = channel.clone();
            let decodes = Arc::new(AtomicUsize::new(0));
            let counter = decodes.clone();
            let (buffer, native) = leased(1_u32);
            let finish = completion(
                native,
                Some(channel.clone()),
                Arc::new(Mutex::new(PendingFeedbackQueue::default())),
                uuid::Uuid::nil(),
                move |value| {
                    counter.fetch_add(1, Ordering::Relaxed);
                    (close_feedback_completion(close).notify)().unwrap();
                    Ok((uuid::Uuid::nil(), *value))
                },
            );
            if before {
                (close_feedback_completion(channel.clone()).notify)().unwrap();
            }
            (finish.notify)().unwrap();
            assert_eq!(decodes.load(Ordering::Relaxed), if before { 0 } else { 1 });
            assert!(channel.lock().sender.is_none());
            assert!(channel.lock().pending.is_none());
            assert_eq!(receiver.next().now_or_never(), Some(None));
            assert!(buffer.is_available());
        }
    }

    #[test]
    fn native_uuid_mismatch_rejects_delivery_and_retains_the_channel() {
        let (sender, mut receiver) = mpsc::channel(0);
        let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
        let (buffer, native) = leased(1_u32);
        let finish = completion(
            native,
            Some(channel.clone()),
            Arc::new(Mutex::new(PendingFeedbackQueue::default())),
            uuid::Uuid::from_u128(1),
            |value| Ok((uuid::Uuid::from_u128(2), *value)),
        );
        assert!(matches!((finish.notify)(), Err(Error::RCL_RET_ACTION_CLIENT_INVALID)));
        assert!(receiver.next().now_or_never().is_none());
        assert!(channel.lock().sender.is_some());
        assert!(buffer.is_available());
    }

    #[test]
    fn native_decode_error_recycles_storage_without_touching_the_channel() {
        let (sender, mut receiver) = mpsc::channel::<u32>(0);
        let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
        let (buffer, native) = leased(1_u32);
        let finish = completion(
            native,
            Some(channel.clone()),
            Arc::new(Mutex::new(PendingFeedbackQueue::default())),
            uuid::Uuid::nil(),
            |_| Err(Error::RCL_RET_BAD_ALLOC),
        );
        assert!(matches!((finish.notify)(), Err(Error::RCL_RET_BAD_ALLOC)));
        assert!(receiver.next().now_or_never().is_none());
        assert!(channel.lock().sender.is_some());
        assert!(buffer.is_available());
    }
}
