# Native action status decoding and retained-goal tracking

Direct native status decoding and fewer redundant UUID lookups reduce complete Rust action
lifecycle p50 by 3.4–4.8% under CycloneDDS and 22.4% under Fast DDS versus `441bcd4`.
Fast DDS p99 decreases by 28.4% for success and 37.6% for cancellation; completion rate increases
by 30–32%. Both RMWs pass the preselected 1.05 median regression limit for p50/p95/p99 and
inverse throughput. Fast DDS success is now close to rclcpp in this workload. Cancellation
and CycloneDDS still have measurable gaps, so this does not establish general action leadership.

## Change and correctness boundaries

The old client converted the whole native `GoalStatusArray` to an owned Rust message, including
one UUID byte vector per retained goal, then constructed UUID/status updates. The new private
[status module](../../r2r/src/action_clients/status.rs) reads initialized native records directly
and allocates only the update vector. Native middleware allocation and finalization still occur.
The decoder checks null storage, size/capacity and `isize::MAX` slice bounds before access.
Empty arrays remain valid, and unknown wire statuses retain the existing Unknown mapping.

Typed and untyped clients update only existing public goal entries. Result-map lookup and
feedback removal are limited to terminal goals with a feedback observer. Feedback/result
observers can outlive the public goal handle: a live result observer retains feedback, while
feedback-only observation or a dropped result receiver closes at terminal status. Notifications
and native destruction still occur outside the client lock. The owned-message decoder remains
a test oracle; no public API or ROS wire format changes.

## Decode-stage evidence

An optimized ignored unit benchmark uses the same native fixture for full owned-message
conversion and direct decoding. Both paths produce identical UUID/status updates. Five runs
on CPU 24 consume and destroy each output; fixture setup is excluded. The owned path runs
before the native path within each repetition, so this is supporting stage evidence, separate
from the independent-binary transport A/B below.

| Retained statuses | Owned Rust conversion and decode | Direct native decode | Less time |
| ---: | ---: | ---: | ---: |
| 1 | 48.88 ns | 14.82 ns | 69.7% |
| 128 | 4,782.80 ns | 104.61 ns | 97.8% |
| 1024 | 37,687.44 ns | 709.11 ns | 98.1% |

The [stage report](action-status-decode-2026-10-28-stage.json) records iteration counts, all
repetitions, medians, compiler and executable/source hashes. These figures include allocation
and destruction time, but do not constitute measured allocation counts. Baseline profiling
of Rust client/C++ server success sampled allocator and UUID hashing functions, motivating
this slice; the 535-sample profile is directional evidence, not precise cost attribution.

## Complete lifecycle A/B

The [existing action protocol](../../tools/bench_ros/ACTION.md) is unchanged: deferred acceptance,
result-observer setup, readiness service, exactly one verified feedback, feedback acknowledgement
or cancellation, and terminal result. The exact feedback/result sequence, terminal status and
endpoint counts are checked in every language direction. Reliable volatile KeepLast(16), native
status defaults, the 100 ms wait cap, one active goal and 10-second result retention all remain
equal. Consecutive goals build the retained status cohort; neither feedback nor retention is
removed to make this optimization look better.

Each RMW runs five repetitions of 1000 measured goals after 50 warmup goals for both success
and cancellation. Seven pairs cover baseline/baseline, Rust/Rust, C++/C++, both candidate/C++
directions and both baseline/C++ directions; pair order reverses on alternating repetitions.
Baseline `441bcd4` and candidate use separate Cargo targets, identical current harness source
and lockfile. Source hashes include the new native status module and untyped client.

Times below are microseconds and medians of repetition percentiles, not percentiles of batch
averages. Rates count complete validated lifecycles per second.

| RMW | Scenario | Baseline p50 | Candidate p50 | C++ p50 | Baseline p99 | Candidate p99 | C++ p99 | Baseline goals/s | Candidate goals/s | C++ goals/s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| CycloneDDS | Success | 648.79 | 617.88 | 600.12 | 1055.16 | 1006.26 | 972.92 | 1584 | 1647 | 1648 |
| CycloneDDS | Cancel | 850.14 | 821.16 | 748.78 | 1375.34 | 1322.89 | 1242.93 | 1199 | 1247 | 1328 |
| Fast DDS | Success | 415.26 | 322.18 | 324.43 | 650.92 | 466.00 | 482.70 | 2341 | 3043 | 3015 |
| Fast DDS | Cancel | 532.91 | 413.67 | 396.88 | 949.68 | 592.36 | 612.23 | 1798 | 2376 | 2488 |

Relative to same-run C++ p50, CycloneDDS success/cancellation remain 3.0%/9.7% higher;
Fast DDS success is 0.7% lower and cancellation 4.2% higher. Treat the small success difference
as near parity, not broad superiority. Mixed-pair and acceptance/cancellation-stage results
remain in the reports. The earlier [action matrix](action-lifecycle-2026-10-28.md) is historical
evidence; before/after claims here use the interleaved baseline from this run.

## Environment, artifacts and gates

The host is a Core i9-13900K, with clients pinned to CPU 24 and servers to CPU 25, including
middleware threads. ROS 2 Jazzy, Rust 1.85 release and GCC 13.3 `-O3 -DNDEBUG` use default ISA
targeting. DDS domains 141/142 and unique action/service names isolate runs. No task-owned build
or test overlaps measurement. Other shared-host jobs had unrestricted CPU affinity and may
contend with these CPUs; this limits extrapolation and small differences in particular.

* [CycloneDDS A/B](action-status-decode-2026-10-28-cyclonedds.json) and [raw samples](action-status-decode-2026-10-28-cyclonedds.samples.json.gz)
* [Fast DDS A/B](action-status-decode-2026-10-28-fastdds.json) and [raw samples](action-status-decode-2026-10-28-fastdds.samples.json.gz)
* [Decode-stage report](action-status-decode-2026-10-28-stage.json)

Each A/B report contains 70 endpoint-pair records and 70,000 RTT samples, with matching
acceptance/cancellation stage samples. The offline verifier checks complete matrices, counts,
payload identities, every percentile/rate/median, compressed sample hashes and the passing gate:

```sh
python3 tools/bench_ros/verify_transport.py --require-performance-pass \
  docs/benchmarks/action-status-decode-2026-10-28-cyclonedds.json \
  docs/benchmarks/action-status-decode-2026-10-28-fastdds.json
```

Decode oracles cover 0, 1, 9, 128 and 1024 statuses, all seven defined wire states and unknown
values; invalid native bounds are rejected before access. Typed/untyped tests cover observer
lifetime and terminal cleanup. Optimized Jazzy status checks execute 14 tests under each RMW,
and the complete optimized library passes 409 tests with two manual benchmarks ignored.
Native Humble/Kilted containers pass 13/14 optimized status tests respectively, covering the
fixed UUID layout and observer boundaries. Workspace unit/integration/doc tests, all-target
Clippy, doc-only workspace compilation and formatting checks pass; the benchmark harness passes
24 unit tests. Distribution
performance remains Jazzy-only; Iron, Lyrical and Rolling retain CI coverage without new local
runtime measurements in this slice.

Multi-goal concurrency, sustained feedback streams, memory/allocation counts, networked hosts,
other hardware and multithread scaling remain open. Native middleware status allocation and
retained-result storage are still part of the path; the private decoder can evolve independently
without making full owned-message conversion a permanent boundary.
