# Parallel Node execution validation

[`ParallelExecutor`](../../PARALLEL_EXECUTION.md) assigns owned Nodes to a fixed
native worker budget, with exclusive reusable RCL waits per group. The serial
baseline failed the independent-Node progress test while a synchronous callback
was blocked. That original failure is retained in `logs/serial-baseline.log.gz`;
it is a correctness baseline, not a throughput measurement.

The final regression strengthens the readiness handshake: discovery completes
before spawn, the slow callback announces its actual thread and remains blocked,
and only then does the application publish the native message for the other
callback. Different groups must progress before release on different threads.
Nodes in the same group must wait until release and use the same thread.
The [example](../../../r2r/examples/parallel_executor.rs) validates 32 exact RPC
responses while another group is blocked, then stop/restart, another sum and
removal of all three Nodes.

Thirteen focused cases cover configured/actual worker budgets, stable Node IDs,
round-robin/explicit placement, pre-start cancellation, skipped empty groups,
weak expiry, node reuse, 32 native service response identities, error/panic/drop
cleanup, simultaneous errors with skipped group indices, empty and three-group
shutdown, and service-driven lifecycle activation with native `/clock` and
positive-domain backwards reactivation deadlines. Three wait-set implementation
and guard addresses must be nonzero and disjoint; both remain unchanged after
stop/restart. The storage observer is compiled only for native unit tests.

Native failure notifies all weak peer controls on the failing thread before
completion is sent. The unpolled-task test checks actual native thread completion
before awaiting the aggregate result. Error completion releases the healthy
groups' captured resources before returning the lowest failing configured index.
All worker controls exist before the first thread starts, so early cancellation
also reaches a peer before its guard is created.

The [source manifest](source-manifest.json) records the parent commit, 15 changed
source/reproduction-script hashes and the installed Jazzy RCL wait contract.
RCL explicitly permits concurrent waits only with unique sets and unique
contents; sharing a guard between wait sets is undefined behavior. This slice
reuses exclusive Executor ownership and adds no unsafe pointer-sharing API.

The final environment/profile counts and compressed raw gate logs are recorded
in [results.json](results.json). Reproduction commands use the pinned Rust 1.85:

```sh
bash docs/validation/parallel-executor-2026-10-30/run-parallel-jazzy.bash
bash docs/validation/parallel-executor-2026-10-30/run-parallel-compat.bash
python3 docs/validation/parallel-executor-2026-10-30/run-isolation-preflight.py
```

Compatibility source/cache/image mounts are specific to the recorded host.
Equivalent native environments can run the same cargo gates directly.

## Final gates

| Native environment | Parallel cases | Executor-family cases | Optimized complete example |
| --- | ---: | ---: | --- |
| Jazzy / CycloneDDS | 13 debug and release | Full library/workspace coverage | Pass |
| Jazzy / Fast DDS | 13 debug and release | Full optimized library coverage | Pass |
| Humble / Fast DDS | 13 release | 92 release | Pass |
| Kilted / CycloneDDS | 13 release | 93 release | Pass |
| Kilted / Fast DDS | 13 release | 93 release | Pass |

Jazzy's full debug workspace suite and both optimized library suites each pass
722 main-library cases, with eight existing manual measurements ignored. All
22 main-library Rustdoc cases pass. Three focused public API Rustdoc cases
compile ownership usage and reject reuse of a moved Node or running executor.
Warning-denied all-target workspace Clippy passes in Jazzy, Humble and Kilted;
formatting and diff checks pass. The true no-ROS container compiles all targets
and verifies 46 public documentation artifacts and 150 methods, including all
four parallel execution types and their 19 methods.

Counts are top-level summaries; subprocess shutdown and guard tests retain their
own output in the full logs. The early Jazzy debug pass had 12 cases before the
native-storage oracle was added. `logs/jazzy-final-debug.log.gz` contains the final
13-case rerun under both RMWs; final workspace/optimized/compatibility gates also
include the storage oracle. The final source manifest is checked again before
archiving.

## Performance prerequisite

After all local builds/tests finished, the saved isolation preflight required
three consecutive idle samples with load1 at most 2.0 and no competing compilers
or the detected GPU compilation workload. It timed out: observed load1 ranged
from 4.98 to 6.06, with external Zig work in every sample and a GPU workload in
the first sample. `performance-isolation-preflight.json` retains all five
observations and the unchanged passing requirements. No performance measurement
started and no throughput, resource-use or C++ multithreaded advantage is claimed
for this slice. The pool remains an explicit capability API; ordinary serial
spin behavior is covered by the full gates.

Cancellation remains cooperative: a user callback must return before its own
worker can stop. Same-Node callback groups, dynamic running placement, work
stealing, native reactor integration, allocation/resource scaling and isolated
Rust/rclcpp multithreaded performance comparisons remain open work in the goal.
