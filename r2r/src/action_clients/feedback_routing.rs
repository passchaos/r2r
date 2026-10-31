//! Preserve caller ownership when an erased feedback payload cannot be delivered.

use super::*;

#[cfg(test)]
use crate::allocation_probe;

// Keep the ownership boundary in the typed caller; forwarding adds no independent work.
#[inline(always)]
pub(crate) fn route<T: Send + 'static>(
    channels: &HashMap<uuid::Uuid, FeedbackSender<T>>, pending: &Arc<Mutex<PendingFeedbackQueue>>,
    uuid: &uuid::Uuid, slot: &mut Option<Box<dyn Any>>,
) -> Result<Option<ActionClientCompletion>> {
    let payload = slot.take().ok_or(Error::RCL_RET_ACTION_CLIENT_INVALID)?;
    let payload = match payload.downcast::<T>() {
        Ok(payload) => payload,
        Err(payload) => {
            *slot = Some(payload);
            return Err(Error::RCL_RET_ACTION_CLIENT_INVALID);
        }
    };
    if let Some(channel) = channels.get(uuid) {
        Ok(feedback_completion_with_queue(
            Arc::clone(channel),
            *payload,
            Some((*uuid, Arc::clone(pending))),
        ))
    } else {
        // Re-erasing this Box only restores its vtable; the allocation stays unchanged.
        *slot = Some(payload);
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{FutureExt, StreamExt};

    struct Payload {
        value: u32,
        owner: Weak<Mutex<()>>,
        drops: Arc<AtomicUsize>,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            let owner = self.owner.upgrade().unwrap();
            assert!(owner.try_lock().is_some(), "routing released a payload under its caller lock");
            self.drops.fetch_add(1, Ordering::Relaxed);
        }
    }
    fn address(slot: &Option<Box<dyn Any>>) -> *const () {
        &**slot.as_ref().unwrap() as *const dyn Any as *const ()
    }

    #[test]
    fn unmapped_feedback_restores_the_original_box_until_caller_unlocks() {
        let owner = Arc::new(Mutex::new(()));
        let drops = Arc::new(AtomicUsize::new(0));
        let mut slot: Option<Box<dyn Any>> = Some(Box::new(Payload {
            value: 7,
            owner: Arc::downgrade(&owner),
            drops: drops.clone(),
        }));
        let original = address(&slot);
        let channels = HashMap::<uuid::Uuid, FeedbackSender<Payload>>::new();
        let pending = Arc::new(Mutex::new(PendingFeedbackQueue::default()));
        {
            let _guard = owner.lock();
            assert!(route(&channels, &pending, &uuid::Uuid::nil(), &mut slot)
                .unwrap()
                .is_none());
            assert_eq!(address(&slot), original);
            assert_eq!(drops.load(Ordering::Relaxed), 0);
        }
        drop(slot);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert!(pending.lock().is_empty());
    }

    #[test]
    fn wrong_feedback_type_restores_original_erasure_and_empty_slot_is_rejected() {
        let owner = Arc::new(Mutex::new(()));
        let drops = Arc::new(AtomicUsize::new(0));
        let mut slot: Option<Box<dyn Any>> = Some(Box::new(Payload {
            value: 7,
            owner: Arc::downgrade(&owner),
            drops: drops.clone(),
        }));
        let original = address(&slot);
        let channels = HashMap::<uuid::Uuid, FeedbackSender<u32>>::new();
        let pending = Arc::new(Mutex::new(PendingFeedbackQueue::default()));
        {
            let _guard = owner.lock();
            assert!(matches!(
                route(&channels, &pending, &uuid::Uuid::nil(), &mut slot),
                Err(Error::RCL_RET_ACTION_CLIENT_INVALID)
            ));
            assert_eq!(address(&slot), original);
            assert!(slot.as_ref().unwrap().is::<Payload>());
            assert_eq!(drops.load(Ordering::Relaxed), 0);
        }
        drop(slot);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert!(matches!(
            route(&channels, &pending, &uuid::Uuid::nil(), &mut None),
            Err(Error::RCL_RET_ACTION_CLIENT_INVALID)
        ));
    }

    #[test]
    fn tracked_feedback_moves_to_delivery_without_destruction_under_caller_lock() {
        let owner = Arc::new(Mutex::new(()));
        let drops = Arc::new(AtomicUsize::new(0));
        let mut slot: Option<Box<dyn Any>> = Some(Box::new(Payload {
            value: 42,
            owner: Arc::downgrade(&owner),
            drops: drops.clone(),
        }));
        let (sender, mut receiver) = mpsc::channel::<Payload>(0);
        let channels = HashMap::from([(
            uuid::Uuid::nil(),
            Arc::new(Mutex::new(FeedbackChannel::new(sender))),
        )]);
        let pending = Arc::new(Mutex::new(PendingFeedbackQueue::default()));
        let delivery = {
            let _guard = owner.lock();
            let delivery = route(&channels, &pending, &uuid::Uuid::nil(), &mut slot)
                .unwrap()
                .unwrap();
            assert!(slot.is_none());
            assert_eq!(drops.load(Ordering::Relaxed), 0);
            delivery
        };
        (delivery.notify)().unwrap();
        let payload = receiver.next().now_or_never().unwrap().unwrap();
        assert_eq!(payload.value, 42);
        drop(payload);
        assert_eq!(drops.load(Ordering::Relaxed), 1);
        assert!(pending.lock().is_empty());
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "manual optimized routing stage; LD_PRELOAD allocation probe required"]
    fn benchmark_borrowed_feedback_routing_without_allocations() {
        const ITERATIONS: usize = 2_000_000;
        let probe = allocation_probe::Probe::load()
            .unwrap()
            .expect("load libr2r_bench_probe.so with LD_PRELOAD");
        let channels = HashMap::<uuid::Uuid, FeedbackSender<u32>>::new();
        let pending = Arc::new(Mutex::new(PendingFeedbackQueue::default()));
        let owner = Mutex::new(());
        let uuid = uuid::Uuid::nil();
        for rejected in [false, true] {
            let mut slot: Option<Box<dyn Any>> = if rejected {
                Some(Box::new(1_u64))
            } else {
                Some(Box::new(1_u32))
            };
            let original = address(&slot);
            for _ in 0..1000 {
                let _guard = owner.lock();
                let result = route(&channels, &pending, &uuid, &mut slot);
                assert_eq!(result.is_err(), rejected);
            }
            let mut stats = allocation_probe::NativeStats::default();
            assert_eq!(unsafe { (probe.begin)() }, 0);
            let begin = std::time::Instant::now();
            let mut errors = 0;
            for _ in 0..ITERATIONS {
                let _guard = owner.lock();
                errors += std::hint::black_box(route(&channels, &pending, &uuid, &mut slot))
                    .is_err() as usize;
            }
            let elapsed_ns = begin.elapsed().as_nanos();
            assert_eq!(unsafe { (probe.end)(&mut stats) }, 0);
            assert_eq!(address(&slot), original);
            assert_eq!(errors, if rejected { ITERATIONS } else { 0 });
            assert_eq!(
                stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls,
                0
            );
            assert_eq!(
                stats.free_calls
                    + stats.failed_allocations
                    + stats.requested_bytes
                    + stats.size_overflows
                    + stats.counter_overflows,
                0
            );
            println!(
                "{}",
                serde_json::json!({"boundary":"borrowed erased feedback routing and uncontended caller mutex; no decoding or ROS transport", "rejected":rejected, "iterations":ITERATIONS,"elapsed_ns":elapsed_ns,"allocations":0,"frees":stats.free_calls,"requested_bytes":stats.requested_bytes})
            );
        }
    }
}
