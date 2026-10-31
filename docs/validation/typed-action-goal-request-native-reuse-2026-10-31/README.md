# Typed action goal-request native reuse validation

All typed goal-send APIs keep their existing Rust futures and options. `ActionClient<T>` handles and
their clones now share one native SendGoal request slot. Goal construction and Rust-to-native copy
remain before the action-client mutex; concurrent sends check out independent storage while the
slot is occupied.

A custom Fibonacci-compatible action oracle counts native SendGoal request construction while a
standard Fibonacci server rejects exact goals. It verifies reuse across sequential goals and a
cloned client, copy failure before RCL/state registration, later recovery, endpoint destruction and
final-clone cleanup. Jazzy runs it in debug/release under both RMWs; Humble/Fast DDS and Kilted/both
RMWs run it in debug.
Focused/workspace Clippy and the true no-ROS gate pass; the complete Jazzy release suite reports
822 passed with 20 manual stages ignored under each RMW.

The five-repetition stage prepares 10,000 generated Fibonacci goal requests per path:

| Native preparation | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh | 10,000 | 10,000 | 200,000 |
| Reused | 0 | 0 | 0 |

Every repetition has identical counts. Reuse removes one allocation/free and 20 requested bytes per
goal. This is a fixed per-goal preparation saving; the stage excludes RCL/DDS and does not qualify
its elapsed diagnostics.

```sh
bash docs/validation/typed-action-goal-request-native-reuse-2026-10-31/run-jazzy.bash
bash docs/validation/typed-action-goal-request-native-reuse-2026-10-31/run-compat.bash
bash docs/validation/typed-action-goal-request-native-reuse-2026-10-31/run-no-ros.bash
python3 docs/validation/typed-action-goal-request-native-reuse-2026-10-31/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
