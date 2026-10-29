# Await native QoS diagnostics

Publishers, subscriptions and scoped native callback subscriptions expose `qos_events(&mut node,
kind)`. `QosEventStream` implements `Stream<Item = r2r::Result<QosEvent>>`, so diagnostics compose
with message handling, services and runtime I/O. Events enter the same native wait set used by
`Node`, the shared `Executor` and background `ExecutorTask`; no diagnostic polling timer is added.

```rust
use futures::StreamExt;
use r2r::{PublisherEventKind, QosEvent};

let mut events = publisher.qos_events(&mut node, PublisherEventKind::DeadlineMissed)?;
let application = async {
    while let Some(event) = events.next().await {
        if let QosEvent::OfferedDeadlineMissed(status) = event? {
            report_deadline(status.total_count, status.total_count_change);
        }
    }
    Ok(())
};
```

Drive the node or executor while awaiting the application. The runnable example does this through
`spin_until_future_complete_with_options(..., FutureExecutionOptions::wake_driven())`:

```sh
cargo run -p r2r --example qos_event_diagnostics
```

Publisher kinds cover incompatible QoS, offered deadline, lost liveliness, matching and
incompatible type. Subscription kinds cover incompatible QoS, requested deadline, changed
liveliness, lost messages, matching and incompatible type. Matching/type kinds are exposed from
Iron onward. The selected RMW may reject an event with `RCL_RET_UNSUPPORTED`; registration then
leaves no observer or native event behind. Lifecycle publisher events remain observable in every
activation state. Event enums are nonexhaustive to allow future native status kinds.

One endpoint/event kind has one observer. Duplicate registration and manual `take_*` calls while
the stream owns that counter return `QosEventAlreadyObserved`. Other event kinds and endpoint
message traffic proceed independently. Dropping the stream schedules cleanup; spin once before
registering a replacement, or call `Node::destroy_qos_event_stream` for immediate removal.

The Rust side retains one taken snapshot. When full, only that event is omitted from the next
native wait. Further changes accumulate in the middleware's cumulative/change counters until the
slot is read. Reading wakes the current executor to rearm the native event. This is bounded status
observation: it preserves native counter semantics and does not represent every deadline miss as
a separate Rust item. Slow observers do not create an unbounded queue or a native-ready busy loop.

Destroying an endpoint finalizes its events before finalizing the publisher/subscription. An
already-taken snapshot is delivered first, followed by one endpoint-invalid error and stream
termination. Dropping a node releases its native context lock before terminal observers are
woken. Waker notification and retirement occur outside the stream slot lock.

Some RMW wait caches retain event status conditions after an event list or ordinary endpoint
membership changes. Event-bearing wait sets therefore invalidate their native cache only when
the populated layout changes, preserving ordinary and action offsets during reconstruction.
Stable layouts reuse storage. Cache invalidation is isolated in `nodes/wait_set/event_layout.rs`;
future generation-based membership tracking can replace its layout collection without changing
the stream ownership, native counter or backpressure contracts. Nodes without observers allocate no registry/channel/event-array
storage, create no additional guard and use the original ordinary wait path.

Run the nearby native boundaries with the ROS environment sourced:

```sh
cargo test -p r2r --lib qos_event -- --test-threads=1
cargo test -p r2r --lib --release qos_event -- --test-threads=1
```

These cases cover native incompatible/deadline statuses, duplicate/manual observer ownership,
bounded counters and idle waits, external consumption wakes after executor transfer, endpoint
and node destruction, shared-node spans, action response indices after same-count membership
changes, all event kinds and unsupported rollback, stable layout reuse and waker lock reentry.
The independent Rust/rclcpp oracle checks offered/requested reliability incompatibility in both
directions; this establishes diagnostic behavior, not end-to-end performance leadership.
