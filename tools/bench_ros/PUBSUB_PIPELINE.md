# Pipelined and fanout pub/sub comparison

`pubsub_pipeline.py` measures independent Rust and rclcpp processes with more than one message in
flight. Cases use `elements:window:fanout`: the ping process maintains the requested publication
window, every echo subscription validates each sequence and complete payload, and the echo process
publishes one ordered response only after all fanout copies arrive. A final empty control reaches
every echo subscription.

Reports retain per-message RTT, ping publish time, echo fanout-collect time and echo publish time.
The formal regression gate covers complete RTT p50/p95/p99 and inverse throughput. Stage samples
are diagnostic and still verified, but do not replace the application boundary. All endpoint
counts, fanout counts, checksums, window maxima, summary medians, raw-sample digests and declared
receive paths are recomputed offline.

```sh
python3 tools/bench_ros/build.py --workload pubsub_pipeline --baseline-ref <revision> \
  --output-dir target/pubsub-pipeline
python3 tools/bench_ros/pubsub_pipeline.py \
  --rust target/pubsub-pipeline/r2r-candidate \
  --baseline-rust target/pubsub-pipeline/r2r-baseline \
  --cpp target/pubsub-pipeline/rclcpp-comparison \
  --cpu-ping 24 --cpu-echo 25 --domain-id 120 --rmw rmw_cyclonedds_cpp \
  --cases 0:1:1,4096:16:1,4096:8:4 \
  --iterations 1000 --warmup 50 --repetitions 5 \
  --matrix-scope regression --max-regression 1.05 \
  --rust-receive-path native_callback --baseline-receive-path stream \
  --isolation-samples 3 --isolation-interval 20 --isolation-timeout 600 \
  --isolation-max-load1 2 \
  --build-manifest target/pubsub-pipeline/build-manifest.json \
  --output target/pubsub-pipeline/cyclonedds-ab.json
python3 tools/bench_ros/verify_pubsub_pipeline.py --require-performance-pass \
  --require-current-sources \
  target/pubsub-pipeline/cyclonedds-ab.json
```

Set `R2R_BENCH_PUBSUB_RECEIVE_PATH=stream|native_callback|reusable_typed|batch_typed` before building the Rust
candidate. The builder enables the selected cfg only when its public API exists; older sources fall
back to `stream`. Endpoints report `receive_path=native_callback|reusable_typed|batch_typed|stream|callback`;
the runner and verifier reject a binary/configuration mismatch. Formal runs require three consecutive idle samples, reject active
compiler/linker processes before measurement, and monitor `/proc` throughout the matrix. The
offline performance verifier also requires this isolation gate; a numerically green but contended
report cannot pass promotion. The runner and monitor are pinned away from both measurement CPUs,
and that affinity is recorded in every report. `--require-current-sources` recomputes candidate, harness and C++
source hashes from the checkout, so a stale green binary/report cannot pass either. Full matrices
omit the baseline and use all four Rust/C++ language directions. Benchmark runs clear inherited
DDS profile files and disable intra-process transport.

`reusable_typed` keeps the ordinary safe Rust message surface while recycling native and decoded
message storage. Its current dual-RMW full-direction protocol and isolation failure are archived in
[`reusable-typed-pipeline-2026-10-31`](../../docs/validation/reusable-typed-pipeline-2026-10-31/README.md).
Those short runs are correctness evidence; use the same strict `--max-rclcpp-ratio 1.0` isolation
gate for performance promotion.

`batch_typed` keeps the same per-message RTT, checksum, ordering and window accounting while using
one `rcl_take_sequence` for up to `window` messages. Ping processes settle every item in a delivered
batch and replenish the request window per item. Echo processes keep one independent cursor per
fanout subscription and publish a response only after all cursors reach the same sequence. The
Rust channel holds one complete batch, so batch delivery changes the native-take/channel boundary
without weakening the existing workload oracle.

Supplying either formal ratio gate requires at least three idle preflight samples. The verifier
rejects nonfinite regression thresholds and a passing runtime summary whose maximum observed
load exceeds its declared limit, even when its violation list is empty. Pipeline build manifests
cover all workspace crate sources, build scripts and saved Rust bindings, including extracted
native receive modules and their tests.

Formal rclcpp matrices use `--max-rclcpp-ratio 1.0`. The gate compares the same-language
Rust/Rust and C++/C++ application boundaries and requires Rust RTT p50/p95/p99 plus inverse
throughput to be no worse in every selected case. Mixed-language rows remain interoperability
diagnostics. Verify them with `--require-rclcpp-pass --require-current-sources`.
