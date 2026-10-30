use super::*;
use std::{
    sync::{mpsc, Condvar, Mutex as StdMutex},
    thread,
};

pub(super) struct Release(pub(super) Arc<(StdMutex<bool>, Condvar)>);
impl Drop for Release {
    fn drop(&mut self) {
        let (lock, ready) = &*self.0;
        *lock.lock().unwrap() = true;
        ready.notify_all();
    }
}

#[test]
fn callback_group_workers_progress_while_another_group_is_blocked() {
    let mut slow = CallbackGroup::new();
    let mut healthy = CallbackGroup::new();
    let blocked = Arc::new((StdMutex::new(false), Condvar::new()));
    let release = Release(blocked.clone());
    let (started, start) = mpsc::channel();
    let (progress, seen) = mpsc::channel();
    let input_slow = slow
        .input(1, move |_: ()| {
            started.send(thread::current().id()).unwrap();
            let (lock, ready) = &*blocked;
            let mut released = lock.lock().unwrap();
            while !*released {
                released = ready.wait(released).unwrap();
            }
            Ok(())
        })
        .unwrap();
    let input_healthy = healthy
        .input(1, move |_: ()| {
            progress.send(thread::current().id()).unwrap();
            Ok(())
        })
        .unwrap();
    let groups = vec![slow, healthy];
    let ids: Vec<_> = groups.iter().map(CallbackGroup::id).collect();
    let scheduler = scheduler::Scheduler::new(None, groups, 2).unwrap();
    let workers: Vec<_> = (0..2)
        .map(|worker| {
            let scheduler = scheduler.clone();
            thread::spawn(move || scheduler.run(worker))
        })
        .collect();
    input_slow.push(()).unwrap();
    let slow_thread = start.recv_timeout(Duration::from_secs(3)).unwrap();
    input_healthy.push(()).unwrap();
    let progress = seen.recv_timeout(Duration::from_millis(200));
    scheduler.cancel().unwrap();
    drop(release);
    for worker in workers {
        worker.join().unwrap().unwrap();
    }
    assert_ne!(slow_thread, progress.expect("blocked group stalled a different group"));
    let (native, groups) = scheduler.recover();
    assert!(native.is_none());
    assert_eq!(groups.iter().map(CallbackGroup::id).collect::<Vec<_>>(), ids);
    assert_eq!(groups.len(), 2);
    drop(input_slow);
    drop(input_healthy);
}

#[test]
fn callback_group_coalesced_wakes_never_execute_one_group_concurrently() {
    let active = Arc::new(AtomicUsize::new(0));
    let completed = Arc::new(AtomicUsize::new(0));
    let mut group = CallbackGroup::new();
    let mut producers = Vec::new();
    for _ in 0..4 {
        let active = active.clone();
        let completed = completed.clone();
        producers.push(
            group
                .input(32, move |_: usize| {
                    assert_eq!(active.fetch_add(1, Ordering::AcqRel), 0);
                    thread::yield_now();
                    assert_eq!(active.fetch_sub(1, Ordering::AcqRel), 1);
                    completed.fetch_add(1, Ordering::Release);
                    Ok(())
                })
                .unwrap(),
        );
    }
    let scheduler = scheduler::Scheduler::new(None, vec![group], 4).unwrap();
    let workers: Vec<_> = (0..4)
        .map(|worker| {
            let scheduler = scheduler.clone();
            thread::spawn(move || scheduler.run(worker))
        })
        .collect();
    let senders: Vec<_> = producers
        .into_iter()
        .map(|producer| {
            thread::spawn(move || {
                for value in 0..1000 {
                    let mut value = value;
                    loop {
                        if producer.is_ready() {
                            match producer.push(value) {
                                Ok(()) => break,
                                Err(returned) => value = returned,
                            }
                        }
                        thread::yield_now();
                    }
                }
                producer
            })
        })
        .collect();
    let producers: Vec<_> = senders
        .into_iter()
        .map(|sender| sender.join().unwrap())
        .collect();
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while completed.load(Ordering::Acquire) != 4000 && std::time::Instant::now() < deadline {
        thread::yield_now();
    }
    scheduler.cancel().unwrap();
    for worker in workers {
        worker.join().unwrap().unwrap();
    }
    assert_eq!(completed.load(Ordering::Acquire), 4000);
    drop(scheduler.recover());
    drop(producers);
}

#[test]
fn callback_group_worker_failure_cancels_unpolled_peers_and_preserves_owned_state() {
    let mut bad = CallbackGroup::new();
    let id = bad.id();
    let input = bad
        .input(1, |_: ()| Err(Error::RCL_RET_INVALID_ARGUMENT))
        .unwrap();
    let mut idle = CallbackGroup::new();
    let peer = idle
        .input(1, |_: ()| panic!("canceled peer ran user work"))
        .unwrap();
    let scheduler = scheduler::Scheduler::new(None, vec![bad, idle], 2).unwrap();
    input.push(()).unwrap();
    let first = scheduler.clone();
    let first = thread::spawn(move || first.run(0));
    let outcome = first.join().unwrap();
    assert!(
        matches!(outcome, Err(scheduler::Failure::Group { id: failed, source: Error::RCL_RET_INVALID_ARGUMENT }) if failed == id)
    );
    peer.push(()).unwrap();
    let second = scheduler.clone();
    thread::spawn(move || second.run(1))
        .join()
        .unwrap()
        .unwrap();
    let (_, groups) = scheduler.recover();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].id(), id);
    drop(groups);
    assert!(input.is_closed());
    assert!(peer.is_closed());
}
