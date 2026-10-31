# Timer reset event validation

`TimerResetEvents` is a lossless async counterpart to rclcpp's synchronous timer reset callback.
It applies to ordinary, synchronous callback, exclusive/reentrant grouped and lifecycle timers.
The first item replays resets before registration, rapid resets coalesce with exact counts, observer
replacement preserves unconsumed resets, and native destruction closes the stream after pending
delivery. A custom panicking waker is caught before control returns through the RCL C callback.

Jazzy and Kilted use `rcl_timer_set_on_reset_callback`. Humble predates that symbol in both RCL and
rclcpp; r2r supplies the same public contract through its centralized successful-reset boundary.
Reset state uses a lazy `OnceLock`, so timers with no observer add no heap allocation. The required
runtime matrix is Jazzy with CycloneDDS/Fast DDS, Humble with Fast DDS and Kilted with both RMWs.

The focused suite covers late registration, exact coalescing and totals, one-observer rejection,
observer handoff, explicit reset, lifecycle activation/reactivation, callback-group generation
reconciliation, all three timer ownership forms, panic containment and endpoint teardown. The
finite example performs three resets without spinning and verifies deterministic stream closure.

Both Jazzy RMWs pass six focused cases in debug and release, the example and workspace all-target
Clippy; their complete release suites each pass 822 cases with twenty manual stages ignored.
Humble/Fast DDS and Kilted/both RMWs pass the same six debug cases, example and focused Clippy. The
true no-ROS gate compiles every target and verifies 84 public pages plus 341 required methods.

```sh
bash docs/validation/timer-reset-events-2026-10-31/run-jazzy.bash
bash docs/validation/timer-reset-events-2026-10-31/run-compat.bash
bash docs/validation/timer-reset-events-2026-10-31/run-no-ros.bash
python3 docs/validation/timer-reset-events-2026-10-31/verify.py
```

Every compilation is pinned to one CPU with one Cargo/Rayon worker and a 10 GiB virtual/container
memory ceiling. This is API, ownership and compatibility evidence; it makes no timer latency claim.
