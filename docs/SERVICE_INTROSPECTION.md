# Service introspection across Rust and C++

On ROS 2 Iron and newer, typed/untyped clients and services can publish the
standard generated service `Event` messages on `<resolved-service-name>/_service_event`.
This allows Rust endpoints to participate in the same service tracing and
diagnostic tools as rclcpp endpoints. Humble has no native service-introspection
API; the capability remains explicitly unavailable there.

```rust
use r2r::{QosProfile, ServiceIntrospectionState};

let event_qos = QosProfile::default().keep_last(16).transient_local();
service.configure_introspection(
    &mut node, ServiceIntrospectionState::Contents, event_qos.clone(),
)?;
client.configure_introspection(
    &mut node, ServiceIntrospectionState::Contents, event_qos,
)?;
```

Configure each endpoint on its owning node. A foreign node is rejected, and a
destroyed client reports an invalid handle. Configuration uses that node's
managed ROS clock, including `/clock` when `TimeSource::enable_sim_time` is
active. The endpoint's native lifetime keeps the clock valid. Modes can change
without replacing the service/client or changing its native request identity.

| Mode | Event metadata | Request/response payload |
| --- | --- | --- |
| `Off` | No event publisher | None |
| `Metadata` | Timestamp, event type, request sequence, client identity | Empty sequences |
| `Contents` | Same metadata | One request for request events, or one response for response events |

For a typed `AddTwoInts` endpoint, subscribe to `AddTwoInts::Event` using the
same reliability/durability settings as its event publishers. The four event
types are `REQUEST_SENT`, `REQUEST_RECEIVED`, `RESPONSE_SENT` and
`RESPONSE_RECEIVED`. Request and response sequences are bounded to one element
by the ROS service event interface. Event order between two publishers is not
globally ordered; correlate by native request sequence and origin identity.

Some middleware represents a client GID differently from the request writer
GUID received by a server. Client-origin events use `Client::get_gid()`, while
server-origin events follow `ServiceRequest::request_info().writer_guid`.
Do not require those two representations to be byte-identical. The generated
event stores the original native identity without rewriting it.

## Independent interoperability workflow

The Rust [endpoint example](../r2r/examples/service_introspection_interop.rs)
and independent [rclcpp endpoint](../tools/ros_oracles/service_events.cpp)
exercise typed/untyped Rust clients against C++ services and typed/untyped Rust
services against C++ clients. Both subscribe to the native service event topic.
The [runner and offline oracle](../tools/ros_oracles/service_events.py) verify
complete event sets against independently observed native call identities.

```sh
python3 tools/ros_oracles/service_events.py build --output-dir target/service-events
python3 tools/ros_oracles/service_events.py run --binaries target/service-events \
  --rmw rmw_cyclonedds_cpp --domain-id 142 --output target/service-events/cyclonedds.json
python3 tools/ros_oracles/service_events.py run --binaries target/service-events \
  --rmw rmw_fastrtps_cpp --domain-id 144 --output target/service-events/fastdds.json
python3 tools/ros_oracles/service_events.py verify \
  target/service-events/cyclonedds.json target/service-events/fastdds.json
```

Each pair keeps its service, client and event subscriptions alive across
`Off -> Metadata -> Contents -> Off -> Contents`. Every phase executes three
distinct sum requests. Both endpoints must observe all twelve events in enabled
phases, no events in disabled phases, and zero/two discovered event publishers.
They continue spinning for 200 ms after completion to catch delayed or duplicate
publication. The Rust endpoint obtains its frozen simulated timestamp through
the public managed `/clock` path; C++ passes a native ROS clock with the same
override to rclcpp introspection. The exact timestamp is 12 seconds plus 34 ns.
Clients first use their public service-availability wait. Fast DDS exposed an
initial harness failure when the untyped Rust client submitted a request before
discovery; the final workflow uses the readiness API in both languages.

One RMW run covers four pairs, forty endpoint phases, sixty actual requests
and 288 event observations across both subscribers. Saved reports contain all
call metadata, event contents, compiler/package versions and source/binary
hashes. The validator rejects missing/duplicate events, wrong identities,
timestamps, payloads, modes, endpoint combinations and native call sequences.
These are correctness and interoperability checks, not performance measurements.

[Saved Jazzy/Kilted evidence](interop/service-introspection-2026-10-29/README.md)
contains four passing native matrices, 240 requests and 1152 event observations,
including preserved startup and missing-middleware failures. Both RMWs also pass
the original native ownership, timestamp, mode and retained-history suites.

The supported-distribution CI is configured to run the matrix under CycloneDDS
and Fast DDS on Iron, Jazzy, Kilted, Lyrical and Rolling, with a separate Humble rejection
boundary. Existing nearby native tests also check foreign ownership and retained
`keep_last(1)` history for late subscribers. The independently launched matrix
keeps event subscriptions alive and uses `keep_last(16)`; it does not replace
those late-subscriber or lifecycle checks.
