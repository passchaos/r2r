# Action-server ready scheduling evidence

The candidate schedules cancellation maintenance by ready server UUID instead
of polling every pending server before each native wait. Twenty genuinely
blocked servers previously received eighty maintenance calls over four unrelated
spins. The candidate requires zero; making only the last server ready requires
one. It also wakes external decisions in existing native waits and preserves
readiness across executor transfer. The [scheduling contract](../../ACTION_SERVER_SCHEDULING.md)
defines queues, bounded passes, guard ownership and correctness cases.

The baseline is `69a379f78e9170dd5093de86c1cf5d81e4790f9d`. Final preparation
and ordinary action A/B pass their preselected 1.05 gates under Jazzy CycloneDDS
and Fast DDS. The initial ordinary CycloneDDS cancellation p99 ratio was
1.085293 and failed. Its reports and complete source patch are retained.
The final candidate borrows small cancellation wakers, removes redundant clones
and allocates a deduplication HashSet only beyond sixteen ready identities.

## Parked-server preparation stage

The harness establishes real pending native action cancellations, registers
their blocked decisions and times only `Node::prepare_spin`. Both independent
optimized library-test binaries include the same harness and TLS maintenance
counter instrumentation. Native waiting, dispatch, discovery and application
throughput are excluded. The baseline archive receives the harness/module
registration and the same Cargo.lock, without the new production scheduling.

Each RMW contains forty records and 800000 individual stage samples: zero,
one, eight and thirty-two servers, 20000 measured iterations after 1000 warmups,
five repetitions of each binary. Runs reverse binary order on alternate
repetitions. All threads are pinned to CPU 24; domains are 132/134. The verifier
requires exactly `servers * iterations` baseline maintenance calls and zero
candidate maintenance calls, and checks actual RMW/affinity, percentile
arithmetic, loop times, record completeness and sample digests.

Values are medians of repetition percentiles, in nanoseconds.

| RMW | Servers | Baseline p50 | Candidate p50 | Baseline p99 | Candidate p99 |
| --- | ---: | ---: | ---: | ---: | ---: |
| CycloneDDS | 0 | 310 | 318 | 338 | 341 |
| CycloneDDS | 1 | 437 | 323 | 468 | 348 |
| CycloneDDS | 8 | 1176 | 321 | 1233 | 338 |
| CycloneDDS | 32 | 4027 | 316 | 4225 | 321 |
| Fast DDS | 0 | 311 | 318 | 321 | 328 |
| Fast DDS | 1 | 438 | 323 | 446 | 330 |
| Fast DDS | 8 | 1171 | 314 | 1188 | 319 |
| Fast DDS | 32 | 4018 | 315 | 4358 | 320 |

The 5% gate covers p50/p95/p99 and complete-loop ns/call for every server count,
including the zero-server control. The thirty-two-server stage p50 improvement
is about 92.2%. It is a comparison against earlier r2r, rather than rclcpp or
an end-to-end application speedup. Preparation still traverses native entity
registries for other duties, and complete wait-set scaling remains open.

* [Final CycloneDDS report](final/prepare/cyclonedds.json), [samples](final/prepare/cyclonedds.samples.json.gz).
* [Final Fast DDS report](final/prepare/fastdds.json), [samples](final/prepare/fastdds.samples.json.gz).
* [Build/source/binary manifest](final/prepare/build-manifest.json).

## Ordinary action transport gate

Independent optimized baseline/candidate Rust binaries and the C++ comparison
run complete successful and canceled action lifecycles. Readiness and feedback
handshakes, result retrieval, exact sequences/checksums and two rejected control
goals are verified. One goal stays in flight, with identical reliable QoS and
native status defaults. Each of seven language pairs executes five repetitions
of 1000 goals after fifty warmups: seventy records and 70000 raw RTT samples
per RMW. Ping/echo processes use CPUs 24/25 and domains 122/124.

The preselected gate compares Rust/Rust against baseline/baseline. Ratios below
are candidate/baseline; inverse throughput uses baseline rate/candidate rate.
No tolerance or scenario was changed after the initial failure.

| RMW | Scenario | p50 ratio | p95 ratio | p99 ratio | Inverse throughput |
| --- | --- | ---: | ---: | ---: | ---: |
| CycloneDDS | Success | 0.9998 | 1.0039 | 1.0073 | 1.0048 |
| CycloneDDS | Cancellation | 0.9963 | 1.0154 | 1.0288 | 1.0131 |
| Fast DDS | Success | 0.9892 | 0.9974 | 0.9678 | 0.9961 |
| Fast DDS | Cancellation | 0.9967 | 1.0006 | 0.9298 | 0.9915 |

* [Final CycloneDDS report](final/action/ready-cyclonedds.json), [samples](final/action/ready-cyclonedds.samples.json.gz).
* [Final Fast DDS report](final/action/ready-fastdds.json), [samples](final/action/ready-fastdds.samples.json.gz).
* [Build/source manifest](final/action/build-manifest.json).
* [Initial failed CycloneDDS report](initial/action/ready-cyclonedds.json) and [samples](initial/action/ready-cyclonedds.samples.json.gz).
* [Initial source patch](initial/candidate.patch.gz); its other reports remain under `initial/`.

Final mixed-language p99 controls remain within 5% of their matching baseline
pairs; they are outside the preselected same-language gate. A separate
[same-baseline-binary CycloneDDS cancellation control](control/baseline-aa-cyclonedds.json)
passes the symmetric 5% p50/p95/p99/rate calibration; [samples](control/baseline-aa-cyclonedds.samples.json.gz)
and [manifest](control/baseline-aa-manifest.json) remain available. This calibrates
only that baseline/scenario run. Earlier short-cohort action tail drift remains
relevant, and this does not establish universal tail gains or Rust/C++ superiority.

## Environment and verification

The host is a Core i9-13900K running Linux 6.8 and native ROS 2 Jazzy. Rust
1.85 release and GCC 13.3 `-O3 -DNDEBUG` use default ISA targeting. Exact
compiler/package versions and binary/source hashes are in the reports.
Formal measurements ran serially without this task's builds/tests competing.
Other unrestricted shared-host jobs remain a limitation for small differences
and tails.

The final workspace and two complete optimized Jazzy library suites pass 564
library tests, with three manual benchmarks ignored. Humble/Kilted each pass
the scheduling, background transfer, observer, all 47 action-server module,
four native cancellation and 19 future-execution cases. The complete optimized
forty-goal cancellation application and all-target Clippy pass in both
distributions. The no-ROS container checks all workspace targets, snapshot and
generator tests, thirteen API artifacts and twenty-eight documented methods.
The [validation record](validation.json) retains exact counts, log digests,
source identity and archive inventory; compressed logs are under `logs/`.
Iron, Lyrical and Rolling have no new local runtime evidence for this slice.

```sh
python3 tools/bench_ros/prepare_stage.py verify \
  docs/benchmarks/server-scheduling-2026-10-29/final/prepare/cyclonedds.json \
  docs/benchmarks/server-scheduling-2026-10-29/final/prepare/fastdds.json
python3 tools/bench_ros/verify_transport.py --require-performance-pass \
  docs/benchmarks/server-scheduling-2026-10-29/final/action/ready-cyclonedds.json \
  docs/benchmarks/server-scheduling-2026-10-29/final/action/ready-fastdds.json \
  docs/benchmarks/server-scheduling-2026-10-29/control/baseline-aa-cyclonedds.json
```

Large mixed workloads, native ingress/fanout, allocation/CPU/memory accounting
and representative C++ application comparisons remain separate requirements
toward the full usability, capability and performance goal.
