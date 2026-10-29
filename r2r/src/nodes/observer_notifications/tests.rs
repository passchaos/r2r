use super::*;
use crate::{
    example_interfaces::{action::Fibonacci, srv::AddTwoInts},
    nodes::{
        future_executor::{drive_future, FutureDriver, FutureGuard},
        Executor,
    },
    Context, Error, FutureExecutionOptions, QosProfile,
};
use futures::{future::BoxFuture, FutureExt};
use std::{sync::atomic::Ordering, time::Duration};

type Wait = BoxFuture<'static, crate::Result<()>>;
type Probe = Box<dyn Fn() -> bool>;

fn observers(node: &mut Node) -> (Vec<Wait>, Vec<Probe>) {
    let prefix = format!("/foreground_cancel_{}", uuid::Uuid::new_v4().simple());
    let typed = node
        .create_client::<AddTwoInts::Service>(
            &format!("{prefix}/typed"),
            QosProfile::services_default(),
        )
        .unwrap();
    let untyped = node
        .create_client_untyped(
            &format!("{prefix}/untyped"),
            "example_interfaces/srv/AddTwoInts",
            QosProfile::services_default(),
        )
        .unwrap();
    let action = node
        .create_action_client::<Fibonacci::Action>(&format!("{prefix}/action"))
        .unwrap();
    let json_action = node
        .create_action_client_untyped(
            &format!("{prefix}/action"),
            "example_interfaces/action/Fibonacci",
        )
        .unwrap();
    let publisher = node
        .create_publisher::<crate::std_msgs::msg::Int32>(
            &format!("{prefix}/typed_data"),
            QosProfile::default(),
        )
        .unwrap();
    let json_publisher = node
        .create_publisher_untyped(
            &format!("{prefix}/json_data"),
            "std_msgs/msg/Int32",
            QosProfile::default(),
        )
        .unwrap();
    let subscription = node
        .subscribe::<crate::std_msgs::msg::Int32>(
            &format!("{prefix}/subscribed"),
            QosProfile::default(),
        )
        .unwrap();
    let waits = vec![
        typed.wait_for_service().unwrap().boxed(),
        untyped.wait_for_service().unwrap().boxed(),
        action.wait_for_action_server().unwrap().boxed(),
        json_action.wait_for_action_server().unwrap().boxed(),
        publisher.wait_for_subscribers().unwrap().boxed(),
        json_publisher.wait_for_subscribers().unwrap().boxed(),
        subscription.wait_for_publishers(node).unwrap().boxed(),
        node.wait_for_graph_change().boxed(),
    ];
    let typed_native = typed.client.upgrade().unwrap();
    let untyped_native = untyped.client.upgrade().unwrap();
    let actions = node.action_clients.clone();
    let publisher_native = publisher.handle.upgrade().unwrap();
    let json_publisher_native = json_publisher.handle.upgrade().unwrap();
    let subscription_probe = node
        .subscriber(subscription.id)
        .unwrap()
        .match_state()
        .unwrap()
        .waiters
        .cancellation_probe();
    let graph_probe = node.graph_change_waiters.cancellation_probe();
    let probes: Vec<Probe> = vec![
        Box::new(move || {
            let _keep_alive = &typed;
            typed_native.lock().poll_available_channels.is_empty()
        }),
        Box::new(move || {
            let _keep_alive = &untyped;
            untyped_native.lock().poll_available_channels.is_empty()
        }),
        Box::new(move || {
            let _keep_alive = (&action, &json_action);
            actions
                .iter()
                .all(|action| action.lock().availability_waiter_count() == 0)
        }),
        Box::new(move || {
            let _keep_alive = &publisher;
            publisher_native.waiter_counts() == (0, 0)
        }),
        Box::new(move || {
            let _keep_alive = &json_publisher;
            json_publisher_native.waiter_counts() == (0, 0)
        }),
        Box::new(move || {
            let _keep_alive = &subscription;
            subscription_probe() == 0
        }),
        Box::new(move || graph_probe() == 0),
    ];
    (waits, probes)
}

fn cancel_during_wait(
    driver: &mut impl FutureDriver, guard: &std::sync::Arc<FutureGuard>, waits: Vec<Wait>,
    probes: Vec<Probe>,
) {
    let observed = guard.clone();
    let triggers = guard.triggers.load(Ordering::Acquire);
    let worker = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while !observed.waiting.load(Ordering::Acquire) {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        drop(waits);
    });
    let result = drive_future(
        driver,
        futures::future::pending::<crate::Result<()>>(),
        Duration::from_millis(150),
        FutureExecutionOptions::wake_driven(),
    );
    worker.join().unwrap();
    assert!(matches!(result, Err(Error::RCL_RET_TIMEOUT)));
    assert!(
        guard.triggers.load(Ordering::Acquire) > triggers,
        "foreground observer cancellation did not wake native wait"
    );
    assert!(probes.iter().all(|probe| probe()), "cancellation left queued observers");
}

#[test]
fn foreground_observer_cancellation_wakes_node_and_shared_executor_before_and_after_binding() {
    for shared in [false, true] {
        for late in [false, true] {
            let context = Context::create().unwrap();
            let mut node = Node::create(context.clone(), "foreground_observers", "").unwrap();
            let guard = FutureGuard::new(&context).unwrap();
            if shared {
                let mut executor = Executor::new(context);
                let early = (!late).then(|| observers(&mut node));
                executor.cache_future_guard(guard.clone());
                let id = executor.add_node(node).unwrap();
                let (waits, probes) =
                    early.unwrap_or_else(|| observers(executor.node_mut(id).unwrap()));
                cancel_during_wait(&mut executor, &guard, waits, probes);
            } else {
                let early = (!late).then(|| observers(&mut node));
                node.cache_future_guard(guard.clone());
                let (waits, probes) = early.unwrap_or_else(|| observers(&mut node));
                cancel_during_wait(&mut node, &guard, waits, probes);
            }
        }
    }
}

#[test]
fn returned_node_rebinds_all_observers_to_its_own_cached_guard() {
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "returned_observers", "").unwrap();
    let own_guard = FutureGuard::new(&context).unwrap();
    node.cache_future_guard(own_guard.clone());
    let (waits, probes) = observers(&mut node);
    let mut executor = Executor::new(context.clone());
    let shared_guard = FutureGuard::new(&context).unwrap();
    executor.cache_future_guard(shared_guard.clone());
    let id = executor.add_node(node).unwrap();
    assert!(executor
        .node_mut(id)
        .unwrap()
        .graph_change_waiters
        .native_wake()
        .unwrap()
        .will_wake(&shared_guard.observer_waker()));
    let mut node = executor.remove_node(id).unwrap();
    assert!(node.graph_change_waiters.native_wake().is_none());
    let triggers = shared_guard.triggers.load(Ordering::Acquire);
    cancel_during_wait(&mut node, &own_guard, waits, probes);
    assert_eq!(shared_guard.triggers.load(Ordering::Acquire), triggers);
    assert!(node
        .graph_change_waiters
        .native_wake()
        .unwrap()
        .will_wake(&own_guard.observer_waker()));
}

#[test]
fn every_observer_cancellation_source_notifies_the_bound_guard() {
    for late in [false, true] {
        let context = Context::create().unwrap();
        let mut node = Node::create(context.clone(), "each_observer_wake", "").unwrap();
        let guard = FutureGuard::new(&context).unwrap();
        let early = (!late).then(|| observers(&mut node));
        node.cache_future_guard(guard.clone());
        let (mut waits, _keep_alive) = early.unwrap_or_else(|| observers(&mut node));
        // ACK cancellation uses the same publisher wake even though its native completion
        // query has no wait-set event. Cancel before any native query can complete it.
        let publisher = node
            .create_publisher::<crate::std_msgs::msg::Int32>(
                &format!("/ack_cancel_{}", uuid::Uuid::new_v4().simple()),
                QosProfile::default(),
            )
            .unwrap();
        waits.push(publisher.wait_for_all_acked_async().unwrap().boxed());
        guard.active.store(true, Ordering::Release);
        for (index, wait) in waits.into_iter().enumerate() {
            let before = guard.triggers.load(Ordering::Acquire);
            drop(wait);
            assert!(
                guard.triggers.load(Ordering::Acquire) > before,
                "observer source {index} did not notify its native owner"
            );
        }
        guard.active.store(false, Ordering::Release);
    }
}

#[test]
fn repeated_binding_skips_entity_access_but_changed_owner_and_unbinding_visit_it() {
    use crate::subscribers::{Subscriber_, SubscriptionHandleResult};
    use std::sync::{atomic::AtomicUsize, Arc};
    struct ProbeSubscriber(Arc<AtomicUsize>);
    impl Subscriber_ for ProbeSubscriber {
        fn handle(&self) -> &r2r_rcl::rcl_subscription_t {
            panic!("binding queried native endpoint")
        }
        fn match_state_mut(&mut self) -> Option<&mut crate::subscribers::SubscriptionMatchState> {
            self.0.fetch_add(1, Ordering::Relaxed);
            None
        }
        fn handle_incoming(&mut self) -> crate::Result<SubscriptionHandleResult> {
            panic!("binding dispatched data")
        }
        fn destroy(&mut self, _: &mut r2r_rcl::rcl_node_t) {}
    }
    let context = Context::create().unwrap();
    let mut node = Node::create(context.clone(), "binding_access", "").unwrap();
    let visits = Arc::new(AtomicUsize::new(0));
    node.add_subscriber(Box::new(ProbeSubscriber(visits.clone())));
    visits.store(0, Ordering::Relaxed);
    let first = FutureGuard::new(&context).unwrap();
    node.set_native_observer_wake(Some(first.observer_waker()));
    assert_eq!(visits.load(Ordering::Relaxed), 1);
    for _ in 0..32 {
        node.set_native_observer_wake(Some(first.observer_waker()));
    }
    assert_eq!(visits.load(Ordering::Relaxed), 1);
    let second = FutureGuard::new(&context).unwrap();
    node.set_native_observer_wake(Some(second.observer_waker()));
    node.set_native_observer_wake(None);
    node.set_native_observer_wake(None);
    assert_eq!(visits.load(Ordering::Relaxed), 3);
}
