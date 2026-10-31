# Runtime-agnostic callback-group task validation

`CallbackGroup::spawn_task` retains arbitrary `Send` futures under the mutually exclusive group
scheduler token. `ReentrantCallbackGroup::spawn_task` chooses the lane with the fewest retained
registrations and keeps that future on the stable lane across stop/restart. Distinct lanes progress
concurrently up to the configured group bound; one future is never polled concurrently and does
not need `Sync`.

The typed action oracles move both a Fibonacci server stream and client goal/feedback/result
workflow into an exclusive group and into two reentrant lanes while the native executor owns RCL
wait/take. Exact feedback, terminal status and result payloads are required.

Eight nearby task tests cover RAII cancellation and worker-side capture retirement, detach, future
errors, deterministic least-populated lane assignment, completed-handle bookkeeping retirement,
group cancellation without polling the application future, explicit reawakening after
`reset_cancel`, and completion removal. Two native
scheduler cases prove that three blocking task polls overlap only to a two-lane bound and preserve
panic cleanup. A stopped executor returns exclusive and reentrant pending tasks with their future
state; each group resumes and completes after reset/restart.

Jazzy executes the pure task and real action cases in debug and release under both RMWs. The
required compatibility matrix is Humble/Fast DDS and Kilted/both RMWs. Each environment also runs
both complete action examples and warnings-denied focused Clippy. The no-ROS gate covers all
targets, both non-Send compile-fail boundaries and 84 public pages/341 required methods.

```sh
bash docs/validation/callback-group-tasks-2026-10-30/run-jazzy.bash
bash docs/validation/callback-group-tasks-2026-10-30/run-compat.bash
bash docs/validation/callback-group-tasks-2026-10-30/run-no-ros.bash
python3 docs/validation/callback-group-tasks-2026-10-30/verify.py
```

Compilation is pinned to one CPU with one Cargo/build-script job and a 10 GiB virtual/container
memory ceiling. This is correctness and ownership evidence; isolated scheduling/throughput
comparison against rclcpp action callback groups remains a separate performance gate.
