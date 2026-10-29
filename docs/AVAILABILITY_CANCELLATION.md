# Availability and matching cancellation progress

Dropping a service/action availability, publisher/subscription matching, graph-change or
reliable-ACK future schedules cancellation on its owning native execution path. Cancellation
must progress even while an unrelated application future remains pending or the background
executor has no ROS traffic to process.

Two native regressions exposed incomplete progress. With 512 canceled availability futures,
the bounded cleanup consumed an initial notification and then entered an indefinite wait with
cancellation tokens still queued. An already-dropped service client retained its native
registration after its last observer was canceled. A separate foreground regression showed
that canceling an observer during a wake-driven Node wait did not trigger its native guard.

## Bounded cleanup and native lifetime

Each waiter cleanup pass consumes at most 16 tokens. Its returned state distinguishes live
observers from queued cancellation work. The executor retains targeted endpoint interest
while either remains and uses a zero native timeout only while cancellation work remains.
Every pass still prepares and dispatches other native work. No full waiter scan, extra timer
or worker is introduced.

After availability maintenance clears a dropped endpoint's last operation, preparation retries
destruction of that same identity before waiting again. Active service/action requests retain
their existing ownership contract. Cleanup-only graph or endpoint work remains visible even
when completion has already removed all senders and only stale cancellation tokens remain.
An ACK that completes during preparation may therefore release its dropped publisher in that
same pass. The future's completed result remains readable after native endpoint destruction;
an incomplete ACK continues to retain the endpoint until completion or cancellation.

Publisher matching and ACK state are read as separate bounded snapshots. A live ACK observer
retains the existing 10 ms query cadence because RCL provides no ACK wait-set event. Canceled
or completed ACK observers release the cadence; cleanup alone does not keep it active.

## Current executor binding

The existing lazy native guard binds all observer cancellation queues for Node future execution
and shared/background executors, including graph and subscription matching. Endpoints created
after binding inherit the owning queue's wake. Removing a node clears the shared wake; reusing
its cached foreground guard restores its own binding.

The existing cleanup queue stores the binding identity. Rebinding to a waker with the same
`will_wake` identity skips entity access. Changing or clearing the owner visits its entities
once. There is no additional Node metadata, per-spin entity binding scan, or eagerly allocated
guard on the already-ready future path. Lazy callback/QoS/sleep registries retain their own
native preparation hooks.

## Verification

Nearby [waiter tests](../r2r/src/availability.rs) verify exact 16-token bounds, preservation of
live observers, cleanup-only state and eventual return to inactive state. Native
[backlog tests](../r2r/src/nodes/executor/background/tests/availability_backlog.rs) exercise
512 cancellations per queue, healthy service/action/matching observers, typed/untyped dropped
endpoint release, graph debris and reliable ACK cadence removal. They inspect actual native
ownership and queue state while execution remains running, then verify indefinite idle waiting.

[Binding tests](../r2r/src/nodes/observer_notifications/tests.rs) cover cancellation during
foreground native waits for Node and shared executors, factories before/after binding, every
individual cancellation source including ACK, node transfer and cached-guard reuse. A separate
entity-access probe verifies that repeated same-owner bindings skip entity visits while owner
changes and unbinding visit them.

```sh
cargo test --locked -p r2r --lib availability -- --test-threads=1
cargo test --locked -p r2r --lib observer_notifications -- --test-threads=1
cargo test --locked -p r2r --lib --release availability -- --test-threads=1
cargo test --locked -p r2r --lib --release observer_notifications -- --test-threads=1
```

Run these with a sourced ROS environment and both supported RMWs. Reproduction logs and
distribution/profile evidence are recorded in [release validation](RELEASE_VALIDATION.md).
These progress guarantees do not establish a Rust/C++ throughput advantage; isolated
representative transport and resource measurements remain a separate requirement.
