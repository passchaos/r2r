# Graceful process-signal shutdown validation

`Context::install_signal_handler` is an explicit process-wide SIGINT/SIGTERM
bridge. The async-signal-safe handler wakes a dedicated thread; native shutdown,
locks and observer callbacks execute on that ordinary thread through the existing
idempotent `Context::shutdown` path.

Nine context integration cases pass under Jazzy/CycloneDDS, Humble/Fast DDS and
Kilted with both CycloneDDS and Fast DDS. The signal case
raises real SIGINT while an empty background Executor owns a native wait, checks
that it returns its Node, creates a fresh context, re-installs the process target
and raises SIGTERM. The standalone example is also launched as a child process,
sent SIGINT and required to print a successful one-Node recovery before exiting
with status zero.

`Context::wait_for_shutdown` supplies a fused runtime-agnostic Future without a
polling thread. Tests cover manual and signal completion, ready-after-shutdown,
dropped observation and implicit final-owner shutdown without retaining Context.

The complete optimized Jazzy main-library suite passes 773 cases with eight
manual measurements ignored under both RMWs. Its first CycloneDDS run exposed an
unrelated action-QoS test that queried the asynchronous graph cache immediately
after endpoint creation. The test now uses the existing register-before-query,
two-second bounded graph-change helper; the isolated counterexample and both
complete reruns pass. Library Clippy passes with warnings denied.

The opt-in API does not alter existing applications or install handlers merely
because r2r is linked. The handler remains registered for the process lifetime;
later calls retarget its weak context reference and do not retain finalized ROS
resources.

```sh
bash docs/validation/graceful-shutdown-2026-10-30/run-jazzy.bash
bash docs/validation/graceful-shutdown-2026-10-30/run-compat.bash
bash docs/validation/lazy-typesupport-2026-10-30/run-compat.bash no-ros
```

Both compilation paths use one allowed CPU and a memory limit of 10 GiB or less.
`results.json` binds sources and compressed gate logs. Windows signal semantics
remain outside this Unix API and the required Humble/Jazzy/Kilted validation
scope.
