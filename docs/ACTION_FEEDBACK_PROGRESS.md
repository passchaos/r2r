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

## Choosing feedback overflow behavior

`ActionClientOptions::feedback_overflow_policy` selects Rust-side buffering
semantics for every goal on that client. The default `Backpressure` retains a
pending sample and pauses shared native feedback ingress until channel capacity
recovers. A slow goal can therefore delay other goals on the same action client.
The native feedback topic is shared; separate per-goal Rust queues cannot provide
unlimited independent retention behind that shared finite native queue.

Applications that need current progress can select `FeedbackOverflowPolicy::Coalesce`:

```rust
let client = node.create_action_client_with_options::<MyAction>(
    "work",
    r2r::ActionClientOptions::default()
        .feedback_channel_capacity(0)
        .feedback_overflow_policy(r2r::FeedbackOverflowPolicy::Coalesce),
)?;
```

Coalescing keeps samples already in the goal's channel in FIFO order and retains
one latest pending item. A subsequent arrival replaces that pending item while
native ingress continues for other goals. Intermediate pending items can be
omitted; untyped JSON-conversion errors follow the same item policy. Native DDS
QoS and history remain separately configurable. With one futures-mpsc sender,
the channel holds its configured capacity plus one reserved slot; together with
the pending item, Rust retains at most `feedback_channel_capacity + 2` items per
tracked goal. Closing the feedback observer or completing a goal retains the
existing stream-cleanup behavior, including disposal of unsent pending items.

The selected policy fits in the private blocked-registration count's high bit;
registration checks its numeric boundary. Channel policy uses existing padding.
Layout oracles compare queue/channel headers against their former shapes for
ordinary typed and JSON feedback. Delivery, replacement and payload retirement
run in completions after releasing the client lock. The sender remains detached
during `try_send`, and its restoration preserves a newer reentrant item if an
older attempted send fails. Superseded payloads are destroyed after releasing
the channel/readiness locks. This introduces no native guard, timer or worker.

The implementation keeps overflow semantics separate from native decoding and
payload storage. Future native-storage reuse or deferred decoding can replace
those private paths while preserving the public item/retention contract and
bounded per-goal state. Coalescing currently decodes each taken native sample;
allocation and complete transport profiling remain additional work.

The complete `action_feedback_coalesce` application leaves one goal unconsumed,
publishes thirty-two updates and verifies another goal's feedback first. It then
checks the queued sample 0, latest pending sample 31, both distinct terminal
results and explicit executor stop:

```sh
env -u CYCLONEDDS_URI cargo run -p r2r --example action_feedback_coalesce
```

Native typed/untyped cases compare both policies with the same thirty-two-sample
input: Backpressure delivers all samples after capacity recovery; Coalesce
delivers the queued prefix and newest pending sample while the other goal
progresses. Background cases stop the node, release capacity while unbound,
transfer it and require the latest sample without another ROS message. Nearby
oracles cover thousands of replacements, capacity 0/1/8, detached success/failure,
payload destructor reentry, weak stale wakes and packed-count boundaries.

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
