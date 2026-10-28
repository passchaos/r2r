# Action-server scheduling by ready identity

Cancellation can remain unresolved while a node handles other ROS traffic.
Previously, the node retained every pending server in a set and polled that
entire set before every native wait. A twenty-server native counterexample
recorded eighty cancellation maintenance calls during four unrelated spins,
although every decision remained blocked.

The node now shares cleanup and ready notifications in the existing
action-server notification allocation. Cleanup retains its own queue. Ready
server UUIDs use a deduplicated queue allocated on first use; ordinary nodes
and idle servers do not allocate readiness state. Up to sixteen identities use
linear deduplication; larger queues allocate a boxed HashSet index and discard
that index when drained. The old node-level pending-server HashSet is removed.
Each preparation examines at most sixteen queued server identities and continues
before a native wait if unexamined identities remain. Small cancellation queues
borrow their cached waker during polling rather than cloning it.

A server caches one weak identity waker when cancellation delivery or maintenance
first needs it. It queues the UUID, then notifies the node after releasing the
notification lock. Repeated signals for a queued identity coalesce. Pending
decisions and full cancel streams stop being maintenance candidates until their
wakers queue work. Completing delivery queues the next decision-registration
stage, including small requests whose application decision arrives immediately.
Request/goal readiness inside a large server remains indexed as described in
[cancellation progress](ACTION_SERVER_CANCELLATION_PROGRESS.md).

Native cancel ingress schedules its server after conversion/delivery. Removing
an endpoint clears both cleanup and ready identities. A stale wake after
destruction may produce a bounded identifier that preparation discards through
the existing UUID index; it cannot restore native ownership. Ready work queued
while a node is stopped/unbound remains available when the node starts in a new
executor. Weak notification ownership follows that current binding.

The existing future guard also supplies a cached weak observer waker. Foreground
future execution and direct shared-executor waits attach action-server
notifications to that guard. An external thread accepting/rejecting a decision
can interrupt the native wait without another ROS message. The guard waker does
not retain the guard/context and does not trigger an inactive guard. Background
execution uses the same cached observer implementation. No additional native
guard, timer or worker thread is introduced.

## Verification boundaries

Native twenty-server tests require zero maintenance calls for unrelated spins,
one call when only the final decision becomes ready, and preserved rejected
responses/terminal results. The background case handles thirty-two ordinary
service roundtrips while all action servers remain blocked. A per-node optional
test probe observes native-thread maintenance; thread-local counters cannot
establish this boundary. That case also queues a decision while unbound, moves
the node and checks that only its identity is processed.

Foreground and direct shared-executor cases reject a decision from an external
thread while an existing native guard waits. Queue oracles cover
0/1/16/17/40/128 identities, deduplication, cleanup/ready burst coalescing,
unlock-before-wake and clearing both lanes on endpoint removal. The original
small/large cancellation and node transfer suites remain relevant gates.

```sh
env -u CYCLONEDDS_URI cargo test -p r2r --lib action_servers::notifications:: -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib action_server_scheduling_tests:: -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib parked_servers_skip_service -- --test-threads=1
```

The manual native stage benchmark creates real parked cancellation requests and
times only `Node::prepare_spin`. It uses optimized library-test binaries and
retains the TLS maintenance instrumentation; it excludes `rcl_wait`, native
dispatch and application throughput. Independent baseline/candidate builds
inject the same harness, retain every timing sample and check actual RMW/CPU
affinity, exact counts and percentile/gate arithmetic. Full action transport
A/B separately protects the ordinary path with the existing five-percent gate.

The [saved scheduling evidence](benchmarks/server-scheduling-2026-10-29/README.md)
compares `69a379f` with independent candidate binaries under both Jazzy RMWs.
Thirty-two blocked servers reduce preparation p50 from 4027/4018 ns to
316/315 ns (CycloneDDS/Fast DDS), while zero-server controls pass the unchanged
5% gate. Both ordinary action matrices also pass that gate. The initial
CycloneDDS cancellation p99 failure is retained with its source patch; the
final candidate removes redundant waker clones and uses the small queue path.
These stage results measure r2r against its earlier implementation and do not
establish full-application superiority over rclcpp.

The complete Jazzy workspace and both optimized RMW library suites pass 564
library cases, with three manual benchmarks ignored. Native Humble/Kilted
scheduling, cancellation, future execution, complete cancellation application
and all-target Clippy gates pass. The no-ROS container verifies all workspace
targets, thirteen API artifacts and twenty-eight documented methods.

```sh
python3 tools/bench_ros/prepare_stage.py build --baseline-ref 69a379f --output-dir target/server-prepare-stage
python3 tools/bench_ros/prepare_stage.py run --binaries target/server-prepare-stage \
  --rmw rmw_cyclonedds_cpp --servers 0,1,8,32 --cpu 24 --domain-id 132 \
  --iterations 20000 --warmup 1000 --repetitions 5 \
  --output target/server-prepare-stage/cyclonedds.json
```

This scopes cancellation maintenance to ready servers. The native wait-set
construction/readiness traversal and deferred endpoint-removal checks retain
their existing behavior. Native ingress, complete mixed-load scaling, resource
accounting and C++ application comparisons remain separate work toward the full
performance goal. Profile, distribution and transport evidence is tracked in
[release validation](RELEASE_VALIDATION.md).
