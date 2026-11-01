# Typed action-server batch native reuse validation

Typed action servers now keep bounded native pools for the goal-info and SendGoal response messages
prepared by `accept_batch`. Requests are still prepared before the server lock is taken, so a batch
of accepted goals reuses its native messages across batches instead of allocating one goal-info and
one response per goal. Pools are capped at the bounded action-server request window (16); larger
user batches keep only the bounded excess and retire the rest. Fallible copies retire their
partially updated container, and a single `accept`/`reject` path uses the same pools.

The integration test sends two batches of four goals through a tracked server. It requires exactly
four native goal-response allocations across both batches and releases them on server destruction.
The five-repetition no-RCL/DDS stage fills batches of sixteen goal-info plus response messages per
path:

| Native batch storage | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh | 320,000 | 320,000 | 5,760,000 |
| Pooled | 0 | 0 | 0 |

Every repetition has identical allocation counts. Pooling removes two allocation/free calls and 36
requested bytes per accepted goal in a batch after warmup. Elapsed diagnostics are retained but are
not timing-qualified.

```sh
bash docs/validation/typed-action-server-batch-native-reuse-2026-11-01/run-jazzy.bash
bash docs/validation/typed-action-server-batch-native-reuse-2026-11-01/run-compat.bash
bash docs/validation/typed-action-server-batch-native-reuse-2026-11-01/run-no-ros.bash
python3 docs/validation/typed-action-server-batch-native-reuse-2026-11-01/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
