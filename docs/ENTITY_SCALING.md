# Native entity and waiter scaling

Node dispatch owns several dense native-entity vectors with identity-to-index
maps. Dropped entries are removed with `swap_remove` and the moved identity is
reindexed. This gives constant-time lookup/removal while preserving the compact
linear layout needed to rebuild an RCL wait set. Replacing these registries with
a different container without measuring complete wait-set preparation can make
the common spin path slower even if isolated removal improves.

The `spin_idle_action_servers` Criterion target now exposes 35 scaling cases:

| Group | Entity counts | Operation |
| --- | --- | --- |
| Idle action servers | 0, 1, 8, 32, 128 | zero-timeout Node spin |
| Idle action clients | 0, 1, 8, 32, 128 | zero-timeout Node spin |
| Mixed wait set | 0, 1, 8, 32, 64 of each kind | subscriptions, services, clients, action servers/clients and one-hour timers; zero-timeout spin |
| Service availability | 0, 1, 8, 32, 128 | register and cancel 16 futures per client, then clean them in one spin |
| Action availability | 0, 1, 8, 32, 128 | register and cancel 16 futures per client, then clean them in one spin |
| Publisher matching | 0, 1, 8, 32, 128 | register and cancel 16 subscriber waits per publisher, then clean them in one spin |
| Subscription matching | 0, 1, 8, 32, 128 | register and cancel 16 publisher waits per subscription, then clean them in one spin |

Benchmark nodes disable rosout and the type-description service so the requested
count is not hidden behind unrelated default endpoints. The zero-entity cases
preserve the Node/guard floor. Entity construction and discovery are outside the
Criterion timing loop; the matrix measures steady idle preparation/wait cleanup,
not startup or graph convergence.

Build and enumerate every topology without collecting timings:

```sh
cargo bench -p r2r --bench spin_idle_action_servers --no-run
cargo bench -p r2r --bench spin_idle_action_servers -- --list
```

A formal run must use an otherwise idle host, fixed CPU affinity, both Jazzy
RMWs, raw Criterion estimates and a same-binary repetition/control. Allocation,
RSS and endpoint construction/destruction need separate process-wide probes.
Until those curves exist, this benchmark is a guard against speculative registry
rewrites rather than evidence that the current layout is optimal or faster than
rclcpp.
