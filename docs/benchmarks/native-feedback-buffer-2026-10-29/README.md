# Reusable native action-feedback container

Typed and untyped action clients now retain one private native feedback message
container. `rcl_action_take_feedback` checks it out, and a RAII lease returns it
after the post-unlock completion finishes. The next take therefore avoids
creating and destroying the outer ROS message on every feedback sample.

The one-slot pool is deliberately private and bounded. Creating a missing
container and destroying displaced storage both happen outside the pool lock.
The lease returns storage on unobserved, closed-channel, successful decode,
decode-error and UUID-mismatch paths. It owns an `Arc` to the slot, so destroying
the action client while a completion exists cannot invalidate the native
message. The existing feedback batch executes each completion before its next
take, making one retained slot sufficient for the normal path. A concurrent
checkout can temporarily create another container; later return keeps one and
retires the other after unlocking.

Untyped feedback conversion now borrows the native message rather than
consuming it. `WrappedNativeMsgUntyped` is `Send` under the existing unsafe
`WrappedTypesupport` contract that already requires owned native allocations to
be movable between threads; the wrapper continues to retain unique ownership
and its matching destructor.

## Correctness and the rejected nested-buffer assumption

Nearby tests prove that creation and retirement are lock-free with destructor
reentry, the same container is checked out again, factory failure does not
invent a cached value, and decode errors return storage without changing the
feedback channel. Existing identity, closure, fallback and direct-delivery
oracles remain applicable.

The real untyped RCL test publishes feedback sequence lengths 8192, 4096, 2048,
1024, 512, 64, 1 and 0. Under both Jazzy RMWs it takes all eight packets, returns
the lease after each completion and observes the same outer native-message
address on every take. The no-observer path still performs zero payload decodes,
the legacy-accessor fallback still decodes eight times, and a separately
observed goal still receives feedback.

The initial oracle also required the nested `sequence.data` address to remain
constant. Both CycloneDDS and Fast DDS rejected that assumption even though the
outer message was reused. Those failures are retained under
[logs/rejected](logs/rejected/). The promoted contract therefore promises only
outer-container reuse. RCL/RMW may replace, resize or reallocate dynamic payload
storage during each take; no payload-capacity reuse claim is made.

## Fixed-CPU outer-container stage

The [stage report](stage/report.json) contains five 100000-message repetitions
for create/destroy and one-slot reuse, pinned to CPU 24. It uses an empty
Fibonacci feedback message and excludes RCL, DDS and payload conversion.

| Path | Allocations/100000 | Frees/100000 | Requested bytes | Median ns/message |
| --- | ---: | ---: | ---: | ---: |
| Create/destroy | 300000 | 300000 | 20800000 | 208.953 |
| One-slot reuse | 200000 | 200000 | 16800000 | 141.841 |

Every repetition removes exactly one allocation and one free per message at
this boundary. Absolute counts include completion/pool test machinery; the
asserted allocation/free delta is the stable result.

## Complete lifecycle allocation evidence

Instrumented independent-process matrices count process-wide allocation entry
points in the complete confirmed-feedback action lifecycle. Each cell contains
1000 measured goals after 50 warmups; values below are medians of five
baseline/baseline and candidate/candidate repetitions. Client is the ping side;
the echo server uses unchanged production code.

| RMW | Scenario | Client allocations baseline -> candidate | Client frees baseline -> candidate | Requested-byte delta |
| --- | --- | ---: | ---: | ---: |
| CycloneDDS | Success | 84038 -> 82032 | 84038 -> 81032 | -79231 |
| CycloneDDS | Cancellation | 96807 -> 94822 | 96807 -> 93822 | -71765 |
| Fast DDS | Success | 141542 -> 139057 | 140543 -> 138058 | -40526 |
| Fast DDS | Cancellation | 158912 -> 156756 | 157912 -> 155756 | -70641 |

The maximum server-side allocation/free median difference is 344 entry points,
versus 1985-2485 fewer client allocations. RMW internals and generated C message
initialization make complete-process deltas implementation dependent; they are
reported as observed totals, not a fixed per-message ABI promise. The
[CycloneDDS](resource/cyclonedds.json) and [Fast DDS](resource/fastdds.json)
reports each retain 70 records, 70000 RTT samples, full endpoint oracles and the
probe counts. Instrumented timing is characterization only and cannot satisfy a
latency gate.

## Ordinary action regression gate

Independent optimized candidate and `3f877ec` baseline binaries use identical
confirmed-feedback inputs, QoS, retention and harness sources. Separate ping
and echo processes run on CPUs 24/25 for 1000 measured goals after 50 warmups,
five alternating repetitions, success and cancellation. The preselected maximum
candidate/baseline ratio remains 1.05 for median p50/p95/p99 and inverse
throughput.

| RMW | Scenario | p50 ratio | p95 ratio | p99 ratio | Inverse throughput |
| --- | --- | ---: | ---: | ---: | ---: |
| CycloneDDS | Success | 1.005926 | 0.995108 | 0.994971 | 0.999687 |
| CycloneDDS | Cancellation | 0.998664 | 0.997293 | 0.999014 | 1.002425 |
| Fast DDS | Success | 0.997520 | 0.983413 | 1.015166 | 0.998193 |
| Fast DDS | Cancellation | 0.996738 | 0.992078 | 0.987493 | 1.003388 |

Both [CycloneDDS](action/cyclonedds.json) and [Fast DDS](action/fastdds.json)
pass, each with 70 records and 70000 RTT samples. Compressed samples and the
[source/build manifest](action/build-manifest.json) are saved beside them. All
78 candidate source hashes match the delivered code. Mixed-direction ratios
relative to matching baseline-language pairs remain within 1.36%; they are
outside the preselected gate and this slice adds no same-binary A/A calibration.
No general rclcpp leadership or universal tail improvement is claimed.

## Validation and remaining boundary

The complete Jazzy workspace passes; its main library has 589 passed and seven
ignored tests, with all integration, package and doc tests also green. Complete
optimized main-library suites pass the same 589/7 under CycloneDDS and Fast DDS.
Each RMW executes 58 feedback cases with four manual measurements ignored.
Formatting and warning-denied all-target Clippy pass.

Fresh compatibility targets prove Humble/Fast DDS at 58/4 plus all-target
Clippy, and Kilted/CycloneDDS/Fast DDS at 58/4 each plus all-target Clippy. The
fresh no-ROS target passes four snapshot transaction tests, every workspace
target, six common and two generator tests, Rustdoc, fifteen API pages and
twenty-nine methods. [validation.json](validation.json) retains exact source,
binary, image, report and compressed-log identities. Iron, Lyrical and Rolling
have no new local runtime evidence for this slice.

This closes repeated outer feedback-message creation/destruction without
changing public APIs or retention semantics. Dynamic sequence/string storage
reuse across RMW takes, borrowed payload delivery, larger/bursty feedback,
concurrent multi-goal scaling and calibrated Rust/rclcpp application leadership
remain open.

```sh
python3 tools/bench_ros/build.py --workload action --baseline-ref 3f877ec \
  --output-dir target/native-feedback-buffer-final
python3 tools/bench_ros/verify_transport.py --require-performance-pass \
  docs/benchmarks/native-feedback-buffer-2026-10-29/action/cyclonedds.json \
  docs/benchmarks/native-feedback-buffer-2026-10-29/action/fastdds.json
```
