# Caller-owned feedback routing and retirement

Native feedback dispatch now borrows an erased payload slot while the client is
locked. Built-in typed/untyped routes consume that slot for scheduled delivery;
unknown goals and rejected types return the original Box for destruction after
unlock. This repairs two counterexamples where a rejected payload's destructor
reentered the client and found its mutex locked. The original owned routing
hook remains supported through a default borrowed adapter. Both native clients
share one private helper, with no new payload-copy or retirement allocation.

## Correctness and allocation stage

The initial typed/untyped tests both fail with `rejected feedback payload
destroyed under native client lock`. Final tests require exactly one destruction
after unlocking. Three nearby cases check unknown-goal original-box identity,
wrong-type restoration, empty-slot rejection and matched delivery with no early
destruction. The existing feedback batch/backpressure/coalescing/transfer suites
remain applicable to scheduled delivery and the legacy adapter.

[Final allocation stage](final/stage/report.json) performs twenty million routes
in ten records: two million unknown-goal or rejected-type operations, five runs
each, pinned to CPU 24. Input Boxes are allocated before each measured window
and retain their original allocation. The verified Linux/glibc probe records
zero measured allocation/free calls and zero requested bytes. It excludes native
decoding, ROS transport and successful-delivery completion allocations.
[Probe oracles](final/stage/probe-oracles.json) cover allocation content, alignment,
failures, epoch/reset behavior, threads and dynamic TLS loading. Five correctness
cases pass, with the manual measurement ignored during ordinary suites.

## Initial failure, analysis and final A/B

Baseline is `599b9d4`, using the identical confirmed-feedback action harness,
inputs, native QoS and retention. The [initial CycloneDDS candidate](initial/cyclonedds.json)
fails the original 1.05 gate: cancellation p99 ratio 1.769215 and inverse
throughput 1.052906. Its [samples](initial/cyclonedds.samples.json.gz), source
manifest, complete candidate patch and log are preserved. The wider promotion
matrix stopped after that failure.

A same-baseline-binary [CycloneDDS cancellation A/A](control/baseline-aa-cyclonedds.json)
passes symmetric 5% same-language calibration. An [instrumented resource run](resource/resource-cyclonedds.json)
finds similar allocations, polls/spins and CPU cost, which does not explain the
earlier large tail difference. Native-symbol/disassembly inspection identifies
an extra out-of-line forwarding call in the candidate. The final helper is
inlined into the typed route; the extra helper symbol/forwarding call disappears.
Final results do not attribute the original 77% tail difference to that call.

Independent optimized final binaries pass both preselected same-language gates.
Ratios below are Rust/Rust candidate percentiles divided by baseline/baseline;
inverse throughput uses baseline rate/candidate rate. Values are medians of five
repetition percentiles/rates.

| RMW | Scenario | p50 ratio | p95 ratio | p99 ratio | Inverse throughput |
| --- | --- | ---: | ---: | ---: | ---: |
| CycloneDDS | Success | 1.002393 | 1.016892 | 1.021657 | 1.007244 |
| CycloneDDS | Cancellation | 0.998386 | 0.991730 | 0.993003 | 0.988902 |
| Fast DDS | Success | 1.000796 | 1.009445 | 1.034569 | 0.997207 |
| Fast DDS | Cancellation | 1.001104 | 1.008589 | 1.005716 | 1.013128 |

* [Final CycloneDDS report](final/action/cyclonedds-v2.json), [samples](final/action/cyclonedds-v2.samples.json.gz).
* [Final Fast DDS report](final/action/fastdds-v2.json), [samples](final/action/fastdds-v2.samples.json.gz).
* [Final build/source manifest](final/action/build-manifest.json).

Mixed controls retain two unresolved success p99 increases: 6.346% for
CycloneDDS Rust/C++ and 11.020% for Fast DDS C++/Rust relative to their matching
baseline-language pairs. Those controls are outside the preselected same-language
gate. The [CycloneDDS success A/A](control/baseline-aa-success-cyclonedds.json)
passes same-language calibration and both symmetric mixed comparisons. The
[Fast DDS success A/A](control/baseline-aa-success-fastdds.json) passes the
same-language calibration but its identical-Rust-binary C++/Rust p99 differs by
7.479%, outside 5%. That mixed-tail precision remains unqualified; it neither
proves nor removes the candidate increase. No universally calibrated improvement
or C++ performance leadership is claimed.

The host is an i9-13900K, native ROS 2 Jazzy, Rust 1.85 release and GCC 13.3
`-O3 -DNDEBUG`. Formal endpoints use CPUs 24/25, one goal in flight, 1000 measured
goals after fifty warmups and five alternating repetitions. Final domains are
220/222; initial/control/resource domains are saved in each report. Both final
RMWs retain seventy records and 70000 raw RTT samples. This task's builds/tests
do not overlap formal measurements. Other shared-host activity is a limitation.

```sh
python3 tools/bench_ros/build.py --workload action --baseline-ref 599b9d4 \
  --output-dir target/feedback-routing-ab
python3 tools/bench_ros/verify_transport.py --require-performance-pass \
  docs/benchmarks/feedback-routing-2026-10-29/final/action/cyclonedds-v2.json \
  docs/benchmarks/feedback-routing-2026-10-29/final/action/fastdds-v2.json
```

## Required validation and remaining work

The complete Jazzy workspace and complete optimized library suites under both
RMWs pass 579 cases, with five manual benchmarks ignored. Both RMWs pass 48
relevant feedback cases. Native Humble/Fast DDS and Kilted/both RMWs pass the same
48 cases and all-target Clippy. Formatting and host all-target Clippy pass. The
no-ROS container compiles every target and verifies fifteen API artifacts and
twenty-nine methods. [Final verification record](validation.json) retains actual
counts, source/binary identities, control boundaries and compressed log digests.
Iron, Lyrical and Rolling have no new local runtime evidence for this slice.

Native decoding still occurs before routing/retention. Deferred native payloads,
larger/multigoal allocation accounting, complete application scaling and qualified
mixed-language tails remain work toward the full usability/capability/performance
goal.
