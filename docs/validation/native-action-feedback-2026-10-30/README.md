# Native delivered action-feedback validation

`ActionClient::send_goal_request_native_feedback` returns a stream of RAII items backed by the
message storage taken by `rcl_action_take_feedback`. Generated goal identity is read before any
owned payload conversion. Dropping a delivered item returns its outer container to the existing
one-slot pool; concurrent retained items may allocate additional containers, of which only one is
cached on return.

The ordinary owned stream remains source and behavior compatible. Native and owned registrations
have separate typed channels and readiness queues, while goal rejection, result-request failure,
stream cancellation, terminal status and result completion close the active registration through
the same post-lock completion boundary. Backpressure stays lossless. Coalesce replaces only the
pending native lease for that goal and retires the superseded lease without decoding it.

Focused tests cover storage identity before delivery and after item drop, 4096-element Coalesce
replacement, custom action rejection before goal transmission, bounded canceled-stream cleanup and
owned/native terminal closure. A real-RCL case accepts and executes a generated Fibonacci goal,
publishes a 4096-element feedback sequence, validates the native envelope and payload, completes
the result and observes stream closure. The runnable example performs the same workflow with
16,384 elements without constructing an owned feedback value on delivery.

The required runtime matrix is Jazzy with both RMWs, Humble/Fast DDS and Kilted with both RMWs.
The current no-ROS all-target/documentation gate verifies 84 public pages and 341 required methods,
including the four native-feedback types and methods added to its inventory.

The independent-process large-feedback harness now declares each Rust ping as `owned` or
`native` and each C++ ping as `rclcpp_callback`. Two final-source dual-RMW protocol smokes cover
all four language directions with exact 4096-element payloads; a same-binary smoke separately
checks owned/native selection. These tiny externally contended runs are protocol evidence only.
The formal five-repetition command requested both the original 1.05 owned/native regression gate
and a strict 1.0 rclcpp gate, but its final-source isolation preflight stopped before measurement
at load1 90.46 with thirteen Zig processes. The saved failure contains no endpoint records or timing
summary.

## Deterministic allocation boundary

The final typed APIs are also exercised without DDS/RCL using the same preinitialized native
Fibonacci feedback message, 4096 `i32` elements, channel send/receive and payload validation. Five
10,000-delivery repetitions produce identical allocation counts:

| Delivery | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Owned Rust payload | 50,000 | 50,000 | 165,600,000 |
| Native RAII lease | 30,000 | 30,000 | 1,680,000 |

Native delivery removes two allocation/free calls per message and 163.92 MB (98.99%) of requested
bytes per repetition. The preload probe passes allocation/content/alignment/failure/epoch/thread/
reset oracles. `allocation-stage.json` retains every count and diagnostic elapsed sample; those
elapsed values are not timing-qualified and are not used for a latency claim.

```sh
bash docs/validation/native-action-feedback-2026-10-30/run-jazzy.bash
bash docs/validation/native-action-feedback-2026-10-30/run-compat.bash
bash docs/validation/native-action-feedback-2026-10-30/run-no-ros.bash
python3 docs/validation/native-action-feedback-2026-10-30/verify.py
python3 tools/bench_ros/native_action_feedback_stage.py verify --require-current-sources \
  docs/validation/native-action-feedback-2026-10-30/allocation-stage.json
```

All compilation is pinned to one allowed CPU with Cargo/build-script parallelism set to one and a
10 GiB virtual/container memory ceiling. These gates establish API, ownership and real-middleware
correctness. Application allocation, latency and throughput against the owned r2r path and rclcpp
require a separately isolated benchmark; no performance result is inferred from these runs.

[`results.json`](results.json) binds the final source files, compressed Humble/Jazzy/Kilted/no-ROS
and Clippy logs, benchmark binaries/build manifest, protocol reports and raw sample archives. The
formal isolation failure is retained separately so a protocol smoke cannot be mistaken for a
qualified timing result.
