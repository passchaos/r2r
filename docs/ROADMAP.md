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

This audit is based on the source tree and local checks through 2026-10-28. It is not a claim of C++ parity or performance leadership.

| Area | Existing foundation | Remaining work |
| --- | --- | --- |
| Async application APIs | Futures and streams, service availability/timeouts, action APIs and typed parameter clients. [Native future execution](FUTURE_EXECUTION.md) bridges external Rust wakes into the ROS wait, with reusable storage and shutdown handling. [Multi-node execution](MULTI_NODE_EXECUTION.md) owns nodes and shares one native wait with ordinary/action readiness spans. Matching uses graph notifications; ACK observers use bounded native queries. Runnable `service_pipeline`, `future_execution`, `pubsub_workflow` and `multi_node_pipeline` examples compose complete workflows. | Compare broader application workflows with rclcpp; add runtime-reactor integration, concurrent callback scheduling, scaling and more complete cancellation/shutdown examples. |
| Messages and native storage | Typed, untyped, raw and loaned message paths; fallible native copies; capacity reuse for primitive and nested-message sequences and string fields. `MessageSerializer<T>` reuses native fields and exposes borrowed wire bytes, with full codec and lifecycle oracles. | Allocation and copy accounting across complete transport paths; extend reuse to string/wstring sequences and investigate middleware output-buffer resizing; keep allocation-failure and capacity invariants explicit. |
| Service events | Iron and newer service/action event bindings; empty and populated native/CDR/JSON roundtrips; bounded request/response rejection; typed/untyped client and server configuration. Four integration tests pass on local Jazzy with both CycloneDDS and Fast DDS, covering mode transitions, simulated timestamps, native identities, ownership, and retained QoS history. | Extend the local Jazzy RMW evidence to the other supported modern distributions and external rclcpp endpoints; maintain the documented middleware-dependent event identity encoding. |
| Graph, QoS, parameters, clocks | Graph queries, endpoint identities and QoS, parameter services/events, managed ROS clocks and jump callbacks. | Audit remaining application-facing RCL and rclcpp features and test real middleware behavior per distribution. |
| Distribution compatibility | Build-time checks and CI cover Humble, Iron, Jazzy, Kilted, Lyrical and Rolling. Library tests and Clippy pass with native Jazzy and containerized Humble/Kilted using Rust 1.85. | Complete runtime and RMW checks for each supported distribution and keep version-specific API boundaries explicit; Iron, Lyrical and Rolling remain CI matrix entries whose current local runtime evidence is not available. |
| Documentation builds | `cargo check --workspace --features doc-only` works with saved bindings. | Saved bindings omit some example packages and new event types; refresh them and make documentation/example test coverage consistent. `--all-targets --features doc-only` currently fails on existing example-package dependencies. |
| Performance evidence | Criterion/message-stage evidence, independent-process pub/sub and service matrices, the [observed action lifecycle/cancel A/B](benchmarks/action-lifecycle-2026-10-28.md), and [native status decoding A/B](benchmarks/action-status-decode-2026-10-28.md). Both language directions and raw RTT/stage samples are covered under two RMWs. | Complete pipelined/fanout pub/sub, dynamic payloads, multi-goal actions, allocation counters and scaling evidence. Full action success is close to rclcpp under Fast DDS; cancellation and CycloneDDS still have latency gaps. |
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

[Reusable serialization evidence](benchmarks/reusable-serialization-2026-10-28.md) measures the
borrowed-byte API alongside existing owned-byte paths under both RMWs. Ownership choice and
middleware output-buffer resizing remain explicit; reduced codec-stage time is not an
end-to-end transport claim.

The [native service matrix](benchmarks/service-roundtrip-2026-10-28.md) covers both language
directions, one and 16 in-flight requests, complete sum/sequence oracles and raw RTT samples under
two RMWs. Its fixed-size results leave dynamic service payloads, cancellation under load and
multithread/action protocols as explicit remaining scope.

The [action matrix](benchmarks/action-lifecycle-2026-10-28.md) confirms acceptance, observed feedback
and successful/canceled results with explicit readiness handshakes. [Local cancellation progress](ACTION_CANCELLATION.md)
now returns promptly after completed decisions and preserves waits during unresolved/backpressured
work. [Direct native status decoding](benchmarks/action-status-decode-2026-10-28.md) and fewer
redundant client UUID lookups reduce complete action p50 by 3.4–4.8% under CycloneDDS and 22.4%
under Fast DDS versus an interleaved `441bcd4` baseline. Fast DDS success is close to same-run
C++; cancellation and CycloneDDS remain open gaps. Retained native storage, concurrent goals,
allocation accounting and scaling need further evidence.

The [retained-status observer-filtering experiment](benchmarks/action-interest-filter-2026-10-28/README.md)
was rejected and fully removed after its final Fast DDS short-cohort cancellation p99 exceeded
the 5% regression limit. Large-cohort gains are retained as experimental evidence rather than
promoted runtime behavior. Next routing work needs a stable short-cohort measurement design
and workload-level profiling; production retains the native decoder at `d45e261`.

[Action allocation/resource evidence](benchmarks/action-allocation-2026-10-28/README.md) now
records process-wide allocation entry points, requested bytes, CPU/context switches and
application polling/wakes under both RMWs. Fresh endpoint cohorts retain full protocol oracles
and native settings. A native status-iterator candidate was removed after a short-tail failure;
same-binary A/A also fails the 5% limit, so improved tail precision remains a prerequisite.

The [pub/sub future workflow](FUTURE_EXECUTION.md) closes two notification-driven execution
gaps: graph changes now wake publisher/subscription matching, and pending ACK queries progress
even without a native event. Only live ACK observers require the 10 ms query interval, with
no extra timer/thread and cleanup after completion or cancellation. Eight focused cases pass
under Jazzy/CycloneDDS, Jazzy/Fast DDS, Humble and Kilted, and the complete match/timer/payload/ACK
example runs in all four environments. This is a correctness and workflow improvement; the
existing external-future A/B does not measure these ROS endpoint paths.

The [borrowed native receive/forwarding experiment](benchmarks/native-views-2026-10-28-rejected/README.md)
is removed after a failed CycloneDDS tail gate, despite 7.6–8.0% large-payload p50 gains.
The comparison now includes 65536-value payloads and explicit receive-path identities. A
same-binary control passes the one-way regression test but fails symmetric calibration for
zero-payload p99/rate. Apparent gains are now checked as calibration drift too. Stable small
transport tails and complete allocation/buffer-lifetime evidence remain prerequisites.

The [multi-node service-application A/B](benchmarks/multi-node-executor-2026-10-28/README.md)
passes the preselected 5% latency, throughput and CPU/request gates under both Jazzy RMWs.
One shared native wait removes historical nonblocking per-node polling. Eight-node/window-16
Rust throughput reaches about 103k requests/s under CycloneDDS and 69k under Fast DDS, versus
97k/58k for the same-run rclcpp application. Eleven shared-wait cases and the complete pipeline
example pass in native Humble/Kilted. Other hardware, payloads, threaded callbacks, reactors,
fanout and cross-distribution performance remain open.

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
