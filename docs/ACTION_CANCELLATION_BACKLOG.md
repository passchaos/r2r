# Cancellation cleanup across bounded action batches

Action result, cancel-response and feedback observers report their cancellation
to the owning client. Each maintenance pass examines at most sixteen entries
per queue. Previously, a pass could consume the original notification, remove
the first sixteen entries and enter a native wait while more entries remained.
Forty canceled observers therefore needed an unrelated ROS event to finish
cleanup.

The client now re-arms its existing maintenance notifier when a bounded drain
leaves **unexamined** entries. It does so after releasing the cancellation queue
lock. Background execution wakes the existing native guard; same-thread future
execution uses a nonblocking spin when a cancellation queue exceeds one pass.
The original per-pass limits and per-client traversal are retained.

The common helper lives beside action-client implementation in
`action_clients/cancellation.rs`. It handles typed/untyped result, cancel and
feedback cleanup. Queue layouts, message storage, response routing and native
entities stay as they were. There is no readiness epoch, extra worker, polling
timer, native guard or new synchronization state in the ordinary action path.
The maintenance result carries the unexamined-work bit from the bounded drain;
it does not make an additional queue query to decide whether to wait.

Canceled entries are checked against their current sender/receiver state, so a
live result/cancel observer is retained and stale identifiers are harmless.
Feedback entries temporarily unavailable because a sender is detached are
requeued after the drain; that pass reports them as retries, not newly unexamined
backlog. An empty/final batch
does not re-arm its notifier. Disabling client maintenance still suppresses
future notifications.

Six dedicated cases cover boundary sizes 0/1/16/17/40/128, exactly bounded drains,
unlock-before-wake, complete cleanup order, active observers, stale tokens,
detached-feedback retry and live typed/untyped forty-observer cancellation.
The native cases freeze client maintenance until every cancellation is visible,
keep goal/status handles alive and wait for cleanup while the executor task is
still running. Same-thread preparation independently verifies a zero timeout
when entries remain after the first pass.

```sh
env -u CYCLONEDDS_URI cargo test -p r2r --lib action_clients::cancellation:: -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib canceled_observer_backlog -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib cancellation_backlog_does_not_sleep -- --test-threads=1
```

This closes canceled-observer cleanup continuation. Pending feedback delivery,
large server cancel-decision/delivery sweeps and calibrated multi-goal performance
remain separate work. The broader [rejected batch-progress prototype](benchmarks/action-batch-progress-2026-10-29/README.md)
retains those counterexamples; its scan-state changes are not used here.
