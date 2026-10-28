# Feedback capacity notifications

The change makes pending action feedback runnable when its consumer releases
channel capacity. Full senders register one-shot goal-specific wakers and leave
the runnable queue. Forty blocked goals are parked in finite sixteen-candidate
passes; blocked work alone supplies no self-continuation. Repeated capacity
notifications coalesce, and wakers hold weak queue references. Typed/untyped
clients share the implementation without changing FeedbackChannel or client
handle layouts. Normal successful sends do not allocate readiness objects.

The first counterexample failed against the original behavior: maintenance
self-notified while feedback was blocked, and channel capacity recovery did not
notify native retry. That failing log is retained alongside the unit, native
and complete slow-consumer application checks. This supplies bounded retry
progress, not unlimited lossless buffering or per-goal native DDS scheduling.

## Ordinary action performance boundary

Independent optimized Rust binaries compare safe baseline `2c2ff89` with the
candidate under both Jazzy RMWs. CPUs 24/25, domain 122, inputs and QoS are fixed.
Seven endpoint combinations cover baseline/baseline, Rust/Rust, C++/C++, and
both Rust/C++ directions for each Rust build. Five interleaved repetitions
contain 1000 measured goals after 50 warmups. Task-owned builds and tests
finished before measurements; the baseline and candidate use the same harness.

Each report retains 70 records and 70000 raw RTT samples. Acceptance, feedback,
cancellation and result oracles pass, as do the unchanged 1.05 p50/p95/p99 and
inverse-throughput regression gates for Rust/Rust versus baseline/baseline.
The mixed-language combinations are interoperability controls, rather than
additional cells of that performance gate. The verifier independently checks
the reports and their sample digests. Ratios below are candidate over baseline;
throughput is inverted so lower is better.

| RMW | Scenario | p50 ratio | p95 ratio | p99 ratio | Inverse throughput |
| --- | --- | --- | --- | --- | --- |
| rmw_cyclonedds_cpp | success | 1.0011 | 0.9991 | 0.9905 | 1.0032 |
| rmw_cyclonedds_cpp | cancellation | 1.0020 | 1.0009 | 1.0098 | 1.0057 |
| rmw_fastrtps_cpp | success | 0.9997 | 0.9975 | 0.9008 | 0.9959 |
| rmw_fastrtps_cpp | cancellation | 1.0022 | 1.0007 | 1.0092 | 1.0074 |

These runs measure the effect on the ordinary action transport path. The
backpressure progress and absence of repeated blocked wakes are covered by
separate native tests. This A/B does not establish faster multigoal throughput
or superiority over rclcpp. Prior action same-binary tail drift remains an
explicit limitation; no calibrated tail improvement is inferred from the
passing regression gate. Larger payloads, fanout, allocation counters and
native feedback ingress remain separate workloads.

The mixed-endpoint controls also retain unfavorable observations: CycloneDDS
C++/Rust cancellation p99 is 1.517 ms versus 1.331 ms for C++/baseline, about
14.0% higher. That cell is outside the preselected same-language gate and its
tail difference remains unresolved. Passing the recorded gate therefore does
not establish that every endpoint combination is within 5% or that the
measurement has calibrated mixed-endpoint tail precision.

The manifest inventories the extracted feedback module and all action/node
sources. One lock/coalescing test was appended after the benchmark build;
[its identity record](post-measurement-test-identity.json) verifies that the
production source prefix is byte-identical to the measured source. The
measurement manifest is preserved without rewriting its identities.

## Runtime and compatibility gates

The full Jazzy debug workspace suite passes, including 546 library cases with
two manual benchmarks ignored. Complete optimized library suites execute 546
cases under both RMWs. Native Humble and Kilted each pass 33 optimized feedback
cases and the optimized complete application. Their all-target workspace
Clippy gates pass, as do Jazzy Clippy and formatting. The no-ROS Ubuntu gate
compiles every workspace target, validates binding snapshots and generator
paths, and checks 13 API artifacts and 28 methods. Final logs are retained in
this directory. Iron, Lyrical and Rolling have no new local runtime evidence.

* [CycloneDDS report](feedback-ready-cyclonedds.json) and
  [raw samples](feedback-ready-cyclonedds.samples.json.gz).
* [Fast DDS report](feedback-ready-fastdds.json) and
  [raw samples](feedback-ready-fastdds.samples.json.gz).
* [Feedback progress contract](../../ACTION_FEEDBACK_PROGRESS.md) and
  [complete application](../../../r2r/examples/action_feedback_backpressure.rs).

```sh
python3 tools/bench_ros/verify_transport.py --require-performance-pass docs/benchmarks/feedback-capacity-2026-10-29/feedback-ready-cyclonedds.json
python3 tools/bench_ros/verify_transport.py --require-performance-pass docs/benchmarks/feedback-capacity-2026-10-29/feedback-ready-fastdds.json
```
