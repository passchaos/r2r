# Action-server cancellation progress

A cancel request may address many goals, and several cancel requests may await
application decisions at once. The native executor must register and finish
that work across bounded passes. Previously, polling sixteen ready decisions
from a forty-goal request left twenty-four unexamined decisions and entered the
requested 250 ms native wait. The complete response was not ready yet, so the
node did not report progress.

## Ready work and bounded passes

The ordinary path retains the existing small cancellation queues. A request
with more than sixteen decisions or a queue exceeding sixteen requests activates
an indexed queue. Large fanout delivery activates indexed goal retry state.
These states are boxed and allocated on demand; an ordinary successful
single-goal request creates neither bulk state nor per-decision readiness objects.

Each active request has stable goal indices and its own runnable decision queue.
The outer queue identifies runnable requests. A blocked decision registers a
one-shot weak waker and leaves runnable work. Its completion queues that goal
and request, with notifications after unlocking. Already completed decisions
are polled without allocating readiness objects. A maintenance pass considers at most sixteen request entries
and sixteen decision entries per examined request.
Large delivery examines at most sixteen runnable goal entries per pass.

Request slots are reused while a backlog is active. Generation checks discard
notifications from a previous occupant. Weak wakers do not retain request state
or native node ownership. Once the backlog ends, the ordinary path resumes and
the bulk queue allocation is released. Public node, action-client, server-stream
and goal APIs keep their existing shapes.

Full fanout cancel streams park their payloads by goal. Releasing one channel's
capacity retries that goal while other full streams stay parked. Remaining
runnable work schedules continuation; all-blocked work eventually stops
self-notifying. Removing a goal queues its stale delivery for disposal. If a
goal disappears while its sender is detached, restoration checks the touched
indices and schedules disposal without scanning all parked goals.

## Ownership and stage handoff

A detached delivery completion temporarily owns the pending request. Maintenance
keeps its bulk wake binding until restoration completes. Finishing the last
delivery creates decision-registration work and notifies that handoff. The
same-thread driver receives a progress bit before a native wait; a bulk delivery
completion receives one followup pass even when it subsequently parks. That
followup has no runnable sender to rotate once every channel is blocked.

Background maintenance uses a weak reference to the existing node notification
allocation. Cached decision and capacity wakers consult its current binding,
including after stop, node removal and transfer to a fresh executor. No native
guard, polling timer or worker thread is added. The internal native server stores
one weak owner pointer; large cancellation state adds optional boxed pointers.

The wire protocol is unchanged. The server responds after delivery and every
decision resolve, retains the native candidate ordering, rejects abandoned
decisions, and attempts accepted native goal transitions before the response.
An accepted cancellation changes the goal to cancelling; the application still
finishes it with `goal.cancel(result)` and observes the independent result future.
Whole response construction and native status publication retain their native
whole-message behavior; the polling limits are not a bound on every RCL operation. Native conversion
still builds per-goal decision channels; the limits describe maintenance polling.

## Complete application and evidence

```sh
env -u CYCLONEDDS_URI cargo run -p r2r --example action_cancel_group
```

The current-thread Tokio application accepts forty goals, uses the public
`cancel_all()` API, accepts twenty cancellations and rejects twenty, then checks
all forty terminal results and explicitly stops the owned executor. It has no
hand-written ROS spin loop. Both result endpoints use KeepLast(64) for the
forty-result burst; cancel streams retain minimal capacity. Native tests use two
concurrent forty-goal requests to fill every stream and recover only the final
goal first. Typed/untyped cases also transfer the blocked node to another
executor and verify that the old guard is not triggered. A two-goal case covers
the ordinary small-stream transfer path.

```sh
env -u CYCLONEDDS_URI cargo test -p r2r --lib action_servers::cancel:: -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib server_cancel_ -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib ready_fanout_cancel_decisions -- --test-threads=1
```

Nearby oracles cover 0/1/16/17/40/128 boundaries, all-completed and all-blocked
work, exact ready selection, slot reuse, abandoned decisions, removed-goal
restoration, weak ownership and unlock-before-wake. Runtime/profile,
distribution and ordinary-path performance results are tracked in
[release validation](RELEASE_VALIDATION.md).

This supplies cancellation registration, capacity recovery and finite progress.
Allocation/resource accounting, cross-server scheduling, native ingress and
larger mixed workloads still need scaling and C++ comparison evidence. It does
not establish universal action throughput or tail-latency leadership.
