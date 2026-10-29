# Batched action status snapshots

This slice turns the multi-goal scaling gap found by the earlier
[concurrent feedback matrix](../action-feedback-scaling-2026-10-29/README.md) into an explicit,
safe action-server API. Existing per-goal methods are unchanged. Applications can group requests
and terminal results so r2r publishes one complete reliable status array per chosen slice instead
of one array per goal transition.

## API and correctness boundary

`ActionServerGoalRequest::{accept_batch, accept_deferred_batch}` and
`ActionServerGoal::{execute_batch, succeed_batch, abort_batch, cancel_batch}` require one server,
unique UUIDs and exact terminal result counts. Native messages are prepared before locking.
Partial transition prefixes publish a final snapshot, and first-error reporting does not skip
later result/status/expiry cleanup. Per-goal feedback, result requests, tenured result storage and
stream closure retain their ordinary semantics.

A real three-goal RCL oracle verifies immediate Executing status, exact 128-element feedback,
successful per-goal results and terminal feedback-stream closure under both Jazzy RMWs. The
`action_goal_batch` application completes four goals through wake-driven future execution under
Jazzy/CycloneDDS, Jazzy/Fast DDS, Humble/Fast DDS and Kilted/both RMWs. Unit oracles cover empty,
duplicate, foreign-server and result-count rejection plus error-ordering boundaries.

## Root-cause stages

The harness now stores per-batch echo stages: accept, readiness wait, execute, feedback publish,
feedback acknowledgement wait and terminal completion. Fast DDS multi-second stalls occur in the
feedback wait after large retained reliable status arrays, not inside feedback publication.
Best-effort status or one-second result retention removes the stall; reliable volatile alone does
not. Result retention and native goal/status lifetime cannot be separated without replacing the
RCL retention contract, so the public API exposes slice selection instead of weakening QoS.

Reports declare `status_path=batched|per_goal`, status QoS, retention, slice size and whether every
Executing status was observed before feedback timing. The verifier rejects path/configuration
mismatches. This caught and invalidated earlier measurements whose candidate binary silently used
the fallback path.

## Qualified baseline gate

Independent optimized `dc4e1c4` baseline/candidate binaries run 1000 measured batches after 50
warmups, five interleaved repetitions, 4096 `i32` feedback values, eight active goals, four
feedback messages per goal, slice size two and one-second result retention. CPUs 24/25 and the
unchanged 1.05 p50/p95/p99/feedback-stage/inverse-throughput limit are fixed. Clients observe all
Executing statuses before the isolated feedback stage.

| RMW | Metric | Baseline | Batched candidate | Candidate / baseline |
| --- | --- | ---: | ---: | ---: |
| Fast DDS | lifecycle p50 | 3.601 ms | 2.857 ms | 0.793 |
| Fast DDS | lifecycle p95 | 4.277 ms | 3.518 ms | 0.823 |
| Fast DDS | lifecycle p99 | 4.770 ms | 3.960 ms | 0.830 |
| Fast DDS | feedback p99 | 1.079 ms | 1.122 ms | 1.040 |
| Fast DDS | feedback messages/s | 9173 | 11621 | 1.267 |
| CycloneDDS | lifecycle p50 | 6.411 ms | 4.803 ms | 0.749 |
| CycloneDDS | lifecycle p95 | 7.067 ms | 5.409 ms | 0.765 |
| CycloneDDS | lifecycle p99 | 7.969 ms | 5.895 ms | 0.740 |
| CycloneDDS | feedback p99 | 0.912 ms | 0.916 ms | 1.005 |
| CycloneDDS | feedback messages/s | 5043 | 6871 | 1.362 |

Both reports pass the preselected gate. Ordinary single-goal success/cancellation matrices also
pass under both RMWs: 140 records and 140000 RTT samples, proving that applications which do not
call the batch API retain their existing performance.

## Same-run rclcpp comparison

The full matrices use identical payloads, QoS, one-second retention, client/server readiness and
result oracles in Rust/Rust, C++/C++ and both mixed directions. They retain actual status/feedback
competition; complete lifecycle and completion rate are the comparison boundary.

| RMW | Pair | lifecycle p50 | p95 | p99 | batches/s |
| --- | --- | ---: | ---: | ---: | ---: |
| Fast DDS | r2r/r2r | 2.809 ms | 3.450 ms | 3.967 ms | 369 |
| Fast DDS | rclcpp/rclcpp | 3.913 ms | 4.499 ms | 4.977 ms | 265 |
| CycloneDDS | r2r/r2r | 4.748 ms | 5.407 ms | 5.921 ms | 218 |
| CycloneDDS | rclcpp/rclcpp | 6.575 ms | 7.206 ms | 8.336 ms | 154 |

r2r lowers all three complete lifecycle percentiles by 20–29% under Fast DDS and 25–29% under
CycloneDDS. Batch completion rate is 39% and 42% higher respectively. This establishes leadership
for this qualified concurrent action workload, not for every ROS application or hardware target.

## Configuration limits and rejected evidence

Slice size is deliberately caller-controlled. With ten-second result retention, Fast DDS slice
two passes, but CycloneDDS feedback p99 fails the 5% gate by 25%; that report is retained under
`rejected/`. Larger slices can improve complete throughput while regressing isolated feedback
tails because faster completion retains more goals inside every status snapshot. The threshold was
not relaxed and no rejected binary was promoted.

Dynamic RMW sequence reuse, borrowed delivered feedback, networked hosts, multithreaded executors
and wider hardware remain open. Iron, Lyrical and Rolling have no new local runtime results.

The [validation record](validation.json) fixes the report, raw-sample, manifest, binary and audit-log
digests. Both archived manifests cover 80 candidate source files and match the final working tree
with zero missing files or hash mismatches.

```sh
python3 tools/bench_ros/verify_action_feedback.py --require-performance-pass \
  docs/benchmarks/action-goal-batching-2026-10-29/action-feedback/cyclonedds-ab.json \
  docs/benchmarks/action-goal-batching-2026-10-29/action-feedback/fastdds-ab.json
python3 tools/bench_ros/verify_transport.py --require-performance-pass \
  docs/benchmarks/action-goal-batching-2026-10-29/ordinary/cyclonedds.json \
  docs/benchmarks/action-goal-batching-2026-10-29/ordinary/fastdds.json
```
