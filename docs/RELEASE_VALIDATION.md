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
pruning, service-driven activation over DDS). At that checkpoint, full debug
and release suites passed 488 tests each. `LifecycleNode::create_publisher` returns a gated
publisher that is activated on reaching `active` and deactivated on leaving it
(weak registry prunes dropped publishers on each transition).
External interoperability was verified with `ros2 lifecycle get/list/set`
against a running r2r node: configure, activate, an invalid command rejected
with the state unchanged, deactivate, cleanup and shutdown all returned the
standard `unconfigured/inactive/active/finalized` states.

Note: the local Jazzy shell exports a stale `CYCLONEDDS_URI=/nav_env/...`;
DDS-backed gates run with that variable unset. Map-only state-machine cases
need no DDS at all.

## Distribution boundaries after node module extraction (2026-10-28)

Native Humble validation found two extraction regressions: the `nodes` module
re-exported Jazzy's `TimerCallInfo` without its Humble/Iron exclusion, and the
action-name resolver became private to the action-client module even though
the action-server factory also uses it. The export now follows the type's
distribution condition and the resolver is visible inside `crate::nodes`.
The test-only graph string helper is also imported only where its caller exists.
Humble passes the action-name case, seven timer cases and library/test/example
Clippy with warnings denied. The native Jazzy workspace suite passes with these
boundaries restored. Iron uses the same guarded APIs; its local runtime matrix
remains unverified.

## Lifecycle publication boundary (2026-10-28)

A new native-message regression case failed against `9780902`: an
unconfigured managed publisher delivered a message through `publish_native`.
The wrapper's unrestricted `Deref<Target = Publisher<T>>` also allowed safe
cloning/coercion to the ordinary publisher and unguarded loan publication.
[Lifecycle publication](LIFECYCLE_PUBLICATION.md) now explicitly gates typed,
native and loaned publication, delegates endpoint queries/waits, and exposes
native loan fields without exposing the underlying publisher or loan object.

Nine publisher cases cover typed and native delivery/drop behavior, preserved
native storage, shared clone activation, failure/error recovery, creation while
active, stale native endpoints, the configurable managed factory, immediate
destruction and foreign-node isolation, loan ownership/errors, returning gated
and unpublished loans, and activation changes between borrowing and publishing.
The complete lifecycle filter executes 33 cases. The native compiler gate
executes seven public API checks: a usable example, rejected publisher
coercion/clone/private-field escapes, rejected loan coercion/private-field
escapes, and rejection of a loan outliving its node.

| Environment | Runtime evidence | Loan path |
| --- | --- | --- |
| Jazzy / CycloneDDS | Workspace debug suite and full optimized library suite: 493 r2r cases pass in each, two manual benchmarks ignored; optimized complete example | Unsupported loaning and foreign-node errors verified |
| Jazzy / Fast DDS | 33 lifecycle cases in debug and release; optimized complete example | Actual loan publish, gated return, manual activation changes and reuse verified |
| Humble / native image default RMW | 33 lifecycle cases, action-name case, seven timer cases; compiler gate, complete example, library/test/example Clippy | Unsupported loaning and foreign-node errors verified |
| Kilted / native image default RMW | 33 optimized lifecycle cases; compiler gate, optimized complete example, library/test/example Clippy | Actual loan publish, gated return, manual activation changes and reuse verified |

The example transfers the lifecycle node into the shared executor and changes
state through native services. It checks exact typed/native/loan payloads,
reliable ACK progress and bounded absence of extra publications in all five
phases. Iron, Lyrical and Rolling are still outside this local runtime evidence.
The compiler gate is registered in Jazzy quality CI. These results establish
the publication contract and retained native ownership, not a transport
performance advantage over C++.

Native Jazzy also passes all-target workspace Clippy with warnings denied,
formatting and the doc-only workspace compilation gate. The existing
`lifecycle_node` example now names its transition function-pointer type so it
passes the same Clippy gate without suppressing the complexity lint.

## Documentation snapshot and optional native metadata (2026-10-28)

The [documentation gate](DOCUMENTATION_BUILDS.md) now compiles all workspace
targets and verifies ten API pages and fourteen methods in Ubuntu 24.04 with
no `/opt/ros` installation. Its explicit Jazzy snapshot contains 23 packages,
saved interface availability, lifecycle and service/action events. Four
snapshot transaction cases verify preparation failure, partial-install rollback,
complete replacement and retention of previously documented packages. All 36
binding files have identical SHA-256 after independent regeneration.

Native Kilted validation exposed an unrelated unsafe metadata assumption:
Fast DDS does not fill response writer-GUID bytes. [Native metadata oracles](NATIVE_SERVICE_METADATA.md)
with three initial byte patterns verify that omission on Jazzy/Humble/Kilted and
full response-identity writes under Jazzy/CycloneDDS. Service metadata storage
now starts initialized; action clients read only their mandatory sequence field.
The typed/untyped integration case compares returned identities with a direct
RCL exchange through the same client instead of assuming a nonzero identity.

Final native debug/release Jazzy suites pass 495 library cases under both RMWs,
with two manual benchmarks ignored. The full Jazzy workspace suite also passes.
Native Humble and Kilted library suites pass 486 and 499 cases respectively,
with two ignored in each; their full-target workspace Clippy gates pass.
Jazzy full-target Clippy, formatting, two metadata counterexamples, 146 action
cases and 55 service cases pass. Humble's now-version-specific `MaybeUninit`
import remains available only where its native client-GID query needs it.

[The corrected service comparison](benchmarks/native-metadata-service-2026-10-28/README.md)
retains fixed CPUs 24/25, two windows, all four Rust/C++ endpoint combinations,
80 records and 400000 raw RTT samples. It compares the corrected application
with C++; the implementation that read uninitialized metadata is not used as
a performance baseline.

## Lifecycle-managed timers (2026-10-28)

[Lifecycle ROS/wall timers](LIFECYCLE_TIMERS.md) now pause the native producer
outside active, restart from the current clock on activation and invalidate
buffered ticks with a generation even when deactivate/reactivate occur between
spins. Manual cancel/reset, explicit destruction, foreign-node rejection and
native timing queries are preserved. Node destruction wakes paused observers.
No additional thread, timer, periodic query or native guard is introduced;
ordinary timer message storage and delivery stay on their existing path.

Fourteen focused Jazzy cases cover native inactive suppression, fresh deadlines,
stale queues, manual pause/startup, failed activation/deactivation, error/shutdown,
drop cleanup, foreign controls, native elapsed/tick metadata, native `/clock`,
backward ROS time and service-driven shared execution. Native elapsed values are
checked against an ordinary timer. The complete sensor example validates 16
publications over two active phases and three paused phases. Humble/Kilted
optimized focused suites execute 13/14 cases; examples and full-target Clippy
pass in both distributions. Humble lacks native `tick_with_info`.

The optimized Jazzy library suite and debug workspace suite each pass 509
library cases, with two manual benchmarks ignored. The Fast DDS optimized
lifecycle filter executes 48 cases. Nine public API compilation boundaries
check managed timer factories/controls and reject ordinary-timer escapes. The
no-ROS container compiles all targets and checks 11 API artifacts/23 methods.

[Default-path performance evidence](benchmarks/lifecycle-timers-2026-10-28/README.md)
compares a safe `88f1ac7` baseline, candidate and rclcpp under both Jazzy RMWs.
The original 5% p50/p95/p99/inverse-throughput gates pass in all window-1/16
cells: 140 records and 700000 raw RTT samples. This service workload measures
the optional managed registry's effect on ordinary nodes; managed-timer scaling
remains a separate performance requirement.

## Owned background executor and runtime I/O (2026-10-29)

[`Executor::spawn`](BACKGROUND_EXECUTION.md) returns an awaitable owned task
backed by one shared native wait thread. Stop/cancellation returns node ownership
without shutting down the context. Drop requests cancellation without joining
on the runtime thread; native errors and inline callback panics release nodes
before completion. Background-specific errors use `ExecutorTaskError`, preserving
the ordinary ROS error representation.

A late-observer regression case first timed out after discovery was complete and
the native wait had started. Background startup now installs a weak notification
waker in the existing observer-cancellation allocation. Service/action matching
and publisher matching/ACK queues are scheduled before triggering the existing
native guard. Cancellation uses the same state, including observers registered
before spawn. The default observer queue layout is retained.

Twelve background cases cover ownership/reuse, cancellation before guard
registration, drop cleanup, pending callbacks, failure/panic teardown, unobserved
completion, late typed/untyped/action/matching/ACK observers, current-thread TCP
I/O and non-Send application futures. Seven observer cases retain bounded
cancellation and registration semantics. The separate context-shutdown case
checks native wake, task completion and rejection of spawn after shutdown.
The complete current-thread Tokio example verifies 32 TCP/ROS-service/pub/sub
roundtrips, ACK progress, explicit stop and a second exchange after reuse.

The Jazzy debug workspace and optimized library suites execute 522 library
cases, with two manual benchmarks ignored. Fast DDS executes all 12 optimized
background cases. Native Humble/Kilted each pass the 12 optimized background
cases, seven observer cases, context shutdown, optimized example and full-target
Clippy. Ten public compilation boundaries include task ownership/Send/result
types. No-ROS compilation covers all targets and checks 13 API artifacts and
28 methods, including `ExecutorTaskError`.

[Performance reports](benchmarks/background-executor-2026-10-29/README.md) retain
the initial failures and same-binary calibration runs. Final default same-thread
and optional background A/B reports under both Jazzy RMWs record passing original
5% p50/p95/p99/CPU/inverse-throughput gates: 432 records and 12960000 RTT samples.
Default processes use CPU 24; all compared background processes and RMW threads
use the same physical CPU set 24/25. The background baseline is the safe manual
native-thread API at `67ea778`.

The earlier background A/A calibration had a 5.2% tail difference after longer
warmup/sample cohorts. The final candidate has no passing matched same-binary
calibration, so passing A/B reports remain limited evidence and do not establish
calibrated tail precision or superiority over C++. Native-thread
handoff costs, scheduling, allocation/resource evidence and reactor integration
without a separate wait thread remain explicit work. The default fast path is
preserved; the concrete improvement is safe async-runtime I/O composition.

