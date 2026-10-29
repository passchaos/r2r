# Typed and scoped native service callbacks

This slice adds direct synchronous service handlers to the existing async ROS service API.
`create_service_callback` receives an owned typed request and returns a response. The native
variant receives a lifetime-bound read-only native request and reusable owned response storage.
Handlers run in Node/shared/background executor dispatch without a service/channel/handler mutex.
An optional separate callback registry leaves deferred service-stream ownership and scheduling
intact. Per-endpoint native request bounds expose the throughput/fairness choice.

## Payload and ownership evidence

Custom generated-compatible type-support wrappers reject any owned request decoding and count
native request/response creation and destruction. Both Jazzy RMWs deliver complete parameter
responses containing 128, 16, 0, 32 and 128 Unicode string values with zero request decodes,
one native response container, stable outer/retained-string addresses and one response destruction
at endpoint teardown. Every independent request take is retired before the next observed reply.
Response allocation failure leaves no native endpoint or registry. The borrowed request has a
compile-fail escape oracle; the response remains a valid replaceable owned native container.

Other focused cases check exact sums and native sequence identities, first-error and panic
recovery without implicit teardown, foreign-node ownership, targeted drop/capture cleanup,
max-two request passes alongside a ready ordinary service and healthy callback, callback/service/
action spans across shared nodes, background guard rebinding and native introspection's exact
request-received/response-sent payloads and sequence numbers. Native service type support is
captured once for endpoint creation and later introspection.

## Independent service protocol

The existing AddTwoInts service harness now detects the callback API. Candidate echo validates
sequence/operand values directly from the scoped request and writes the native response. Baseline
27f2130 uses the same harness with its owned service-stream fallback. Both clients preserve the
same observation order, bounded request window, sum/count/checksum oracle and final stop/
confirmation exchange. Reports record `native_callback|stream` endpoint paths; both online and
offline validation reject mislabeled paths. The builder records complete Rust workspace sources,
build scripts and saved bindings, including callback modules.

Both Jazzy RMWs pass the fourteen-record seven-language-combination matrix at windows one and
sixteen (100 measured requests, ten warmups, one repetition). Native Rust, historical Rust and
rclcpp binaries are independent, with CPUs 24/25 and the same reliable volatile QoS history.
The combined reports contain 2800 complete RTT samples. These short runs occurred on a busy host
and establish protocol/interoperability only. No latency/throughput gate is declared or promoted.
A formal isolated five-repetition application comparison remains required before speed claims.

```sh
python3 tools/bench_ros/verify_transport.py \
  docs/interop/service-callbacks-2026-10-30/protocol/cyclonedds.json \
  docs/interop/service-callbacks-2026-10-30/protocol/fastdds.json
```

## Native and distribution gates

The Jazzy workspace and both optimized RMW library suites pass 670 main-library cases with eight
manual measurements ignored. Eighteen Rustdoc examples, the runnable callback example and
warning-denied all-target Clippy pass. Each RMW executes twelve callback cases; Humble/Fast DDS
executes eleven because service introspection is unsupported. Kilted/both RMWs execute twelve.
The affected service cases, examples and Clippy also pass in those environments. A true no-ROS
container checks all targets, snapshot/oracle/common/generator cases, three ownership compile-fail
cases and 39 public artifacts with 86 required methods.

Iron/Lyrical/Rolling local runtime, dynamic request/response external C++ workloads, callback
execution across multiple threads, saturation/fairness applications, process-wide resource
measurement and formal isolated performance remain broader evidence scope. This slice removes
request channel delivery and supports retained native responses; it does not establish universal
service performance leadership.

## Final artifact identity

`compatibility.json` binds 201 current source/snapshot/harness inputs and 8 compressed logs.
Each protocol report retains the complete candidate/baseline manifests, independent binary
identities, raw RTT digests and actual receive paths. The short protocol runs have an explicit
protocol-only qualification marker and no declared performance gate.

```sh
python3 tools/bench_ros/verify_pubsub_pipeline.py --compatibility-manifest \
  docs/interop/service-callbacks-2026-10-30/compatibility.json
```

The full workspace stage checks the public API, native behavior and eighteen documentation
examples; final optimized native suites and source-identical protocol/build checks validate the
final cached type-support pointer used by endpoint construction and introspection. All saved
protocol hashes match the delivered callback implementation.
