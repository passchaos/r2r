# Query native ROS type descriptions

On Jazzy and newer, `Node::start_type_description_service()` registers the standard
`~/get_type_description` endpoint. It answers requests directly from RCL's live node type cache
through the ordinary Node/shared/background executor wait path, without an application handler
future or request channel. `NodeOptions::start_type_description_service(true)` starts it during
node creation. The `start_type_description_service` CLI/YAML/programmatic parameter override is
also honored when no explicit option is given. The explicit option takes precedence.

r2r keeps this additional endpoint disabled by default, following its explicit infrastructure
configuration. Start it for applications that participate in schema discovery. The C++ default
usually starts it at node creation. `stop_type_description_service()` immediately finalizes only
this service. Startup is idempotent, and restart preserves all unrelated service IDs. Namespace
expansion and service remapping follow native RCL resolution; inspect the actual name through
`type_description_service_name()`.

```rust
let mut node = r2r::Node::create_with_options(
    context, "robot", "/fleet",
    r2r::NodeOptions::default().start_type_description_service(true),
)?;
let client = node.create_type_description_client("/fleet/robot")?;
let ready = client.wait_for_service()?;
let hash = r2r::message_type_hash::<r2r::std_msgs::msg::UInt64>()?;
let query = async {
    ready.await?;
    client.describe("std_msgs/msg/UInt64", &hash, true)?.await
};
// Drive query with the node or shared executor, using wake-driven execution.
```

`TypeDescriptionClient` returns the standard typed response, including the complete referenced
type graph and optional IDL/msg/service/action sources. `describe` rejects an unsuccessful native
response, a mismatched type name or a schema whose canonical RCL-calculated hash differs from the
requested hash. `describe_with_timeout` uses a steady-clock deadline and validates node ownership
before creating work. Clone the client to issue independent concurrent requests on its native
endpoint. Pending-query cancellation follows ordinary service-client semantics.

For large concurrent query windows, configure both endpoint histories through
`start_type_description_service_with_qos` and `create_type_description_client_with_qos`.
The native default is reliable KeepLast(10); reliable delivery still permits older requests to
leave a bounded DDS history before an application takes them. The forty-request oracle uses
matching KeepLast(64) histories and a bounded test deadline. Server dispatch handles at most
sixteen native requests per pass, preserving other entity progress.

`message_type_hash`, `service_type_hash` and `action_type_hash` read generated type-support identity
without constructing payloads. `TopicTypeHash` parses/prints RIHS strings and preserves the format
version. Graph endpoint hashes can be used directly for queries. A zero-version hash is unset and
rejected by `describe`; Humble returns `RCL_RET_UNSUPPORTED` for generated hash access. Iron has
type hashes but not the native description service. The query API requires generated
`type_description_interfaces`; startup returns unsupported when an interface filter omits it.

The native cache describes types currently used by publishers/subscriptions, services/clients
and actions. Shared types stay registered until their last native endpoint is removed. Failure
responses for unknown or removed hashes remain ordinary wire responses, and later valid requests
still work. The client independently checks names because RCL's native server selects by hash.

Each native request take owns an independent initialized container. RCL's request handler expects
uninitialized response fields and initializes them itself. r2r finalizes the previous response
fields before invoking it, retaining only the outer RAII container; response finalization on
subsequent queries or endpoint teardown prevents repeated initialization from leaking strings
and nested type/source sequences. This does not claim dynamic response capacity reuse.

Run the complete example:

```sh
cargo run -p r2r --example type_description
cargo test -p r2r --lib type_description -- --test-threads=1
```

The documentation snapshot includes `type_description_interfaces` so the query API and example
compile without ROS. Native verification covers complete hash/schema/source identity, missing
and invalid hashes, shared endpoint registration counts, service/action schemas, source omission,
remapping, stop/restart, forged same-name schema rejection and multi-request execution.

Some generated C type-source metadata stores a terminator in place of the original trailing
newline. The standard service response would otherwise lose the newline or expose an embedded
NUL. RCL cache conversion can also remove the NUL before the response is formed. r2r checks the installed package resource against the
complete compiled prefix and restores only matching trailing whitespace. Files are cached per
registered type/encoding; repeated replies avoid additional path lookup allocations or reads.
A changed file with different fields or non-whitespace suffix is never substituted. Missing
source files leave the native metadata unchanged, so exact original-source fidelity is verified
separately from schema/hash interoperability. The immutable generated type cache is unchanged. Checked clients apply the same guarded
recovery to C++ or legacy C-backed responses, after validating schema identity; only missing
trailing bytes are appended to the owned Rust source buffer, reusing spare capacity when present.
