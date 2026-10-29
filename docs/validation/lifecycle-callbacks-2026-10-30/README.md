# Lifecycle callback timer validation

The [managed callback timer API](../../LIFECYCLE_CALLBACK_TIMERS.md) follows lifecycle and manual
generations before native advancement. Plain callbacks carry a unit permit; managed callbacks
retain an enabled/applied generation permit through RCL advancement and user invocation.
The permit holds no control mutex. Native activation/reset, one-shot sticky consumption and
current-owner wakeups apply through owned Node/shared/background execution.

The original inherited Node factory started an inactive lifecycle timer immediately. Its
native cancellation assertion failed against parent `423d21f`. That initial regression and
the later native negative-time counterexample remain in the compressed logs.

Eleven Jazzy cases verify inactive startup, activation and between-spin reactivation deadlines,
manual pause, sticky one-shot completion/reset, foreign controls/drop, failed transitions,
stale-ready suppression before native advancement, lock-free user reentry/retirement, panic,
SDK timing, paired native negative-time behavior and external activation without a tick future.
The existing fourteen lifecycle stream-timer cases pass after generation extraction. Regular
callback timer behavior remains covered by the full timer and library gates.

The [shared-executor example](../../../r2r/examples/lifecycle_callback_timer.rs) issues real
standard lifecycle ChangeState requests, validates sixteen exact callback-driven publications
through two active phases and verifies unchanged handler counts through three native paused
phases. The twelve-case compiler gate includes rejects for managed-to-ordinary callback timer
coercion and private-state access.

## Native timepoint boundary

The initial timing test requested successful callbacks after reactivation in a negative ROS
time domain. RCL returned `RCL_RET_ERROR: clock now returned negative time point value`.
The installed `ros-jazzy-rcl` is `9.2.11-1noble.20260911.104031`; the tagged
[native implementation](https://github.com/ros2/rcl/blob/9.2.11/rcl/src/rcl/timer.c#L289)
rejects this before updating last-call time or invoking the callback.

The final tests retain both requirements: positive-domain backwards reactivation produces exact
current native deadlines/timestamps, while negative native calls match the regular timer error,
invoke neither handler, preserve one-shot state and recover on supported-time reset. This SDK
restriction is not hidden by clamping metadata or weakening the call result assertion. The
source URL/digest and original failed test output are in `results.json` and compressed logs.

## Profiles and distributions

Final Jazzy workspace and each RMW optimized library suite pass 709 main-library cases, with
eight manual measurements ignored. Nineteen Rustdoc examples, the lifecycle callback example,
current-thread Tokio workflow and all-target warning-denied Clippy pass. Native Humble/Fast DDS
and Kilted/CycloneDDS plus Fast DDS cover managed callback/timer/executor/callback-family gates,
the complete optimized example, the public compiler gate and all-target Clippy.

The true no-ROS gate checks all targets, required ownership compile-fail examples and 42 public
documentation artifacts with 131 methods. Newer native timing APIs/cases retain the same
Humble/Iron exclusions as regular TimerCallInfo. Iron, Lyrical and Rolling remain supported CI
entries without new local runtime evidence in this slice.

| Native environment | Managed callback | Timer family | Executor | Callback family |
| --- | ---: | ---: | ---: | ---: |
| Humble / Fast DDS | 9 | 47 | 79 | 52 |
| Kilted / CycloneDDS | 11 | 52 | 80 | 56 |
| Kilted / Fast DDS | 11 | 52 | 80 | 56 |

These are matching top-level summaries, including their own subprocess cases.

Exact nonzero filter summaries, native RMW headers, parent commit and changed source hashes are
in `results.json`. Full compressed logs and the recorded scripts reproduce the gates:

```sh
bash docs/validation/lifecycle-callbacks-2026-10-30/run-lifecycle-callback-final-jazzy.bash
bash docs/validation/lifecycle-callbacks-2026-10-30/run-lifecycle-callback-compat.bash
```

Source/cache/image mounts in the compatibility script are specific to the recorded host.
Equivalent sourced native environments can run the same cargo commands. This is capability,
native ownership and lifecycle evidence; isolated representative Rust/C++ throughput/resource
and concurrent scheduling claims remain unproven work in the full goal.
