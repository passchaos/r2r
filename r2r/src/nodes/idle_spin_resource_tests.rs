use super::*;
use crate::allocation_probe;
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual idle spin resource stage; LD_PRELOAD probe required"]
fn benchmark_idle_spin_resource_scaling() {
    let probe = allocation_probe::Probe::load()
        .unwrap()
        .expect("LD_PRELOAD allocation probe required");
    let cases = [
        ("subscriptions", 0usize, 0usize, 0usize, 0usize),
        ("subscriptions", 1, 0, 0, 0),
        ("subscriptions", 8, 0, 0, 0),
        ("subscriptions", 32, 0, 0, 0),
        ("subscriptions", 128, 0, 0, 0),
        ("mixed", 8, 8, 8, 8),
        ("mixed", 32, 32, 32, 32),
    ];
    for (label, subscriptions_count, services_count, clients_count, timers_count) in cases {
        let context = crate::Context::create().unwrap();
        let mut node = Node::create(
            context,
            &format!("idle_spin_resource_{label}_{subscriptions_count}_{services_count}"),
            "",
        )
        .unwrap();
        let mut subscriptions = Vec::new();
        let mut services = Vec::new();
        let mut clients = Vec::new();
        let mut timers = Vec::new();
        for index in 0..subscriptions_count {
            subscriptions.push(
                node.subscribe::<crate::std_msgs::msg::Int32>(
                    &format!(
                        "/idle_spin_sub_{label}_{subscriptions_count}_{services_count}_{index}"
                    ),
                    QosProfile::default(),
                )
                .unwrap(),
            );
        }
        for index in 0..services_count {
            services.push(
                node.create_service::<crate::example_interfaces::srv::AddTwoInts::Service>(
                    &format!(
                        "/idle_spin_srv_{label}_{subscriptions_count}_{services_count}_{index}"
                    ),
                    QosProfile::default(),
                )
                .unwrap(),
            );
        }
        for index in 0..clients_count {
            clients.push(
                node.create_client::<crate::example_interfaces::srv::AddTwoInts::Service>(
                    &format!(
                        "/idle_spin_cli_{label}_{subscriptions_count}_{services_count}_{index}"
                    ),
                    QosProfile::default(),
                )
                .unwrap(),
            );
        }
        for _ in 0..timers_count {
            timers.push(node.create_wall_timer(Duration::from_secs(3600)).unwrap());
        }
        for _ in 0..200 {
            node.try_spin_once(Duration::ZERO).unwrap();
        }
        std::thread::sleep(Duration::from_millis(200));
        let mut stats = allocation_probe::NativeStats::default();
        assert_eq!(unsafe { (probe.begin)() }, 0);
        let begin = Instant::now();
        for _ in 0..10_000 {
            node.try_spin_once(Duration::ZERO).unwrap();
        }
        let elapsed_ns = begin.elapsed().as_nanos();
        assert_eq!(unsafe { (probe.end)(&mut stats) }, 0);
        println!(
            "{}",
            serde_json::json!({
                "case": label,
                "subscriptions": subscriptions_count,
                "services": services_count,
                "clients": clients_count,
                "timers": timers_count,
                "iterations": 10_000,
                "elapsed_ns": elapsed_ns,
                "allocations": stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls,
                "frees": stats.free_calls,
                "requested_bytes": stats.requested_bytes,
                "user_cpu_ns": stats.user_cpu_ns,
                "system_cpu_ns": stats.system_cpu_ns,
                "voluntary_context_switches": stats.voluntary_context_switches,
                "involuntary_context_switches": stats.involuntary_context_switches,
                "process_max_rss_kib": stats.process_max_rss_kib
            })
        );
    }
}

// The variants only hold endpoints alive; their contents are deliberately never read.
#[allow(dead_code)]
enum Entities {
    Subscriptions(Vec<crate::Subscription<crate::std_msgs::msg::Int32>>),
    Services(Vec<crate::Service<crate::example_interfaces::srv::AddTwoInts::Service>>),
    Clients(Vec<crate::Client<crate::example_interfaces::srv::AddTwoInts::Service>>),
    Timers(Vec<crate::Timer>),
    Mixed {
        subscriptions: Vec<crate::Subscription<crate::std_msgs::msg::Int32>>,
        services: Vec<crate::Service<crate::example_interfaces::srv::AddTwoInts::Service>>,
        clients: Vec<crate::Client<crate::example_interfaces::srv::AddTwoInts::Service>>,
        timers: Vec<crate::Timer>,
    },
}

fn create_entities(node: &mut Node, label: &str, count: usize, tag: usize) -> Entities {
    match label {
        "subscriptions" => {
            let mut subscriptions = Vec::new();
            for index in 0..count {
                subscriptions.push(
                    node.subscribe::<crate::std_msgs::msg::Int32>(
                        &format!("/endpoint_build_sub_{tag}_{count}_{index}"),
                        QosProfile::default(),
                    )
                    .unwrap(),
                );
            }
            Entities::Subscriptions(subscriptions)
        }
        "services" => {
            let mut services = Vec::new();
            for index in 0..count {
                services.push(
                    node.create_service::<crate::example_interfaces::srv::AddTwoInts::Service>(
                        &format!("/endpoint_build_srv_{tag}_{count}_{index}"),
                        QosProfile::default(),
                    )
                    .unwrap(),
                );
            }
            Entities::Services(services)
        }
        "clients" => {
            let mut clients = Vec::new();
            for index in 0..count {
                clients.push(
                    node.create_client::<crate::example_interfaces::srv::AddTwoInts::Service>(
                        &format!("/endpoint_build_cli_{tag}_{count}_{index}"),
                        QosProfile::default(),
                    )
                    .unwrap(),
                );
            }
            Entities::Clients(clients)
        }
        "timers" => {
            let mut timers = Vec::new();
            for _ in 0..count {
                timers.push(node.create_wall_timer(Duration::from_secs(3600)).unwrap());
            }
            Entities::Timers(timers)
        }
        "mixed" => {
            let mut subscriptions = Vec::new();
            let mut services = Vec::new();
            let mut clients = Vec::new();
            let mut timers = Vec::new();
            for index in 0..count {
                subscriptions.push(
                    node.subscribe::<crate::std_msgs::msg::Int32>(
                        &format!("/endpoint_build_mix_sub_{tag}_{count}_{index}"),
                        QosProfile::default(),
                    )
                    .unwrap(),
                );
                services.push(
                    node.create_service::<crate::example_interfaces::srv::AddTwoInts::Service>(
                        &format!("/endpoint_build_mix_srv_{tag}_{count}_{index}"),
                        QosProfile::default(),
                    )
                    .unwrap(),
                );
                clients.push(
                    node.create_client::<crate::example_interfaces::srv::AddTwoInts::Service>(
                        &format!("/endpoint_build_mix_cli_{tag}_{count}_{index}"),
                        QosProfile::default(),
                    )
                    .unwrap(),
                );
                timers.push(node.create_wall_timer(Duration::from_secs(3600)).unwrap());
            }
            Entities::Mixed {
                subscriptions,
                services,
                clients,
                timers,
            }
        }
        other => panic!("unknown entity case {other}"),
    }
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "manual endpoint construction resource stage; LD_PRELOAD probe required"]
fn benchmark_endpoint_construction_resource_scaling() {
    let probe = allocation_probe::Probe::load()
        .unwrap()
        .expect("LD_PRELOAD allocation probe required");
    let cases = [
        ("subscriptions", 1usize),
        ("subscriptions", 8),
        ("subscriptions", 32),
        ("subscriptions", 128),
        ("services", 1),
        ("services", 8),
        ("services", 32),
        ("clients", 1),
        ("clients", 8),
        ("clients", 32),
        ("timers", 1),
        ("timers", 8),
        ("timers", 32),
        ("mixed", 8),
        ("mixed", 32),
    ];
    for (tag, (label, count)) in cases.into_iter().enumerate() {
        let context = crate::Context::create().unwrap();
        let mut node =
            Node::create(context, &format!("endpoint_build_{label}_{count}_{tag}"), "").unwrap();
        let mut create_stats = allocation_probe::NativeStats::default();
        assert_eq!(unsafe { (probe.begin)() }, 0);
        let entities = create_entities(&mut node, label, count, tag);
        assert_eq!(unsafe { (probe.end)(&mut create_stats) }, 0);
        let mut destroy_stats = allocation_probe::NativeStats::default();
        assert_eq!(unsafe { (probe.begin)() }, 0);
        drop(entities);
        for _ in 0..200 {
            node.try_spin_once(Duration::ZERO).unwrap();
        }
        assert_eq!(unsafe { (probe.end)(&mut destroy_stats) }, 0);
        let allocations = |stats: &allocation_probe::NativeStats| {
            stats.malloc_calls + stats.calloc_calls + stats.realloc_calls + stats.aligned_calls
        };
        for (phase, stats) in [("create", &create_stats), ("destroy", &destroy_stats)] {
            println!(
                "{}",
                serde_json::json!({
                    "case": label,
                    "entities": count,
                    "phase": phase,
                    "allocations": allocations(stats),
                    "frees": stats.free_calls,
                    "requested_bytes": stats.requested_bytes,
                    "user_cpu_ns": stats.user_cpu_ns,
                    "system_cpu_ns": stats.system_cpu_ns,
                    "process_max_rss_kib": stats.process_max_rss_kib
                })
            );
        }
    }
}
