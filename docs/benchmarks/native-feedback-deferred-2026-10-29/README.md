# Latest-only native decoding for coalesced action feedback

`FeedbackOverflowPolicy::Coalesce` already promises queued FIFO samples plus one
newest pending sample per goal. Its previous ingress path nevertheless converted
every native feedback payload into an owned Rust value before replacing the
pending item. Large intermediate samples therefore paid full clone/JSON costs
even though the policy intentionally never exposed them.

Built-in typed and untyped action clients now retain a native lease as a deferred
pending item while a Coalesce stream lacks capacity. A newer native sample
replaces and retires the older lease without decoding it. Once the sender becomes
writable, the existing readiness queue removes the latest item, decodes exactly
once outside all client/channel/retry locks and delivers the same public owned
feedback type. Default `Backpressure` still decodes immediately and preserves
lossless ordered behavior. Custom unsafe action support whose native UUID
accessor returns the default `None` retains the compatible eager fallback.

`FeedbackChannel` stores either `Ready(T)` or a boxed one-shot deferred decoder.
This adds at most two machine words per tracked goal; on the pinned 64-bit
toolchain the u32 channel grows from 40 to 56 bytes. `PendingFeedbackQueue` keeps
its former size. This bounded metadata cost replaces potentially unbounded work
proportional to every superseded dynamic payload.

## Correctness boundaries

The detached sender is itself an RAII lease. Decode `Err` and panic-unwind both
restore it, while normal send transfers ownership to the existing restoration
path. Deferred replacement and channel closure destroy captured native storage
after releasing channel and retry locks. Closed goals do not decode pending
payloads. A decode UUID mismatch remains an action-client error.

A real untyped RCL oracle uses two concurrent goals and 4096-element Fibonacci
feedback. The blocked goal queues sample 0, receives updates 1 through 8 while
full, and later observes only sample 8. The healthy goal receives sample 99
first. Across those ten native messages, only three payload conversions run:
blocked sample 0, healthy sample 99 and blocked sample 8. The seven superseded
large payloads are never converted to Rust or JSON. CycloneDDS and Fast DDS both
pass. Typed/untyped foreground policy comparisons, background executor transfer,
capacity wakes, exact terminal results and stream closure remain green.

The initial test patch failed to compile because it used named response fields
with the compatibility tuple returned by `send_goal_request()`. The fixed oracle
uses `(goal, result, feedback)`. A later complete feedback run correctly rejected
the former exact channel-size assertion (56 versus 40 bytes for u32); the final
bound explicitly admits one fat pointer. Both failures are retained under
[logs/rejected](logs/rejected/).

## Fixed-CPU large-payload stage

The [stage report](stage/report.json) compares the former eager owned clone with
deferred latest-only decode. Each of five CPU-24 repetitions processes 10000
native arrivals containing 4096 i32 values while the Coalesce channel remains
full. RCL and DDS are excluded; capacity recovery and exact latest-payload checks
run after the measured window.

| Path | Arrival decodes | Allocations | Frees | Requested bytes | Median ns/arrival |
| --- | ---: | ---: | ---: | ---: | ---: |
| Eager owned clone | 10000 | 30000 | 30000 | 165120000 | 845.208 |
| Deferred latest-only | 0 | 20000 | 20000 | 1600000 | 489.608 |

Every repetition removes 10000 allocations/frees and 163.52 MB of allocation
requests. Requested bytes fall about 99.0%; median stage time falls about 42.1%.
The retained latest payload is decoded once after recovery. These figures are a
coalescing/conversion-stage result, not complete application latency.

## Default-path action regression gate

Independent optimized candidate and `42c862b` baseline binaries use identical
confirmed-feedback action inputs, QoS, retention and harness sources. This
ordinary workload uses default `Backpressure`, so it protects the common path
from the new pending enum and policy branch rather than claiming the Coalesce
large-payload gain. Separate processes run on CPUs 24/25 for 1000 measured goals
after 50 warmups, five alternating repetitions. The preselected maximum
candidate/baseline ratio remains 1.05 for median p50/p95/p99 and inverse
throughput.

| RMW | Scenario | p50 ratio | p95 ratio | p99 ratio | Inverse throughput |
| --- | --- | ---: | ---: | ---: | ---: |
| CycloneDDS | Success | 1.006605 | 1.023688 | 1.020646 | 1.010834 |
| CycloneDDS | Cancellation | 1.002084 | 1.007508 | 1.012352 | 1.001396 |
| Fast DDS | Success | 1.002468 | 0.994474 | 0.978490 | 0.999348 |
| Fast DDS | Cancellation | 1.002594 | 1.008727 | 1.026240 | 1.003621 |

Both [CycloneDDS](action/cyclonedds.json) and [Fast DDS](action/fastdds.json)
pass, each with 70 records and 70000 RTT samples. Compressed samples and the
[source/build manifest](action/build-manifest.json) are saved beside them. All
78 candidate source hashes match delivered code. One CycloneDDS repetition has
multi-millisecond tails in both candidate/baseline-related cells and remains in
the raw data. Mixed-direction ratios relative to matching baseline-language
pairs remain within 2.1%; they are outside the gate and this slice adds no new
same-binary calibration. No universal tail or rclcpp leadership claim is made.

## Validation and remaining work

The complete Jazzy workspace passes; its main library has 594 passed and eight
manual measurements ignored, with all integration, package and doc tests green.
Complete optimized main-library suites pass the same 594/8 under both RMWs.
Each RMW passes 63 feedback cases with five manual measurements ignored.
Formatting and warning-denied all-target Clippy pass.

Fresh compatibility targets prove Humble/Fast DDS at 63/5 plus all-target
Clippy, and Kilted/CycloneDDS/Fast DDS at 63/5 each plus all-target Clippy. The
fresh no-ROS target passes four snapshot transaction tests, every workspace
target, six common and two generator tests, Rustdoc, fifteen API pages and
twenty-nine methods. [validation.json](validation.json) retains source, binary,
image, report and compressed-log identities. Iron, Lyrical and Rolling have no
new local runtime evidence.

This closes owned conversion of superseded Coalesce feedback without a new
public API. Feedback that is actually delivered still becomes an owned Rust/JSON
payload. Borrowed delivery, dynamic payload reuse across RMW takes, full
large-payload independent-process latency, burst/fanout and concurrent multi-goal
Rust/rclcpp scaling remain open.

```sh
python3 tools/bench_ros/build.py --workload action --baseline-ref 42c862b \
  --output-dir target/native-feedback-deferred-ab
python3 tools/bench_ros/verify_transport.py --require-performance-pass \
  docs/benchmarks/native-feedback-deferred-2026-10-29/action/cyclonedds.json \
  docs/benchmarks/native-feedback-deferred-2026-10-29/action/fastdds.json
```
