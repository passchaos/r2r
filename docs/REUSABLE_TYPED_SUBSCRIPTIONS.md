# Reusable typed subscription samples

`Node::subscribe_reusable::<T>` returns ordinary safe Rust messages inside
`ReusableSubscriptionSample<T>`. It has the same async stream, graph/QoS/filter helpers and
`MessageInfo` as other subscriptions:

```rust,no_run
# use futures::StreamExt;
# async fn consume(node: &mut r2r::Node) -> r2r::Result<()> {
let mut samples = node.subscribe_reusable::<r2r::std_msgs::msg::UInt64MultiArray>(
    "samples", r2r::QosProfile::default(),
)?;
while let Some(sample) = samples.next().await {
    println!("{} values", sample.data.len());
}
# Ok(())
# }
```

The sample API is opt-in. The public `subscribe`, `subscribe_with_info`, native, raw and callback
contracts are unchanged. Internally, the ordinary typed stream now reuses one native decode
container but still yields independent owned Rust messages. `subscribe_reusable_with_options`
accepts the same channel, endpoint and creation-time content-filter configuration as ordinary
streams.

## Generated field reuse

`WrappedTypesupport::update_from_native` has a source-compatible replacement default for handwritten
types. Generated implementations preserve reusable Rust storage:

- primitive sequences clear and extend their existing `Vec`;
- `RosString` and `RosWString` retain their internal Vec capacity;
- fixed arrays update in place;
- nested messages recurse into their generated update method;
- message/string/wstring sequences update the common prefix, truncate removed values and allocate
  only newly added tail elements.

Each subscription caches one decoded Rust message and one native take container. A sample owns its
Rust value exclusively until drop. Retaining several samples creates distinct values; the most
recently returned value becomes the cache. Rust dynamic field pointers are stable when the next
payload fits their retained capacity, but the outer Rust struct moves by value and has no stable
address guarantee. Samples remain valid after endpoint teardown.

Custom `WrappedTypesupport` implementations compile unchanged and receive the conservative default
`*self = Self::from_native(msg)`. They must override `update_from_native` to claim field-capacity
reuse.

## Allocation boundary

The deterministic no-RCL/DDS stage decodes the same preinitialized 4096-element native message,
sends it through the futures channel and validates the Rust payload. Five exact-count repetitions
of 10,000 samples produce:

| Decode | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh `from_native` | 30,000 | 30,000 | 329,200,000 |
| Reusable `update_from_native` | 20,000 | 20,000 | 1,600,000 |

Reuse removes one allocation/free and 32,760 requested bytes per sample, a 327.60 MB (about
99.51%) reduction per repetition. Elapsed diagnostics are not timing-qualified. End-to-end DDS
latency and throughput against rclcpp remain a separately isolated benchmark requirement.

The ordinary `subscribe<T>` path has a separate stage that fills either a fresh or retained native
container, performs an owned Rust decode, sends and receives through the same futures channel, and
validates 10,000 4096-element messages. Across five exact-count repetitions, native reuse reduces
50,000 allocations/frees and 656.72 MB requested bytes to 30,000 and 328.48 MB: two allocation/free
calls per message and 328.24 MB (about 49.98%) removed per 10,000-message repetition. The owned Rust payload remains
independent, including when earlier messages are retained. This stage also excludes RCL/DDS and
does not qualify its elapsed diagnostics as end-to-end performance.

Run [`reusable_typed_subscription`](../r2r/examples/reusable_typed_subscription.rs) for a finite
4096-to-128 element workflow that verifies ordinary Rust Vec pointer reuse.
