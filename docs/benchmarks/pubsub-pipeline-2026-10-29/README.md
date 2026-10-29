# Scoped native callbacks in pipelined and fanout pub/sub

This slice adds a representative pub/sub workload that the historical stop-and-wait harness did
not cover, then uses it to qualify an opt-in production API. `Node::subscribe_native_callback`
receives a scoped, read-only `NativeMessageRef<'_, T>` view of independently owned native storage during node dispatch.
The owning wrapper and its deallocator cannot escape the callback, so forwarding through
`Publisher::publish_native_ref` avoids the native-to-owned Rust deep
copy and futures-channel send without relying on unsafe cross-take payload reuse.

> **Qualification status:** final-source performance promotion is pending an isolated rerun. The
> first reports passed the numerical gate but their archived candidate and harness manifests differ
> from the final implementation. The current qualification gates additionally reject omitted workspace inputs and invalid
> isolation/performance thresholds. They remain reproducible under
> `rejected/stale-benchmark-source/`, not `performance/`. A later final-source CycloneDDS run was
> invalidated by concurrent Zig compilation and is retained under
> `rejected/contended-final-source/`. Neither evidence set is a promotable final result.

Ordinary `Subscription<T>` streams, raw, untyped and loaned subscriptions are unchanged. The
callback ownership token supports resolved topic, actual QoS, publisher count/wait and immediate
destruction. `NativeCallbackSubscriptionOptions` makes the per-spin throughput/fairness limit,
local-publication filtering and unique-flow request explicit.

## Protocol and oracle

Cases are `elements × window × fanout`. Ping keeps the selected request window in flight. Every
echo subscription validates sequence, layout and every payload value; echo publishes one ordered
response only after all fanout copies arrive. A final empty control must reach every fanout
subscription. Reports verify response/fanout counts, checksums, maximum in-flight messages,
endpoint implementations and `receive_path=native_callback|stream|callback`.

Raw artifacts retain every RTT, ping publish duration, echo fanout-collect duration and echo
publish duration. The formal gate uses complete RTT p50/p95/p99 and inverse throughput. Stage
samples are separately verified diagnostics, not substitutes for the application boundary.

## Pre-final candidate/baseline observation

Independent optimized candidate and `9deaa68` baseline binaries run on CPUs 24/25 with reliable,
volatile `KeepLast(max(16, window))`, localhost DDS and disabled intra-process transport. Each case
has 50 warmups, 1000 measured messages and five interleaved repetitions. The pre-final runs passed
the preselected 1.05 gate in all cases; these numbers must be reproduced by the isolated
final-source rerun before promotion.

| RMW | Case | p50 ratio | p95 ratio | p99 ratio | inverse-throughput ratio |
| --- | --- | ---: | ---: | ---: | ---: |
| CycloneDDS | 0×1×1 | 0.972 | 0.971 | 0.997 | 0.971 |
| CycloneDDS | 4096×16×1 | 0.962 | 0.913 | 0.970 | 0.925 |
| CycloneDDS | 4096×8×4 | 0.909 | 0.938 | 0.953 | 0.900 |
| Fast DDS | 0×1×1 | 0.971 | 0.965 | 1.005 | 0.967 |
| Fast DDS | 4096×16×1 | 0.855 | 0.884 | 0.860 | 0.848 |
| Fast DDS | 4096×8×4 | 0.774 | 0.809 | 0.677 | 0.749 |

The zero-payload rows prove the API does not buy large-payload gains with fixed-cost regressions.
The API remains opt-in because stream ownership and callback-scoped ownership serve different
application contracts.

## Pre-final same-run rclcpp observation

The full matrices use identical payloads, QoS, windows, fanout, validation and CPU affinity in
Rust/Rust, C++/C++ and both mixed directions. Final promotion additionally requires a strict 1.0
same-language gate: Rust/Rust RTT p50/p95/p99 and inverse throughput must be no worse than
C++/C++ for every selected case. Mixed-language rows remain diagnostic rather than gate inputs.

| RMW | Case | r2r p50/p95/p99 | rclcpp p50/p95/p99 | r2r/rclcpp rate |
| --- | --- | --- | --- | ---: |
| CycloneDDS | 0×1×1 | 41/45/50 us | 44/47/50 us | 1.05 |
| CycloneDDS | 4096×16×1 | 837/943/1167 us | 920/1120/1224 us | 1.16 |
| CycloneDDS | 4096×8×4 | 539/630/687 us | 704/875/962 us | 1.32 |
| Fast DDS | 0×1×1 | 48/53/60 us | 54/63/69 us | 1.12 |
| Fast DDS | 4096×16×1 | 616/820/911 us | 714/919/981 us | 1.18 |
| Fast DDS | 4096×8×4 | 501/591/635 us | 719/875/964 us | 1.39 |

The pre-final r2r binary leads every same-language percentile/rate row under both RMWs. This is not
yet final-source evidence. Even after reproduction it remains scoped localhost, single-threaded
evidence, not a networked-host or multithreaded-executor claim.

## Rejected and mixed-language boundaries

The initial 4096×16×4 CycloneDDS A/A has 100–198 ms fanout-collect stalls in both independently
built Rust binaries. Its p99 candidate/baseline ratio is 1.109 and it remains under `rejected/`.
Window four and eight diagnostics show the stable boundary; the qualified fanout case uses window
eight. The threshold was not relaxed.

The CycloneDDS full matrix also contains 155–192 ms p99 stalls in some large-pipeline mixed
Rust/C++ repetitions even though both same-language rows remain stable. Those raw samples remain in
the report; mixed-language saturation needs separate RMW scheduling work before any interoperability
tail claim.

## Verification scope

Real-RCL forwarding and option oracles plus the wake-driven example run under Jazzy/both RMWs,
Humble/Fast DDS and Kilted/both RMWs. Warning-denied all-target Clippy, complete debug/release
suites and a true no-ROS all-target/Rustdoc gate protect the public API. The final performance
archive additionally requires a fresh build manifest covering the benchmark/runtime source set
with zero mismatches; the superseded manifest remains rejected. The final manifest also covers
the complete workspace input set, extracted runtime modules, build scripts and saved bindings.

```sh
python3 tools/bench_ros/verify_pubsub_pipeline.py --require-performance-pass \
  --require-current-sources \
  docs/benchmarks/pubsub-pipeline-2026-10-29/performance/cyclonedds-ab.json \
  docs/benchmarks/pubsub-pipeline-2026-10-29/performance/fastdds-ab.json
python3 tools/bench_ros/verify_pubsub_pipeline.py \
  docs/benchmarks/pubsub-pipeline-2026-10-29/rejected/cyclonedds-window16-fanout4.json
python3 tools/bench_ros/verify_pubsub_pipeline.py --compatibility-manifest \
  docs/benchmarks/pubsub-pipeline-2026-10-29/compatibility.json
```

The commands above apply after `performance/` is repopulated by the isolated final-source run.
Exact report, sample, source, binary and validation identities will be recorded with that run.

## Final ownership and qualification boundaries (2026-10-30)

The callback receive implementation, options and ownership token now form one responsibility
module with nearby native tests. Seven real-middleware cases reject any owned Rust decode, check
exact independent-container destruction after success/error/panic with decreasing dynamic sizes,
preserve an unread sample after allocation failure, bound burst dispatch while an ordinary stream
progresses, and verify capture/publisher-wait cleanup without removing sibling endpoints.
Lifecycle tests check both typed and borrowed-native DDS publication at activation boundaries.

Current compatibility evidence is source-addressed by `compatibility.json`: sixteen source files
and six compressed logs. Jazzy passes 607 main-library cases with eight manual measurements
ignored in the full workspace and both complete optimized RMW suites; all-target Clippy passes.
Humble/Fast DDS and Kilted/both RMWs each pass seven callback and nine lifecycle-publication cases,
the forwarding example and all-target Clippy. The main library now executes Rustdoc examples by
default; all seventeen cases pass, including the two compile-fail ownership boundaries. The true
no-ROS container passes all-target compilation, four snapshot transactions, twelve harness
counterexamples, six common/two generator tests, two ownership doctests and 24 API pages with
54 checked methods. Iron/Lyrical/Rolling still have no new local runtime evidence.

Formal source verification now checks completeness as well as digests: extracted modules, shared
runtime code, generators, build scripts and saved bindings cannot be omitted. The rebuilt
candidate manifest contains 165 workspace inputs, all matching the current checkout. The runner
and verifier enforce at least three idle observations, reject an excessive runtime load even with
an empty violation list, and reject nonfinite thresholds. Performance promotion preserves the
1.05 baseline and 1.0 rclcpp limits. These stronger checks establish qualification boundaries;
they do not make the earlier numerical results current.

The final independent-process protocol smoke covers both Jazzy RMWs, all four language
directions and the same three cases (100 measured messages, ten warmups, one repetition). Each
report verifies twelve records and 1200 RTT/publish/fanout-stage sample tuples against the final
165-input manifest with no omitted or mismatched source. Reports live under `protocol/`; these
short runs on a busy host establish payload/control/ordering behavior, not performance.

A formal five-repetition baseline run with the original 1.05 gate was attempted after the final
build. Its three-idle-sample preflight timed out before measurements: the last observation on
2026-10-29 at 17:05:19 UTC had load1=5.3657 and five active Zig processes. The requirement remained
load1 <= 2 with no compiler/linker process. `logs/final-isolation-preflight.log.gz` retains that
failure. Formal final-source baseline and strict rclcpp matrices remain pending a quiet host.

```sh
python3 tools/bench_ros/verify_pubsub_pipeline.py --require-current-sources \
  docs/benchmarks/pubsub-pipeline-2026-10-29/protocol/cyclonedds.json \
  docs/benchmarks/pubsub-pipeline-2026-10-29/protocol/fastdds.json
python3 tools/bench_ros/verify_pubsub_pipeline.py --compatibility-manifest \
  docs/benchmarks/pubsub-pipeline-2026-10-29/compatibility.json
```
