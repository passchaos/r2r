# Reusable typed subscription validation

`WrappedTypesupport::update_from_native` preserves existing Rust dynamic-field storage for generated
messages while handwritten implementations keep a source-compatible replacement default.
`Node::subscribe_reusable` delivers safe `ReusableSubscriptionSample<T>` items through a normal
async stream and recycles one native take container plus one decoded Rust message.
The default Rust-style `Node::subscribe` API remains an owned-message stream, but now transparently
reuses its internal native decode container between takes. The container is returned before the
owned Rust value is transformed or sent, so retaining one delivered message cannot alias or block
the next native take.

Jazzy's installed interface set runs two generator cases covering primitive sequences,
nested-message sequences and nested RosString capacity. Humble and Kilted include test_msgs and run
four cases, additionally covering string/nested-message sequences and RosWString scalar, fixed array
and sequence reuse. A real `UInt64MultiArray` case requires 4096-to-128 Rust Vec pointer/capacity
retention, distinct storage while samples overlap, most-recent return reuse, exact payloads and
valid access after endpoint teardown. A second real-RCL oracle sends 4096 then 64 elements through
the default typed stream, requires one native-container construction, and retains both independently
owned Rust results.

Jazzy runs generator and stream cases in debug/release under Cyclone DDS and Fast DDS.
Humble/Fast DDS and Kilted/both RMWs run them in debug. Every environment runs the finite example
and focused r2r/r2r_msg_gen Clippy. Workspace all-target Clippy and the true no-ROS gate pass; the
latter verifies 84 public pages and 341 methods. The complete Jazzy release library suite passes
822 cases with 20 manual stages ignored under each RMW.

The deterministic five-repetition stage decodes one preinitialized 4096-element native message,
sends the Rust result through the futures channel and validates it:

| Decode | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh | 30,000 | 30,000 | 329,200,000 |
| Reusable | 20,000 | 20,000 | 1,600,000 |

Every 10,000-delivery repetition has identical counts. Reuse removes one allocation/free and
327.60 MB (about 99.51%) requested bytes. Elapsed diagnostics are not timing-qualified and do not
establish DDS/rclcpp performance leadership.

The separate default-subscription stage isolates the transparent native-container reuse while
preserving a newly allocated owned Rust output on every delivery:

| Native decode storage | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh native container | 50,000 | 50,000 | 656,720,000 |
| Reused native container | 30,000 | 30,000 | 328,480,000 |

Across each of five 10,000-message repetitions, the default path removes two allocations/frees per
message and 328.24 MB (about 49.98%) of requested bytes. The remaining allocation volume includes
the independent owned Rust payload required by the public API. This stage also excludes RCL/DDS,
and its elapsed values are diagnostic only.

```sh
bash docs/validation/reusable-typed-subscriptions-2026-10-31/run-jazzy.bash
bash docs/validation/reusable-typed-subscriptions-2026-10-31/run-compat.bash
bash docs/validation/reusable-typed-subscriptions-2026-10-31/run-no-ros.bash
python3 docs/validation/reusable-typed-subscriptions-2026-10-31/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
