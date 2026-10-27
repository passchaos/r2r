# Rust/rclcpp native service roundtrip

The first service matrix validates real `AddTwoInts` requests/responses in all four language
combinations with CycloneDDS and Fast DDS. In this fixed-size workload, Rust/Rust single-request
RTT medians are about 6–7% below C++/C++. With 16 requests in flight, Rust/Rust completes about
22% more requests per second under CycloneDDS and 45% more under Fast DDS. This is scoped service
evidence; it is not a claim of overall ROS language performance leadership.

## Workload and fairness

The [service harness](../../tools/bench_ros/SERVICE.md) uses separate client/server processes,
single-threaded application execution, a 100 ms spin wait cap, and matching reliable volatile
ServicesQoS with KeepLast(max(16, window)). There are no rosout, parameter service/event or
intra-process endpoints. Every request has a monotonically increasing sequence as operand `a`
and a deterministic signed operand `b`; the server verifies both and returns the exact sum.
The client validates every response and count. A stop request/response and final confirmation
establish complete delivery before exit.

Warmup consists of 100 fully drained requests. Each of five alternating-order repetitions then
completes 5000 measured requests. Window 1 permits one request in flight; window 16 permits at
most 16. Both clients observe responses in submission order, so per-request RTT includes queueing
within the window. Throughput includes submission, allocation and client sum validation; RTT
starts before the async request call and ends on response retrieval, before client validation.
Server validation remains within RTT. Larger-window results are pipeline/queueing measurements,
not stop-and-wait latency.

Rust uses `Client::request` futures and `Service` streams; C++ uses `async_send_request` futures
and a normal service callback. Framework scheduling and batching remain part of the public API
comparison. The runtime implementation is unchanged in this slice. Required per-language message
construction, conversion, serialization and response completion costs are included at their
documented boundaries.

## Same-language median results

Times are microseconds and are medians of each repetition's individual RTT percentiles. Rates
are completed requests per second, not publications or request submissions.

| RMW | In flight | Rust p50 | C++ p50 | Rust p99 | C++ p99 | Rust requests/s | C++ requests/s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| CycloneDDS | 1 | 43.61 | 46.52 | 51.97 | 54.38 | 22,709 | 21,286 |
| CycloneDDS | 16 | 300.35 | 370.83 | 369.29 | 440.62 | 53,154 | 43,454 |
| Fast DDS | 1 | 47.85 | 51.29 | 63.71 | 65.95 | 20,236 | 19,072 |
| Fast DDS | 16 | 281.74 | 423.40 | 564.38 | 674.28 | 52,730 | 36,307 |

Both Rust/C++ directions pass the same strict sequence, operand and sum oracle. Their individual
results are retained, allowing client and server contributions to be compared separately. For
example, the CycloneDDS 16-window C++ client/Rust server pair completes about 50,799 requests/s,
while Rust client/C++ server completes about 44,123. The mixed results support investigating
server/executor batching before choosing further service optimizations.

## Environment and reproducibility

The host is a Core i9-13900K. Client and server processes, including their RMW threads, are
pinned to CPU 24 and CPU 25. ROS 2 Jazzy, Rust 1.85 release and GCC 13.3 `-O3 -DNDEBUG` use
default ISA targeting. Each run uses an isolated DDS domain and unique service name. No
task-owned build/test overlaps performance measurement; unrelated shared-host activity on other
CPUs remains a limitation. Binary, source, compiler, package and environment records accompany
every report.

* [CycloneDDS report](service-roundtrip-2026-10-28-cyclonedds.json) and [raw RTT samples](service-roundtrip-2026-10-28-cyclonedds.samples.json.gz)
* [Fast DDS report](service-roundtrip-2026-10-28-fastdds.json) and [raw RTT samples](service-roundtrip-2026-10-28-fastdds.samples.json.gz)
* [Build/run instructions and ownership boundaries](../../tools/bench_ros/SERVICE.md)
* [Bounded-concurrency application example](../../r2r/examples/service_pipeline.rs)

Each RMW report has 40 endpoint-pair records and 200,000 raw RTT samples. The shared offline
validator checks window/language/repetition completeness, every endpoint count/checksum, sample
percentiles, throughput calculations, gzip hashes and summary medians:

```sh
python3 tools/bench_ros/verify_transport.py \
  docs/benchmarks/service-roundtrip-2026-10-28-cyclonedds.json \
  docs/benchmarks/service-roundtrip-2026-10-28-fastdds.json
```

The C++ source gained a pre-Jazzy QoS-overload guard after the performance runs. Rebuilding its
Jazzy branch produced a byte-identical measured executable; reports retain both original source
hashes and that compatibility rebuild check. Humble and Kilted separately build Rust/C++ binaries
and pass the four-combination window-1/window-16 smoke matrix under their native Fast DDS.
The 64-request `buffer_unordered(16)` application example runs under both local RMWs and in both
containers. These are compatibility/correctness checks, not cross-distribution performance claims.

## Remaining scope

This establishes current-library performance and interoperability, not a before/after runtime
optimization or a recorded regression pass. Add large/dynamic service payloads, timeout/cancel
workloads, broader concurrency, multi-thread scaling, memory/allocation accounting and action
protocols before claiming comprehensive superiority. The existing regression baseline support
remains available for future implementation changes.
