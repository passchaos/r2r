use criterion::{black_box, criterion_group, criterion_main, Criterion};
use r2r::{example_interfaces::action::Fibonacci, Context, Node};
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

criterion_group!(benches, idle_action_servers, idle_action_clients);
criterion_main!(benches);
