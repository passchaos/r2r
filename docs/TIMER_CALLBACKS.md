# Native synchronous timer callbacks

`Node::create_wall_timer_callback` registers a steady-clock periodic handler, and
`create_timer_callback` follows the node's managed ROS clock. They run directly during native
Node/shared/background dispatch. No tick channel or async runtime is needed for synchronous
work. The [runnable example](../r2r/examples/native_timer_callbacks.rs) publishes and validates
32 typed values, explicitly starts a paused wall timer, observes a one-shot ROS timer and
destroys the endpoints.

```rust
let timer = node.create_wall_timer_callback(
    std::time::Duration::from_millis(10),
    move || publisher.publish(&message),
)?;
```

`CallbackTimerOptions::autostart(false)` initializes the native timer as canceled.
`timer.reset(&mut node)` starts a fresh native period. `oneshot(true)` cancels the native timer
after its call is recorded and before invoking the handler; it remains canceled even when
the handler returns an error or panics. Explicit reset rearms it.

The ownership token exposes clock type, validity, cancel/reset, period exchange and signed
native timing queries. Operations require the owning node, like ordinary timer controls;
a foreign node returns `RCL_RET_TIMER_INVALID`. `Node::destroy_callback_timer` removes the
exact endpoint immediately. Dropping a token marks it inactive and schedules owner-thread
cleanup through the current weak native guard. A callback already in progress can finish;
a dropped token from a completed wait snapshot does not start a new call.

## Timing and error behavior

On Jazzy and newer SDKs, the `_with_info` factories receive `TimerCallInfo`. Expected/actual
call timestamps come from `rcl_timer_call_with_info`; elapsed time follows the existing timer
query semantics. Humble/Iron support plain callbacks and signed token queries without these
newer timestamp factories. ROS-clock timers pause with simulated time and follow native
deadline adjustments across backwards jumps.

Native time advancement happens before user code. Each ready timer is invoked at most once
per native dispatch; missed periods follow RCL behavior rather than an application catch-up
loop. Handlers must do bounded synchronous work. A handler's `Result` participates in
[independent ready dispatch](DISPATCH_ERRORS.md), so other ready work runs before the first
operational error is returned. Panics unwind normally in Rust; handlers are not installed as
C function pointers. The timer stays registered for explicit reset, later calls or cleanup.

Handlers are `Send` because node ownership can move to an executor thread. Timing operations,
native teardown and user callbacks remain on the owning execution thread. User code and
captured-state retirement hold no notification, context or managed-clock lock. Initialization
borrows the pinned timer so failed construction releases captured state after those locks.

On `LifecycleNode`, the same factory names return lifecycle-managed callback registrations.
[Managed callback timers](LIFECYCLE_CALLBACK_TIMERS.md) pause natively outside active state and
acquire an activation-generation permit before advancing the native timer. Explicit
`Node::create_timer_callback(&mut managed, ...)` remains available for regular registrations.

## Storage and validation

A lazily allocated callback registry owns pinned boxes containing native RCL timer storage,
handler state and optional wall-clock ownership in one allocation. Native addresses survive
vector growth and node transfer. Plain handlers call the native timer without constructing
elapsed metadata or a tick message. Ordinary timer storage and channels retain their existing
layout. The separate registry and readiness span preserve the route to future callback
scheduling without introducing a per-timer mutex or per-callback thread.

[Nearby cases](../r2r/src/nodes/timer_callbacks/tests.rs) cover periodic/one-shot/reset controls,
native pointer stability, foreign ownership, construction/drop/destruction retirement, token
drop after native wait, ROS clock pause/backwards jumps, exact SDK timing fields, error/panic
recovery, mixed shared timer/sleep/service/action offsets and current guard bindings after
background startup and returned-node reuse. A Rustdoc counterexample rejects non-Send captures.

```sh
cargo test --locked -p r2r --lib callback_timer_ -- --test-threads=1
cargo test --locked -p r2r --lib --release callback_timer_ -- --test-threads=1
cargo run --locked --release -p r2r --example native_timer_callbacks
```

Distribution/profile evidence is tracked in [release validation](RELEASE_VALIDATION.md).
This provides a synchronous timer API; isolated representative Rust/C++ throughput and resource
comparisons remain necessary for performance claims.
