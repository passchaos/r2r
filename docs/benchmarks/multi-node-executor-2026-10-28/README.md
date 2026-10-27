# Shared native multi-node executor performance

The new `Executor` replaces historical multi-node nonblocking polling with one reusable native
RCL wait. Both Jazzy RMWs pass the preselected 1.05 regression limits for request p50/p95/p99,
inverse throughput and process CPU/request, including the single-node control. In the eight-node,
16-request-window application Rust completes about 103k requests/s with CycloneDDS and 69k
with Fast DDS; same-run rclcpp completes about 97k and 58k. These are scoped application results,
not a claim of general ROS performance leadership.

## Application and boundaries

Each process creates one, two or eight nodes. Every node owns an `AddTwoInts` service, and
node zero owns a client for each service. Requests are routed by `sequence % nodes`; each
service validates its complete ordered sequence and operand, and each response validates the
exact sum. One or sixteen requests stay in flight. Warmup finishes before measurement starts,
all measured responses finish before teardown, and aggregate service counts/checksums cover
warmup plus every measured request.

The historical Rust library is `cdccecd`. For one node its unchanged public native future
execution drives the application. For multiple nodes its fastest single-threaded public path
polls every node with a zero native timeout and polls the Rust application. It adds no artificial
per-node blocking delay. Candidate Rust uses the shared native executor with notification-driven
waiting. rclcpp uses `SingleThreadedExecutor`, normal service callbacks and async client response
callbacks that replenish the same request window. Rust uses service streams and response futures
with `buffer_unordered(window)`. Framework scheduling and these public API costs remain measured.

Reliable volatile ServicesQoS, KeepLast(max(16, window)), default ISA targeting, disabled rosout,
parameter services/events and intra-process communication are equal. RTT starts before the
async request call and ends after response retrieval, before sum validation. Windowed RTT
includes queueing; throughput includes validated completed requests. Process CPU time counts
all application/RMW threads inside the measured window, per completed request. It is not an
allocation counter or a per-thread profiler.

## Median results

Tables use medians of five repetition percentiles/rates. RTT is in microseconds, rate in
completed requests/s, and CPU time in microseconds/request. Each repetition has 5000 measured
requests after 100 warmups; language order reverses on alternating repetitions.

| CycloneDDS nodes/window | Historical p50 | Shared p50 | C++ p50 | Shared requests/s | C++ requests/s | Shared CPU/req | C++ CPU/req |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 / 1 | 11.921 | 12.050 | 14.367 | 79591 | 68748 | 12.347 | 14.524 |
| 1 / 16 | 115.523 | 114.876 | 194.579 | 128022 | 81890 | 7.570 | 12.186 |
| 2 / 1 | 121.361 | 13.152 | 15.608 | 71762 | 63419 | 13.481 | 15.755 |
| 2 / 16 | 163.471 | 111.815 | 175.446 | 125319 | 90260 | 7.713 | 11.069 |
| 8 / 1 | 730.100 | 18.507 | 21.320 | 49348 | 46326 | 19.119 | 21.565 |
| 8 / 16 | 489.135 | 120.564 | 173.653 | 103092 | 97243 | 8.423 | 10.277 |

| Fast DDS nodes/window | Historical p50 | Shared p50 | C++ p50 | Shared requests/s | C++ requests/s | Shared CPU/req | C++ CPU/req |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 / 1 | 19.686 | 19.779 | 22.013 | 49516 | 44842 | 20.195 | 22.296 |
| 1 / 16 | 186.285 | 182.677 | 306.815 | 79685 | 51649 | 12.567 | 19.359 |
| 2 / 1 | 130.742 | 20.582 | 23.269 | 47612 | 42141 | 21.003 | 23.729 |
| 2 / 16 | 217.071 | 168.204 | 287.481 | 78452 | 54781 | 12.745 | 18.253 |
| 8 / 1 | 778.179 | 26.887 | 30.826 | 36018 | 31909 | 27.763 | 31.338 |
| 8 / 16 | 552.621 | 182.616 | 305.896 | 68791 | 57646 | 14.536 | 17.348 |

Eight-node/window-one throughput rises from about 1353/1278 requests/s historically to
49348/36018, with lower CPU/request. Eight-node/window-16 throughput rises from about
30345/26524 to 103092/68791. Compared with same-run C++, that window-16 rate is about 6.0%/19.3%
higher and CPU/request about 18.0%/16.2% lower. Single-node p50 is within roughly 1.1% of its
historical control; all preselected metrics pass the 5% limits. p95/p99, all repetitions and
individual RTTs remain in the reports.

## Environment, artifacts and gates

The host is a Core i9-13900K. Every thread of each application, including RMW threads, is
pinned to CPU 24. Applications run serially; no task-owned build or test overlaps formal
measurement. Unrestricted shared-host Zig jobs remain a limitation for tails and small
differences. Jazzy, Rust 1.85 release and GCC 13.3 `-O3 -DNDEBUG` use default ISA targeting.
DDS domains 209/210 and unique service names isolate runs. Exact binary/source hashes, compiler
versions, package versions and baseline/candidate build manifests are recorded.

* [CycloneDDS report](cyclonedds.json) and [450000 RTT samples](cyclonedds.samples.json.gz).
* [Fast DDS report](fastdds.json) and [450000 RTT samples](fastdds.samples.json.gz).
* [Executor API and lifetime/dispatch contracts](../../MULTI_NODE_EXECUTION.md).
* [Complete runnable workflow](../../../r2r/examples/multi_node_pipeline.rs).

The verifier checks driver/language/RMW identities, complete counts/checksums, sample digests,
all node/window/repetition combinations, every percentile, throughput/CPU arithmetic, summary
medians and the recorded gate:

```sh
python3 tools/bench_ros/build.py --workload executor --baseline-ref cdccecd \
  --output-dir target/multi-node-ab
python3 tools/bench_ros/executor.py run \
  --baseline target/multi-node-ab/r2r-baseline --rust target/multi-node-ab/r2r-candidate \
  --cpp target/multi-node-ab/rclcpp-comparison --cpu 24 --nodes 1,2,8 --windows 1,16 \
  --iterations 5000 --warmup 100 --repetitions 5 --max-regression 1.05 \
  --rmw rmw_cyclonedds_cpp --domain-id 209 \
  --build-manifest target/multi-node-ab/build-manifest.json \
  --output target/multi-node-ab/cyclonedds.json
python3 tools/bench_ros/executor.py verify --require-performance-pass \
  docs/benchmarks/multi-node-executor-2026-10-28/cyclonedds.json \
  docs/benchmarks/multi-node-executor-2026-10-28/fastdds.json
```

Eleven executor cases pass under both Jazzy RMWs and native Humble/Kilted; original future
execution retains eighteen cases. Full optimized Jazzy validation passes 438 tests with two
manual benchmarks ignored, and workspace tests, Clippy and library doc-only checks pass. The
complete three-node pipeline runs in all four environments. Performance remains Jazzy-only;
the native C++ application also builds and passes its eight-node/window-16 smoke in Humble and
Kilted. These unconstrained smoke runs are correctness evidence, not distribution performance.
Iron, Lyrical and Rolling retain supported CI entries without new local runtime measurements.

Dynamic payloads, native/Rust allocation accounting, threaded callback scheduling, independent
runtime reactors, larger/fanout workloads, networked hosts and other hardware remain additional
work toward the full language/capability/performance goal.
