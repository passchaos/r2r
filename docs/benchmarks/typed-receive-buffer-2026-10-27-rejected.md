# Rejected typed receive-container reuse experiment

The experiment is **not part of the active runtime**. It failed the preselected 1.05
candidate/baseline regression limit: the 4096-value roundtrip p99 was 6.3% above baseline.
The implementation was removed after measurement. The delivered change is the stronger
interleaved baseline/candidate/C++ transport gate and its reproducible counterexample.

## Implementation and correctness findings

The candidate retained one native C message container inside each typed subscriber. Before
subsequent middleware takes it finalized owned fields and zeroed their storage. Complete Rust
messages were copied out before the next take, so previously returned stream values stayed
independent. Manual types without a reset implementation used fresh native allocations.

A preliminary version simply reused the populated native structure. This was unsuitable for
the installed CycloneDDS C deserializer: its generic sequence path invokes sequence initialization
without finalizing previous native data, overwriting the old allocation. The revised candidate
explicitly released old native fields before deserialization. Five receive tests passed under
both RMWs, covering container creation/destruction counts, owned-message independence, metadata,
allocation failures, reset failures, backpressure and nested trajectories. A separate native
reset test passed for nested field destruction and repeated reset. Correctness success did not
override the performance gate.

## Interleaved measurements

The [transport harness](../../tools/bench_ros/TRANSPORT.md) compared baseline `c661305`, the
candidate and rclcpp with the same current benchmark source, lockfile, compiler options and
payload. Seven language pairs cover candidate/candidate, C++/C++, both candidate/C++ directions,
baseline/baseline and both baseline/C++ directions. Each payload size has five repetitions of
5000 measured roundtrips after 100 warmup messages. Full payload and sequence oracles pass.

ROS 2 Jazzy and CycloneDDS were measured on the i9-13900K. An attempted CPU 30/31 run overlapped
an unrelated Zig task pinned to CPU 30 and was stopped; it is excluded from the report.
The retained run uses CPU 24/25, an independent DDS domain and unique topics. The unrelated
task remained on CPU 30; no task-owned build or test overlapped measurement. Shared-host effects
remain a limitation, so the numbers do not establish universal regression or improvement.

Times below are microseconds and are the median repetition percentiles of individual RTTs:

| Payload values | Baseline p50 | Candidate p50 | Baseline p99 | Candidate p99 |
| --- | ---: | ---: | ---: | ---: |
| 0 | 40.872 | 41.023 | 48.048 | 47.176 |
| 32 | 41.909 | 41.492 | 50.739 | 47.930 |
| 4096 | 85.339 | 84.653 | 107.911 | 114.710 |

Large-message median RTT improved by approximately 0.8%, but p99 exceeded the regression
limit. This small median difference is insufficient evidence for promoting an implementation
that adds a reset hook and native lifecycle complexity. The default typed subscription remains
on its previous allocation behavior. No performance claim is made for the rejected candidate.

## Reproducible evidence

* [Raw endpoint report](typed-receive-buffer-2026-10-27-rejected.json), including the failed gate.
* [All 525,000 RTT samples](typed-receive-buffer-2026-10-27-rejected.samples.json.gz), referenced by SHA-256.
* [Archived candidate patch](typed-receive-buffer-2026-10-27-rejected.patch.gz), applying to the baseline
  runtime; decompress before applying. The report records its gzip hash and library source hashes.
  It is an experiment artifact.

The offline validator verifies all 105 records, seven-pair matrix completeness, samples,
percentiles, throughput calculations and gate consistency:

```sh
python3 tools/bench_ros/verify_transport.py \
  docs/benchmarks/typed-receive-buffer-2026-10-27-rejected.json
```

Integrity validation accepts valid evidence of failure. Requiring a passing performance gate
correctly rejects this report:

```sh
python3 tools/bench_ros/verify_transport.py --require-performance-pass \
  docs/benchmarks/typed-receive-buffer-2026-10-27-rejected.json
```

Future receive optimizations should reduce payload copies or use an explicit middleware-aware
storage boundary. Simple populated-container reuse is not a safe universal RMW strategy.
