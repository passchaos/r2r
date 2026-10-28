# Canceled-observer batch continuation

The final change supplies one continuation when a bounded cancellation drain
leaves unexamined work, and returns that same progress bit to node preparation.
It has no extra queue-length query, scan epoch or runtime state. Result, feedback
observer and cancel-response cleanup keep their sixteen-entry per-pass limits;
detached-feedback retries are requeued after progress is recorded.

The safe `f3bc7ad` baseline (runtime identical to `df69693`) and candidate use
independent optimized binaries on fixed CPUs 24/25, DDS domain 122, identical
QoS and success/cancellation workloads. Seven combinations retain baseline/
baseline, Rust/Rust, C++/C++ and both Rust/C++ directions for both Rust builds.
Five interleaved repetitions contain 1000 measured and 50 warmup goals each.
Builds/tests completed before measurements.

Both final RMW reports record passing original 1.05 p50/p95/p99 and inverse
throughput gates. Each contains 70 records and 70000 raw RTT samples; complete
acceptance, feedback, cancel and result oracles pass. Ratios are candidate over
baseline; throughput is inverted so lower is better.

| RMW | Scenario | p50 ratio | p95 ratio | p99 ratio | Inverse throughput |
| --- | --- | --- | --- | --- | --- |
| rmw_cyclonedds_cpp | success | 0.9890 | 1.0154 | 1.0203 | 0.9952 |
| rmw_cyclonedds_cpp | cancellation | 0.9897 | 1.0041 | 0.9862 | 0.9925 |
| rmw_fastrtps_cpp | success | 0.9944 | 0.9917 | 0.8699 | 0.9922 |
| rmw_fastrtps_cpp | cancellation | 0.9920 | 0.9946 | 0.9843 | 0.9963 |

`initial/` retains the first candidate, which queried three cancellation queue
lengths for every maintained client. It failed CycloneDDS cancellation p99 by
11.85% and Fast DDS success p99 by 13.47%. The final implementation removes the
additional query and derives continuation from the bounded drain's existing
locked state. Both initial failures remain visible; their source inventories
and raw samples are not replaced by the final reports.

This workload checks the ordinary action transport path after the cleanup fix.
Native tests separately confirm forty-observer cancellation without unrelated
input, same-thread nonblocking continuation, boundary counts, live observer
preservation, queue unlock and the final disabled-notifier boundary. This is a
correctness improvement with a recorded passing regression gate, not a claim
of faster multigoal throughput. Prior broader action-tail A/A drift remains a
limitation; no calibrated tail improvement is inferred from this A/B alone.

The final manifest includes node preparation and cancellation implementation
sources. Those cancellation/preparation hashes match the final source tree.
An existing graph-discovery test was moved and made notification-driven after
measurement; only the test section of `nodes.rs` changed.
Feedback delivery/backpressure and large server cancellation decision/delivery
scans remain the separate gaps documented by the earlier rejected prototype.

```sh
python3 tools/bench_ros/verify_transport.py --require-performance-pass docs/benchmarks/cancellation-continuation-2026-10-29/progress-cyclonedds.json
python3 tools/bench_ros/verify_transport.py --require-performance-pass docs/benchmarks/cancellation-continuation-2026-10-29/progress-fastdds.json
```
