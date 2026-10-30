# Callback group execution validation

[CallbackExecutor](../../CALLBACK_GROUPS.md) now transfers one owned native wait
job and mutually exclusive callback group jobs across a fixed total worker
budget. Scoped native subscriptions, typed/native services and wall/ROS timers
register directly in groups. Native message ownership crosses the worker
boundary; native endpoint ownership remains with the one Node job.

The [complete example](../../../r2r/examples/callback_groups.rs) validates 32
exact native forwards and RPC responses on one Node while another group's
timer handler is held at an explicit barrier. It releases the handler, stops
all workers, recovers state and verifies another request after restart.

Thirty focused cases cover queue bounds and owned rejection, capacity/current
owner wakes, 20000-input close/publication retirement, same-Node progress,
group exclusion with four producers/workers, pre-start cancellation, missing
membership, Node/handler reuse, native errors/panic/drop cleanup, unchanged
request sequence identity and reply storage across 32 requests, handler query
reentry, service finalization during blocked user work, native timer invocation
timestamps, cancel/reset, one-shot policy and lifecycle generations.

The service finalization oracle queries actual rcl_service_is_valid before
releasing the handler. The client receives no late response after finalization.
Timer metadata is checked against a controlled native ROS clock: selection at
120 ns does not advance; worker invocation at 180 ns reports expected 120 ns,
actual 180 ns and elapsed 80 ns. Old lifecycle selections cannot consume a
new activation at a backwards-clock deadline; one-shot/manual pause is sticky.
Real ChangeState requests retain those controls after Node transfer and reuse.

Full rings/in-flight timers are omitted from native waiting. The unread-sample
test confirms zero subscription wait capacity at pressure, resumes capacity
after a worker poll and retains all 32 DDS values in order. Grouped subscription
options are checked against native RCL configuration, and QoS endpoint teardown
reports the subscription-invalid terminal result before closing the stream.

| Environment | Focused optimized cases | Executor-family cases | Complete optimized example |
| --- | ---: | ---: | --- |
| Jazzy / CycloneDDS | 30 | Full library/workspace coverage | Pass |
| Jazzy / Fast DDS | 30 | Full optimized library coverage | Pass |
| Humble / Fast DDS | 29 | 121 | Pass |
| Kilted / CycloneDDS | 30 | 123 | Pass |
| Kilted / Fast DDS | 30 | 123 | Pass |

Both Jazzy RMWs also pass 30 default-profile focused cases. Jazzy full debug
workspace and both optimized library suites pass 752 main-library cases, with
eight existing manual measurements ignored. All 25 main-library Rustdoc cases
pass. The three new public cases compile owned configuration and reject
running-owner reuse and scoped native message escape. All-target workspace
Clippy passes with warnings denied in Jazzy, Humble and Kilted. Humble lacks
the newer TimerCallInfo API/case; its nonzero count is retained explicitly.

The true no-ROS gate compiles all targets and checks 58 public documentation
artifacts and 226 methods. Iron, Lyrical and Rolling remain supported CI entries
without additional local runtime evidence in this slice.

After all local builds/tests ended, the isolation prerequisite still failed.
Five samples had load1 7.06–7.62 and external Zig compilation in every sample,
with GPU work in the first sample. No A/B measurement started. The passing
requirements remain three consecutive idle samples, load1 at most 2.0 and no
competing compilation/GPU workload. Raw observations are retained in
[performance-isolation-preflight.json](performance-isolation-preflight.json).

Exact top-level/subprocess summaries, image identities, parent commit, hashes
and compressed raw logs are in [results.json](results.json) and
[source-manifest.json](source-manifest.json). Native wait/service/timer header
hashes preserve the SDK synchronization requirements used by the implementation.
Source/cache/image mounts below are specific to the recorded host.

```sh
bash docs/validation/callback-groups-2026-10-30/run-callback-jazzy.bash
bash docs/validation/callback-groups-2026-10-30/run-callback-compat.bash
python3 docs/validation/callback-groups-2026-10-30/run-isolation-preflight.py
```

Cancellation remains cooperative for user work already executing. Reentrant
groups, live configuration and native reactor integration remain open.
Allocation/resource scaling and CPU-pinned interleaved Rust/rclcpp comparisons
are still required for the complete goal; capability closure and routing
structure do not establish C++ performance leadership.
