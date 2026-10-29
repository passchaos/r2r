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

This audit is based on the source tree and local checks through 2026-10-29. It is not a claim of C++ parity or performance leadership.

| Area | Existing foundation | Remaining work |
| --- | --- | --- |
| Async application APIs | Futures/streams, service availability/timeouts, action APIs and typed parameters. [Native future execution](FUTURE_EXECUTION.md) and [multi-node execution](MULTI_NODE_EXECUTION.md) share native waits and weak cancellation. [Owned background tasks](BACKGROUND_EXECUTION.md) compose runtime I/O and ROS with late-observer wakes. [Background cleanup](BACKGROUND_CLEANUP.md) wakes targeted endpoint destruction, last-operation cleanup and delayed action-cancel decisions/backpressure. The current-thread `tokio_executor` example verifies TCP, services, pub/sub, ACKs and node reuse. | Concurrent callback/native scheduling, calibrated background tails/resource use and broader application comparisons. Native reactor integration without a separate wait thread, large-load cancellation and scaling remain open. |
| Messages and native storage | Typed, untyped, raw and loaned message paths; fallible native copies; capacity reuse for primitive and nested-message sequences and string fields. `MessageSerializer<T>` reuses native fields and exposes borrowed wire bytes, with full codec and lifecycle oracles. | Allocation and copy accounting across complete transport paths; extend reuse to string/wstring sequences and investigate middleware output-buffer resizing; keep allocation-failure and capacity invariants explicit. |
| Service events | Iron and newer service/action event bindings; empty and populated native/CDR/JSON roundtrips; bounded request/response rejection; typed/untyped client and server configuration. [Service introspection](SERVICE_INTROSPECTION.md) has four native integration cases and an independent typed/untyped Rust/rclcpp matrix under Jazzy/Kilted with CycloneDDS/Fast DDS: mode transitions, simulated timestamps, exact native identities and complete events. Native ownership/late-subscriber history cases also pass in all four environments. | Run the newly required Iron/Lyrical/Rolling CI matrix and retain live results; extend payload/QoS and diagnostic application coverage while preserving middleware-dependent identity encoding. |
| Graph, QoS, parameters, clocks | Graph queries, endpoint identities and QoS, parameter services/events, managed ROS clocks and jump callbacks. | Audit remaining application-facing RCL and rclcpp features and test real middleware behavior per distribution. |
| Managed node lifecycle | Native lifecycle state machine, standard services and transition events. [Lifecycle publication](LIFECYCLE_PUBLICATION.md) gates all typed/native/loaned publication. [Managed ROS/wall timers](LIFECYCLE_TIMERS.md) pause natively outside active, preserve manual cancellation and discard stale queued ticks across reactivation. Native-service examples run through the shared executor; APIs appear in the Jazzy documentation snapshot. | Broader lifecycle application comparisons, concurrent transition/callback scheduling and the complete distribution/endpoint matrix. |
| Distribution compatibility | Build-time checks and CI cover Humble, Iron, Jazzy, Kilted, Lyrical and Rolling. Library tests and Clippy pass with native Jazzy and containerized Humble/Kilted using Rust 1.85. | Complete runtime and RMW checks for each supported distribution and keep version-specific API boundaries explicit; Iron, Lyrical and Rolling remain CI matrix entries whose current local runtime evidence is not available. |
| Documentation builds | The [Jazzy snapshot](DOCUMENTATION_BUILDS.md) contains 23 packages, lifecycle/service-event bindings and saved interface availability. All targets compile without ROS; nineteen API pages and forty-one methods are checked, including managed timers, background tasks/errors, feedback overflow and typed/runtime-typed batched action-server methods. Refresh validates candidates before replacing existing bindings, rolls back partial failure and retains Cargo artifacts. Independent regeneration produces identical SHA-256 for all 36 snapshot files. | Keep snapshots aligned with application APIs; extend platform/documentation evidence and continue the native distribution/RMW matrix. |
| Performance evidence | Criterion/message-stage evidence, independent-process pub/sub and service matrices, the [observed action lifecycle/cancel A/B](benchmarks/action-lifecycle-2026-10-28.md), [native status decoding A/B](benchmarks/action-status-decode-2026-10-28.md), [native feedback identity routing](benchmarks/native-feedback-identity-2026-10-29/README.md), [outer native feedback reuse](benchmarks/native-feedback-buffer-2026-10-29/README.md), [latest-only Coalesce decoding](benchmarks/native-feedback-deferred-2026-10-29/README.md), [large-payload concurrent feedback scaling](benchmarks/action-feedback-scaling-2026-10-29/README.md), [batched action status snapshots](benchmarks/action-goal-batching-2026-10-29/README.md) and their [runtime-typed extension](benchmarks/action-goal-batching-untyped-2026-10-29/README.md). The qualified 4096-element/eight-goal workload covers all language directions and raw lifecycle/stage samples. With slice two and one-second result retention, r2r lowers complete p50/p95/p99 by 20–29% and raises throughput by 39–42% versus same-run rclcpp under both RMWs; the runtime-typed extension retains the typed-path 5% gate. | Complete pipelined/fanout pub/sub, dynamic payload reuse across RMW takes, borrowed delivered feedback, multithread/network scaling and calibrated wider-hardware evidence. Batch slice/retention qualification remains explicit; ten-second CycloneDDS feedback p99 is a retained failing boundary. |
| Module structure | `nodes/` owns spin prepare/dispatch, executors, timers, graph/wait sets, options/parameters, node creation and responsibility-specific endpoint factories. Action client and server factories/registry cleanup now live in separate modules; `nodes.rs` is 8761 lines and its test module starts at line 477. Action batch algorithms and runtime-type adapters are also separate. Lifecycle publication separates endpoint operations, loan ownership and nearby tests. A common notification queue owns optional cleanup wakes; node-specific binding stays in `background_notifications`. Humble/Iron name-resolution boundaries are preserved. | Reorganize the remaining test-heavy node module by responsibility. Benchmark entity scaling before replacing duplicated Vec+HashMap/swap-remove registries. Keep unit tests near implementation and integration tests at module or crate boundaries. |

## Execution order

The [action batch-progress prototype](benchmarks/action-batch-progress-2026-10-29/README.md)
was not promoted after the final Fast DDS success/cancellation p99 gate failed.
Production now [continues canceled-observer cleanup batches](ACTION_CANCELLATION_BACKLOG.md)
using the bounded drain's progress flag. [Feedback capacity recovery](ACTION_FEEDBACK_PROGRESS.md)
parks full senders and notifies runnable retries by goal, retaining bounded
sixteen-candidate passes and stopping self-notifications when all goals are blocked.
Explicit `FeedbackOverflowPolicy::Coalesce` now keeps queued samples and each
goal's newest pending item while native ingress continues for other goals.
[Feedback policy evidence](benchmarks/feedback-overflow-2026-10-29/README.md)
checks typed/untyped thirty-two-sample inputs, current executor binding and
payload retirement outside delivery locks. Default Backpressure semantics and
the original double-RMW 5% ordinary action gates remain enforced.
[Caller-owned feedback routing](benchmarks/feedback-routing-2026-10-29/README.md)
now preserves rejected/unmapped payloads until the client lock is released,
without a retirement allocation. Native reentry oracles and twenty-million-route
allocation checks pass. The final same-language A/B gates pass under both RMWs;
initial failures and mixed-success p99/calibration gaps remain explicitly archived.
[Native feedback identity routing](benchmarks/native-feedback-identity-2026-10-29/README.md)
now reads the generated native goal UUID before payload construction. Unobserved
feedback is taken and retired with zero decodes; observed feedback enters its
typed channel without an erased Box. Custom action wrappers retain the legacy
fallback by default. Real large-payload/no-observer oracles, fixed-CPU allocation
counts and the final ordinary action gates pass under both Jazzy RMWs. Observed
payload construction, retained native buffers and calibrated concurrent
multi-goal scaling remain open.
[Reusable outer native feedback storage](benchmarks/native-feedback-buffer-2026-10-29/README.md)
now keeps one RAII-managed message per action client and returns it on all
post-lock completion paths. Lock-reentry oracles, real decreasing-size takes,
fixed-CPU allocation counts, complete-process probes and both ordinary action
gates pass. Both RMWs changed nested sequence addresses during the rejected
stronger oracle, so dynamic payload capacity reuse is explicitly not claimed.
Borrowed delivery and concurrent multi-goal scaling remain open.
[Deferred latest-only Coalesce decoding](benchmarks/native-feedback-deferred-2026-10-29/README.md)
retains the newest native lease while a snapshot stream is full and constructs
an owned payload only when it can be delivered. Real two-goal 4096-element
oracles execute three decodes for ten native messages under both RMWs. The local
stage removes 10000 owned clones and 163.52 MB requested bytes per 10000
superseded arrivals; default Backpressure action gates pass. Deferred metadata
costs at most two machine words per tracked goal. Delivered borrowed payloads,
complete burst latency and concurrent scaling remain open.
[Large-payload concurrent action feedback](benchmarks/action-feedback-scaling-2026-10-29/README.md)
now measures 4096-element feedback, eight active goals and four messages per goal through
independent Rust/rclcpp processes in all four language directions. Current production beats
same-run rclcpp by 1.1–3.5% across CycloneDDS feedback percentiles and 3.7% in message rate,
but Fast DDS exposes periodic 1.8–3.0 second Rust-server tails and 66% lower throughput.
Keyed status indexing and bounded latest-status drains were rejected after their Fast DDS
p50/p95 gates failed, even when they sharply improved p99. The next action slice must profile
server accept/execute/status/terminal stages and separate reliable status progress from the
feedback critical path without taxing every batch.
[Batched action status snapshots](benchmarks/action-goal-batching-2026-10-29/README.md)
now provide explicit typed and runtime-typed accept/deferred-execute/terminal group methods with prepare-before-lock,
same-server/unique-UUID checks, per-goal results and first-error cleanup semantics. The runner
records and verifies batched versus per-goal identity plus six server stages. Slice two with
one-second retention passes the original 5% baseline gate under both RMWs; complete Rust action
lifecycle beats same-run rclcpp p50/p95/p99 by 20–29% and throughput by 39–42%. Ordinary action
success/cancellation gates still pass. Ten-second retention passes Fast DDS but fails CycloneDDS
feedback p99 and remains a configuration boundary, not an automatic middleware branch. Runtime-typed
JSON result decoding and erased-type validation finish before native state changes; the same batched
status path is shared without cloning goal payloads.
[Runtime-typed batching evidence](benchmarks/action-goal-batching-untyped-2026-10-29/README.md)
adds mixed-erased-type and JSON-failure oracles, typed-client/JSON-server examples across the
Humble/Jazzy/Kilted matrix, and an independent `fe938e4` same-batched-path guard. Both RMWs pass;
the candidate raises CycloneDDS batch rate by 3.0% and Fast DDS by 0.4% in the qualified workload.
[Server cancellation progress](ACTION_SERVER_CANCELLATION_PROGRESS.md) now uses
indexed request/goal queues for large cancellation work and follows current
executor bindings for both large and ordinary cancel streams. The complete
forty-goal `cancel_all()` application checks partial acceptance and every result.
The node now [schedules cancellation maintenance by ready server identity](ACTION_SERVER_SCHEDULING.md),
with deduplicated notifications and at most sixteen servers examined per pass.
Blocked servers are not revisited for unrelated traffic; decisions from other
threads wake existing foreground/shared/background guards and survive transfer.
[Independent preparation-stage and ordinary action A/B](benchmarks/server-scheduling-2026-10-29/README.md)
pass the original 5% gates under both Jazzy RMWs, with the initial failed
candidate retained. Native middleware feedback ingress/fanout, native wait-set
scaling, allocation accounting and larger mixed workloads remain open. The archived broader
prototype is not used to establish these implementations; qualified tail
precision remains a separate requirement.

1. Complete the remaining service-event distribution evidence after the [Jazzy/Kilted external endpoint matrix](interop/service-introspection-2026-10-29/README.md): the required CI now covers Iron/Lyrical/Rolling too, with a distinct Humble unsupported boundary. Expand diagnostic applications, payloads and QoS without weakening native identity or event-content oracles.
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

[Independent executor cancellation](benchmarks/executor-control-2026-10-28/README.md) retains
that shared-wait performance with cancellation state allocated only on handle creation. Both
default and explicitly enabled modes pass the original 5% gates under both Jazzy RMWs:
360 records and 1.8 million RTT samples. The initial always-allocated candidate failed one
6.78% p99 cell; its report and A/A calibration are preserved alongside the redesigned candidate.
Eighteen optimized executor tests and the background cancel/join/reuse example pass with both
Jazzy RMWs and native Humble/Kilted. Handles are weak, requests persist until reset, and native
wakes coalesce without shutting down the context or adding another thread/timer/guard.

## Rust ROS 2 ecosystem comparison

A design-level comparison of the four practical Rust ROS 2 clients (r2r, rclrs, safe_drive, ros2-client), including middleware strategy, execution models, feature coverage and maturity, is in [RUST_ROS2_COMPARISON.md](RUST_ROS2_COMPARISON.md).

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
cargo check --workspace --all-targets --features doc-only
cargo test -p r2r_msg_gen --features doc-only --lib message_path
python3 tests/check_documentation.py
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
