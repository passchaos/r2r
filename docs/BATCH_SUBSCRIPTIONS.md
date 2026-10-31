# Batched subscriptions

`Node::subscribe_batches::<T>` delivers safe Rust messages acquired by one native
`rcl_take_sequence` call. `Node::subscribe_native_batches::<T>` exposes the matching generated C
messages without decoding them into owned Rust values. Both return an ordinary async
`Subscription`, so graph, QoS, content-filter and cancellation helpers remain unchanged.

```rust,no_run
use futures::StreamExt;
use r2r::{std_msgs::msg::UInt64MultiArray, QosProfile};

# async fn receive(node: &mut r2r::Node) -> r2r::Result<()> {
let mut batches = node.subscribe_batches::<UInt64MultiArray>(
    "/samples",
    QosProfile::default().keep_last(64),
    16,
)?;
while let Some(batch) = batches.next().await {
    for (message, info) in batch.iter() {
        println!("{} values from {:?}", message.data.len(), info.publisher_gid);
    }
}
# Ok(())
# }
```

The maximum batch size must be greater than zero. A delivered batch contains between one and that
maximum; it is not padded and does not wait for a full batch. The RMW operation preserves the
subscription queue order and returns consecutive samples. Rust `channel_capacity` counts complete
batches rather than individual messages. Dispatch performs at most one sequence take for this
subscription in one spin pass, bounding its work before other ready entities run.

## Ownership and reuse

`SubscriptionBatch<T>` owns decoded Rust values and aligned `MessageInfo` entries. Generated
`WrappedTypesupport::update_from_native` implementations update existing primitive Vec, string,
wstring, fixed-array and nested-message fields. Once a batch is dropped, its decoded storage can be
used by a later delivery. A retained batch is never overwritten: the next delivery allocates a
separate batch, and either value remains valid after the subscription endpoint and Node are gone.
Payload decoding traverses native messages without constructing metadata; each RMW info entry is
converted exactly once when the aligned result array is filled.

`NativeSubscriptionBatch<T>` owns every generated native message it exposes. Its iterator returns
read-only messages plus copied metadata; references cannot escape the batch. The RMW pointer and
metadata arrays use one recyclable slot. Generated native message containers are reinitialized in
place before a recycled sequence is taken again, preserving their outer CStruct addresses while
retiring all nested storage. Jazzy CycloneDDS accepted the first dynamic
`UInt64MultiArray` sequence take but attempted to finalize invalid nested storage when the same C
contents were supplied to a second take. Reinitializing those containers preserves safe behavior
under both CycloneDDS and Fast DDS without claiming middleware-specific nested-capacity reuse.
Handwritten `WrappedTypesupport` implementations retain a source-compatible non-mutating fallback;
only containers written by the preceding take are replaced unless they explicitly implement the
same reset contract. Successful takes refresh exactly the `sequence.size` prefix on reuse, so a
capacity-16 subscription that receives one message performs one reset rather than sixteen. A
runtime error leaves an unknown valid sequence and deliberately falls back to refreshing the full
capacity before retrying.

This distinction is important:

- typed batches reuse ordinary Rust dynamic-field capacity across deliveries;
- native batches amortize the sequence arrays, combine middleware takes and retain generated outer
  message addresses, but do not promise reuse of nested native payload buffers;
- retaining multiple batches always provides independent ownership;
- neither batch type borrows its endpoint, so endpoint teardown does not invalidate delivered data.

## Endpoint options and errors

`subscribe_batches_with_options` and `subscribe_native_batches_with_options` accept the same
`SubscriptionOptions` or `SubscriptionConfiguration<SubscriptionOptions>` as ordinary streams.
That includes channel capacity, local-publication behavior, unique network-flow requirements and a
creation-time middleware content filter. Runtime filter query/update/clear and all `Subscription`
endpoint helpers continue to work on the returned handle.

A zero maximum returns `RCL_RET_INVALID_ARGUMENT` before an endpoint is created. Allocation and
native take errors are returned through the existing executor error path. An empty middleware
queue is ordinary idle readiness and never produces an empty batch.

## Evidence boundary

The focused real-middleware cases publish complete bursts, prove that one stream item contains four
ordered messages with matching metadata, exercise consecutive sequence takes, retain peer batches,
verify typed dynamic-field addresses across shrinking updates and access batches after endpoint
teardown. The same finite example and focused cases pass under Jazzy/both RMWs, Humble/Fast DDS
and Kilted/both RMWs.

This establishes correctness and storage behavior, not throughput leadership. An isolated
independent-process r2r/rclcpp pipeline must measure latency, throughput, CPU and allocations before
the batch path is promoted as a performance win. The public Jazzy rclcpp subscription API exposes
individual `take` operations rather than an equivalent sequence stream, so that comparison must
account for the deliberately different application delivery boundary.

The current-source independent-process harness now exercises this exact batch API under both Jazzy
RMWs, three pipeline/fanout cases and all four Rust/C++ language directions. Its 12 records and 1200
samples per RMW establish protocol behavior only; the strict performance command stopped at the
isolation preflight on the contended host. See the
[typed batch pipeline archive](validation/batch-typed-pipeline-2026-10-31/README.md).

The deterministic no-RCL/DDS reset stage refreshes and refills four 4096-element generated native
messages. Across each 10,000-batch repetition, allocating replacements performs 90,000
allocations/frees; in-place reset performs 40,000. It removes five allocation/free calls per batch
and 3.2 MB of requested bytes per repetition. Elapsed stage values are diagnostic only.

A second deterministic sparse stage models a maximum batch of sixteen after a one-message take and
refills that one 4096-element message. Across 10,000 iterations, full-capacity refresh performs
160,000 native reset transactions while the successful-take prefix performs 10,000, removing
150,000 unnecessary init/fini transactions. Allocation/requested-byte counts remain essentially the
same because only the one actually refilled dynamic payload allocates; elapsed values remain
diagnostic rather than timing-qualified.

Run the finite example with:

```sh
cargo run -p r2r --example batch_subscription
```

The focused native gate is:

```sh
cargo test -p r2r --lib subscribers::batch_tests -- --test-threads=1
```
