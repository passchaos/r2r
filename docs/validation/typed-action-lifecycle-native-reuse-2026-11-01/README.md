# Typed action result-request and goal-response native reuse validation

Typed action clients now retain reusable native storage for the two remaining per-goal boundaries
between goal transmission and result tracking. The public Rust API is unchanged. A GetResult
request buffer is shared by cloned `ActionClient` handles, while the native endpoint owns one
SendGoal response container.

Response completions now run after releasing the native client lock and before the next bounded
response take. This both preserves wake/reentrant safety and returns a cached response lease before
the following take or idle probe. A real Fibonacci workflow verifies reuse across accepted goals
and handle clones, endpoint cleanup, result-request copy-failure retirement and successful retry.

The five-repetition no-RCL/DDS stage performs 10,000 combined result-request preparations and
goal-response fills/decodes per path:

| Native lifecycle storage | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh | 20,000 | 20,000 | 280,000 |
| Reused | 0 | 0 | 0 |

Every repetition has identical allocation counts. Reuse removes two allocation/free calls and 28
requested bytes per goal. Elapsed diagnostics are retained but are not timing-qualified.

Focused debug/release tests pass under both Jazzy RMWs, Humble/Fast DDS and both Kilted RMWs.
Workspace all-target Clippy and the true no-ROS gate pass; the latter verifies 84 public pages and
341 required methods. The complete Jazzy release library suite passes 825 cases with 22 manual
stages ignored under each RMW.

```sh
bash docs/validation/typed-action-lifecycle-native-reuse-2026-11-01/run-jazzy.bash
bash docs/validation/typed-action-lifecycle-native-reuse-2026-11-01/run-compat.bash
bash docs/validation/typed-action-lifecycle-native-reuse-2026-11-01/run-no-ros.bash
python3 docs/validation/typed-action-lifecycle-native-reuse-2026-11-01/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
