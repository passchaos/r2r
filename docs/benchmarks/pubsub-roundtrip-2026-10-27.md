# Independent-process Rust/rclcpp pub/sub roundtrip

The first end-to-end pub/sub comparison passes complete payload and sequence validation in all
four language combinations under CycloneDDS and Fast DDS. Rust/Rust small-message RTT medians
are 5–6% below C++/C++ in this workload. At 4096 payload values, medians are within 1–2% and
Rust/Rust p99 is up to 3% higher. These results establish an interoperability and performance
baseline; they do not prove overall Rust/C++ superiority.

## Workload

Two separate processes exchange `std_msgs/msg/UInt64MultiArray` through localhost DDS. One
sequence value and 0, 32 or 4096 deterministic payload values form each request. The responder
validates every value, layout and sequence before typed publication of the echo. The initiator
validates the complete response before sending the next request. Each pair requires exactly one
matched peer in each direction, receives every sequence once, and exchanges an explicit completion
message before exit. Five repetitions of 5000 measured roundtrips plus 100 warmup roundtrips
cover Rust/Rust, C++/C++, Rust ping/C++ echo and C++ ping/Rust echo.

Both applications use reliable volatile KeepLast(16), single-threaded application execution,
a 100 ms blocking spin wait cap, no rosout or parameter endpoints, and no intra-process transport.
Rust uses normal typed r2r messages and streams; C++ uses normal typed rclcpp publishers and
subscription callbacks. One request is in flight, so the rate is **stop-and-wait roundtrips per
second**, not saturation throughput. RTT starts before request publication and ends after response
retrieval, before initiator validation; responder validation remains inside RTT.

The host is an Intel Core i9-13900K, with ping pinned to CPU 30 and echo to CPU 31. ROS 2 Jazzy,
Rust 1.85 release, and GCC 13.3 `-O3 -DNDEBUG` are used. The binaries run with no competing
build/test tasks, independent DDS domains and unique topics. Pair order reverses on alternating
repetitions. Exact package versions, binary/source hashes and transport settings are recorded in
the reports. CPU affinity applies to RMW threads created by each process too.

## Same-language median results

The table reports the median of each repetition's individual-operation p50/p99. Latency is in
microseconds; lower is better. Each report retains all four combinations, every endpoint count
and checksum, and the full raw RTT samples.

| RMW | Extra payload values | Rust p50 | C++ p50 | Rust p99 | C++ p99 | Rust roundtrips/s | C++ roundtrips/s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| CycloneDDS | 0 | 41.02 | 43.69 | 50.36 | 53.12 | 23,911 | 22,628 |
| CycloneDDS | 32 | 41.78 | 44.41 | 49.62 | 52.03 | 23,557 | 22,298 |
| CycloneDDS | 4096 | 85.27 | 83.53 | 105.10 | 103.89 | 11,176 | 11,417 |
| Fast DDS | 0 | 48.64 | 51.09 | 69.55 | 69.33 | 19,749 | 18,948 |
| Fast DDS | 32 | 48.80 | 51.84 | 65.37 | 64.02 | 19,722 | 18,877 |
| Fast DDS | 4096 | 73.44 | 72.71 | 93.16 | 90.44 | 12,677 | 12,916 |

Rust/C++ directions remain close to the same-language combinations and pass the same strict
oracle. Large payload and Fast DDS tail latency still leave measurable gaps. Optimizations
should next distinguish native copying/allocation, scheduling, queueing and transport costs,
and extend beyond one-message-in-flight workloads.

## Evidence and reproduction

* [CycloneDDS report](pubsub-roundtrip-2026-10-27-cyclonedds.json) and [raw RTT samples](pubsub-roundtrip-2026-10-27-cyclonedds.samples.json.gz)
* [Fast DDS report](pubsub-roundtrip-2026-10-27-fastdds.json) and [raw RTT samples](pubsub-roundtrip-2026-10-27-fastdds.samples.json.gz)
* [Build, run and verify instructions](../../tools/bench_ros/TRANSPORT.md)

Each RMW report contains 60 validated endpoint-pair records and 300,000 RTT samples. The offline
verifier checks the gzip digest, matrix completeness, counts, checksums, implementation identity,
every percentile, throughput calculation and summary median. Missing combinations, duplicate
records or corrupted artifacts fail verification.

The release probe initially exposed a [runtime registration failure](../RELEASE_VALIDATION.md)
where state mutations were hidden inside `debug_assert!`. This was fixed and checkpointed in
`0921b59`. All measurements here use the corrected library. Full optimized Jazzy library tests,
Humble/Kilted optimized lifecycle gates, debug scoped tests, Clippy and doc-only builds validate
the change; no before/after latency claim is made against the incorrect release behavior.

This matrix does not cover pipelining, fanout, same-process native/loaned paths, multithreaded
executors, service or action workflows, allocation counts, other hardware, or cross-RMW transport.
Those remain necessary for the broader usability, functionality and performance goal.
