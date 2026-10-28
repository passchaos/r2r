# Lifecycle publication

`LifecycleNode::create_publisher` and `create_publisher_with_options` create
managed publishers, including when explicit network-flow options are needed. A publisher starts
deactivated unless the node is already active. Successful lifecycle transitions
reconcile the shared activation flag with the settled state; activation failure,
deactivation, shutdown and error recovery leave publication disabled whenever
the settled state is not active. Clones share that flag. `on_activate` and
`on_deactivate` also permit explicit publisher control.

All three publication paths check activation when they publish:

| Path | Activated | Deactivated |
| --- | --- | --- |
| `publish(&message)` | Convert and publish the Rust message | Return `Ok(())` without publication |
| `publish_native(&mut native)` | Publish the caller's reusable C message | Return `Ok(())`; preserve message fields, storage address and capacity |
| `borrow_loaned_message(&mut node)?.publish()` | Transfer the native loan to DDS | Return the loan to the middleware without publication |

Loaning remains available while deactivated, when the middleware supports it.
The returned `LifecycleLoanedMessage` exposes native fields through
`Deref`/`DerefMut` and keeps the owning node mutably borrowed. Activation is
checked at publication, so a loan borrowed before manual deactivation cannot
publish while deactivated; a loan borrowed while inactive may publish after
manual activation. Dropping an unpublished loan returns it. Unsupported loaning
reports `RCL_RET_UNSUPPORTED`; borrowing with another node reports
`RCL_RET_INVALID_ARGUMENT`. Native publisher errors still propagate on activated
publication. As with rclcpp, a gated drop returns success, including after the
owning node has gone away. Inspect `is_activated` when that distinction matters.

The activation check does not wait for concurrent in-flight publications.
A publication that observes activation may finish during deactivation, and
already-published DDS data is not retracted. Holding a native loan also holds
the ordinary publisher's native lock; drop or publish it before querying the
same endpoint again.

Endpoint QoS, identity, matching, network-flow and event queries, liveliness,
subscriber waits and acknowledgement waits remain available in every state.
The public wrapper no longer implements `Deref<Target = Publisher<T>>`: that
previously let `publish_native`, loan publication, explicit dereferencing and
cloning the underlying publisher bypass activation. Call the managed
publisher's methods directly. Code that needs an ordinary `Publisher<T>` should
create a separate ordinary publisher through `Node`. Use
`publisher.destroy(&mut node)` for immediate endpoint cleanup; other clones
then report stale native handles on activated publication. Passing a foreign
node consumes that clone without destroying endpoints there, following
`Node::destroy_publisher`. Dropping the last clone schedules normal spin cleanup.

Run the complete service-driven workflow:

```sh
env -u CYCLONEDDS_URI cargo run -p r2r --example lifecycle_publication
```

It hands the managed node to the shared executor, invokes native `change_state`
services, and verifies typed/native messages, acknowledgements and middleware
loans when supported. Only the active phase delivers messages; bounded receive
waits verify the unconfigured, inactive, deactivated and finalized phases.

The correction preserves reusable native storage and in-place loan filling.
It adds the same activation check required by typed lifecycle publication,
without changing ordinary publisher or executor paths. This is a correctness
and API improvement; no end-to-end performance advantage is claimed.

Focused runtime and public API gates:

```sh
env -u CYCLONEDDS_URI cargo test -p r2r --lib lifecycle:: -- --test-threads=1 --nocapture
env -u CYCLONEDDS_URI cargo test -p r2r --lib --release lifecycle:: -- --test-threads=1 --nocapture
python3 tests/lifecycle_publication_api.py
```

The compiler gate checks one usable public API example and six rejected
publication/lifetime escapes. It is part of the native Jazzy quality CI job.
Runtime evidence and distribution boundaries are recorded in
[release validation](RELEASE_VALIDATION.md).
