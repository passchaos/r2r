# Native message reuse: Rust/rclcpp comparison

The final candidate reduces repeated Rust-to-native message copy time by 84–85% for the small
fixtures and 35–37% for the 4096-integer fixture compared with `a6de146`. Small-fixture copy time
is 5–8% below rclcpp in these runs; the large fixture is close to rclcpp. This is a measured
message-stage improvement, not a claim of end-to-end ROS performance leadership.

## Workload and environment

The [harness](../../tools/bench_ros/README.md) uses one `Int32MultiArray` dimension labelled
`samples`, matching size/stride, and 0, 32 or 4096 deterministic signed integers. Independent
Rust baseline, Rust candidate and rclcpp binaries validate the complete message and identical
CDR bytes. Serialization returns freshly owned bytes from a reused middleware buffer;
deserialization creates a freshly owned message. Copying uses an initialized destination in
both languages.

Runs use ROS 2 Jazzy, Rust 1.85 release optimization, GCC 13.3 with `-O3 -DNDEBUG`, and a Core
i9-13900K pinned to CPU 30. Each stage/input pair uses the same calibrated iteration count for
all three binaries, a minimum run target of 200 ms, and five repetitions with alternating
order. Compilation and tests are finished before measurement. Raw reports record package
versions, binary/source hashes, build settings, every measurement and medians:

* [CycloneDDS raw report](native-message-reuse-2026-10-27-cyclonedds.json)
* [Fast DDS raw report](native-message-reuse-2026-10-27-fastdds.json)

## Median copy time

Time is nanoseconds per operation; lower is better.

| RMW | Integers | Rust baseline | Rust candidate | rclcpp | Reduction vs baseline |
| --- | ---: | ---: | ---: | ---: | ---: |
| CycloneDDS | 0 | 79.86 | 11.97 | 12.72 | 85.0% |
| CycloneDDS | 32 | 90.62 | 14.41 | 15.21 | 84.1% |
| CycloneDDS | 4096 | 467.36 | 294.17 | 287.10 | 37.1% |
| Fast DDS | 0 | 76.86 | 11.91 | 12.90 | 84.5% |
| Fast DDS | 32 | 91.21 | 14.43 | 15.68 | 84.2% |
| Fast DDS | 4096 | 458.20 | 295.47 | 293.21 | 35.5% |

Both RMW runs pass the candidate/baseline 1.05 median regression limit across all measured
stages. CycloneDDS codec medians stay within approximately 1% of baseline. Fast DDS has up to
4.2% more deserialization time for the empty fixture and 5.2% less serialization time for that
fixture. This bounds the observed change; it does not establish that unaffected codec paths
became faster or slower on all systems.

## Change and correctness evidence

Primitive sequences reuse capacity instead of finalizing and initializing it every update.
Growth allocates replacement storage before discarding the old sequence, preserving previous
values if allocation fails. Generated nested-message sequences reuse initialized capacity and
retain hidden elements after logical shrinking; native finalizers still destroy capacity
elements. String assignment copies directly into sufficient capacity, including embedded NUL
bytes and the final terminator, and otherwise uses the native allocation path.

Tests cover all primitive element kinds, empty/shrinking/regrowing sequences, failed growth,
nested trajectory payloads, string content equivalence against native `assignn`, and retained
pointer/capacity invariants. The public API and ROS wire format remain consistent. Native
capacity can remain above logical length until destruction, matching vector reuse; callers
that must release a high-water allocation can replace the native message.

The runnable `native_message_reuse` example sends 4096 → 1024 → 0 → 4096 elements through
`publish_native` and verifies received payloads under both RMWs. The native capacity remains
4096. Workspace and Clippy verification use Jazzy plus containerized Humble/Kilted; the
benchmark comparison itself is Jazzy-only.

## Limits and next work

Batch p50/p95/p99 fields average up to 1000 operations and are not individual-message latency
percentiles. Pointer-change observations are not allocation counters. No target-native ISA,
custom allocator, executor, DDS transport or intra-process optimization is involved. Full
pub/sub, service/action throughput and latency, allocation accounting, thread scaling and
cross-hardware evidence remain required for the broader Rust/C++ goal.
