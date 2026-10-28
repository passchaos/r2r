# Independent executor cancellation acceptance

`ExecutorHandle` stops ROS execution without shutting down the context and permits the owner
to reset and reuse its nodes. Shared cancellation storage is created only when `handle()` is
requested. The default path avoids cancellation atomic loads; the enabled path checks persistent
cancellation at execution boundaries and coalesces native wakes through the existing guard.

Both the default and explicitly enabled paths pass the original 1.05 regression limit against
shared-executor library `aacbcf1`. Enabled-control medians show no regression larger than 1.33%
under CycloneDDS and 2.42% under Fast DDS across all selected latency, throughput and CPU metrics.
These are steady service-application costs with a live cancellation handle, rather than a
measurement of cancellation-request latency or general ROS performance leadership.

## Workload and gate

The unchanged [multi-node service application](../multi-node-executor-2026-10-28/README.md)
uses one, two or eight nodes, one or sixteen in-flight `AddTwoInts` requests, complete per-node
sequence/operand checks and response sum validation. Historical and candidate Rust binaries
both use the shared native executor; rclcpp uses `SingleThreadedExecutor` with the same node
and request-window dimensions. `--baseline-shared` makes the baseline driver identity explicit.

`--control-enabled` sets `R2R_BENCH_CONTROL_ENABLED=1` only for the candidate. The candidate
creates and retains its handle before driving setup/warmup/measured work. Each record declares
whether control is enabled; the verifier rejects a missing or unexpected mode. Default runs
leave that environment variable unset for every binary. The two modes use the same candidate
binary, with identical library code, inputs and QoS.

Each matrix has five alternating-order repetitions of 5000 measured requests after 100 warmups.
The preselected gate checks medians of repetition p50/p95/p99, inverse completed throughput and
process CPU/request, independently for all six node/window combinations. It is unchanged from
the failed first candidate. Four matrices retain 360 records and 1,800,000 RTT samples.

| Mode / RMW | Largest candidate/baseline regression ratio | Metric / nodes / window | Gate |
| --- | ---: | --- | --- |
| Enabled / CycloneDDS | 1.013238 | rtt_ns_p95 / 1 / 1 | Pass |
| Enabled / Fast DDS | 1.024175 | rtt_ns_p99 / 8 / 16 | Pass |
| Default / CycloneDDS | 1.041692 | rtt_ns_p95 / 2 / 1 | Pass |
| Default / Fast DDS | 1.023140 | cpu_per_request_ns / 8 / 1 | Pass |

## Enabled-control median results

Latency and CPU time are in microseconds; rate is completed requests/s. Each row reports
medians of five repetition percentiles/rates. The reports preserve p95/p99 and every raw sample.

### CycloneDDS

| Nodes / window | Baseline p50 | Enabled p50 | C++ p50 | Enabled requests/s | C++ requests/s | Enabled CPU/req | C++ CPU/req |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 / 1 | 11.953 | 12.052 | 14.274 | 79455 | 69222 | 12.370 | 14.426 |
| 1 / 16 | 115.913 | 116.292 | 192.821 | 126755 | 82420 | 7.672 | 12.127 |
| 2 / 1 | 13.087 | 13.028 | 15.573 | 72424 | 63302 | 13.385 | 15.796 |
| 2 / 16 | 109.932 | 110.251 | 175.425 | 123941 | 90375 | 7.639 | 11.059 |
| 8 / 1 | 18.253 | 18.283 | 21.627 | 49550 | 45669 | 18.880 | 21.894 |
| 8 / 16 | 122.874 | 121.408 | 174.070 | 98440 | 97200 | 8.471 | 10.287 |

### Fast DDS

| Nodes / window | Baseline p50 | Enabled p50 | C++ p50 | Enabled requests/s | C++ requests/s | Enabled CPU/req | C++ CPU/req |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 / 1 | 19.494 | 19.740 | 21.777 | 49577 | 45341 | 20.168 | 22.054 |
| 1 / 16 | 184.953 | 185.759 | 306.158 | 78460 | 51790 | 12.744 | 19.307 |
| 2 / 1 | 20.833 | 20.676 | 23.386 | 47280 | 42214 | 21.149 | 23.688 |
| 2 / 16 | 167.471 | 169.985 | 286.111 | 77550 | 55408 | 12.889 | 18.044 |
| 8 / 1 | 26.844 | 26.913 | 30.759 | 35886 | 32139 | 27.862 | 31.112 |
| 8 / 16 | 180.998 | 183.432 | 305.193 | 68505 | 57690 | 14.597 | 17.332 |

## Rejected first candidate and calibration

The first implementation allocated shared control for every executor. Its original CycloneDDS
matrix failed nodes=2/window=16 p99: candidate/baseline was 1.067814, exceeding 1.05. Other
metrics in that cell passed. The [original failed report](rejected-initial/cyclonedds.json) and
[raw samples](rejected-initial/cyclonedds.samples.json.gz) are retained.

A [same-candidate-binary A/A](rejected-initial/aa-cyclonedds.json) passed the one-way regression
test in that cell but had a symmetric p99 ratio of 1.093832. That control exposes insufficient
precision for a 5% tail claim; it does not overturn the original failure. The implementation
was redesigned before the four acceptance matrices. No failed round was discarded or replaced
inside a matrix, and no gate dimension or tolerance was relaxed.

## Environment and reproduction

The host is a Core i9-13900K running ROS 2 Jazzy. Rust 1.85 release and GCC 13.3 `-O3 -DNDEBUG`
use default ISA targeting. Every application and all its RMW threads are pinned to CPU 24.
All four formal runs are serial, with no task-owned build/test overlapping measurement.
Unrestricted unrelated shared-host jobs remain a limitation for tails and small differences.
Domains 208/207 and unique service prefixes isolate the two RMWs.

Reports include binary/source hashes, the exact baseline revision and build manifest, compiler
and ROS package versions, complete counts/checksums and compressed sample digests. The
[candidate patch](candidate.patch.gz) includes runtime changes and benchmark/tool support.
The [exact measured runner](measurement-executor.py) matches the report's source hash. Its
legacy generic scheduling text mentions historical polling; `baseline_shared=true` and the
verified `shared_wait` baseline records establish the actual baseline path. The current runner
clarifies this metadata wording without changing measurement or validation.

```sh
python3 tools/bench_ros/build.py --workload executor --baseline-ref aacbcf1 \
  --output-dir target/executor-control-ab
python3 tools/bench_ros/executor.py run --baseline-shared --control-enabled \
  --baseline target/executor-control-ab/r2r-baseline \
  --rust target/executor-control-ab/r2r-candidate \
  --cpp target/executor-control-ab/rclcpp-comparison \
  --cpu 24 --nodes 1,2,8 --windows 1,16 --iterations 5000 --warmup 100 \
  --repetitions 5 --max-regression 1.05 --domain-id 208 --rmw rmw_cyclonedds_cpp \
  --build-manifest target/executor-control-ab/build-manifest.json \
  --output target/executor-control-ab/enabled-cyclonedds.json
```

Repeat with `rmw_fastrtps_cpp` and domain 207 for Fast DDS. Repeat each RMW without
`--control-enabled` for the default mode. Verify all four archived matrices:

```sh
python3 tools/bench_ros/executor.py verify --require-performance-pass \
  docs/benchmarks/executor-control-2026-10-28/enabled-cyclonedds.json \
  docs/benchmarks/executor-control-2026-10-28/enabled-fastdds.json \
  docs/benchmarks/executor-control-2026-10-28/default-cyclonedds.json \
  docs/benchmarks/executor-control-2026-10-28/default-fastdds.json
```

[Executor cancellation contracts](../../MULTI_NODE_EXECUTION.md) describe persistent requests,
weak handle lifetime, native wake ownership, completion races and owner-coordinated reset.
Seven new cases bring executor coverage to 18 tests under both Jazzy RMWs and native Humble/
Kilted. All four environments run the release [background control example](../../../r2r/examples/executor_control.rs):
8192 payload values, 32 service requests, cancellation/join and another request after reuse.
The original future driver retains 18 cases. The Python harness passes 47 unit tests, including
shared-baseline and enabled-control identity counterexamples.
Full optimized Jazzy library validation passes 445 tests with two manual benchmarks ignored.
Workspace tests, all-target Clippy, formatting and workspace library doc-only compilation pass.

Performance evidence remains Jazzy-only. Iron, Lyrical and Rolling retain supported CI entries
without new local runtime measurements in this slice. Dynamic payloads, threaded callbacks,
runtime reactors, fanout, allocation accounting, networked hosts and other hardware remain open.
