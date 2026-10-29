# Runtime-typed batched action goals

This slice extends the explicit action status-snapshot transaction from generated Rust action
types to runtime-selected JSON action servers. `ActionServerGoalRequestUntyped` now exposes batch
accept/deferred accept, and `ActionServerGoalUntyped` exposes batch execute, succeed, abort and
cancel. Existing single-goal APIs and the typed batch API remain source compatible.

## Correctness and ownership boundary

The first erased request/goal dispatches to its generated action type. Every remaining member must
downcast to that exact type; the shared typed batch core then validates one native server, unique
UUIDs and exact result counts. All JSON results are decoded only after these structural checks and
before native response construction, server locking or state changes. A malformed result therefore
leaves the whole slice recoverable. Empty slices remain no-ops.

The erased adapter passes references to the typed batch core rather than cloning goal payloads.
Typed calls retain their existing allocation shape: direct slices enter the iterator-based
same-server/UUID validator without constructing a `Vec<&Goal>`. Native result messages are still
prepared before the server lock, changed prefixes still publish a final snapshot, and first-error
reporting still attempts later result/status/expiry cleanup.

Two real-RCL oracles run under both Jazzy RMWs. They cover mixed erased-type rejection, empty and
result-count boundaries, malformed JSON without state change, deferred accept/execute, 128-element
feedback, exact success/abort/cancel results and terminal feedback closure. The wake-driven
`action_goal_batch_untyped` example completes four generated client goals through a runtime-typed
JSON server under Jazzy/both RMWs, Humble/Fast DDS and Kilted/both RMWs.

## Typed-path performance guard

This change adds runtime-typed capability; it does not claim JSON conversion is faster than a
generated Rust type. Because the shared typed validator was refactored, an independent optimized
`fe938e4` baseline protects the already-qualified typed path. Both binaries report
`status_path=batched`, use CPUs 24/25, 4096 `i32` feedback values, eight active goals, four feedback
messages per goal, slice size two, one-second result retention, 50 warmups, 1000 measured batches
and five interleaved repetitions. The unchanged 1.05 p50/p95/p99/feedback/inverse-throughput gate
passes under both RMWs.

| RMW | Metric | `fe938e4` | Candidate | Candidate / baseline |
| --- | --- | ---: | ---: | ---: |
| CycloneDDS | lifecycle p50 | 4.760 ms | 4.795 ms | 1.007 |
| CycloneDDS | lifecycle p95 | 5.451 ms | 5.416 ms | 0.994 |
| CycloneDDS | lifecycle p99 | 6.103 ms | 6.001 ms | 0.983 |
| CycloneDDS | feedback p99 | 0.924 ms | 0.907 ms | 0.981 |
| CycloneDDS | feedback messages/s | 6671 | 6871 | 1.030 |
| Fast DDS | lifecycle p50 | 2.874 ms | 2.859 ms | 0.995 |
| Fast DDS | lifecycle p95 | 3.507 ms | 3.460 ms | 0.987 |
| Fast DDS | lifecycle p99 | 3.941 ms | 3.925 ms | 0.996 |
| Fast DDS | feedback p99 | 1.116 ms | 1.117 ms | 1.001 |
| Fast DDS | feedback messages/s | 11599 | 11648 | 1.004 |

The build manifest covers 82 candidate source files with zero missing files or hash mismatches.
Each report contains 10 endpoint records and 10000 raw batch samples.

## Compatibility and structure

The Jazzy single-thread workspace passes with 600 main-library cases and eight ignored manual
measurements. Optimized main-library suites pass 600/8 under both RMWs. Humble/Fast DDS and
Kilted/both RMWs each execute the two new focused cases and the four-goal example; their all-target
warning-denied Clippy gates pass. The no-ROS gate compiles all targets and verifies 19 Rustdoc pages
and 41 methods, including all twelve typed/runtime-typed batch entry points.

Action-server factory/registry ownership moved from `nodes.rs` into
`nodes/action_server_api.rs`; runtime-type adapters live in `action_servers/untyped.rs`, while
transaction algorithms and nearby real-RCL tests stay in `action_servers/batch.rs`. `nodes.rs`
drops from 8940 to 8761 lines and its test module now starts at line 477.

Two environment failures are retained. Humble running as an unprivileged UID without a writable
ROS log directory fails an existing context test during logging initialization; setting
`ROS_LOG_DIR=/tmp/r2r-ros-log` resolves it. The older `r2r_kilted_validation` image does not contain
CycloneDDS; the qualified Kilted run uses the recorded dual-RMW image instead. An earlier passing
A/B was also superseded after a test-only source edit changed the manifest identity; the archived
reports come from rebuilt final-source binaries.

```sh
python3 tools/bench_ros/verify_action_feedback.py --require-performance-pass \
  docs/benchmarks/action-goal-batching-untyped-2026-10-29/performance/cyclonedds.json \
  docs/benchmarks/action-goal-batching-untyped-2026-10-29/performance/fastdds.json
cargo run -p r2r --example action_goal_batch_untyped
```

Exact source, binary, report, sample, image and validation-log identities are recorded in
[validation.json](validation.json).
