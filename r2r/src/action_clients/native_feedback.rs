//! Route known native identity before decoding its payload.

use super::*;

#[cfg(test)]
use super::feedback_routing::allocation_probe;

pub(crate) fn completion<N: 'static, T: Send + 'static>(
    native: N, channel: Option<FeedbackSender<T>>, pending: Arc<Mutex<PendingFeedbackQueue>>,
    uuid: uuid::Uuid, decode: impl FnOnce(N) -> Result<(uuid::Uuid, T)> + 'static,
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
            let closed = { channel.lock().is_closed() };
            if closed {
                drop(native);
                drop(decode);
                return Ok(true);
            }
            let (decoded_uuid, payload) = decode(native)?;
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
                    let operation = if direct {
                        completion(
                            value,
                            observed.then(|| channel.clone()),
                            pending.clone(),
                            id,
                            move |value| {
                                counter.fetch_add(1, Ordering::Relaxed);
                                Ok((id, value))
                            },
                        )
                    } else {
                        action_feedback_decode_completion(client.clone(), value, move |value| {
                            counter.fetch_add(1, Ordering::Relaxed);
                            Ok((id, Box::new(value)))
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
                let (id, decoded) = (support.destructure_feedback_msg)(untyped);
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
        let finish = {
            let _locked = owner.lock();
            completion(
                native,
                None::<FeedbackSender<u32>>,
                Arc::new(Mutex::new(PendingFeedbackQueue::default())),
                uuid::Uuid::nil(),
                move |native| {
                    counter.fetch_add(1, Ordering::Relaxed);
                    drop(native);
                    drop(decoder_resource);
                    Ok((uuid::Uuid::nil(), 1))
                },
            )
        };
        assert_eq!(drops.load(Ordering::Relaxed), 0);
        (finish.notify)().unwrap();
        assert_eq!(decodes.load(Ordering::Relaxed), 0);
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
        let finish = completion(42_u32, Some(channel), queue, id, move |value| {
            counter.fetch_add(1, Ordering::Relaxed);
            Ok((id, value))
        });
        assert_eq!(decodes.load(Ordering::Relaxed), 0);
        (finish.notify)().unwrap();
        assert_eq!(receiver.next().now_or_never(), Some(Some(42)));
        assert_eq!(decodes.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn closing_during_decode_or_before_completion_cannot_restore_a_feedback_sender() {
        for before in [false, true] {
            let (sender, mut receiver) = mpsc::channel(0);
            let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
            let close = channel.clone();
            let decodes = Arc::new(AtomicUsize::new(0));
            let counter = decodes.clone();
            let finish = completion(
                1_u32,
                Some(channel.clone()),
                Arc::new(Mutex::new(PendingFeedbackQueue::default())),
                uuid::Uuid::nil(),
                move |value| {
                    counter.fetch_add(1, Ordering::Relaxed);
                    (close_feedback_completion(close).notify)().unwrap();
                    Ok((uuid::Uuid::nil(), value))
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
        }
    }

    #[test]
    fn native_uuid_mismatch_rejects_delivery_and_retains_the_channel() {
        let (sender, mut receiver) = mpsc::channel(0);
        let channel = Arc::new(Mutex::new(FeedbackChannel::new(sender)));
        let finish = completion(
            1_u32,
            Some(channel.clone()),
            Arc::new(Mutex::new(PendingFeedbackQueue::default())),
            uuid::Uuid::from_u128(1),
            |value| Ok((uuid::Uuid::from_u128(2), value)),
        );
        assert!(matches!((finish.notify)(), Err(Error::RCL_RET_ACTION_CLIENT_INVALID)));
        assert!(receiver.next().now_or_never().is_none());
        assert!(channel.lock().sender.is_some());
    }
}
