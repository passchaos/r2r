# Typed action result-response native reuse validation

Typed action result futures continue to deliver independent owned Rust results. Each typed action
client endpoint now retains one native GetResult response container. Its lease crosses the existing
detached decode and goal-cleanup completion, then returns after the owned result has been created.

A real Fibonacci workflow completes 4096- and 64-element results, requires the same cached native
address, preserves the retained first Rust Vec while decoding the second, and rejects a stale client
clone after explicit endpoint destruction. Jazzy runs it in debug/release under both RMWs;
Humble/Fast DDS and Kilted/both RMWs run it in debug.

The five-repetition stage fills and decodes 10,000 generated Fibonacci result responses per path,
each containing 4096 `i32` values:

| Native result storage | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh | 30,000 | 30,000 | 328,000,000 |
| Reused | 10,000 | 10,000 | 163,840,000 |

Every repetition has identical counts. Reuse removes two allocation/free calls and 16,416 requested
bytes per result. The remaining allocation is the owned Rust result Vec. The stage excludes RCL/DDS
and does not qualify elapsed diagnostics.

Focused debug/release tests pass under both Jazzy RMWs, Humble/Fast DDS and both Kilted RMWs.
Workspace all-target Clippy and the true no-ROS gate pass; the latter verifies 84 public pages and
341 required methods. The complete Jazzy release library suite passes 823 cases with 21 manual
stages ignored under each RMW.

```sh
bash docs/validation/typed-action-result-response-native-reuse-2026-11-01/run-jazzy.bash
bash docs/validation/typed-action-result-response-native-reuse-2026-11-01/run-compat.bash
bash docs/validation/typed-action-result-response-native-reuse-2026-11-01/run-no-ros.bash
python3 docs/validation/typed-action-result-response-native-reuse-2026-11-01/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
