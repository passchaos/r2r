# Typed action status native reuse validation

Typed action clients now keep one endpoint-owned native `GoalStatusArray` take container. The lease
is held through the existing detached direct-native decode, then returned before status updates and
cleanup complete. Public goal handles and status APIs remain unchanged.

A real Fibonacci oracle completes two accepted goals and requires distinct successful status takes,
the same cached native address across goal lifecycles, correct succeeded results and stale-client
rejection after explicit endpoint destruction. Jazzy runs it in debug/release under both RMWs;
Humble/Fast DDS and Kilted/both RMWs run it in debug.
Focused/workspace Clippy and the true no-ROS gate pass; the complete Jazzy release suite reports
822 passed with 20 manual stages ignored under each RMW.

The five-repetition stage fills and directly decodes 10,000 status arrays per path, each with 128
goal statuses:

| Native status storage | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh | 30,000 | 30,000 | 57,840,000 |
| Reused | 10,000 | 10,000 | 21,760,000 |

Every repetition has identical counts. Reuse removes two allocation/free calls and 3,608 requested
bytes per status message. The remaining allocation is the decoded update Vec. The stage excludes
RCL/DDS and its elapsed diagnostics are not timing-qualified.

```sh
bash docs/validation/typed-action-status-native-reuse-2026-11-01/run-jazzy.bash
bash docs/validation/typed-action-status-native-reuse-2026-11-01/run-compat.bash
bash docs/validation/typed-action-status-native-reuse-2026-11-01/run-no-ros.bash
python3 docs/validation/typed-action-status-native-reuse-2026-11-01/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
