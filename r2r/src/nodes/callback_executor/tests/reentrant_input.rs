use super::*;

#[test]
fn reentrant_input_dropping_one_receiver_keeps_other_receivers_and_native_source_open() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let callbacks: Vec<_> = (0..2)
        .map(|_| {
            let seen = seen.clone();
            move |value: usize| {
                seen.lock().push(value);
                Ok(())
            }
        })
        .collect();
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (producer, mut receivers) = input::channel_many(2, flag, callbacks).unwrap();
    drop(receivers.pop().unwrap());
    assert!(!producer.is_closed());
    producer.push(42).unwrap();
    let wake = futures::task::noop_waker();
    assert!(Pin::new(&mut receivers[0])
        .poll(&mut TaskContext::from_waker(&wake))
        .is_pending());
    assert_eq!(*seen.lock(), [42]);
    drop(receivers);
    assert!(producer.is_closed());
}

#[test]
fn reentrant_input_parallel_receivers_take_each_owned_input_exactly_once() {
    use futures::task::noop_waker;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let callbacks: Vec<_> = (0..4)
        .map(|_| {
            let seen = seen.clone();
            move |value: usize| {
                seen.lock().push(value);
                Ok(())
            }
        })
        .collect();
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (producer, receivers) = input::channel_many(64, flag, callbacks).unwrap();
    for value in 0..64 {
        producer.push(value).unwrap();
    }
    let workers: Vec<_> = receivers
        .into_iter()
        .map(|mut receiver| {
            std::thread::spawn(move || {
                let wake = noop_waker();
                for _ in 0..64 {
                    assert!(Pin::new(&mut receiver)
                        .poll(&mut TaskContext::from_waker(&wake))
                        .is_pending());
                }
                receiver
            })
        })
        .collect();
    let receivers: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    let mut values = seen.lock().clone();
    values.sort_unstable();
    assert_eq!(values, (0..64).collect::<Vec<_>>());
    assert!(!producer.is_closed());
    drop(receivers);
    assert!(producer.is_closed());
}

#[test]
fn reentrant_input_close_during_publication_retires_payloads_before_native_producer_drop() {
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Release);
        }
    }
    let callbacks: Vec<_> = (0..4)
        .map(|_| |_: Payload| panic!("unpolled receiver invoked"))
        .collect();
    let (producer, receivers) =
        input::channel_many(32, Arc::new(std::sync::atomic::AtomicBool::new(false)), callbacks)
            .unwrap();
    let producer = Arc::new(producer);
    let retired = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let sender = producer.clone();
    let count = retired.clone();
    let begin = barrier.clone();
    let sender = std::thread::spawn(move || {
        begin.wait();
        for _ in 0..20000 {
            if let Err(payload) = sender.push(Payload(count.clone())) {
                drop(payload);
            }
            std::thread::yield_now();
        }
    });
    barrier.wait();
    std::thread::yield_now();
    drop(receivers);
    sender.join().unwrap();
    assert!(producer.is_closed());
    assert_eq!(retired.load(Ordering::Acquire), 20000);
}

#[test]
fn reentrant_input_cancel_retains_queue_until_reset_and_close_wakes_every_receiver() {
    let seen = Arc::new(AtomicUsize::new(0));
    let callbacks: Vec<_> = (0..3)
        .map(|_| {
            let seen = seen.clone();
            move |value: usize| {
                seen.fetch_add(value, Ordering::Release);
                Ok(())
            }
        })
        .collect();
    let flag = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let (producer, mut receivers) = input::channel_many(1, flag.clone(), callbacks).unwrap();
    let wakes: Vec<_> = (0..3)
        .map(|_| Arc::new(WakeCount(AtomicUsize::new(0))))
        .collect();
    producer.push(42).unwrap();
    assert!(!producer.is_ready());
    for (receiver, wake) in receivers.iter_mut().zip(&wakes) {
        let waker = futures::task::waker(wake.clone());
        assert!(Pin::new(receiver)
            .poll(&mut TaskContext::from_waker(&waker))
            .is_pending());
    }
    assert_eq!(seen.load(Ordering::Acquire), 0);
    flag.store(false, Ordering::Release);
    let wake = futures::task::waker(wakes[0].clone());
    assert!(Pin::new(&mut receivers[0])
        .poll(&mut TaskContext::from_waker(&wake))
        .is_pending());
    assert_eq!(seen.load(Ordering::Acquire), 42);
    let before: Vec<_> = wakes
        .iter()
        .map(|wake| wake.0.load(Ordering::Acquire))
        .collect();
    producer.close();
    for (wake, before) in wakes.iter().zip(before) {
        assert_eq!(wake.0.load(Ordering::Acquire), before + 1);
    }
    for receiver in &mut receivers {
        assert!(matches!(
            Pin::new(receiver).poll(&mut TaskContext::from_waker(&wake)),
            Poll::Ready(Ok(()))
        ));
    }
}
