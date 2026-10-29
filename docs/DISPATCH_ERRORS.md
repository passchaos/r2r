# Independent ready work and callback errors

`Node::try_spin_once` and shared executor dispatch attempt independent entities from the
current native readiness snapshot before returning the first operational error. A failed
subscription or service callback therefore does not skip ready timers, service responses,
ordinary request streams, other callbacks or action operations in the same node.

Each failing endpoint or action channel stops its current bounded take loop. Other ready
channels still receive their own existing budget: subscription options retain their configured
limit, service requests retain their endpoint limit, and action feedback/goal/cancel/result
takes retain sixteen-candidate limits. Native QoS event errors continue to close the affected
observer while other ready registrations and ordinary entities are dispatched. Error reporting
uses one stack-local first-error outcome; it adds no node state, error list, timer or worker.

Native readiness sampling and dispatch ownership remain on the owning execution thread. Action
completion construction stays under the existing client/server mutex boundary; actual delivery,
conversion, cleanup notifications and captured payload retirement retain their existing
post-lock completion paths. Ordinary and action dispatch live in separate nearby modules.

The result describes this one dispatch attempt. RMW readiness can still yield an empty take,
and backpressure still parks work according to its channel contract. Invalid native wait-set
layout checks remain errors. Panics continue to unwind normally. Future execution returns the
dispatch error, and a background task still follows its existing failure/ownership contract;
this does not introduce automatic retries or change how long native endpoints remain owned.

## Preserve an application future when recovering

Passing an owned future into a completion call transfers its ownership for that call. To
report a recoverable error and resume the same application state, retain the pinned future
in the caller and pass `application.as_mut()` on each call. Use one overall deadline so
retries do not restart the application's timeout or issue its requests twice.

The [runnable example](../r2r/examples/dispatch_recovery.rs) publishes an invalid input,
reports exactly one callback error and resumes one retained non-Send application future. A
healthy native service returns the exact sum and request sequence, and a wall timer completes.
The example uses wake-driven execution with no async runtime or secondary spin thread.

```sh
cargo run --locked -p r2r --example dispatch_recovery
```

## Native and channel verification

[Nearby native tests](../r2r/src/nodes/spin/dispatch/tests.rs) rebuild actual RCL wait sets
without consuming data until the requested subscription/timer/client/service/action families
are jointly ready. One dispatch must run healthy callbacks, expose an ordinary request,
complete the exact client response identity, deliver the timer tick and expose the action
goal despite errors. No later spin can rescue a skipped entity.

An [allocation-failure case](../r2r/src/nodes/spin/dispatch/tests/client_allocation.rs) fails one
response allocator before its RMW take and verifies another client's exact response in that
same snapshot. It retains both native endpoints and the unread failed response. Distinct
service reply topics exclude a foreign client's response from the native readiness oracle.

[Action-channel cases](../r2r/src/nodes/spin/dispatch/tests/actions.rs) inject multiple channel
failures, preserve the first error, verify exact later response values and sixteen-take bounds,
reject unready channel access and acquire the actual client mutex during every completion.

```sh
cargo test --locked -p r2r --lib ready_dispatch_ -- --test-threads=1
cargo test --locked -p r2r --lib --release ready_dispatch_ -- --test-threads=1
```

Run the native cases under each supported middleware and distribution. This closes independent
dispatch progress after operational errors; concurrent workers, wide entity scaling and
isolated representative Rust/C++ performance evidence remain separate work.
