# Concurrent action feedback comparison

`action_feedback_roundtrip` compares independent r2r and rclcpp action endpoints with
large feedback payloads and multiple simultaneously active goals. It complements the
single-goal lifecycle workload in [ACTION.md](ACTION.md); it is not a replacement for
the success/cancellation protocol there.

Each measured batch submits `goal_window` Fibonacci goals before waiting for responses.
The server accepts them, and the regression protocol requires every client to observe Executing
before an `AddTwoInts` phase-0 request releases feedback. The server publishes `feedback_per_goal`
rounds in goal order. Every feedback contains `elements` deterministic `i32` values and
the client validates every value. A phase-1 request acknowledges complete feedback
consumption, after which the server succeeds every goal with an exact one-element
result. A rejected control goal proves that all measured results finished before server
exit.

The lifecycle sample starts before the first goal submission and ends after all results.
With `--status-sync required`, the feedback-stage sample starts after all Executing statuses and
ends after the complete feedback
window. Discovery, warmup and the rejected control are excluded. Goal/result/cancel and
feedback QoS are reliable volatile KeepLast(max(16, `goal_window * feedback_per_goal`));
status uses the native action default and result retention is ten seconds. Both endpoints
use one spin/executor thread and intra-process communication is disabled.

Build the optimized Rust and C++ executables plus the optional allocation probe:

```sh
python3 tools/bench_ros/build.py --workload action_feedback --with-probe \
  --output-dir target/action-feedback
```

Run a full four-direction matrix on two idle CPUs:

```sh
python3 tools/bench_ros/action_feedback.py \
  --rust target/action-feedback/r2r-candidate \
  --cpp target/action-feedback/rclcpp-comparison \
  --cpu-ping 24 --cpu-echo 25 --domain-id 160 \
  --rmw rmw_cyclonedds_cpp --sizes 4096 --goal-windows 8 \
  --feedback-per-goal 4 --iterations 1000 --warmup 50 --repetitions 5 \
  --status-batch-size 2 --result-timeout-ms 1000 --status-sync none \
  --build-manifest target/action-feedback/build-manifest.json \
  --output target/action-feedback/cyclonedds.json
python3 tools/bench_ros/verify_action_feedback.py \
  target/action-feedback/cyclonedds.json
```

`--baseline-rust` adds independent baseline/baseline and mixed baseline/C++ pairs.
`--max-regression 1.05` records the preselected same-language p50/p95/p99, feedback-stage
and inverse-throughput gate. `--matrix-scope regression` runs only baseline/baseline and
candidate/candidate for a focused A/B; the default remains the full interoperability
matrix. Instrumented `--probe` runs cannot claim a latency gate.

`--status-batch-size` chooses how many requests/results each Rust batch call receives. When a run
contains several goal-window dimensions, the slice must be supplied explicitly and fit all of them.
`--result-timeout-ms` and `--status-qos` set equal endpoint semantics. The builder detects the
batch API independently in candidate and baseline source; every endpoint reports
`status_path=batched|per_goal`, and the offline verifier rejects a mismatch. Regression matrices
use `--status-sync required`. Full Rust/rclcpp matrices use `--status-sync none` because rclcpp can
publish its first status before its client goal handle exists; those reports compare complete
lifecycle/throughput and retain actual status/feedback competition rather than claiming an
isolated feedback-stage comparison.

Reports retain every endpoint count and checksum, summary medians, binary/source/package
identities and deterministic compressed lifecycle/feedback samples. The offline verifier
recomputes all percentiles, rates, summary rows, matrices and recorded gates. A verifier
pass establishes report integrity, not a performance pass; use
`--require-performance-pass` only for reports with a recorded passing gate.
