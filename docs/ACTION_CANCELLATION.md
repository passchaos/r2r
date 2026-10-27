# Action cancellation progress and spin waits

After an application accepts or rejects `ActionServerCancelRequest`, the owning node applies
the decision during the next `try_spin_once`. Before this change, it could send the cancel
response and change the native goal state, then enter `rcl_wait` for the full caller timeout.
The application's next poll of `is_cancelling()` and terminal `cancel(result)` could therefore
be delayed by an otherwise idle 100 ms spin. The independent-process action oracle reproduced
this delay even though native cancellation responses were delivered correctly.

The node now treats completed local cancel responses as progress. It drains native events
already ready with a zero wait and returns so application logic can observe the transition.
It keeps the requested timeout when cancel decisions are unresolved or delivery is backpressured.
This avoids replacing the delay with a busy loop. Timeout argument validation still runs before
the effective native wait is shortened. Cancellation acceptance and rejection both follow this
rule; terminal results remain an explicit application operation.

Three nearby tests verify the boundary through native action requests:

```sh
cargo test -p r2r --lib action_cancel_tests -- --test-threads=1
cargo test -p r2r --lib --release action_cancel_tests -- --test-threads=1
```

The tests require completed decisions to return well before a 250 ms wait expires, unresolved
decisions to retain an idle wait, and a zero-capacity cancel channel to remain backpressured
without unnecessary spinning. They check native goal state, accepted/rejected cancel responses
and terminal result payloads. Both local RMWs plus Humble and Kilted provide the compatibility
boundary; the optimized Jazzy library suite remains the release gate.

The [action benchmark](../tools/bench_ros/ACTION.md) measures success and cancellation lifecycles
through both language directions. It includes explicit observer-readiness and feedback
acknowledgement handshakes so exactly one feedback is consumed before a terminal result. Those
auxiliary requests belong to its latency boundary and must not be interpreted as bare action
protocol latency. The [deterministic example](../r2r/examples/action_lifecycle.rs) uses local
futures-rs handshakes and a wall-clock deadline to demonstrate deferred execution, feedback,
success and cancellation without randomized outcomes or an async-runtime dependency.
