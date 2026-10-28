# Default service execution after lifecycle timer support

The safe `88f1ac7` baseline and candidate use independent optimized binaries,
identical AddTwoInts requests, reliable/volatile QoS and windows 1/16. CPUs
24/25 and DDS domain 122 are fixed. Seven language pairs retain baseline/
baseline, candidate/candidate, C++/C++ and both Rust/C++ directions. Pair order
alternates across five repetitions, each with 5000 measured requests and 100
warmup requests. Builds/tests completed before measurement.

Each RMW report contains 70 records and 350000 raw RTT samples. The preselected
maximum ratio is 1.05 for median p50/p95/p99 and inverse throughput. Both
reports record a passing regression gate and pass `--require-performance-pass`.

| RMW | Window | p50 ratio | p95 ratio | p99 ratio | Inverse throughput ratio |
| --- | --- | --- | --- | --- | --- |
| cyclonedds | 1 | 1.0070 | 1.0155 | 1.0311 | 1.0090 |
| cyclonedds | 16 | 0.9858 | 0.9907 | 0.9783 | 0.9848 |
| fastdds | 1 | 1.0044 | 1.0123 | 0.8988 | 0.9981 |
| fastdds | 16 | 1.0216 | 0.9954 | 0.9491 | 1.0038 |

This workload uses ordinary service nodes without managed timers. It validates
the effect of the optional registry and preparation branch on that default
path; it does not measure managed-timer throughput. The feature uses native
paused timers, an unchanged ordinary tick representation and no added thread,
periodic query or guard. Lifecycle-specific behavior is covered by native timer,
clock, generation, shutdown and shared-executor tests.

The source manifest includes node and lifecycle modules. An additional backward
ROS-time regression test was added after the benchmark build; it does not alter
the runtime implementation measured here. Wider timer/application/hardware
scaling and the full Rust/C++ goal remain separate requirements.

```sh
python3 tools/bench_ros/verify_transport.py --require-performance-pass docs/benchmarks/lifecycle-timers-2026-10-28/cyclonedds.json
python3 tools/bench_ros/verify_transport.py --require-performance-pass docs/benchmarks/lifecycle-timers-2026-10-28/fastdds.json
```
