# Independent service introspection evidence

Typed/untyped Rust clients and services interoperate with independent rclcpp
endpoints and observe the same native service events. Four complete matrices
pass under Jazzy/Kilted with CycloneDDS/Fast DDS: sixteen pairs, 160 endpoint
phases, 240 actual requests and 1152 event observations across both subscribers.
The [service tracing workflow](../../SERVICE_INTROSPECTION.md) describes APIs,
native identity semantics and reproducible commands.

## Protocol and oracle

Each pair owns one client, one service and two event subscriptions in independent
processes. Rust uses typed messages or runtime JSON; C++ uses its generated native
message interfaces. Both subscribers retain their identities across
`Off -> Metadata -> Contents -> Off -> Contents`. Clients wait for service
availability before sending three distinct ordered requests per phase. Each
reply must equal the complete sum oracle. Native request sequence numbers from
client responses and server request headers must agree, remain unique across
phases and correlate with all four event types.

Both observers require exactly twelve events in each enabled phase, with empty
payloads in Metadata and exact request/response payloads in Contents. Off phases
require no events and zero discovered event publishers. Both processes keep
spinning for 200 ms after completion, and graph convergence remains bounded by
the phase deadline. Enabled phases require two discovered event publishers.
These checks catch missing, duplicated, stale and incorrectly configured events.

Rust sets its managed ROS clock to 12.000000034 through the public TimeSource
and `/clock` path. C++ supplies a native ROS clock with the same override to
rclcpp introspection. Every event must carry that exact timestamp. Client-origin
identity is checked against the native client GID; server-origin identity is
checked against the server's native request writer GUID. Their encodings differ
under CycloneDDS in all four tested endpoint combinations, and the oracle
preserves that distinction. Event order between publishers may differ, while
the complete keyed observations must match.

## Passing matrices

| Distribution | RMW package/version | Endpoint phases | Requests | Event observations | Report |
| --- | --- | ---: | ---: | ---: | --- |
| Jazzy | CycloneDDS 2.2.4 | 40 | 60 | 288 | [JSON](jazzy/cyclonedds.json) |
| Jazzy | Fast DDS 8.4.4 | 40 | 60 | 288 | [JSON](jazzy/fastdds.json) |
| Kilted | CycloneDDS 4.0.3 | 40 | 60 | 288 | [JSON](kilted/cyclonedds.json) |
| Kilted | Fast DDS 9.3.5 | 40 | 60 | 288 | [JSON](kilted/fastdds.json) |

The RMW versions above are ROS `rmw_*` package versions. Reports retain all
request/response metadata and both endpoint event streams. Embedded manifests
record source/binary hashes and compiler versions; runtime metadata records ROS
packages including rclcpp 28.1.22/29.5.11 and rcl 9.2.11/10.1.5. All four final
reports use byte-identical current Rust/C++ source. Jazzy domains are 142/144;
Kilted domains are 148/150. These are native correctness/interoperability runs,
with no latency/throughput claim or pinned-CPU performance comparison.

```sh
python3 tools/ros_oracles/service_events.py verify \
  docs/interop/service-introspection-2026-10-29/jazzy/cyclonedds.json \
  docs/interop/service-introspection-2026-10-29/jazzy/fastdds.json \
  docs/interop/service-introspection-2026-10-29/kilted/cyclonedds.json \
  docs/interop/service-introspection-2026-10-29/kilted/fastdds.json
python3 -m unittest discover -s tools/ros_oracles -p test_service_events.py
```

## Preserved failures and verification

The [initial Fast DDS run](initial-fastdds/fastdds-failed.json) timed out before
the untyped Rust client's first phase completed. The harness sent before native
service discovery; the final code uses the existing `wait_for_service()` API.
Its original source, manifest, endpoint stderr logs and passing initial Cyclone
report remain in `initial-fastdds/`. The first Kilted validation image lacked
CycloneDDS and failed the package metadata precondition. A separate derived
validation image installs it; the [original failure](logs/service-events-kilted-missing-rmw.log.gz)
and image-build log are preserved. Failed cells were not skipped or promoted.

Five adversarial Python cases validate exact events, source identities,
timestamps, payload/state behavior, complete matrices and native sequences.
The original four native service-introspection cases pass under all four
distribution/RMW environments, including ownership and retained history for a
late subscriber. Jazzy workspace/release tests pass 564 library cases with three
manual benchmarks ignored. Formatting and all-target Clippy pass. Humble
compiles the example and verifies exit 2 from both unsupported entry points,
its support-boundary case and all-target Clippy. The no-ROS container compiles
all targets, passes snapshot/generator checks and verifies thirteen API artifacts
and twenty-eight methods. [Validation and archive identities](validation.json)
retain the exact observed counts and compressed log digests.

CI requires both RMWs for Iron/Jazzy/Kilted/Lyrical/Rolling and separately checks
Humble rejection, uploading native reports and endpoint logs. Iron, Lyrical and
Rolling have no new local runtime evidence and their live CI results remain
unverified. Broader payloads, mixed event QoS, late-subscriber external endpoints
and complete diagnostic applications remain additional scope.
