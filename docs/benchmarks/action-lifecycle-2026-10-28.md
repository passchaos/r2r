# Action lifecycle and cancellation-progress evidence

The action comparison validates accepted goals, exactly one observed feedback and terminal
success/cancellation through all four Rust/rclcpp client/server combinations under CycloneDDS
and Fast DDS. It exposed and fixed a timeout-sized local cancellation delay. After that fix,
the complete observed lifecycle still has measurable Rust/C++ gaps: Rust/Rust p50 is 11–13%
higher under CycloneDDS and 29–33% higher under Fast DDS in the full workload. This evidence
establishes the remaining action work instead of implying general Rust performance leadership.

This is the initial matrix at `441bcd4`. The subsequent
[native status decoding A/B](action-status-decode-2026-10-28.md) reduces complete lifecycle p50
by 3.4–22.4% against an interleaved baseline and brings Fast DDS success close to rclcpp.
Use that same-run comparison for the latest measured gap; the matrix below remains historical.

## Protocol and fairness

The [action harness](../../tools/bench_ros/ACTION.md) sends native Fibonacci goals in increasing
order. Every feedback and result must equal `[order, order+1, order+2]`. Scenario 0 succeeds;
scenario 1 accepts cancellation and returns a Canceled result. Every endpoint count, goal
sequence, feedback/result payload and terminal state is checked. Rejected control goals -1/-2
terminate the pair only after all measured results have been consumed.

C++ can discard feedback arriving before accepted-goal handling installs its observer. Both
servers therefore accept deferred, and both clients use the same auxiliary ServicesQoS readiness
handshake before execution/feedback. Success also confirms feedback consumption before result
completion. In the cancel scenario, the cancel request provides that confirmation. These
handshakes are **included** in lifecycle RTT; the workload is not bare action acceptance or
goal/result latency. Acceptance time ends after accepted-goal retrieval and result-request
submission; cancellation time ends at accepted-cancel response retrieval.

Both processes use single-threaded application execution, a 100 ms native wait cap, matching
reliable volatile KeepLast(16) for goal/result/cancel/feedback, native status defaults and
10-second result retention. There is one active goal at a time. Native results/statuses are
retained across consecutive goals during that interval, so cohort cost is part of the measured
workload. There are no rosout or parameter endpoints, and intra-process transport is disabled.

## Full current Rust/C++ matrix

Each RMW runs five alternating-order repetitions of 1000 measured goals after 50 warmup goals,
for success and cancellation. Reported latency is the median repetition p50/p99 of individual
lifecycles, in microseconds; the rate includes application validation and protocol handshakes.

| RMW | Scenario | Rust p50 | C++ p50 | Rust p99 | C++ p99 | Rust goals/s | C++ goals/s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| CycloneDDS | Success | 664.95 | 598.37 | 1085.83 | 952.83 | 1526 | 1681 |
| CycloneDDS | Cancel | 865.25 | 765.13 | 1404.46 | 1254.76 | 1183 | 1324 |
| Fast DDS | Success | 423.41 | 329.37 | 651.89 | 478.60 | 2325 | 2998 |
| Fast DDS | Cancel | 531.14 | 397.88 | 941.48 | 593.08 | 1812 | 2497 |

The mixed directions are retained in the raw reports. Under Fast DDS, C++ client/Rust server
has lower success p50 than C++/C++, while Rust client/C++ server remains close to Rust/Rust.
That pattern points to client-side goal/result tracking and native conversion as useful next
profiling targets; it does not prove a particular cause without further stage/allocation evidence.

## Cancellation-progress A/B

The baseline library `a4c3fd5` applied local cancel decisions during spin maintenance, then
entered the full native wait. Applications checking `is_cancelling()` before submitting a
terminal result consequently slept for about 100 ms even after the cancel response was sent.
The fix returns promptly after completed local cancel responses while still draining ready
native work. Unresolved decisions and backpressured delivery retain the requested wait.

The separate A/B uses the same current harness source and lockfile in independent baseline and
candidate binaries, seven language combinations, five repetitions, scenario 1 only, 50 measured
goals and 5 warmup goals. Both retain results for 10 seconds; this short cohort stays below that
interval even in the slow baseline. Its numbers must not be compared directly with the larger
1000-goal full-matrix cohort above.

| RMW | Baseline cancel p50 | Fixed cancel p50 | Baseline p99 | Fixed p99 |
| --- | ---: | ---: | ---: | ---: |
| CycloneDDS | 100649.69 μs | 290.08 μs | 100926.23 μs | 309.19 μs |
| Fast DDS | 100833.37 μs | 306.72 μs | 101119.78 μs | 376.61 μs |

This removes approximately 99.7% of that timeout-dominated lifecycle delay. Both A/B reports
pass the preselected 1.05 candidate/baseline median limits for p50/p95/p99 and inverse throughput.
Those gates cover the measured cancellation scenario, not every possible action workload.

## Artifacts and environment

The host is a Core i9-13900K, clients pinned to CPU 24 and servers to CPU 25. ROS 2 Jazzy,
Rust 1.85 release and GCC 13.3 `-O3 -DNDEBUG` use default ISA targeting. Native middleware
threads inherit process affinity. DDS domains and action names are isolated; no task-owned
build or test overlaps measurement. Other shared-host workloads on other CPUs remain a limit.

* [CycloneDDS lifecycle report](action-lifecycle-2026-10-28-cyclonedds.json) and [raw samples](action-lifecycle-2026-10-28-cyclonedds.samples.json.gz)
* [Fast DDS lifecycle report](action-lifecycle-2026-10-28-fastdds.json) and [raw samples](action-lifecycle-2026-10-28-fastdds.samples.json.gz)
* [CycloneDDS cancel A/B](action-cancel-progress-2026-10-28-cyclonedds.json) and [raw samples](action-cancel-progress-2026-10-28-cyclonedds.samples.json.gz)
* [Fast DDS cancel A/B](action-cancel-progress-2026-10-28-fastdds.json) and [raw samples](action-cancel-progress-2026-10-28-fastdds.samples.json.gz)
* [Cancellation behavior and targeted tests](../ACTION_CANCELLATION.md)
* [Deterministic application example](../../r2r/examples/action_lifecycle.rs)

Each lifecycle report contains 40 records, 40,000 RTT samples and matching acceptance/cancellation
stage samples. Each cancellation A/B contains 35 records and 1750 RTT samples. SHA-256 references,
binary/source hashes, package/compiler versions, lifecycle counts and exact gate outcomes are
included. The offline verifier checks matrix completeness, counts, stages, percentiles, rate
calculations, gzip integrity and medians:

```sh
python3 tools/bench_ros/verify_transport.py \
  docs/benchmarks/action-lifecycle-2026-10-28-cyclonedds.json \
  docs/benchmarks/action-lifecycle-2026-10-28-fastdds.json
python3 tools/bench_ros/verify_transport.py --require-performance-pass \
  docs/benchmarks/action-cancel-progress-2026-10-28-cyclonedds.json \
  docs/benchmarks/action-cancel-progress-2026-10-28-fastdds.json
```

Three cancellation boundary tests cover accepted/rejected decisions, unresolved wait behavior
and zero-capacity delivery backpressure. Native action smoke matrices and the example run on
Humble/Kilted as compatibility checks, while optimized Jazzy library tests, Clippy and doc-only
checks cover the implementation. Performance comparison is Jazzy-only. Larger feedback streams,
multi-goal concurrency, acceptance-only use, memory/allocation counters, networked hosts and
multi-thread scaling remain necessary before the broader goal can be claimed complete.
