# Entity-scaling benchmark validation

The existing fixed 128-action-endpoint microbenchmark could not distinguish the
Node floor from linear entity cost and did not cover mixed RCL wait sets. The
expanded benchmark defines 35 cases over 0/1/8/32/64/128 entities and four
families of 16-waits-per-entity cancellation cleanup.

The single-core Jazzy gate compiles the optimized Criterion binary, constructs
and lists all 35 topologies, and runs warning-denied Clippy. Mixed count 64 creates
64 subscriptions, services, clients, action servers, action clients and timers
on one Node. The widest action and waiter cases construct 128 endpoints.

No timings from the recorded host are promoted: external Zig/GPU work kept load1
above 20. The matrix is ready for an isolated dual-RMW run and deliberately does
not justify a production registry change yet.

```sh
bash docs/validation/entity-scaling-2026-10-30/run-jazzy.bash
```

`results.json` binds the benchmark, documentation, reproduction script and
compressed gate log.
