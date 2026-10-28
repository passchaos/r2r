# Default executor after background cleanup notifications

Safe baseline `6f97881` and candidate run the same AddTwoInts application
through the ordinary same-thread shared executor. Independent release binaries,
CPU 24, nodes 1/8, windows 1/16, reliable volatile QoS, 30000 measured requests,
5000 warmup requests and nine interleaved repetitions are fixed. All application
and RMW threads share the CPU; builds/tests were stopped during measurement.

Both Jazzy RMW reports pass the preselected 1.05 p50/p95/p99/CPU-per-request and
inverse-throughput gates. Each contains 108 records and 3240000 raw RTT samples;
counts, sequences, payloads and checksum oracles pass for Rust baseline,
candidate and independent rclcpp application.

| RMW | Nodes | Window | p99 ratio | CPU/request ratio | Inverse throughput |
| --- | --- | --- | --- | --- | --- |
| rmw_cyclonedds_cpp | 1 | 1 | 0.9997 | 1.0024 | 1.0024 |
| rmw_cyclonedds_cpp | 1 | 16 | 1.0237 | 1.0254 | 1.0252 |
| rmw_cyclonedds_cpp | 8 | 1 | 1.0063 | 1.0035 | 1.0027 |
| rmw_cyclonedds_cpp | 8 | 16 | 1.0138 | 1.0135 | 1.0211 |
| rmw_fastrtps_cpp | 1 | 1 | 0.9877 | 0.9872 | 0.9888 |
| rmw_fastrtps_cpp | 1 | 16 | 1.0031 | 1.0025 | 1.0025 |
| rmw_fastrtps_cpp | 8 | 1 | 0.9918 | 0.9916 | 0.9916 |
| rmw_fastrtps_cpp | 8 | 16 | 1.0164 | 0.9977 | 0.9977 |

This default-path workload checks the effect of the shared queue storage,
cleanup and action maintenance changes on ordinary nodes. It does not measure
cleanup throughput or promise stronger background tail precision. Native
cleanup and delayed decision behavior have dedicated live-worker oracles.

The original build manifest and all compressed samples are retained. The
notification helper's source hash is recorded in `notification-source.json`;
the builder now includes it in future manifests. Registry-count assertions and
the untyped action-observer test were completed after benchmark compilation;
they do not alter the runtime measured here.

```sh
python3 tools/bench_ros/executor.py verify --require-performance-pass docs/benchmarks/background-cleanup-2026-10-29/cleanup-default-cyclonedds.json
python3 tools/bench_ros/executor.py verify --require-performance-pass docs/benchmarks/background-cleanup-2026-10-29/cleanup-default-fastdds.json
```
