# r2r development roadmap

The goal is to make Rust the strongest practical language in the ROS ecosystem: r2r should exceed the C++ experience in ease of use, capability, and performance. All three dimensions need evidence; a successful build or a faster microbenchmark alone does not establish that goal.

Supported ROS 2 distributions start at **Humble**: Humble, Iron, Jazzy, Kilted, Lyrical and Rolling. Foxy and Galactic are outside the supported scope. Older-version branches and CI jobs are removed so development and verification target the supported matrix.

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
| Messages and native storage | Typed, untyped, raw and loaned message paths; fallible native copies; capacity reuse for primitive and nested-message sequences and string fields, with empty/shrinking/growing and native oracle tests. | Allocation and copy accounting across complete transport paths; extend reuse to string/wstring sequences and codec scratch storage; keep allocation-failure and capacity invariants explicit. |
| Service events | Iron and newer service/action event bindings; empty and populated native/CDR/JSON roundtrips; bounded request/response rejection; typed/untyped client and server configuration. Four integration tests pass on local Jazzy with both CycloneDDS and Fast DDS, covering mode transitions, simulated timestamps, native identities, ownership, and retained QoS history. | Extend the local Jazzy RMW evidence to the other supported modern distributions and external rclcpp endpoints; maintain the documented middleware-dependent event identity encoding. |
| Graph, QoS, parameters, clocks | Graph queries, endpoint identities and QoS, parameter services/events, managed ROS clocks and jump callbacks. | Audit remaining application-facing RCL and rclcpp features and test real middleware behavior per distribution. |
| Distribution compatibility | Build-time checks and CI cover Humble, Iron, Jazzy, Kilted, Lyrical and Rolling. Library tests and Clippy pass with native Jazzy and containerized Humble/Kilted using Rust 1.85. | Complete runtime and RMW checks for each supported distribution and keep version-specific API boundaries explicit; Iron, Lyrical and Rolling remain CI matrix entries whose current local runtime evidence is not available. |
| Documentation builds | `cargo check --workspace --features doc-only` works with saved bindings. | Saved bindings omit some example packages and new event types; refresh them and make documentation/example test coverage consistent. `--all-targets --features doc-only` currently fails on existing example-package dependencies. |
| Performance evidence | Criterion benches, an independent Rust/rclcpp message-stage harness and [native-message reuse evidence](benchmarks/native-message-reuse-2026-10-27.md). The [independent-process pub/sub matrix](benchmarks/pubsub-roundtrip-2026-10-27.md) covers both language directions with raw individual RTT samples under two RMWs. | Add pipelined/fanout pub/sub, service and action workloads, allocation counters and scaling measurements. Current stage and stop-and-wait comparisons do not establish overall superiority over C++. |
| Module structure | Small modules exist for availability, wait sets, time sources and parameter clients. | Decompose the large node module by stable responsibility; keep unit tests near implementation and integration tests at module or crate boundaries. |

## Execution order

1. Finish the service-event distribution and external endpoint matrix after the local Jazzy vertical slice: correct bindings, event publication controls, native endpoint integration, and an executable example.
2. Validate the Humble-and-newer distribution boundaries and update saved documentation bindings, with independent gates for each version-dependent API.
3. Build representative Rust/C++ application pairs and a reproducible performance matrix. Use these workloads to select allocation, copying, wakeup and scaling optimizations.
4. Improve application ergonomics and module boundaries in the affected slice; validate it independently and checkpoint completed changes.

Performance-sensitive changes follow: correctness and counterexamples/oracles, local benchmarks, pinned-CPU interleaved A/B, then the affected compatibility matrix and full verification appropriate to the change. A failed early gate stops promotion. Temporary implementations must expose a replaceable boundary and preserve the path to higher throughput, concurrency, locality and incremental processing.

Optimized runtime behavior is checked with the [release validation gates](RELEASE_VALIDATION.md).
State registration and cleanup must execute outside debug assertions; the complete Jazzy library
suite runs with assertions disabled in CI as well as under the supported-distribution test matrix.

The [typed receive-container experiment](benchmarks/typed-receive-buffer-2026-10-27-rejected.md)
was rejected by the 5% tail-latency regression gate and removed. Interleaved transport comparisons
now retain baseline/baseline and both baseline/C++ directions alongside the candidate matrix.
Reports explicitly record the gate outcome; integrity validation cannot substitute for a passing
performance gate. Next receive work should remove payload copies while respecting each RMW's
native initialization contract.

## Reproducible local gates

With the ROS environment sourced and the repository-pinned Rust toolchain:

```sh
cargo test -p r2r_common tests::service_events_follow_ros_distro_support -- --exact
cargo test -p r2r_msg_gen --lib message_path
cargo test -p r2r --lib service_events
cargo test -p r2r --lib service_introspection::tests -- --test-threads=1
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

The local shell currently inherits a CycloneDDS configuration URI pointing at an unavailable
`/nav_env/conf/cyclonedds.xml`. Local runtime checks clear that URI only for their test process:

```sh
env -u CYCLONEDDS_URI cargo test -p r2r --lib service_introspection::tests -- --test-threads=1
env -u CYCLONEDDS_URI RMW_IMPLEMENTATION=rmw_fastrtps_cpp cargo test -p r2r --lib service_introspection::tests -- --test-threads=1
env -u CYCLONEDDS_URI cargo run -p r2r --example service_introspection
```

Service event tests compare client-side GIDs to the native client identity and server-side GIDs
to the native request writer identity. CycloneDDS can encode these differently. Sequence numbers
and identities are checked against their corresponding native sources. Fast DDS graph discovery
does not expose publisher history depth; a late-subscriber test verifies the actual retained event
count with `keep_last(1)` and transient-local durability instead.

Humble lacks `rmw_get_gid_for_client`; typed and untyped `Client::get_gid` report
`RCL_RET_UNSUPPORTED`, while stale handles still report `RCL_RET_CLIENT_INVALID`. Humble and
Iron action names are expanded and remapped before native endpoint creation so relative names
retain the node namespace and invalid names retain the public action-specific error. Kilted
and newer action status constants use the renamed RCL spellings.

Kilted/Fast DDS can return an empty address for a subscription network-flow endpoint, also
observed through an independent native C query. `NetworkFlowEndpoint` preserves that text;
`ip_address()` returns `None` for an unspecified address and rejects malformed nonempty IP text.
