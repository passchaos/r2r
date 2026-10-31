# Typed batch pub/sub pipeline protocol

This archive connects `Node::subscribe_batches` to the independent-process Rust/rclcpp
pipelined/fanout workload. The Rust binary reports `receive_path=batch_typed`; the C++ binary keeps
its normal callback path. Payload construction, reliable volatile QoS, request windows, fanout,
CPU placement, process boundaries and the per-message validation/RTT boundary are unchanged.

Batch delivery does not weaken the oracle. Ping settles and times every message in a returned
batch, replenishing one request-window slot per message. Echo retains one cursor per fanout
subscription, validates each cursor against the same sequence and publishes exactly one response
only after all copies arrive. The requested maximum batch size equals the request window; the Rust
channel holds one complete batch. Zero-payload/window-one remains a fixed-cost control.

Both Jazzy RMWs run `0×1×1`, `4096×16×1` and `4096×8×4` through Rust/Rust, C++/C++,
Rust-ping/C++-echo and C++-ping/Rust-echo. Each report contains 12 records and 1200 exact
RTT/publish/fanout-stage samples. Payloads, counts, checksums, window maxima, final controls,
receive-path identity, samples and the complete 277-input source manifest verify.

A second dual-RMW regression smoke executes the same-source `reusable_typed/reusable_typed` and
`batch_typed/batch_typed` pairs for all three cases. Each report contains six records and 600
samples. These short results verify both binary/path selections and the shared oracle; they remain
non-isolated diagnostics, not a performance result.

These one-repetition, 100-message runs occurred on a heavily contended host and are protocol
evidence only. The formal five-repetition command combines a 1.05 same-source reusable-typed
regression gate with the strict 1.0 rclcpp gate. It stopped before measurement at load1 90.46 with
fourteen Zig processes. Its saved report has no records or summary and records
`measurement_started=false`. No timing value from the protocol smoke is promoted.

```sh
R2R_BENCH_PUBSUB_RECEIVE_PATH=batch_typed \
  python3 tools/bench_ros/build.py --workload pubsub_pipeline \
  --output-dir target/batch-typed-pipeline
python3 tools/bench_ros/verify_pubsub_pipeline.py --require-current-sources \
  docs/validation/batch-typed-pipeline-2026-10-31/protocol/cyclonedds.json \
  docs/validation/batch-typed-pipeline-2026-10-31/protocol/fastdds.json
python3 docs/validation/batch-typed-pipeline-2026-10-31/verify.py
```

All compilation used one CPU and a 10 GiB memory ceiling. The archived formal command already binds
same-source `batch_typed`, `reusable_typed` and rclcpp binaries; an idle-host completion remains
required before claiming latency or throughput leadership.
