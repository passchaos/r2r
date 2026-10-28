# Owned background executor and default-path performance

The optional `Executor::spawn` API is compared with the safe `67ea778` baseline.
The native wait runs on one thread and the same service application future on
another in both background implementations. The baseline uses a manual thread,
weak cancellation handle and join; the candidate owns an `ExecutorTask`.
Default comparisons keep application futures and the native shared wait on the
same thread. rclcpp's independent single-thread executor retains the identical
AddTwoInts count, payload, sequence, checksum and bounded concurrency oracles.

Final comparisons use nodes 1/8, windows 1/16, 30000 requests after 5000 warmup
requests and nine interleaved repetitions. Every final report has 108 records
and 3240000 RTT samples. The original 1.05 threshold covers median p50/p95/p99,
CPU/request and inverse throughput. Default processes and all RMW threads use
CPU 24. Background processes and all RMW threads use the same allowed physical
CPU set 24/25 for baseline, candidate and C++; this is not a one-core comparison.
Builds/tests are stopped during measurement. Binary, compiler, native package,
source and affinity metadata remain in the reports.

The final default-path reports pass all gates under both RMWs. The final background
A/B reports also record passing gates under both RMWs. Ratios below are candidate divided
by baseline; throughput is inverted so lower is better.

| Mode | RMW | Nodes | Window | p99 ratio | CPU/request ratio | Inverse throughput |
| --- | --- | --- | --- | --- | --- | --- |
| default | rmw_cyclonedds_cpp | 1 | 1 | 0.9874 | 0.9978 | 0.9978 |
| default | rmw_cyclonedds_cpp | 1 | 16 | 0.9850 | 0.9854 | 0.9854 |
| default | rmw_cyclonedds_cpp | 8 | 1 | 1.0094 | 0.9997 | 1.0051 |
| default | rmw_cyclonedds_cpp | 8 | 16 | 0.9866 | 0.9896 | 0.9917 |
| default | rmw_fastrtps_cpp | 1 | 1 | 1.0045 | 0.9929 | 0.9929 |
| default | rmw_fastrtps_cpp | 1 | 16 | 0.9911 | 0.9872 | 0.9872 |
| default | rmw_fastrtps_cpp | 8 | 1 | 1.0014 | 1.0052 | 1.0052 |
| default | rmw_fastrtps_cpp | 8 | 16 | 0.9866 | 0.9860 | 0.9860 |
| background | rmw_cyclonedds_cpp | 1 | 1 | 0.9798 | 0.9888 | 0.9814 |
| background | rmw_cyclonedds_cpp | 1 | 16 | 0.9994 | 0.9961 | 1.0086 |
| background | rmw_cyclonedds_cpp | 8 | 1 | 1.0216 | 0.9951 | 0.9961 |
| background | rmw_cyclonedds_cpp | 8 | 16 | 0.9705 | 0.9887 | 0.9802 |
| background | rmw_fastrtps_cpp | 1 | 1 | 0.9784 | 0.9922 | 0.9921 |
| background | rmw_fastrtps_cpp | 1 | 16 | 0.9635 | 0.9930 | 0.9884 |
| background | rmw_fastrtps_cpp | 8 | 1 | 0.9530 | 1.0015 | 1.0055 |
| background | rmw_fastrtps_cpp | 8 | 16 | 0.9851 | 0.9920 | 0.9873 |

## Calibration and limitations

All initial reports are retained in `initial/`. The first background Fast DDS
A/B failed the original threshold. A same-binary control on one CPU also failed,
with a 2.53x symmetric p95 difference at node-1/window-16. A short two-core control
failed p99 by 24.1%; a long two-core control (30000 requests, 5000 warmup, nine
repetitions) reduced drift to one 5.2% p99 cell, still above the threshold.
These are failed calibration results, not passing performance evidence.

The original default-path short cohort failed one p99 cell by 5.4%. The candidate
then separated background errors from the common ROS error representation and
moved the optional observer waker into the existing cancellation allocation,
retaining the default queue layout. Its final long default comparisons pass.
The old error/queue-layout implementation is not retained as production code.

A recorded passing background A/B is not sufficient for an unqualified tail
latency claim when the initial background A/A failed and the final candidate has no passing
matched same-binary calibration.
The reports establish protocol correctness and limited measured comparisons;
background scheduling efficiency and calibrated tails remain open work. Native
thread handoff adds CPU/wakeup costs, and this workload does not establish that
background execution beats C++ or the existing same-thread executor. The API's
concrete gain is safe async-runtime I/O composition, ownership and cleanup.

The default comparisons retain the original same-thread fast path. Broad ROS
performance leadership still requires dynamic payloads, callbacks, multi-goal
work, resource accounting, scaling and cross-platform/distribution evidence.

The final observer cancellation test was added after benchmark compilation; it
changes tests only. Original source snapshots are recorded in each build
manifest. All passing reports can be independently checked:

```sh
python3 tools/bench_ros/executor.py verify --require-performance-pass docs/benchmarks/background-executor-2026-10-29/redesigned-default-cyclonedds.json
python3 tools/bench_ros/executor.py verify --require-performance-pass docs/benchmarks/background-executor-2026-10-29/redesigned-default-fastdds.json
python3 tools/bench_ros/executor.py verify --require-performance-pass docs/benchmarks/background-executor-2026-10-29/redesigned-background-cyclonedds.json
python3 tools/bench_ros/executor.py verify --require-performance-pass docs/benchmarks/background-executor-2026-10-29/redesigned-background-fastdds.json
```
