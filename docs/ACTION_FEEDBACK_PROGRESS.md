# Action feedback capacity notifications

Typed and untyped feedback streams keep the same bounded message channels and
one retained payload per goal. A full channel previously requeued its goal with
a noop sender waker. Repeated maintenance could self-notify while the consumer
was blocked, yet consuming a message did not notify native delivery.

`PendingFeedbackQueue` now contains runnable retry identities. A goal whose
sender remains full registers a one-shot readiness waker and leaves the runnable
queue. Releasing channel capacity enqueues that goal and notifies the existing
action-client maintenance queue after unlocking. Repeated notifications from
one registration coalesce; a subsequent pending poll re-arms the channel.

The queue tracks blocked registrations separately from runnable identities.
Forty initially blocked goals are examined in batches of at most sixteen,
then produce no further self-notifications. When only the last goal becomes
writable, only that UUID returns to the runnable queue. Stale retries after
channel closure are discarded. Wakers hold weak queue references, so they do
not retain native clients/nodes; dropping a cached registration releases its
blocked count. A transferred node's notifications use its current queue binding.

Ready retry work remaining after a bounded pass returns a progress bit through
the existing maintenance call, allowing another pass before a long same-thread
wait. Actual blocked feedback alone does not return that bit. No new worker,
timer or native guard is created. FeedbackChannel and client handle layouts stay
unchanged; the shared pending queue adds one blocked-registration counter.
Readiness objects are created only for pending retries, rather than normal
successful sends. The feedback implementation and close-by tests live in
`action_clients/feedback.rs` and `feedback_tests.rs`.

The original feedback ingress policy remains bounded by its retained native
and Rust payload capacities. This change supplies capacity notification and
retry progress; it does not provide an unlimited lossless queue or independently
scheduled native feedback subscription for each goal. Middleware feedback
traffic and larger payload/fanout scaling remain separate evidence.

Run a complete slow-consumer application:

```sh
env -u CYCLONEDDS_URI cargo run -p r2r --example action_feedback_backpressure
```

It accepts a goal, sends two ordered feedback samples into a minimal channel,
waits while the Tokio consumer is delayed, verifies both samples and then a
successful result. There is no hand-written ROS spin loop and no extra ROS
message to make capacity recovery progress.

Focused gates:

```sh
env -u CYCLONEDDS_URI cargo test -p r2r --lib feedback_tests:: -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib feedback_capacity_recovery -- --test-threads=1
```

The unit oracles cover finite scans, UUID selection, notification coalescing,
stale registration ownership and reentrant sender restoration. Native cases
cover typed/untyped streams, repeated full/ready cycles, ordered payloads and
the same-thread future driver. Performance and distribution evidence is tracked
in [release validation](RELEASE_VALIDATION.md).
