use criterion::{black_box, criterion_group, criterion_main, Criterion};
use r2r::{
    example_interfaces::{action::Fibonacci, srv::AddTwoInts},
    Context, Node, QosProfile,
};
use std::time::Duration;

const ACTION_ENDPOINT_COUNT: usize = 128;

fn idle_action_servers(c: &mut Criterion) {
    let context = Context::create().expect("could not create benchmark context");
    let mut node = Node::create(context, "idle_action_server_benchmark", "")
        .expect("could not create benchmark node");
    let _servers = (0..ACTION_ENDPOINT_COUNT)
        .map(|index| {
            node.create_action_server::<Fibonacci::Action>(&format!(
                "/idle_action_server_benchmark_{index}"
            ))
            .expect("could not create benchmark action server")
        })
        .collect::<Vec<_>>();

    node.try_spin_once(Duration::ZERO)
        .expect("could not warm up benchmark node");

    c.bench_function("spin_idle_128_action_servers", |b| {
        b.iter(|| {
            node.try_spin_once(black_box(Duration::ZERO))
                .expect("could not spin benchmark node")
        })
    });
}

fn idle_action_clients(c: &mut Criterion) {
    let context = Context::create().expect("could not create benchmark context");
    let mut node = Node::create(context, "idle_action_client_benchmark", "")
        .expect("could not create benchmark node");
    let _clients = (0..ACTION_ENDPOINT_COUNT)
        .map(|index| {
            node.create_action_client::<Fibonacci::Action>(&format!(
                "/idle_action_client_benchmark_{index}"
            ))
            .expect("could not create benchmark action client")
        })
        .collect::<Vec<_>>();

    node.try_spin_once(Duration::ZERO)
        .expect("could not warm up benchmark node");

    c.bench_function("spin_idle_128_action_clients", |b| {
        b.iter(|| {
            node.try_spin_once(black_box(Duration::ZERO))
                .expect("could not spin benchmark node")
        })
    });
}

fn service_availability_notifications(c: &mut Criterion) {
    let context = Context::create().expect("could not create benchmark context");
    let mut node = Node::create(context, "service_availability_benchmark", "")
        .expect("could not create benchmark node");
    let clients = (0..ACTION_ENDPOINT_COUNT)
        .map(|index| {
            node.create_client::<AddTwoInts::Service>(
                &format!("/service_availability_benchmark_{index}"),
                QosProfile::default(),
            )
            .expect("could not create benchmark service client")
        })
        .collect::<Vec<_>>();

    c.bench_function("schedule_and_cancel_128x16_service_availability_waits", |b| {
        b.iter(|| {
            let waits = clients
                .iter()
                .flat_map(|client| {
                    (0..16).map(|_| {
                        Node::is_available(client).expect("could not register availability")
                    })
                })
                .collect::<Vec<_>>();
            drop(waits);
            node.try_spin_once(black_box(Duration::ZERO))
                .expect("could not clean canceled availability waits")
        })
    });
}

fn action_availability_notifications(c: &mut Criterion) {
    let context = Context::create().expect("could not create benchmark context");
    let mut node = Node::create(context, "action_availability_benchmark", "")
        .expect("could not create benchmark node");
    let clients = (0..ACTION_ENDPOINT_COUNT)
        .map(|index| {
            node.create_action_client::<Fibonacci::Action>(&format!(
                "/action_availability_benchmark_{index}"
            ))
            .expect("could not create benchmark action client")
        })
        .collect::<Vec<_>>();

    c.bench_function("schedule_and_cancel_128x16_action_availability_waits", |b| {
        b.iter(|| {
            let waits = clients
                .iter()
                .flat_map(|client| {
                    (0..16).map(|_| {
                        Node::is_available(client).expect("could not register availability")
                    })
                })
                .collect::<Vec<_>>();
            drop(waits);
            node.try_spin_once(black_box(Duration::ZERO))
                .expect("could not clean canceled availability waits")
        })
    });
}

criterion_group!(
    benches,
    idle_action_servers,
    idle_action_clients,
    service_availability_notifications,
    action_availability_notifications
);
criterion_main!(benches);
