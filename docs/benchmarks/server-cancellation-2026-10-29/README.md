# Indexed action-server cancellation progress

A native forty-goal counterexample entered a 250 ms wait after a bounded
sixteen-decision pass even though the remaining decisions were ready. The
change supplies ready-indexed delivery and decision work, bounded continuation,
and weak notifications that follow the current executor after node transfer.
The ordinary small queues remain on their existing path; bulk state is boxed
and allocated on demand. Nearby tests cover boundaries through 128 entries,
exact ready selection, request-slot reuse, abandoned decisions, stale weak
wakes, goal removal and detached sender restoration.

Native typed/untyped tests issue two simultaneous forty-goal requests into
minimal cancel channels. They verify all-blocked quiescence, recovery of only
the final goal while 39 remain full, rejection/partial acceptance, exact accepted
UUID sets and all terminal results. They transfer the blocked node to a fresh
executor and retain the old guard to check that no cached wake uses it. A
two-goal case exercises the ordinary small-stream transfer path. The public
`cancel_all()` example validates twenty accepted and twenty rejected goals.

## Ordinary-path performance gate

The safe `b15e12b` baseline and candidate are independent optimized binaries
built from the same harness and dependency lock. The manifest includes all
three extracted action-server module sources and node preparation/factory
sources. CPUs 24/25, domain 122, inputs, QoS and the original 1.05 gate are fixed.
Task-owned builds and tests finished before measurement. Seven endpoint pairs
include baseline/baseline, Rust/Rust, C++/C++, and both Rust/C++ directions for
each Rust build. Five interleaved repetitions per success/cancellation scenario
contain 1000 measured and 50 warmup goals.

Each RMW report retains 70 records and 70000 raw RTT samples with all protocol
oracles passing. Both original same-language p50/p95/p99/inverse-throughput
gates pass. Ratios are Rust/Rust over baseline/baseline; throughput is inverted.

| RMW | Scenario | p50 ratio | p95 ratio | p99 ratio | Inverse throughput |
| --- | --- | --- | --- | --- | --- |
| rmw_cyclonedds_cpp | success | 0.9787 | 0.9882 | 0.9501 | 0.9768 |
| rmw_cyclonedds_cpp | cancellation | 1.0012 | 1.0012 | 1.0004 | 0.9974 |
| rmw_fastrtps_cpp | success | 1.0144 | 1.0224 | 1.0049 | 1.0146 |
| rmw_fastrtps_cpp | cancellation | 0.9976 | 1.0006 | 0.9597 | 1.0008 |

The mixed-language pairs are interoperability controls outside that preselected
performance gate. Unfavorable p99 observations exceeding 1.05 are retained:

| RMW | Candidate pair | Scenario | p99 over matching baseline pair |
| --- | --- | --- | --- |
| rmw_cyclonedds_cpp | rust/cpp | success | 1.0549 |
| rmw_cyclonedds_cpp | cpp/rust | cancellation | 1.0540 |
| rmw_fastrtps_cpp | cpp/rust | success | 1.1160 |

Those tail differences remain unresolved. Prior action same-binary calibration
drift still limits precision; this run has no matched passing calibration.
Passing the recorded gate does not establish that every endpoint pair is within
5%, a calibrated tail improvement, or superiority over C++. Large cancellation
throughput, cross-server scheduling, native ingress, allocation/resource counts
and mixed-load application comparisons remain separate evidence.

## Runtime boundaries and artifacts

The full Jazzy debug workspace suite and both RMWs' complete optimized library
suites pass 557 library cases, with two manual benchmarks ignored. Humble and
Kilted each pass 45 optimized action-server cases, four native scheduling cases,
the same-thread counterexample, optimized application and all-target Clippy.
The no-ROS Ubuntu gate compiles every workspace target, checks snapshots and
generators, and validates 13 API artifacts/28 methods. Final logs and their
identities are recorded in [validation](validation.json). Iron, Lyrical and
Rolling have no new local runtime evidence for this slice.

The native forty-result burst first timed out at terminal result 5 using the
default result endpoint depth of 10. Both result endpoints now use KeepLast(64)
for the forty-result concurrency, while cancel channels retain capacity zero.
The earlier failing diagnostic is preserved. This test configuration does not
change the ordinary-path benchmark's settings.

* [CycloneDDS report](ready-cyclonedds.json) and [raw samples](ready-cyclonedds.samples.json.gz).
* [Fast DDS report](ready-fastdds.json) and [raw samples](ready-fastdds.samples.json.gz).
* [Progress and ownership contracts](../../ACTION_SERVER_CANCELLATION_PROGRESS.md).
* [Complete application](../../../r2r/examples/action_cancel_group.rs).
* [Original failure](logs/server-cancel-before.log.gz) and
  [result-burst diagnostic](logs/server-cancel-native-diagnostic.log.gz).

```sh
python3 tools/bench_ros/build.py --baseline-ref b15e12b --workload action --output-dir target/server-cancel-comparison
python3 tools/bench_ros/verify_transport.py --require-performance-pass docs/benchmarks/server-cancellation-2026-10-29/ready-cyclonedds.json
python3 tools/bench_ros/verify_transport.py --require-performance-pass docs/benchmarks/server-cancellation-2026-10-29/ready-fastdds.json
```
