# Background entity cleanup and action maintenance

Dropping a public endpoint queues its identity for the owning node's next
prepare/dispatch cycle. A background executor can wait indefinitely without
unrelated traffic; queued cleanup must wake that native wait itself. The
original idle-drop test failed: no guard notification occurred and publisher
and client native ownership persisted until the execution task was stopped.

Subscriptions, services, publishers, ordinary/managed timers and typed/untyped
service/action clients and action-server streams now schedule cleanup through a
notification queue. Background startup binds its existing weak guard waker to
these queues. The first entry in an empty queue wakes the native thread after
the queue lock is released; later entries coalesce until that queue is drained.
Per-entity identity/index removal and ordinary drop ownership remain unchanged.

The wake lives beside the existing queue storage in its shared allocation. The
public handle's Arc and the node's queue pointer retain their existing layout.
Ordinary nodes have no extra native guard or query timer. All wake calls occur
after the relevant identity is visible; no queue lock is held across the wake.
Removing a node from the executor unbinds the previous native wake for independent
reuse, including observer and action-server maintenance notifications.

## Pending operations and cancellation

An outstanding client response future can keep a dropped client registered.
Canceling its last response now removes the response state and wakes the native
owner to retry endpoint destruction. Both typed and untyped paths are covered.

Action result/feedback/cancel-observer cleanup queues use the same notification
boundary. Dropping a goal/status registration also notifies the owner. If the
initial endpoint-removal pass saw pending action work, maintenance can clear the
last operation later in the same preparation. That maintained identity is
retried before entering the next native wait, rather than waiting for traffic.

Server cancellation decisions previously used a noop waker. Background action
servers now register their existing native waker while polling decision futures.
Accepting/rejecting or abandoning a decision can therefore complete the cancel
response without an unrelated ROS event. A full cancel stream registers the same
waker for sender readiness so consuming a queued request schedules native retry.
Decision and delivery scans retain their existing bounded batch sizes.

An endpoint with outstanding work is not indiscriminately removed. Native
finalization stays on the node's execution thread. Dropping a service handle may
invalidate request response handles according to the ordinary service contract.

## Verification

Seven native background cases verify idle ordinary endpoint destruction,
last-response cancellation, typed/untyped action endpoint destruction,
typed/untyped action-observer cancellation, delayed cancel decisions and
unbinding a transferred node. Weak native ownership is observed while the task
is running; stopping the task cannot substitute for timely cleanup. Exact native
registry counts are checked after the idle-drop exchange. Unit cases verify
unlock-before-wake, coalescing, decision wakeup and backpressure readiness.

```sh
env -u CYCLONEDDS_URI cargo test -p r2r --lib executor::background::tests::cleanup:: -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib notifications:: -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib action_servers::tests -- --test-threads=1
```

Release/profile and distribution evidence is tracked in
[release validation](RELEASE_VALIDATION.md). Default executor A/B measures the
notification queue's effect on ordinary nodes; it does not claim that cleanup
notifications improve application throughput. The improvement is native
resource release and cancellation progress without polling or unrelated input.
