use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use r2r::{
    example_interfaces::{action::Fibonacci, srv::AddTwoInts},
    std_msgs::msg::UInt64,
    Context, Node, NodeOptions, QosProfile,
};
use std::time::Duration;

const ENTITY_COUNTS: &[usize] = &[0, 1, 8, 32, 128];
const MIXED_COUNTS: &[usize] = &[0, 1, 8, 32, 64];
const WAITS_PER_ENTITY: usize = 16;

fn node(name: &str) -> Node {
    Node::create_with_options(
        Context::create().expect("could not create benchmark context"),
        name,
        "",
        NodeOptions::default()
            .enable_rosout(false)
            .start_type_description_service(false),
    )
    .expect("could not create benchmark node")
}

fn configure(group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>) {
    group.sample_size(30);
    group.warm_up_time(Duration::from_secs(1));
    group.measurement_time(Duration::from_secs(2));
}

fn idle_action_servers(c: &mut Criterion) {
    let mut group = c.benchmark_group("spin_idle_action_servers");
    configure(&mut group);
    for &count in ENTITY_COUNTS {
        let mut node = node(&format!("idle_action_server_benchmark_{count}"));
        let _servers = (0..count)
            .map(|index| {
                node.create_action_server::<Fibonacci::Action>(&format!(
                    "/idle_action_server_benchmark_{count}_{index}"
                ))
                .expect("could not create benchmark action server")
            })
            .collect::<Vec<_>>();
        node.try_spin_once(Duration::ZERO)
            .expect("could not warm up benchmark node");
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                node.try_spin_once(black_box(Duration::ZERO))
                    .expect("could not spin benchmark node")
            })
        });
    }
    group.finish();
}

fn idle_action_clients(c: &mut Criterion) {
    let mut group = c.benchmark_group("spin_idle_action_clients");
    configure(&mut group);
    for &count in ENTITY_COUNTS {
        let mut node = node(&format!("idle_action_client_benchmark_{count}"));
        let _clients = (0..count)
            .map(|index| {
                node.create_action_client::<Fibonacci::Action>(&format!(
                    "/idle_action_client_benchmark_{count}_{index}"
                ))
                .expect("could not create benchmark action client")
            })
            .collect::<Vec<_>>();
        node.try_spin_once(Duration::ZERO)
            .expect("could not warm up benchmark node");
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                node.try_spin_once(black_box(Duration::ZERO))
                    .expect("could not spin benchmark node")
            })
        });
    }
    group.finish();
}

fn mixed_idle_wait_set(c: &mut Criterion) {
    let mut group = c.benchmark_group("spin_idle_mixed_wait_set");
    configure(&mut group);
    for &count in MIXED_COUNTS {
        let mut node = node(&format!("mixed_wait_set_benchmark_{count}"));
        let qos = QosProfile::default();
        let _subscriptions = (0..count)
            .map(|index| {
                node.subscribe::<UInt64>(
                    &format!("/mixed_wait_set_topic_{count}_{index}"),
                    qos.clone(),
                )
                .expect("could not create benchmark subscription")
            })
            .collect::<Vec<_>>();
        let _services = (0..count)
            .map(|index| {
                node.create_service::<AddTwoInts::Service>(
                    &format!("/mixed_wait_set_service_{count}_{index}"),
                    qos.clone(),
                )
                .expect("could not create benchmark service")
            })
            .collect::<Vec<_>>();
        let _clients = (0..count)
            .map(|index| {
                node.create_client::<AddTwoInts::Service>(
                    &format!("/mixed_wait_set_service_{count}_{index}"),
                    qos.clone(),
                )
                .expect("could not create benchmark client")
            })
            .collect::<Vec<_>>();
        let _action_servers = (0..count)
            .map(|index| {
                node.create_action_server::<Fibonacci::Action>(&format!(
                    "/mixed_wait_set_action_{count}_{index}"
                ))
                .expect("could not create benchmark action server")
            })
            .collect::<Vec<_>>();
        let _action_clients = (0..count)
            .map(|index| {
                node.create_action_client::<Fibonacci::Action>(&format!(
                    "/mixed_wait_set_action_{count}_{index}"
                ))
                .expect("could not create benchmark action client")
            })
            .collect::<Vec<_>>();
        let _timers = (0..count)
            .map(|_| {
                node.create_wall_timer(Duration::from_secs(3600))
                    .expect("could not create benchmark timer")
            })
            .collect::<Vec<_>>();
        node.try_spin_once(Duration::ZERO)
            .expect("could not warm up mixed benchmark node");
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                node.try_spin_once(black_box(Duration::ZERO))
                    .expect("could not spin mixed benchmark node")
            })
        });
    }
    group.finish();
}

fn service_availability_notifications(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_cancel_service_availability_waits");
    configure(&mut group);
    for &count in ENTITY_COUNTS {
        let mut node = node(&format!("service_availability_benchmark_{count}"));
        let clients = (0..count)
            .map(|index| {
                node.create_client::<AddTwoInts::Service>(
                    &format!("/service_availability_benchmark_{count}_{index}"),
                    QosProfile::default(),
                )
                .expect("could not create benchmark service client")
            })
            .collect::<Vec<_>>();
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                let waits = clients
                    .iter()
                    .flat_map(|client| {
                        (0..WAITS_PER_ENTITY).map(|_| {
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
    group.finish();
}

fn action_availability_notifications(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_cancel_action_availability_waits");
    configure(&mut group);
    for &count in ENTITY_COUNTS {
        let mut node = node(&format!("action_availability_benchmark_{count}"));
        let clients = (0..count)
            .map(|index| {
                node.create_action_client::<Fibonacci::Action>(&format!(
                    "/action_availability_benchmark_{count}_{index}"
                ))
                .expect("could not create benchmark action client")
            })
            .collect::<Vec<_>>();
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                let waits = clients
                    .iter()
                    .flat_map(|client| {
                        (0..WAITS_PER_ENTITY).map(|_| {
                            Node::is_available(client).expect("could not register availability")
                        })
                    })
                    .collect::<Vec<_>>();
                drop(waits);
                node.try_spin_once(black_box(Duration::ZERO))
                    .expect("could not clean canceled action availability waits")
            })
        });
    }
    group.finish();
}

fn publisher_wait_notifications(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_cancel_publisher_waits");
    configure(&mut group);
    for &count in ENTITY_COUNTS {
        let mut node = node(&format!("publisher_wait_benchmark_{count}"));
        let publishers = (0..count)
            .map(|index| {
                node.create_publisher::<UInt64>(
                    &format!("/publisher_wait_benchmark_{count}_{index}"),
                    QosProfile::default(),
                )
                .expect("could not create benchmark publisher")
            })
            .collect::<Vec<_>>();
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                let waits = publishers
                    .iter()
                    .flat_map(|publisher| {
                        (0..WAITS_PER_ENTITY).map(|_| {
                            publisher
                                .wait_for_subscribers()
                                .expect("could not register subscriber wait")
                        })
                    })
                    .collect::<Vec<_>>();
                drop(waits);
                node.try_spin_once(black_box(Duration::ZERO))
                    .expect("could not clean canceled publisher waits")
            })
        });
    }
    group.finish();
}

fn subscription_wait_notifications(c: &mut Criterion) {
    let mut group = c.benchmark_group("schedule_cancel_subscription_waits");
    configure(&mut group);
    for &count in ENTITY_COUNTS {
        let mut node = node(&format!("subscription_wait_benchmark_{count}"));
        let subscriptions = (0..count)
            .map(|index| {
                node.subscribe::<UInt64>(
                    &format!("/subscription_wait_benchmark_{count}_{index}"),
                    QosProfile::default(),
                )
                .expect("could not create benchmark subscription")
            })
            .collect::<Vec<_>>();
        group.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                let mut waits = Vec::with_capacity(subscriptions.len() * WAITS_PER_ENTITY);
                for subscription in &subscriptions {
                    for _ in 0..WAITS_PER_ENTITY {
                        waits.push(
                            subscription
                                .wait_for_publishers(&mut node)
                                .expect("could not register publisher wait"),
                        );
                    }
                }
                drop(waits);
                node.try_spin_once(black_box(Duration::ZERO))
                    .expect("could not clean canceled subscription waits")
            })
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    idle_action_servers,
    idle_action_clients,
    mixed_idle_wait_set,
    service_availability_notifications,
    action_availability_notifications,
    publisher_wait_notifications,
    subscription_wait_notifications,
);
criterion_main!(benches);
