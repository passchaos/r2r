# Native timer callback validation

The [timer callback API](../../TIMER_CALLBACKS.md) adds direct synchronous wall/ROS handlers,
manual startup, one-shot rearming and owner-scoped native controls. A pinned allocation owns
the native timer, handler and optional steady clock. A lazy registry provides a disjoint native
timer span and targeted current-owner cleanup without an ordinary tick channel.

Eleven Jazzy cases verify periodic/one-shot/reset/period behavior, every owning/foreign control
boundary, native address stability through registry growth, captured-state retirement without
context/clock locks, dropped-token suppression after a completed wait, paused/backwards ROS
clock deadlines and exact native expected/actual/elapsed time. Error and panic cases retain the
native endpoint for reset, while ready peers preserve dispatch progress. Native shared tests
mix callback/stream/sleep timers with service and action spans; background and returned-node
cases observe current weak guard triggers and timely retirement while execution remains alive.

The [example](../../../r2r/examples/native_timer_callbacks.rs) validates 32 exact callback-driven
publications, explicitly resumes a canceled wall timer, observes one-shot ROS completion and
destroys native resources. The Rustdoc counterexample rejects an Rc captured by a callback that
can move to a native executor thread.

## Recorded gates

Jazzy debug workspace and both RMW optimized library runs pass 698 main-library cases. Eight
manual measurement cases remain ignored. Nineteen Rustdoc cases, both optimized callback
examples, both current-thread Tokio workflows and warning-denied all-target Clippy pass.

Native Humble/Fast DDS and Kilted/CycloneDDS plus Fast DDS run optimized callback/timer/executor/
callback-family gates, the complete callback example and all-target Clippy. Humble executes ten
new cases; SDK call-info APIs and their exact timestamp test are excluded consistently with
ordinary TimerCallInfo support. Iron follows that same guarded API boundary but has no new
local runtime result here. Lyrical/Rolling remain supported CI entries without new local runtime
evidence in this slice.

| Native environment | Callback timer | Timer family | Executor | Callback family |
| --- | ---: | ---: | ---: | ---: |
| Humble / Fast DDS | 10 | 38 | 79 | 43 |
| Kilted / CycloneDDS | 11 | 41 | 80 | 45 |
| Kilted / Fast DDS | 11 | 41 | 80 | 45 |

These are matching top-level summaries; subprocess cases are included in the parent suite.

A container with no `/opt/ros` checks all targets, native ownership compile-fail examples and
public documentation. The required inventory contains 41 public artifacts and 109 methods,
including all callback timer factories, native controls and option builders.

`results.json` records executed summaries, RMW headers, parent commit and exact changed source
identities. The complete compressed logs retain examples, Clippy and actual filtered counts.
The archived scripts reproduce the native and container gates from the repository root:

```sh
bash docs/validation/timer-callbacks-2026-10-30/run-timer-callback-final-jazzy.bash
bash docs/validation/timer-callbacks-2026-10-30/run-timer-callback-compat.bash
```

The compatibility script records its images, cache/source mounts, domains and RMW choices.
Those cache paths are specific to the recorded host; equivalent sourced native environments
can run the same cargo gates directly.

## Scope

This is capability and ownership evidence. There is no claim of Rust/C++ throughput leadership
or completed isolated performance qualification. Plain callbacks avoid constructing a Rust tick
or channel send, but representative application/resource comparisons still need measured proof.
Lifecycle stream timers keep their existing contract; dedicated lifecycle callback wrappers and
concurrent callback workers remain separate roadmap items.
