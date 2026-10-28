# Optimized runtime validation

r2r validates runtime behavior with debug assertions disabled as well as with the default
test profile. The Jazzy quality job runs the complete library suite with `cargo test --release`.
Version-specific and middleware integration gates still cover the supported Humble-and-newer
matrix.

## Release-only registration failure fixed on 2026-10-27

The first optimized, independent-process pub/sub roundtrip probe failed while querying the
subscription's matched publishers. Entity-map registrations had been written inside
`debug_assert!`, so Cargo's default release profile removed the mutations along with the
assertion. Debug tests passed because those expressions were evaluated there.

The correction evaluates state mutations before debug assertions in all fourteen affected
locations:

* Subscription registration/rebuild, service, timer, publisher and action-server registration.
* Removal of the active parameter-transaction marker on guard destruction and initialization
  of provider-owned parameter names.
* Typed and untyped action reverse-map cleanup when goal responses or result requests finish.

The assertions still check the previous/removed values in debug builds. Their absence cannot
change registration, cleanup or provider behavior in optimized builds. The fix preserves the
existing lookup structures and synchronization.

## Gates

With a sourced ROS environment, run the smallest directly relevant gates before broader tests:

```sh
cargo test -p r2r --lib --release indexes -- --test-threads=1
cargo test -p r2r --lib --release parameter_transaction_guard -- --test-threads=1
cargo test -p r2r --lib --release provider -- --test-threads=1
cargo test -p r2r --lib --release -- --test-threads=1
```

Filtered tests must execute nonzero relevant cases. The new typed/untyped action tests require
both reverse indexes to be empty after out-of-order response/request cleanup. Existing entity
tests cover registration, targeted destruction, swap-removal, index rebuilding and automatic
cleanup. Parameter tests verify repeated operations after guard destruction and provider-backed
reads. Full optimized Jazzy library validation now passes 445 tests, with two manual benchmarks ignored.
Humble and Kilted each pass nine optimized index cases and the parameter-transaction gate.

The [future-execution pub/sub boundary](FUTURE_EXECUTION.md) adds eight cases for graph-driven
matching and bounded pending ACK queries. All 18 executor cases pass under both Jazzy RMWs and
native Humble/Kilted; the complete payload/timer/ACK example runs in each environment. Release
validation exposed two discovery tests querying local graph caches before their asynchronous
update. The [graph tests](../r2r/src/nodes/tests/graph.rs) now register a graph waiter before
checking visibility and use a two-second overall deadline. Exact type, endpoint, QoS and count
assertions remain after discovery. These three cases plus graph broadcast/cancellation pass
under both Jazzy RMWs and native Humble/Kilted.

The [multi-node execution gates](MULTI_NODE_EXECUTION.md) add eleven cases for one shared native
wait, disjoint ordinary/action readiness spans, node removal and endpoint cleanup, native ACK
cadence, external/non-Send futures and shutdown. They pass under both Jazzy RMWs and native
Humble/Kilted. The refactored single-node path retains all eighteen future-execution cases and
the complete optimized library suite.

The [end-to-end roundtrip harness](../tools/bench_ros/TRANSPORT.md) exercises native DDS transport
through optimized application binaries, including both Rust/C++ directions. This boundary is
useful for catching profile-dependent failures that compilation or debug-only unit tests miss.

Keep state-changing expressions outside every `debug_assert!`, `debug_assert_eq!` and
`debug_assert_ne!`. Assertions should inspect results already produced by normal execution.

## Native lifecycle (managed node) support added on 2026-10-28

The lifecycle feature ships in two layers: a map-only safe wrapper over
`rcl_lifecycle_state_machine_t` (`LifecycleStateMachine`, no node/DDS entities)
and `LifecycleNode`, which owns a regular `Node` plus one machine and serves
the five standard lifecycle services and the `~/transition_event` publisher
as native r2r entities. Transition semantics match `rclcpp_lifecycle`:
external command edge, user callback on the intermediate state, generic
`transition_success`/`transition_failure`/`transition_error` edge, plus the
error-processing callback; label-based requests take precedence over ids.

Targeted gates (debug and `--release`, nonzero matched cases each):

```sh
cargo test -p r2r --lib lifecycle -- --test-threads=1
cargo test -p r2r --lib --release lifecycle -- --test-threads=1
```

29 lifecycle cases execute: 13 state-machine cases (full 11-state/25-edge graph
oracle taken from the installed Jazzy rcl via a C probe, happy path,
failure/error/shutdown edges, invalid triggers), 12 managed-node cases over
native r2r service clients/subscribers including the unified `Executor`, and
4 `LifecyclePublisher` cases (gating until active, manual toggle, dead-entity
pruning, service-driven activation over DDS). Full debug and release suites
now pass 488 tests each. `LifecycleNode::create_publisher` returns a gated
publisher that is activated on reaching `active` and deactivated on leaving it
(weak registry prunes dropped publishers on each transition).
External interoperability was verified with `ros2 lifecycle get/list/set`
against a running r2r node: configure, activate, an invalid command rejected
with the state unchanged, deactivate, cleanup and shutdown all returned the
standard `unconfigured/inactive/active/finalized` states.

Note: the local Jazzy shell exports a stale `CYCLONEDDS_URI=/nav_env/...`;
DDS-backed gates run with that variable unset. Map-only state-machine cases
need no DDS at all.

