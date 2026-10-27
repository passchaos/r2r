# Rejected retained-action status filtering experiment

The observer-filtering runtime candidate was rejected and removed. Its final Fast DDS
short-cohort cancellation p99 increased by 8.9%, exceeding the preselected 5% regression
limit. Production keeps the native status decoder and routing at `d45e261`. Faster retained
cohort measurements do not justify promoting a candidate that fails another required boundary.

## Profiling and proposed routing

A profile of `d45e261` Rust/Rust cancellation with CycloneDDS sampled substantial client
UUID hashing: `BuildHasher::hash_one` 10.65%, `Uuid::hash` 7.42% and `apply_status_updates`
7.10% among 310 user CPU samples, with none lost. The server's 614 samples were primarily
middleware functions. The [profile artifact](baseline-profile.json) retains exact executable
and source hashes, endpoint counts and perf reports; sampled percentages are directional
evidence, not exact cost attribution. Profiling timings are excluded from performance claims.

The native status message contains goals retained for 10 seconds, while the client often
observes only one active goal. The proposed helper snapshots public-goal and feedback observer
UUIDs into a stack array and skips irrelevant historical statuses before map routing. Feedback
keys are necessary because feedback/result observers can outlive the public goal handle.

The direct path was limited to batches of at least 16 statuses, at most four distinct observer
UUIDs, at most eight combined map entries and combined map capacity at most 64. Larger maps
and short messages kept ordinary routing. Setup was bounded even after formerly large maps
became small. The revised version dispatched once per batch so fallback routing kept its
original per-status loop. No public concurrency or UUID restriction was proposed.

Unit oracles covered observer unions, overlaps, nil and irrelevant UUIDs, threshold/capacity
boundaries, repeated updates and update order. Typed/untyped lifecycle checks applied more
than 1024 irrelevant terminal statuses while preserving live result observation, feedback-only
streams and terminal closure after dropping a result receiver. The candidate passed 16
optimized status tests under each Jazzy RMW, 15/16 under Humble/Kilted, Clippy and full library
checks. These correctness results did not establish performance acceptance.

## Routing-stage measurements

The ignored release benchmark measured snapshot setup plus public-goal/feedback map routing
with identical UUID fixtures. It checked exact routed observer counts; native decode, map
construction and actual terminal channel cleanup were excluded. Five repetitions on CPU 24
ran ordinary routing before filtering. This is supporting stage evidence, separate from
independent-binary lifecycle A/B.

| Statuses | Distinct observers | Map routing | Proposed filtering | Candidate/baseline |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 1 | 26.25 ns | 28.52 ns | 1.086 |
| 16 | 1 | 390.15 ns | 51.66 ns | 0.132 |
| 16 | 4 | 367.90 ns | 172.26 ns | 0.468 |
| 128 | 1 | 2831.45 ns | 166.59 ns | 0.059 |
| 128 | 4 | 3008.11 ns | 606.83 ns | 0.202 |
| 1024 | 1 | 23111.27 ns | 1070.00 ns | 0.046 |
| 1024 | 4 | 22613.04 ns | 3984.79 ns | 0.176 |
| 1024 | 5 | 22565.52 ns | 22535.25 ns | 0.999 |
| 1024 | 16 | 24062.08 ns | 24172.78 ns | 1.005 |

The [stage report](routing-stage.json) stores all 90 records, medians and source/executable
hashes. Large retained batches improved substantially, while a single-status batch had about
2.3 ns additional setup. The stage result alone neither explains the failed lifecycle tail
gate nor establishes acceptable fallback behavior.

## Prototype complete-lifecycle results

The [action protocol](../../../tools/bench_ros/ACTION.md) retained deferred acceptance,
result-observer setup, readiness service, exactly one verified feedback, feedback acknowledgement
or accepted cancellation and terminal result. Exact payloads, endpoint counts and states were
checked in every language direction. All endpoints retained native status defaults, 10-second
results, reliable volatile KeepLast(16), a 100 ms wait cap and one active goal. Consecutive
goals build the retained native cohort; the experiment did not remove feedback or retention.

Baseline `d45e261` and the batch-dispatch prototype used independent Cargo targets with the
same harness and lockfile. Seven pairs covered baseline/baseline, Rust/Rust, C++/C++, both
prototype/C++ directions and both baseline/C++ directions. Each RMW ran five alternating-order
repetitions of 1000 measured goals after 50 warmups. Times are microseconds and medians of
individual-lifecycle repetition percentiles.

| RMW | Scenario | Baseline p50 | Prototype p50 | Baseline p99 | Prototype p99 | p50 decrease |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| CycloneDDS | Success | 621.17 | 598.46 | 1006.37 | 981.08 | 3.7% |
| CycloneDDS | Cancel | 825.43 | 803.93 | 1332.44 | 1297.17 | 2.6% |
| Fast DDS | Success | 325.58 | 301.69 | 471.69 | 428.22 | 7.3% |
| Fast DDS | Cancel | 412.80 | 390.56 | 596.37 | 556.31 | 5.4% |

These prototype reports pass the preselected 1.05 median limits for p50/p95/p99 and inverse
throughput. They represent an experimental binary before the accompanying graph-test update;
they are not measurements of shipped routing or the final rejected executable.

* [CycloneDDS prototype](prototype-cyclonedds.json) and [raw samples](prototype-cyclonedds.samples.json.gz)
* [Fast DDS prototype](prototype-fastdds.json) and [raw samples](prototype-fastdds.samples.json.gz)
* [CycloneDDS prototype short cohort](prototype-short-cyclonedds.json) and [raw samples](prototype-short-cyclonedds.samples.json.gz)
* [Fast DDS prototype short cohort](prototype-short-fastdds.json) and [raw samples](prototype-short-fastdds.samples.json.gz)

Each full prototype report has 70 records and 70,000 RTT samples with acceptance/cancellation
stages. Its short reports have 210 records and 2100 RTT samples each. They pass integrity and
recorded performance checks, but final acceptance still failed as described below.

## Failed short-cohort gates

The fallback boundary used 15 alternating-order repetitions of 10 measured goals after five
warmups, preserving the same protocol/QoS/retention. A process sees at most 15 goals, below the
filter threshold. These short runs must not be compared directly with the 1000-goal cohort.

The first version selected routing mode inside the status loop. Its Fast DDS short success
gate failed with p50 ratio 1.024, p99 ratio 1.190 and inverse-throughput ratio 1.055. The
[report](rejected-inloop-fastdds.json), [samples](rejected-inloop-fastdds.samples.json.gz) and
[exact patch](rejected-inloop-fastdds.patch.gz) are retained. Moving dispatch outside the loop
improved code structure; subsequent prototype reports passed both short gates. This does not
prove the cause of the initial tail failure.

Final library validation exposed an existing graph-count test that queried client/service
counts before asynchronous native cache updates. Its repair waits for a graph change before
retrying and preserves the original count assertions with a two-second deadline. Rebuilding
after that test-only change produced a different executable hash, so final A/B was restarted
against the actual final executable rather than inheriting prototype measurements.

The restarted Fast DDS short cancellation gate failed:

| Metric | Baseline | Final candidate | Candidate/baseline |
| --- | ---: | ---: | ---: |
| p50 | 291.872 μs | 297.121 μs | 1.018 |
| p95 / p99 | 326.232 μs | 355.339 μs | 1.089 |
| Completion rate (inverse ratio) | 3416.43 goals/s | 3319.24 goals/s | 1.029 |

The final [report](rejected-final-fastdds.json), [samples](rejected-final-fastdds.samples.json.gz)
and [patch](rejected-final-fastdds.patch.gz) preserve the failure and exact source hashes. The
patch includes both the runtime experiment and graph-test repair; only the latter is retained
in the working library. Final long-cohort measurements were not rerun after this early failure.
All observer-filtering runtime code, tests and ignored routing benchmark were removed.

Fast DDS short timings vary substantially, and each p99 uses only ten samples. These results
do not identify the cause of failure: scheduling, layout or runtime effects remain uncertain.
That uncertainty does not override the selected gate. Further routing work needs a more stable
short-cohort measurement design, workload-level profiling and multi-goal evidence before a
candidate can be promoted.

## Reproduction and scope

The host was a Core i9-13900K, clients and RMW threads pinned to CPU 24, servers and RMW threads
to CPU 25. ROS 2 Jazzy, Rust 1.85 release, GCC 13.3 `-O3 -DNDEBUG` and default ISA targeting
matched. Unique endpoint names and domains isolated runs. Domains 158/159 cover full prototypes,
156/157 short prototypes and 168 the final rejected Fast DDS run. No task-owned build/test
overlapped measurements. Other shared-host jobs had unrestricted affinity and can contend;
small differences and extrapolation remain limited.

Verify every report's matrix, counts, stage samples, percentiles, rates, medians and gzip hash:

```sh
python3 tools/bench_ros/verify_transport.py \
  docs/benchmarks/action-interest-filter-2026-10-28/prototype-cyclonedds.json \
  docs/benchmarks/action-interest-filter-2026-10-28/prototype-fastdds.json \
  docs/benchmarks/action-interest-filter-2026-10-28/prototype-short-cyclonedds.json \
  docs/benchmarks/action-interest-filter-2026-10-28/prototype-short-fastdds.json \
  docs/benchmarks/action-interest-filter-2026-10-28/rejected-inloop-fastdds.json \
  docs/benchmarks/action-interest-filter-2026-10-28/rejected-final-fastdds.json
```

`--require-performance-pass` is appropriate for the prototype records and must fail for both
rejected reports. Integrity verification alone is not acceptance. Native status allocation,
server retention cost, sustained feedback, concurrent-goal throughput, memory/allocation counts
and multithread scaling remain open work toward the broader goal.

After removing the experiment, all three client source files match `d45e261` byte for byte
and the private observer module is absent. The restored optimized library passes 409 tests
with two manual benchmarks ignored, and workspace unit/integration/doc tests pass. The retained
graph-cache test repair passes independently with both Jazzy RMWs and native Humble/Kilted;
it preserves all existing count assertions and does not add production routing behavior.
