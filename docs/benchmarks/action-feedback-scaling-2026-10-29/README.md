# Large-payload concurrent action feedback matrix

This slice adds a reproducible Rust/rclcpp workload for the gap that the ordinary
single-goal action matrix did not cover: multiple active goals sharing one feedback
topic, multiple feedback messages per goal and large dynamic sequences. The benchmark,
runner and offline verifier are promoted; both attempted runtime optimizations are not.

## Protocol and correctness boundary

Each batch keeps eight goals active, then publishes four round-robin feedback messages
per goal. Every message carries 4096 deterministic `i32` values. Explicit readiness and
acknowledgement handshakes guarantee that all observers exist before publication and all
feedback is consumed before results complete. Exact goal order, every payload element,
all 32 feedback messages, every result, batch/goal checksums and a final rejected control
goal are verified in every Rust/Rust, C++/C++, Rust/C++ and C++/Rust run.

Final source-identical smoke matrices pass under CycloneDDS and Fast DDS with four
records and twelve measured batches each. [ACTION_FEEDBACK.md](../../../tools/bench_ros/ACTION_FEEDBACK.md)
documents the workload and commands. The saved build manifest inventories 79 Rust source
files plus the C++ harness/support sources. The final clean Rust/C++ binary SHA-256 values
are `31628c20...5ec6` and `86b18471...6e6b`.

## Current production scaling evidence

The long reports use 1000 measured batches after 50 warmups, three interleaved
repetitions, CPUs 24/25 and the 4096 × 8 goals × 4 feedback configuration. The
`baseline` rows are unmodified production commit `0a5f829`; rejected candidate rows are
retained in the same reports but are not used as current-production results.

| RMW | Endpoint pair | Feedback p50 | Feedback p95 | Feedback p99 | Feedback messages/s |
| --- | --- | ---: | ---: | ---: | ---: |
| CycloneDDS | current r2r/r2r | 7.658 ms | 10.819 ms | 11.918 ms | 1624 |
| CycloneDDS | rclcpp/rclcpp | 7.743 ms | 11.217 ms | 12.245 ms | 1566 |
| Fast DDS | current r2r/r2r | 1.129 ms | 1.808 ms | 2172.149 ms | 887 |
| Fast DDS | rclcpp/rclcpp | 3.093 ms | 5.625 ms | 15.198 ms | 2606 |

CycloneDDS current r2r is 1.1–3.5% lower across feedback p50/p95/p99 and 3.7%
higher in feedback-message rate than same-run rclcpp. Complete lifecycle p50/p95/p99
are also 1.2–3.6% lower. This is a measured workload lead, not an ecosystem-wide claim.

Fast DDS contradicts a general leadership claim. Current r2r has a much lower p50 but
periodic 1.8–3.0 second stalls around three clusters per 1000 batches; p99 is 143 times
rclcpp and throughput is 66% lower. Cross-language rows isolate the failure to workloads
with the Rust action server plus a status reader that does not aggressively drain: a Rust
client against a C++ server has 2.988 ms p50, 15.180 ms p99 and 2860 messages/s, while a
C++ client against a Rust server reproduces multi-second tails. Raw sample positions and
all endpoint directions remain in the reports.

## Allocation and profile evidence

The instrumented CycloneDDS matrix covers 0/4096 elements and one/eight active goals.
For 20 measured 4096 × 8 × 4 batches, client-side current r2r uses 14186 malloc calls
and 45.43 MB requested bytes versus rclcpp's 19897 and 55.10 MB. The Rust server uses
26117 malloc calls and 36.50 MB versus rclcpp's 29385 and 37.96 MB. These are process-wide
entry-point counts, separate from latency evidence.

An 8000-sample `cycles:u` profile of the current Rust client/C++ server path attributes
12.99% to `BuildHasher::hash_one`, 9.91% to `apply_status_updates`, 9.57% to UUID hashing
and only 2.84% to `memmove`. This identified retained-status processing as a real scaling
cost, but the complete A/B results below overrule the local hotspot result.

## Rejected runtime candidates

Three candidates were independently built against `0a5f829`; all use the same current
harness and a preselected 1.05 gate. Production was restored byte-for-byte after failure.

| Candidate | Fast DDS feedback p50 ratio | p95 ratio | p99 ratio | Inverse throughput | Outcome |
| --- | ---: | ---: | ---: | ---: | --- |
| Unified keyed status-interest index | 2.290 | 2.253 | 0.00198 | 0.269 | Reject |
| Drain/coalesce up to 16 status snapshots | 1.192 | 1.369 | 0.00529 | 0.663 | Reject |
| Drain/coalesce up to 2 status snapshots | 1.217 | 1.171 | 0.916 | 0.913 | Reject |

The keyed index reduced an 8192-retained/8-active local lookup stage from about 25.87 to
5.63 ns/status and passed CycloneDDS, but Fast DDS moved sparse multi-second stalls into
continuous p50/p95 cost. Draining sixteen snapshots nearly removed the tail and improved
throughput about 51%, yet still failed center latency. A limit of two neither removed the
tail nor passed p50/p95. Tail improvement is not accepted by weakening the 5% gate.

## Next boundary

The next production attempt must attribute time inside Rust server accept/execute/status,
feedback publish and terminal-status stages, then decouple reliable status progress from
the feedback critical path without synchronously processing extra snapshots in every batch.
Status payload construction, result retention and Fast DDS writer/reader backpressure must
be measured separately. Borrowed delivered feedback and RMW dynamic sequence reuse remain
open after that server/status issue. No runtime behavior changes are promoted by this slice.
