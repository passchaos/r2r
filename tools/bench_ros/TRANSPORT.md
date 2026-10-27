# End-to-end ROS pub/sub roundtrip

Two independent processes exchange `std_msgs/msg/UInt64MultiArray` messages through DDS. A ping
process keeps one request in flight; an echo process verifies the complete layout, sequence and
payload before publishing the response. Each request carries a monotonically increasing sequence
plus 0, 32 or 4096 deterministic payload elements. Both endpoints require exactly one matched
publisher/subscription, count every message, validate its contents, and exchange an explicit
completion acknowledgement. Unexpected peers, missing messages, duplicate/out-of-order payloads,
timeouts or invalid result records fail the run.

Four combinations are measured: Rust/Rust, C++/C++, Rust ping/C++ echo and C++ ping/Rust echo.
Rust uses normal typed publishers/subscriptions with a single-threaded `try_spin_once` loop.
C++ uses normal typed publishers/subscriptions with `SingleThreadedExecutor::spin_once`.
Both wait at most 100 ms per spin, use reliable volatile KeepLast(16), disable rosout and have
no parameter services or event publisher. Intra-process transport is disabled; processes use
localhost DDS on an isolated ROS domain. This measures application API, conversion, scheduling
and transport together. It does not measure a pipelined throughput workload or same-process
zero-copy communication.

The optional `pubsub_view_roundtrip` harness supports explicit borrowed-receive prototypes and
an owned fallback. Build with `build.py --workload native_views`; the builder enables its
borrowed mode only when the native-view API exists. Run with `--native-views`, declaring
`--rust-receive-path owned` for the fallback and `--baseline-native-views` for a view baseline.
The verifier rejects endpoint path labels that disagree with the report metadata. The current
library has no promoted view API; the [rejected prototype](../../docs/benchmarks/native-views-2026-10-28-rejected/README.md)
is retained for isolated replay.

`--same-binary-control --max-regression 1.05` requires byte-identical Rust executables and
records a separate symmetric calibration gate. Latency and rate differences count in both
directions, so an apparent improvement cannot hide an unstable A/A control.

```sh
python3 tools/bench_ros/build.py --workload transport --output-dir target/ros-transport
python3 tools/bench_ros/transport.py \
  --rust target/ros-transport/r2r-candidate \
  --cpp target/ros-transport/rclcpp-comparison \
  --cpu-ping 30 --cpu-echo 31 --domain-id 102 --rmw rmw_cyclonedds_cpp \
  --sizes 0,32,4096 --iterations 5000 --warmup 100 --repetitions 5 \
  --build-manifest target/ros-transport/build-manifest.json \
  --output target/ros-transport/cyclonedds.json
```

Use two distinct idle CPUs and a DDS domain unused by other jobs. Stop competing builds and
tests before measurement. Change `--rmw` to `rmw_fastrtps_cpp` for the Fast DDS matrix. The runner
alternates combination order each repetition and generates unique topic names for every pair.
It clears inherited DDS profile files; the report records all selected transport conditions.

RTT begins immediately before the initiator publishes and ends when its application receives
the echo, before initiator payload validation. Echo validation is inside the roundtrip. Warmup,
discovery and teardown are excluded. Throughput reports completed stop-and-wait roundtrips per
second and includes initiator validation between operations. RTT percentiles use individual
operations; summary rows are the median of each repetition's p50/p95/p99 and throughput.

Every raw endpoint result is retained in the report. Individual RTT samples are stored in a
deterministic gzip JSON sidecar, referenced by filename and SHA-256, allowing independent
percentile/count verification. Binary and source hashes, compiler/ROS/RMW versions, CPU/domain
settings and timestamps accompany the report. Results apply to this hardware, middleware and
workload; broader Rust/C++ leadership requires service/action, larger fanout, pipelining and
multi-thread scaling workloads as well.

Run `python3 -m unittest discover -s tools/bench_ros -p 'test_*.py'` to verify that the result
validator rejects incorrect counts, sequences, RMW identity, percentile values and timings.
Use `python3 tools/bench_ros/verify_transport.py <report.json>` to independently recheck every
archived endpoint, individual RTT sample, percentile and summary median against the gzip sidecar.

## Interleaved library regression comparison

Build an independent Rust baseline with the same current benchmark source and lockfile, then
pass `--baseline-rust` to the transport runner:

```sh
python3 tools/bench_ros/build.py --workload transport --baseline-ref c661305 \
  --output-dir target/receive-buffer-transport
python3 tools/bench_ros/transport.py \
  --rust target/receive-buffer-transport/r2r-candidate \
  --baseline-rust target/receive-buffer-transport/r2r-baseline \
  --cpp target/receive-buffer-transport/rclcpp-comparison \
  --cpu-ping 30 --cpu-echo 31 --domain-id 115 --rmw rmw_cyclonedds_cpp \
  --sizes 0,32,4096 --iterations 5000 --warmup 100 --repetitions 5 \
  --max-regression 1.05 \
  --build-manifest target/receive-buffer-transport/build-manifest.json \
  --output target/receive-buffer-transport/cyclonedds.json
```

This adds baseline/baseline, baseline/C++ and C++/baseline to the four candidate combinations.
All seven pairs run in alternating order under the same environment. The offline verifier
requires the complete seven-combination matrix. `--max-regression` checks candidate/candidate
against baseline/baseline for the median repetition p50, p95, p99 and inverse throughput,
writing the raw evidence before failing if any metric exceeds the allowed ratio. A failed
performance gate requires investigation before promoting the implementation.

The report records the regression limit, pass/fail outcome and metric ratios. Integrity
verification remains useful for a rejected experiment and does not claim that its performance
passed. Add `--require-performance-pass` to `verify_transport.py` when a passing regression gate
is a requirement; absent or failed gates are then rejected.
