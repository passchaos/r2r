# Reusable typed pub/sub pipeline protocol

This archive connects `subscribe_reusable` to the existing independent-process Rust/rclcpp
pipelined/fanout workload. It changes only the Rust receive path; payload construction, reliable
volatile QoS, windows, fanout, process boundaries, CPU placement, validation, response ordering and
rclcpp application remain identical.

The Rust binary reports `receive_path=reusable_typed`; C++ reports `callback`. The builder enables a
separate `r2r_bench_reusable_typed` cfg only when explicitly requested and the public API exists.
Historical source trees without the API fall back to `stream`, and runner/verifier provenance
checks reject a mislabeled binary.

Each Jazzy RMW runs three cases—`0×1×1`, `4096×16×1`, `4096×8×4`—through Rust/Rust, C++/C++,
Rust-ping/C++-echo and C++-ping/Rust-echo. The two reports each contain 12 endpoint-pair records and
1200 exact RTT/publish/fanout-stage sample tuples. Payloads, counts, checksums, window maxima,
fanout completion, final controls, sample hashes and the complete workspace source manifest all
verify. The current manifest contains 277 build inputs, including the batch-subscription and timer
reset-event modules/tests even though this workload continues to select the independent
`reusable_typed` path.

These one-repetition, 100-message runs occurred on a heavily contended host and are protocol
evidence only. The refreshed formal five-repetition command requested the strict 1.0 rclcpp gate
but stopped before measurement at load1 90.50 with fourteen Zig processes. Its saved report has zero
records, zero summary rows and `measurement_started=false`.

```sh
R2R_BENCH_PUBSUB_RECEIVE_PATH=reusable_typed \
  python3 tools/bench_ros/build.py --workload pubsub_pipeline \
  --output-dir target/reusable-typed-pipeline
python3 tools/bench_ros/verify_pubsub_pipeline.py --require-current-sources \
  docs/validation/reusable-typed-pipeline-2026-10-31/protocol/cyclonedds.json \
  docs/validation/reusable-typed-pipeline-2026-10-31/protocol/fastdds.json
python3 docs/validation/reusable-typed-pipeline-2026-10-31/verify.py
```

All compilation used one CPU and a 10 GiB memory ceiling. An isolated final-source latency/
throughput comparison against rclcpp remains required before performance promotion.
