# Native QoS event streams and independent C++ diagnostics

This slice closes the application-facing diagnostic polling gap: endpoint QoS statuses now form
bounded Rust streams driven by native event readiness in the existing Node/shared/background
executor wait sets. It establishes ownership, status-counter behavior and native interoperability.
It does not establish an application performance improvement.

Publisher adapters cover typed/untyped/lifecycle publishers. Subscription adapters cover ordinary,
raw/untyped message streams and native callback ownership tokens. One Rust-side status slot pauses
only its event's native takes while full, leaving further changes in middleware counters until
consumption. The current executor is notified when capacity returns or the stream is dropped.
Duplicate observers and manual takes while an event stream owns the counter are rejected.
Endpoint events finalize before endpoint teardown. Buffered status precedes one endpoint-invalid
error and stream closure; node destruction wakes after releasing the native context lock.

## Native correctness

The 22 Jazzy cases cover incompatible reliability, actual deadline and liveliness expiry,
connect/disconnect counts, full-slot counter accumulation without idle spinning, observer and
foreign-node ownership, automatic/explicit endpoint and node destruction, lock-reentry wakers,
lifecycle diagnostics outside active, runtime-type/native callback adapters, current-executor
capacity wakes after transfer, shared event spans, and complete action result delivery after an
ordinary same-count membership change. Stable layouts retain native wait-set storage. Nodes
without observers allocate no event registry/channel/native-event array or extra guard.

A CycloneDDS counterexample failed the idle full-slot assertion: after an event left the wait
set, repeated native waits returned in tens of microseconds instead of sleeping. Diagnostic
samples showed only the guard ready and zero event capacity. The initial failure and native
readiness traces remain under `rejected/`. Portable event layout invalidation now reconstructs
the native wait cache when event-bearing membership changes, restoring ordinary/action arrays
at identical indices. Both RMWs pass the original bounded/idle behavior oracle. Stable-layout
reuse has a separate gate, so cache reconstruction is not performed on every spin.

## Native endpoint oracle

Independent Rust and rclcpp processes create one pair of incompatible reliability endpoints in
each direction. Both implementations must observe exactly one offered and one requested status,
with Reliability as the native policy kind. Both endpoints keep entities alive until both
observations are received, then consume explicit finish controls. Build manifests cover all
workspace sources, build scripts, saved bindings and endpoint/oracle code. The runner checks those
hashes and binary hashes before launch; the offline verifier checks complete endpoint status
records and can require current sources.

```sh
python3 tools/ros_oracles/qos_events.py build --output-dir target/qos-events-interop
python3 tools/ros_oracles/qos_events.py run --binaries target/qos-events-interop \
  --rmw rmw_cyclonedds_cpp --output target/qos-events-interop/cyclonedds.json
python3 tools/ros_oracles/qos_events.py run --binaries target/qos-events-interop \
  --rmw rmw_fastrtps_cpp --output target/qos-events-interop/fastdds.json
python3 tools/ros_oracles/qos_events.py verify --require-current-sources \
  docs/interop/qos-event-streams-2026-10-30/cyclonedds.json \
  docs/interop/qos-event-streams-2026-10-30/fastdds.json
```

CI requires the independent oracle under both RMWs for Humble, Iron, Jazzy, Kilted, Lyrical and
Rolling, saving each distribution's reports. Local runtime evidence covers Jazzy/both RMWs,
Humble/Fast DDS and Kilted/both RMWs. Iron/Lyrical/Rolling and external C++ QoS endpoints outside
Jazzy have no new local runtime evidence. Message-loss/incompatible-type generation, hostile
callback duration, concurrent native dispatch and application performance remain wider scope.

## Validation artifacts

The full Jazzy workspace and both complete optimized RMW suites pass 629 main-library tests
with eight manual measurements ignored; seventeen Rustdoc examples and all-target warning-denied
Clippy pass. Final compatibility executes 21 QoS cases on Humble/Fast DDS and 22 on each Kilted
RMW, plus 71 executor cases per environment, diagnostic examples and all-target Clippy. The true
no-ROS container passes all-target compilation, snapshot/oracle/common/generator tests, two
ownership compile-fail cases and 28 API pages with 60 required methods.

`compatibility.json` binds 178 source inputs and 8 compressed validation/build logs to the delivered
checkout. `cyclonedds.json` and `fastdds.json` include independent endpoint records and the same
source/binary identities. The original failed full-slot/cache traces are retained rather than
used as passing evidence.

```sh
python3 tools/bench_ros/verify_pubsub_pipeline.py --compatibility-manifest \
  docs/interop/qos-event-streams-2026-10-30/compatibility.json
```

A fresh pub/sub performance isolation attempt on this continuation also timed out before any
measurements, with load1=3.4507 and four Zig processes. The log is retained as
`rejected/pubsub-isolation-preflight.log.gz`; it proves the isolation requirement was enforced,
not a latency or throughput result. The performance matrix must be rebuilt for these native
event wait-set changes and rerun on a quiet host.
