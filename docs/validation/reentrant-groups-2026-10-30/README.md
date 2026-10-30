# Bounded reentrant callback groups

[CallbackExecutor](../../CALLBACK_GROUPS.md) accepts ReentrantCallbackGroup with a
fixed invocation limit across all its sources. Public subscription/service/timer
factories require Fn + Send + Sync. One native Node/wait job feeds independently
owned lane receivers; no shared handler mutex or per-invocation worker thread is
introduced. Native response storage is initialized once per service lane.

The [complete example](../../../r2r/examples/reentrant_callbacks.rs) explicitly
waits for the first same-service handler to enter a blocking barrier, verifies
the second RPC response, then releases the first. It stops and recovers the group,
registers another timer through the stopped Node/group borrow, and verifies a
response after restart. Three workers include native waiting and two lanes.

Nineteen new focused cases check independent owned inputs, exact-once delivery,
all-receiver closure wakes, cancellation retaining input until reset, and 20000
close/publication retirements while the native producer remains alive. They also
cover mixed group order/type/concurrency recovery, zero/one-lane boundaries,
error/panic cancellation, shared capture retirement, native same-source worker
identities and request identities, distinct reusable response addresses across
concurrent requests and restart, and actual native service finalization while
both user bodies remain blocked. Neither late response may use that endpoint.

Timer tests require two overlapping bodies from the same timer on distinct
workers, no invocation above the group bound, and no queued invocation after
cancel. Controlled ROS-clock metadata preserves expected 120 ns, actual 180 ns,
and elapsed 80 ns. One-shot completion precedes selection release. Lifecycle
factories retain native activation/manual generations, reject a stale selection
without consuming new readiness, preserve manual startup/one-shot pause, and
remain gated through native ChangeState service requests after Node transfer.

The final module gate includes all thirty predecessor exclusive-group cases.
Exclusive service and timer factory definitions were separated into cohesive
modules without changing their definitions; their native resource code is shared.
Public Rustdoc rejects mutable unsynchronized captures and scoped native escape.
The no-ROS inventory requires the new group, executor methods, typed/native
factories and every lifecycle timer override. Six pure queue/scheduler cases run inside the native module gate; doc-only
checks compile their registration. The no-ROS environment provides declarations
for compilation and cannot link the complete library unit-test binary.

The reproduction scripts use Rust 1.85 and the native SDK/container mounts of
the recorded host. They run focused checks first, then examples, ownership
Rustdoc, full workspace/library validation, warning-denied Clippy, native
Humble/Kilted compatibility and a true no-ROS all-target/documentation gate.

```sh
bash docs/validation/reentrant-groups-2026-10-30/run-jazzy.bash
bash docs/validation/reentrant-groups-2026-10-30/run-compat.bash
python3 docs/validation/reentrant-groups-2026-10-30/run-isolation-preflight.py
```

| Environment | Module cases | Executor-family/full library | Optimized examples and native Clippy |
| --- | ---: | --- | --- |
| Jazzy / CycloneDDS | 49 debug, 49 release | 771 workspace and optimized main-library cases | Pass |
| Jazzy / Fast DDS | 49 debug, 49 release | 771 optimized main-library cases | Pass |
| Humble / Fast DDS | 47 release | 139 executor-family cases | Pass |
| Kilted / CycloneDDS | 49 release | 142 executor-family cases | Pass |
| Kilted / Fast DDS | 49 release | 142 executor-family cases | Pass |

All 27 main-library Rustdoc cases pass. Eight existing manual measurements remain
ignored. Humble excludes the newer native timestamp cases; nonzero counts are
recorded per distribution. The true no-ROS gate compiles all targets, runs common
and generator tests plus twelve public Rustdoc cases, and verifies 59 public
documentation objects and 257 methods. Its unused test-helper warnings are
limited to len, source_id and native_validity_probe, whose runtime consumers are
native-only. Iron, Lyrical and Rolling remain CI entries without new local
runtime evidence in this slice.

After all our builds/tests completed, performance isolation failed: 5 samples
reported load1 6.51–8.70, with external Zig compilation present throughout.
No A/B measurement started. The unchanged requirement remains three consecutive
idle samples, load1 at most 2.0, and no competing build/GPU work. Raw observations
are in [performance-isolation-preflight.json](performance-isolation-preflight.json).

[results.json](results.json) records exact summaries, image identities and log
hashes. [source-manifest.json](source-manifest.json) records final source and SDK
header hashes. The failed additional no-ROS unit-binary link attempt is retained
separately; its correction changes only the validation entry point. Final native
runtime sources stayed unchanged through all accepted gates.

Live configuration, a thread-free native runtime reactor, wider application
coverage and CPU-pinned independent interleaved Rust/rclcpp latency, throughput,
allocation and resource comparisons remain work toward the full goal. Bounded
reentrant capability and owned scheduling do not establish C++ performance
leadership. Cancellation stays cooperative for a handler already executing.
