# Driving Rust futures with ROS events

`Node::spin_until_future_complete` now installs a Rust waker that can interrupt the native
ROS wait. External channels and independently driven work can complete promptly while the
node processes services, actions, subscriptions, timers and graph events. The existing API
keeps a 10 ms periodic fallback for futures that do not notify their waker.

`spin_until_future_complete_with_options` exposes the waiting policy. Notification-driven
execution avoids periodic wakeups:

```rust
let options = r2r::FutureExecutionOptions::wake_driven();
let result = node.spin_until_future_complete_with_options(
    async {
        let input = receiver.await.map_err(|_| r2r::Error::RCL_RET_ERROR)?;
        client.request(&MyRequest { input })?.await
    },
    std::time::Duration::from_secs(3),
    options,
)?;
```

The [runnable example](../r2r/examples/future_execution.rs) joins an external channel with a
real service request/response and non-Send local state. It needs no async runtime or separate
ROS spin loop. The future itself stays on the calling thread. Independently running sources
must make progress themselves; this synchronous method does not run another runtime's I/O
reactor. A current-thread runtime reactor cannot progress while its only thread is inside a
blocking call to this method.

For a future that needs periodic polling, use
`FutureExecutionOptions::default().poll_interval(Duration::from_millis(5))`. Zero intervals
are rejected with `RCL_RET_INVALID_ARGUMENT`. An already-ready future completes even with a
zero timeout and does not create native wake storage. Pending zero-timeout work returns
`RCL_RET_TIMEOUT`. The overall timeout uses steady wall-clock time and is independent of ROS
simulated time. ROS and application errors remain propagated through the returned Result.

The guard condition is initialized lazily and reused by the owning node. One atomic state
coalesces concurrent notifications and coordinates waiting without lost wakeups. Self-woken
futures still drain ready ROS work with a zero wait, preserving callback progress. Once the
native wait returns, ROS callback delivery only requests another future poll and avoids a
redundant native trigger.

Run completion, timeout and unwinding disarm the waker. Stale wakers contain weak native
references and do not retain the node/context or complete subsequent work. RCL guard storage
has a stable address; trigger calls hold an Arc, excluding concurrent finalization. The
guard is registered only in its owning node's wait set. Weak context shutdown registrations
signal active waits before native invalidation, so notification-driven pending work is also
interrupted by `Context::shutdown`.

The [nearby tests](../r2r/src/nodes/future_executor/tests.rs) cover native wait interruption,
wakes during polling, 4000 concurrent notifications coalescing to one native trigger,
periodic-only futures, ready/zero deadlines, non-Send values, stale wakers, panic recovery,
service-request fairness, service appearance and shutdown. Native wait return codes verify
that a signal interrupted the wait instead of relying solely on elapsed-time thresholds.

```sh
env -u CYCLONEDDS_URI cargo test -p r2r --lib future_executor -- --test-threads=1
env -u CYCLONEDDS_URI RMW_IMPLEMENTATION=rmw_fastrtps_cpp \
  cargo test -p r2r --lib future_executor -- --test-threads=1
env -u CYCLONEDDS_URI cargo run -p r2r --example future_execution
```

[Independent Rust baseline/candidate evidence](benchmarks/future-wake-2026-10-28/README.md)
measures external notification-to-return latency. It is separate from DDS service/action
roundtrip or overall executor throughput comparisons.

Ten native cases pass under both Jazzy RMWs and in native Humble/Kilted environments. The
`future_execution` example and success/cancellation `action_lifecycle` example run with
notification-driven execution in all four environments. Iron, Lyrical and Rolling remain
supported CI matrix entries without new local runtime evidence in this slice. Full Jazzy
release/workspace checks, all-target Clippy and doc-only compilation pass.
