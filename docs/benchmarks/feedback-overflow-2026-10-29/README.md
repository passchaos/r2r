# Goal-specific feedback coalescing evidence

`FeedbackOverflowPolicy::Coalesce` retains already-buffered feedback in FIFO
order and one latest pending item per goal, while native feedback ingress
continues for other goals. `Backpressure` remains the default. The native
counterexample previously timed out waiting for a second goal while the first
stream was full. The final typed/untyped tests verify both policies against
identical thirty-two-sample input, exact terminal results and stream closure.
[Feedback contracts and the complete application](../../ACTION_FEEDBACK_PROGRESS.md)
describe selection, bounded memory, native QoS and observation lifetimes.

## Default-path action A/B

Independent optimized baseline `b7af300` and candidate Rust binaries use the
same confirmed-feedback action harness. The unchanged 1.05 regression gate
covers success and cancellation p50/p95/p99 and inverse throughput. Both
Jazzy RMWs pass, with seventy records and 70000 raw RTT samples each. All seven
language pairs and native goal/feedback/result/cancel/control oracles remain
required. This measures the default Backpressure path. The explicitly different
sample-retention semantics of Coalesce are covered by functional oracles.

Ratios are candidate/baseline; inverse throughput uses baseline rate/candidate
rate. Values are based on medians of five repetition percentiles/rates.

| RMW | Scenario | p50 | p95 | p99 | Inverse throughput |
| --- | --- | ---: | ---: | ---: | ---: |
| CycloneDDS | Success | 0.994420 | 0.984562 | 0.972070 | 0.993352 |
| CycloneDDS | Cancellation | 1.010393 | 1.005308 | 0.987048 | 1.008277 |
| Fast DDS | Success | 0.985042 | 0.989212 | 0.963626 | 0.994128 |
| Fast DDS | Cancellation | 0.995651 | 1.000018 | 0.974586 | 1.002499 |

* [CycloneDDS report](action/cyclonedds.json) and [raw RTT samples](action/cyclonedds.samples.json.gz).
* [Fast DDS report](action/fastdds.json) and [raw RTT samples](action/fastdds.samples.json.gz).
* [Source/build manifest](action/build-manifest.json).

Rust/C++ and C++/Rust p99 controls are within 5% of their respective matching
baseline-language pairs in both scenarios/RMWs. These controls are outside the
preselected same-language gate. No new same-binary calibration is asserted, and
earlier short-cohort tail drift remains relevant. Passing these gates does not
establish universally calibrated tail gains or C++ performance leadership.

The host is an i9-13900K with native Jazzy, Rust 1.85 release and GCC 13.3
`-O3 -DNDEBUG`, using default ISA targeting. Independent ping/echo processes run
on CPUs 24/25, domains 180/182, one goal in flight, fifty warmups and 1000 goals
per repetition. Native goal/result/cancel/feedback QoS and ten-second retention
remain identical. Runs alternate language order; this task's builds/tests do
not overlap formal measurement. Other shared-host activity remains a limitation.

```sh
python3 tools/bench_ros/build.py --workload action --baseline-ref b7af300 \
  --output-dir target/feedback-overflow-ab
python3 tools/bench_ros/verify_transport.py --require-performance-pass \
  docs/benchmarks/feedback-overflow-2026-10-29/action/cyclonedds.json \
  docs/benchmarks/feedback-overflow-2026-10-29/action/fastdds.json
```

## Local completion stage

The [stage report](stage/report.json) times only optimized library-test feedback
completions with u32 payloads on CPU 24. Five runs each perform two million
normal send/receive operations and 1999999 pending-item replacements, with
checksums, exact retained items and successful capacity recovery. It excludes
native ingress, conversion, DDS, complete application throughput and tail
distributions. Normal delivery costs about 145 ns/call; replacing pending items
costs about 41 ns/call in this local fixture. These operations have different
semantics and are characterization data rather than an improvement ratio.
The queue is 96 bytes and u32 feedback channel 40 bytes, matching legacy layout
oracles for this toolchain. Typed-vector and untyped JSON channel shapes are
checked separately. The initial parser attempt missed the test harness's inline
JSON prefix; its successful raw stage output is retained as a diagnostic.

## Correctness boundaries

Nearby oracles cover 4096 replacements with capacities 0/1/8, original queued
prefixes, exact newest pending values, detached success/failure with reentrant
arrival, no payload destruction under client/channel/readiness locks, closed
channels, stale weak capacity notifications and packed count limits. A policy
bit uses existing count storage; channel policy fits existing padding. Native
passes stay bounded to sixteen feedback candidates, and blocked coalescing
retries do not prevent reading the shared native topic. Delivery construction
keeps the client lock short and runs channel work in the subsequent completion.

Foreground typed/untyped cases compare both policies. Background typed/untyped
cases receive the healthy goal first, stop/unbind the node, consume the queued
prefix while unbound, verify the old guard remains inactive, transfer ownership
and receive the latest pending sample without another ROS message. Both exact
result payloads and feedback stream closure remain required. The complete public
`action_feedback_coalesce` application demonstrates the same two-goal workflow.

Jazzy workspace and both complete optimized library suites pass 574 cases with
four manual benchmarks ignored. Both RMWs pass the 43 relevant feedback cases,
and the complete optimized application runs under each. Formatting and all-target
Clippy pass. Native Humble/Fast DDS and Kilted/CycloneDDS/Fast DDS each pass the
same 43 feedback cases with one manual benchmark ignored and run the complete
optimized application. Their all-target Clippy gates pass. The no-ROS container
compiles every target, passes snapshot/generator checks and verifies fifteen API
artifacts and twenty-nine methods, including the new policy enum and setter.
[Final validation and source/archive identities](validation.json) retain exact
counts and compressed log hashes. Iron, Lyrical and Rolling have no new local
runtime evidence for this slice; all raw logs remain under `logs/`.

Coalescing continues to decode each native sample before retention. Deferred
native storage, allocation accounting, larger payloads, fanout and complete
multi-goal Rust/C++ performance comparisons remain additional work toward the
full language/usability/capability/performance goal.
