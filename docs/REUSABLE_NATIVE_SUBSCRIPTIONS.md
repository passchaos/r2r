# Reusable native subscription samples

`Node::subscribe_native_reusable::<T>` returns a normal async `Subscription` whose items are
`NativeSubscriptionSample<T>`. Each sample dereferences to the generated native C message and
includes its `MessageInfo`:

```rust,no_run
# use futures::StreamExt;
# async fn consume(node: &mut r2r::Node) -> r2r::Result<()> {
let mut samples = node.subscribe_native_reusable::<r2r::std_msgs::msg::UInt64MultiArray>(
    "samples", r2r::QosProfile::default(),
)?;
while let Some(sample) = samples.next().await {
    println!("{} native values", sample.data.size);
}
# Ok(())
# }
```

The API is opt-in; `subscribe_native`, typed streams, raw streams and scoped callbacks retain their
existing definitions. `subscribe_native_reusable_with_options` accepts the same channel, endpoint
and creation-time content-filter configuration as other stream factories.

## Ownership and recycling

Every subscription owns one recyclable native container slot. A delivered sample exclusively owns
its container. Dropping it returns the container to the slot; if another sample still owns the
previous container, the native take allocates a distinct one. When several samples return
concurrently, the most recently returned container is cached and the displaced value is finalized
after releasing the pool lock. A live sample is therefore never overwritten or finalized early.

The container and native payload are caller-owned, not an RMW loan. Samples may outlive the native
subscription and Node; dropping them never calls through a finalized `rcl_subscription_t`. This is
different from `take_loaned_message`, which borrows the Node until middleware storage is returned.

Outer-container address reuse is guaranteed after a sample is dropped and no other sample has
occupied the slot. Dynamic sequence/string buffer preservation across `rcl_take` is RMW-specific
and is deliberately not promised. Applications must treat all nested native addresses as changing
on every delivery.

## Allocation boundary

The deterministic no-RCL/DDS stage fills the same 4096-element generated native message, sends it
through the same futures channel and validates the payload. Across five exact-count repetitions of
10,000 samples:

| Delivery | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh native container | 40,000 | 40,000 | 329,440,000 |
| Reusable RAII sample | 20,000 | 20,000 | 1,360,000 |

Recycling removes two allocation/free calls per sample and 328.08 MB (about 99.59%) of requested
bytes at this boundary. The stage elapsed diagnostics are not timing-qualified. End-to-end DDS
latency against rclcpp requires a separately isolated benchmark.

Run [`reusable_native_subscription`](../r2r/examples/reusable_native_subscription.rs) for a finite
dynamic-payload workflow that verifies container reuse across two deliveries.
