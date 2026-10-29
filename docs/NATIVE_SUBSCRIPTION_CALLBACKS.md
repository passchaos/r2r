# Scoped native subscription callbacks

Ordinary `Subscription<T>` streams remain the default ergonomic receive API. They construct an
owned Rust message and place it in a bounded futures channel, so messages can outlive the node spin
that received them.

For forwarding, filtering and native codecs where ownership is unnecessary,
`Node::subscribe_native_callback` invokes a closure during native node dispatch:

```rust
let output = node.create_publisher::<MyMessage>("output", qos.clone())?;
let forward = output.clone();
let callback = node.subscribe_native_callback::<MyMessage, _>(
    "input",
    qos,
    move |message, info| {
        inspect_metadata(info);
        forward.publish_native_ref(&message)
    },
)?;
```

The callback receives `NativeMessageRef<'_, T>`, a lifetime-bound, read-only view with private
provenance, not the owning `NativeMsg<T>` wrapper. It dereferences to `T::CStruct` for direct
inspection, but safe Rust cannot move the native C struct contents, allocation, or deallocator out
of the invocation.
r2r drops the independently created native container after the callback returns.
`Publisher::publish_native_ref` accepts only this scoped view and publishes directly from it. This avoids a
native-to-owned Rust deep copy and a channel send without assuming that an RMW can safely reuse
populated dynamic sequence storage across takes. The ordinary stream, raw, untyped and loaned
receive APIs are unchanged.

Callbacks must be `Send` because their owning node can move between executor threads. They run
synchronously inside `Node::spin_once`/`Executor` dispatch: do bounded work, do not call a spin
method recursively, and return an `r2r::Result`. An error is returned by the active spin operation.

[Independent ready dispatch](DISPATCH_ERRORS.md) continues ready timer/service/action work
after a callback error and reports the first error after that snapshot is dispatched.
Panics remain normal Rust unwinding and never cross a native callback boundary.
Native storage is released when a callback returns an error or unwinds. The taken sample is
consumed; the subscription remains registered and can receive later samples. Failure to allocate
the independent take container returns `RCL_RET_BAD_ALLOC` before taking a sample, so a later spin
can retry the unread message.

`NativeCallbackSubscriptionOptions::max_messages_per_spin` controls the throughput/fairness
boundary. The default is sixteen, matching stream subscription batching. Higher explicit values
drain larger native bursts in one dispatch; a value of zero is rejected. Endpoint options for
ignoring local publications and requiring a unique network flow are also available.

The returned `NativeCallbackSubscription` is an ownership token with topic, QoS, publisher-count
and publisher-wait helpers. Dropping it schedules normal cleanup; use
`Node::destroy_native_callback_subscription` for deterministic native teardown.
Cleanup also releases captured application state and completes pending publisher waits with the
subscription-invalid error. Other subscriptions retain their endpoint identities.

The scoped implementation and its unit oracles live in `subscribers/native_callback/`. The real
middleware cases count native allocations/destructions across decreasing dynamic payload sizes,
reject any owned Rust decode, inject allocation failure, recover from callback errors and panic,
and check bounded burst dispatch alongside a ready ordinary stream.

Run the forwarding example without a hand-written spin loop:

```sh
cargo run -p r2r --example native_callback_forward
```

The [pipelined/fanout evidence](benchmarks/pubsub-pipeline-2026-10-29/README.md) compares the scoped
callback path against the historical owned stream and same-run rclcpp under CycloneDDS and Fast
DDS. It also retains the larger window×fanout boundary that failed calibration.
