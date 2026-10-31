# Batched subscription validation

`Node::subscribe_batches` and `Node::subscribe_native_batches` deliver one ordered async batch for
one `rcl_take_sequence` operation. The public batch values own their messages and metadata, can
outlive endpoint teardown and never overwrite retained peers. Generated typed messages update
existing Rust dynamic fields in place.

The first Jazzy/CycloneDDS oracle exposed an unsafe middleware boundary: a second sequence take
into the same `UInt64MultiArray` C containers crashed while finalizing nested
`MultiArrayDimension` storage. The promoted implementation reuses the RMW pointer and metadata
arrays and failure-safely reinitializes generated native message containers at stable outer
addresses before another take. Successful takes now refresh only the prior `sequence.size` prefix;
runtime errors retain the conservative full-capacity recovery. Typed Rust batch
storage remains recyclable and preserves every tested Vec address/capacity across a 4096-to-64
element shrink. Typed decode traverses payloads without constructing throwaway metadata, then
converts each aligned RMW info entry exactly once.

Each native environment runs four real-RCL focused cases plus two pure conversion/fallback cases:

- zero maximum batch size fails before endpoint creation;
- native four-message bursts preserve order/metadata, survive consecutive takes, retain peers and
  outlive endpoint destruction;
- a capacity-16 stream taking one/four/one messages refreshes exactly the previous successful prefix;
- typed bursts preserve order and dynamic capacity while retained batches remain independent.
- handwritten types replace only touched slots and preserve every pointer after injected allocation
  failure.
- typed decode converts each native metadata record exactly once.

The creation-filter oracle exercises both new factories. Fast DDS delivers only the retained
matching sample; CycloneDDS returns the existing explicit unsupported result. The finite example
also verifies that one batch contains four messages and all four decoded Vec allocations are
reused. Jazzy runs focused cases in debug and release under both RMWs. Humble/Fast DDS and
Kilted/both RMWs run the debug cases. Every environment runs the example and focused Clippy; final
workspace all-target Clippy denies warnings. The no-ROS all-target and Rustdoc inventory checks 84
public pages and 341 required methods.

The deterministic four-message reset stage records 90,000 replacement allocations/frees versus
40,000 in-place allocations/frees per 10,000 batches. In-place reset removes five allocation/free
calls per batch and 3.2 MB of requested bytes per repetition. Stage elapsed values are not
timing-qualified.

The sparse reset stage compares capacity-wide and taken-prefix work for 10,000 capacity-16 batches
after one successful message. Reset calls fall from 160,000 to 10,000, removing fifteen native
init/fini transactions per batch. Both paths refill the same one 4096-element message, so allocation
counts remain approximately equal; elapsed samples are diagnostic only.

These are correctness, ownership and compatibility results. They do not establish a latency or
throughput advantage over rclcpp. Isolated independent-process comparison remains a separate gate.

```sh
bash docs/validation/batch-subscriptions-2026-10-31/run-jazzy.bash
bash docs/validation/batch-subscriptions-2026-10-31/run-compat.bash
bash docs/validation/batch-subscriptions-2026-10-31/run-no-ros.bash
python3 docs/validation/batch-subscriptions-2026-10-31/verify.py
python3 tools/bench_ros/native_batch_sparse_reset_stage.py verify --require-current-sources \
  docs/validation/batch-subscriptions-2026-10-31/sparse-reset-stage.json
```

Every compilation command is pinned to one allowed CPU, uses one Cargo/Rayon worker and has a 10
GiB virtual/container memory ceiling.
