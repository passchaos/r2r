# r2r development roadmap

The goal is to make Rust the strongest practical language in the ROS ecosystem: r2r should exceed the C++ experience in ease of use, capability, and performance. All three dimensions need evidence; a successful build or a faster microbenchmark alone does not establish that goal.

## Acceptance criteria

| Dimension | Required outcome | Evidence |
| --- | --- | --- |
| Ease of use | Common nodes, subscriptions, services, actions, parameters, shutdown, and diagnostics have safe, composable APIs and runnable examples. Ownership, cancellation, buffering, and failure behavior are explicit. | Representative applications built with r2r and rclcpp; compare setup, application code, error handling, and runtime integration. |
| Capability | Application-level ROS features work through Rust across the supported distributions and RMW implementations, including introspection, graph queries, QoS events, simulated time, and middleware interoperability. | A feature and distribution matrix with integration tests against native ROS endpoints; unsupported cells remain visible. |
| Performance | Representative Rust applications exceed equivalent C++ applications in measured throughput, latency, resource use, or scaling, without sacrificing semantics. | Same ROS/RMW versions, inputs, QoS, hardware, CPU affinity, buffering, and build settings; independent binaries in interleaved A/B runs. Report latency distributions, throughput, allocation counts, and memory use. |

## Current evidence and gaps

This audit is based on the source tree and local checks on 2026-10-27. It is not a claim of C++ parity or performance leadership.

| Area | Existing foundation | Remaining work |
| --- | --- | --- |
| Async application APIs | Futures and streams, service availability and timeouts, action goal/result/cancel APIs, typed parameter clients. | Compare complete application workflows with rclcpp; reduce repetitive executor/spin setup and improve cancellation and shutdown examples. |
| Messages and native storage | Typed, untyped, raw and loaned message paths; fallible native copies; serialization tests. | Allocation and copy accounting across paths; reuse nested message sequence capacity; validate error and allocation-failure behavior before optimizing. |
| Service events | Iron and newer service/action event bindings; empty and populated native/CDR/JSON roundtrips; bounded request/response rejection. | Public client/server configuration, metadata/content/off transitions, event stream interoperability, simulated timestamps, and a runnable example. |
| Graph, QoS, parameters, clocks | Graph queries, endpoint identities and QoS, parameter services/events, managed ROS clocks and jump callbacks. | Audit remaining application-facing RCL and rclcpp features and test real middleware behavior per distribution. |
| Distribution compatibility | CI declares Foxy, Galactic, Humble, Iron, Jazzy and Lyrical; local Jazzy checks and a Foxy container are available. | Foxy currently fails library compilation with 32 errors also present at HEAD `42cca4a`, including unavailable QoS APIs, descriptor fields, and clock/timer APIs. Restore or explicitly revise the supported matrix; run other declared distributions. |
| Documentation builds | `cargo check --workspace --features doc-only` works with saved bindings. | Saved bindings omit some example packages and new event types; refresh them and make documentation/example test coverage consistent. `--all-targets --features doc-only` currently fails on existing example-package dependencies. |
| Performance evidence | Criterion benches for deserialization, message metadata, idle action endpoints and availability scheduling. | Add an equivalent rclcpp baseline and end-to-end pub/sub, service and action workloads. Existing Rust-only measurements cannot establish superiority over C++. |
| Module structure | Small modules exist for availability, wait sets, time sources and parameter clients. | Decompose the large node module by stable responsibility; keep unit tests near implementation and integration tests at module or crate boundaries. |

## Execution order

1. Close the service-event vertical slice: correct bindings, event publication controls, native endpoint integration, and an executable example.
2. Restore the declared distribution boundaries and update saved documentation bindings, with independent gates for each version-dependent API.
3. Build representative Rust/C++ application pairs and a reproducible performance matrix. Use these workloads to select allocation, copying, wakeup and scaling optimizations.
4. Improve application ergonomics and module boundaries in the affected slice; validate it independently and checkpoint completed changes.

Performance-sensitive changes follow: correctness and counterexamples/oracles, local benchmarks, pinned-CPU interleaved A/B, then the affected compatibility matrix and full verification appropriate to the change. A failed early gate stops promotion. Temporary implementations must expose a replaceable boundary and preserve the path to higher throughput, concurrency, locality and incremental processing.

## Reproducible local gates

With the ROS environment sourced and the repository-pinned Rust toolchain:

```sh
cargo test -p r2r_common tests::service_events_follow_ros_distro_support -- --exact
cargo test -p r2r_msg_gen --lib message_path
cargo test -p r2r --lib service_events
cargo test -p r2r --lib msg_types::
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo test --workspace
```

Documentation library and pure generator checks:

```sh
cargo check --workspace --features doc-only
cargo test -p r2r_msg_gen --features doc-only --lib message_path
```

Check the executed test count for every filtered gate. Keep different feature/distribution builds in separate target directories when they run concurrently. Serialize benchmark workloads with competing compilation and testing.
