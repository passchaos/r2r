# Native future wake responsiveness

The default `Node::spin_until_future_complete` responds to an external Rust future notification
with p50 around 4.6 microseconds, versus roughly 9 milliseconds for library `b22d27e`. A native
guard condition now bridges the Rust waker into the ROS wait. Both RMWs pass the preselected
1.05 median regression limits for p50/p95/p99 and inverse completion rate.

## Workload and boundaries

Independent historical and candidate executables compile the same current
[benchmark source](../../../r2r/benches/future_execution.rs) and lockfile against their library
versions. The benchmark calls the unchanged default API in both binaries. A persistent external
thread receives a request, sleeps for one millisecond, timestamps immediately before sending
the sequence through a futures-rs oneshot channel, then waits for the next request. The node
validates every sequence and completion. Its future is pending while the node waits; there
are no ROS pub/sub, service, action or parameter endpoints, and rosout is disabled.

Latency begins at the producer timestamp before channel send and ends after the node returns
the completed future, before sequence validation. Producer delay is excluded from that latency.
Completion rate includes the one-millisecond producer delay, submission and validation. The
baseline empty-node wait used a condvar and the 10 ms periodic poll cap; channel notifications
could not interrupt it. The candidate lazily initializes/reuses a native guard and can return
promptly. The default periodic fallback remains 10 ms; both versions retain a one-second overall
deadline per completion. Warmup initializes the wake path before measurement.

Each RMW has five alternating-order repetitions of 200 measured completions after ten warmups,
with identical input/delay and CPU affinity. Percentiles are individual notification latencies;
the table uses medians of repetition percentiles.

| RMW | Baseline p50 | Candidate p50 | Baseline p99 | Candidate p99 | Baseline completions/s | Candidate completions/s |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| CycloneDDS | 9000.04 μs | 4.62 μs | 9157.45 μs | 43.23 μs | 99.02 | 920.39 |
| Fast DDS | 8999.23 μs | 4.66 μs | 9131.41 μs | 48.60 μs | 99.02 | 920.06 |

The delay-dominated rate improves roughly 9.3 times in this workload. This is external-future
responsiveness rather than a ROS language or DDS throughput comparison. One CycloneDDS
candidate repetition had p99 around 2.21 ms; all raw repetitions remain available, and the
median table does not promise a hard real-time bound or every-round p99 below 50 μs.

## Environment and evidence

The host is a Core i9-13900K. All threads of each executable, including the external producer
and RMW background threads, are pinned to CPU 24. ROS 2 Jazzy and Rust 1.85 release use default
ISA targeting. Domains 201/202 isolate RMW runs. No task-owned build or test overlaps formal
measurement; unrestricted shared-host jobs are a limitation, especially for tails.

* [CycloneDDS report](cyclonedds.json) and [raw samples](cyclonedds.samples.json.gz)
* [Fast DDS report](fastdds.json) and [raw samples](fastdds.samples.json.gz)
* [Public API, lifetime and shutdown contract](../../FUTURE_EXECUTION.md)

Each report contains ten records and 2000 notification samples. It records exact binary/source
hashes, baseline revision, compiler/environment, completion counts/checksums, windows and the
gate outcome. The offline verifier checks all counts, samples, percentiles, completion rates,
matrix completeness, summary medians and the recorded gate.

```sh
python3 tools/bench_ros/build.py --workload future --baseline-ref b22d27e \
  --output-dir target/future-wake-ab
python3 tools/bench_ros/future.py run \
  --baseline target/future-wake-ab/r2r-baseline \
  --candidate target/future-wake-ab/r2r-candidate \
  --build-manifest target/future-wake-ab/build-manifest.json \
  --cpu 24 --domain-id 201 --rmw rmw_cyclonedds_cpp \
  --iterations 200 --warmup 10 --delay-us 1000 --repetitions 5 \
  --max-regression 1.05 --output target/future-wake-ab/cyclonedds.json
python3 tools/bench_ros/future.py verify --require-performance-pass \
  docs/benchmarks/future-wake-2026-10-28/cyclonedds.json \
  docs/benchmarks/future-wake-2026-10-28/fastdds.json
```

Ten native boundary tests cover notification races/lifetime, ROS availability/fairness and
shutdown and pass under both Jazzy RMWs and native Humble/Kilted. The complete optimized
Jazzy library passes 419 tests; workspace checks, Clippy, doc-only compilation and the
notification-driven service/action examples pass. Performance measurement remains Jazzy-only.
Broader executor
scaling, integration with independent runtime reactors, networked workloads and ROS endpoint
performance remain additional acceptance work toward the full goal.
