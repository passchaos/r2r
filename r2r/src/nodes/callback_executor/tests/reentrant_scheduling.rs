use super::*;
use std::thread;

#[test]
fn reentrant_group_error_and_panic_cancel_lanes_and_recover_mixed_group_order() {
    for panic in [false, true] {
        let before = CallbackGroup::new();
        let after = CallbackGroup::new();
        let mut group = ReentrantCallbackGroup::new(2).unwrap();
        let callbacks: Vec<_> = (0..2)
            .map(|_| {
                move |_: usize| {
                    if panic {
                        panic!("intentional reentrant callback panic");
                    }
                    Err(Error::RCL_RET_INVALID_ARGUMENT)
                }
            })
            .collect();
        let (producer, consumers) = group.prepare_inputs(2, callbacks).unwrap();
        group.commit_inputs(consumers);
        let ids = [before.id(), group.id(), after.id()];
        let scheduler = scheduler::Scheduler::new(
            None,
            vec![
                OwnedGroup::Exclusive(before),
                OwnedGroup::Reentrant(group),
                OwnedGroup::Exclusive(after),
            ],
            2,
        )
        .unwrap();
        producer.push(11).unwrap();
        let worker = scheduler.clone();
        let result = thread::spawn(move || worker.run(0)).join().unwrap();
        if panic {
            assert!(matches!(result, Err(CallbackWorkerError::Panic)));
        } else {
            assert!(
                matches!(result,Err(CallbackWorkerError::Group {id,source:Error::RCL_RET_INVALID_ARGUMENT}) if id==ids[1])
            );
        }
        let worker = scheduler.clone();
        thread::spawn(move || worker.run(1))
            .join()
            .unwrap()
            .unwrap();
        let (_, mut groups) = scheduler.recover();
        assert_eq!(groups.iter().map(OwnedGroup::id).collect::<Vec<_>>(), ids);
        assert_eq!(groups[1].reentrant_mut().unwrap().concurrency(), 2);
        assert!(groups[1].cancelled().load(Ordering::Acquire));
        drop(groups);
        assert!(producer.is_closed());
    }
}

#[test]
fn reentrant_group_zero_concurrency_is_rejected_and_single_lane_stays_reentrant() {
    assert!(matches!(ReentrantCallbackGroup::new(0), Err(Error::RCL_RET_INVALID_ARGUMENT)));
    assert!(matches!(
        ReentrantCallbackGroup::new(usize::MAX),
        Err(Error::RCL_RET_INVALID_ARGUMENT)
    ));
    let group = ReentrantCallbackGroup::new(1).unwrap();
    let id = group.id();
    let scheduler = scheduler::Scheduler::new(None, vec![group], 1).unwrap();
    scheduler.run(0).unwrap();
    let (_, mut groups) = scheduler.recover();
    assert_eq!(groups[0].id(), id);
    assert_eq!(groups[0].reentrant_mut().unwrap().concurrency(), 1);
    assert!(groups[0].exclusive_mut().is_none());
}
