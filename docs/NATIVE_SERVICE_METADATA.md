# Native service metadata

RMW service metadata can be partially populated. Independent RCL exchanges with
zero, `0xA5` and `0x5A` initial contents confirm that Fast DDS leaves all sixteen
response writer-GUID bytes untouched on the locally tested Jazzy, Humble and
Kilted installations. CycloneDDS writes all sixteen bytes on local Jazzy.
Payloads and native sequence numbers are checked in every exchange.

The old typed/untyped client paths allocated `MaybeUninit<rmw_service_info_t>`
and formed the complete structure after a successful take. Untouched GUID
bytes made that read invalid. The same risk exists for optional timestamps in
request metadata. Typed/untyped services and clients and built-in lifecycle
services now initialize the complete metadata storage before passing it to RCL.
Unwritten identities/timestamps remain zero; supplied bits are copied exactly.

`ServiceResponseInfo::writer_guid` represents the RMW-supplied identity. Zero
means it was not supplied. Its encoding can differ from the writer GUID observed
by the server on the original request. Sequence numbers still correlate the
response with its original request; timestamps preserve native values or zero
for unsupported fields.

Action clients only need the response sequence number for routing. The six
typed/untyped goal/cancel/result receive paths now read that initialized scalar
through a raw field projection, without forming a request-id value containing
potentially uninitialized GUID bytes. This avoids zeroing unused action metadata
and preserves the native dispatch path.

Two nearby tests cover a service implementation that writes only a sequence
number and an action header whose GUID is deliberately left uninitialized.
The typed/untyped integration case also performs direct RCL requests through
the same clients, then compares Rust-returned identity bytes to that native
observation. It retains payload, original-request sequence and timestamp
assertions. Nonzero identities and middleware omission are both exercised.

Run the independent native oracle with a sourced ROS environment:

```sh
env -u CYCLONEDDS_URI python3 tools/ros_oracles/service_metadata.py
```

It compiles an independent C++ RCL application and reports per-byte write masks,
payload/sequence checks, native timestamps, RMW/package/compiler versions and
source hashes. This is an observed native API contract, rather than an assumed
GUID encoding.

[The corrected Rust/C++ service comparison](benchmarks/native-metadata-service-2026-10-28/README.md)
retains complete protocol oracles under both Jazzy RMWs, one and sixteen requests
in flight, and 400000 raw RTT samples. At window sixteen, current Rust throughput
is 23.1% above C++ with CycloneDDS and 45.8% above C++ with Fast DDS on the measured
CPUs. It measures the corrected application; no performance delta against the
implementation that read uninitialized fields is claimed.

Focused gates before the full native suites:

```sh
env -u CYCLONEDDS_URI cargo test -p r2r --lib native_metadata:: -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib service_clients_can_return_response_metadata -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib action -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib service -- --test-threads=1
```

The publication/response paths keep their existing buffering, ownership and
completion behavior. This correction adds stack initialization only where the
complete metadata is exposed; it introduces no allocation, worker, timer or
global synchronization.
