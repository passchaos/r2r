# Middleware-side content-filtered subscriptions

`ContentFilterOptions` configures the selected RMW to reject samples before they enter r2r's
native take and Rust-side queue. Expressions use the SQL-like field syntax implemented by the
middleware; `%0` through `%99` refer to supplied string parameters:

```rust,no_run
# fn install(node: &mut r2r::Node) -> r2r::Result<()> {
let filter = r2r::ContentFilterOptions::new("data > %0")
    .expression_parameters(["10"]);
let subscription = node.subscribe_with_options::<r2r::std_msgs::msg::Int32>(
    "numbers", r2r::QosProfile::default(),
    r2r::SubscriptionOptions::default().content_filter(filter),
)?;
# Ok(())
# }
```

`SubscriptionOptions`, `NativeCallbackSubscriptionOptions` and `GroupedSubscriptionOptions` stay
small and `Copy`; their `content_filter` builder returns an owned `SubscriptionConfiguration<T>`
only when filter strings are needed. Existing calls that pass the original options compile
unchanged. The filter is placed in `rcl_subscription_options_t` before native initialization.
Successful init transfers the nested allocation to the subscription; failed setup finalizes it
immediately. If an RMW accepts the option but leaves CFT disabled, r2r finalizes the new endpoint
and reports `RCL_RET_UNSUPPORTED` instead of silently creating an unfiltered subscription.

The same `Subscription<T>` methods cover typed, native, runtime-typed JSON, serialized and
native/typed batch streams. `NativeCallbackSubscription` and `GroupedSubscription` expose the identical
`is_content_filter_enabled`, `set_content_filter`, `get_content_filter` and
`clear_content_filter` contract, including exclusive and reentrant callback groups. A grouped or
executor-owned Node must be stopped and recovered before its filter can be changed.

`set_content_filter` validates interior NUL bytes and the native 100-parameter limit before
entering RCL. A foreign Node returns `RCL_RET_SUBSCRIPTION_INVALID`. Setting an empty expression or
calling `clear_content_filter` disables filtering. The native option structure is finalized after
every set/get operation, including errors; r2r does not retain user string pointers.

Support is an RMW capability. `RCL_RET_UNSUPPORTED` is returned unchanged, and r2r does not silently
decode and filter rejected samples locally. That keeps DDS history, bandwidth, deserialization and
Rust queue behavior honest. In the current Jazzy runtime evidence, Fast DDS supports apply,
parameterized delivery, live replacement and clear; Cyclone DDS reports unsupported. Humble and
Kilted results are recorded separately by the validation matrix.

Creation-time evidence publishes transient-local history before constructing the subscription and
proves that only the matching retained sample reaches ordinary, native-batch and typed-batch Fast
DDS delivery. Changing a live filter may
still interact with samples already retained after creation; filter expression grammar and
supported field types remain middleware-specific.

Run [`content_filtered_subscription`](../r2r/examples/content_filtered_subscription.rs) for a
finite `Int32` workflow that applies `data > 10`, replaces it with `data < 10`, and clears it. The
example exits cleanly with an explicit message when the selected RMW lacks native support.
