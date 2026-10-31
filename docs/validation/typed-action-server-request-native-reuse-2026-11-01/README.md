# Typed action-server request native reuse validation

Each typed action-server endpoint now retains one native SendGoal request container and one native
GetResult request container. Public `ActionServer` streams, goal handles and result semantics are
unchanged. Request conversion and delivery still happen after releasing the native server lock;
the existing bounded request drivers execute each completion before the next take, so both leases
return before another request or idle probe.

A real Fibonacci client drives two complete accepted-goal/result lifecycles through a compatible
tracked server. It requires one native allocation per request type across both goals and verifies
that explicit server destruction releases both cached containers.

The five-repetition no-RCL/DDS stage fills and decodes 10,000 SendGoal/GetResult request pairs per
path:

| Native server request storage | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh | 20,000 | 20,000 | 360,000 |
| Reused | 0 | 0 | 0 |

Every repetition has identical allocation counts. Reuse removes two allocation/free calls and 36
requested bytes per goal. Elapsed diagnostics are retained but are not timing-qualified.

```sh
bash docs/validation/typed-action-server-request-native-reuse-2026-11-01/run-jazzy.bash
bash docs/validation/typed-action-server-request-native-reuse-2026-11-01/run-compat.bash
bash docs/validation/typed-action-server-request-native-reuse-2026-11-01/run-no-ros.bash
python3 docs/validation/typed-action-server-request-native-reuse-2026-11-01/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
