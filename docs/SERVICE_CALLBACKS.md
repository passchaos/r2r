# Register synchronous service handlers

`Node::create_service_callback` registers a short typed handler taking `(request, info)` and
returning `Result<Response>`. `create_service_native_callback` receives a lifetime-bound,
read-only `NativeMessageRef<Request>` plus a mutable owned native response buffer and request
metadata. Both run directly during Node/shared/background executor dispatch, with no internal
service, channel or handler mutex held around application code.

```rust
let service = node.create_service_callback::<AddTwoInts::Service, _>(
    "sum", QosProfile::services_default(),
    |request, _info| Ok(AddTwoInts::Response { sum: request.a + request.b }),
)?;
```

For fixed fields, the native path can produce a reply without constructing either owned payload:

```rust
let service = node.create_service_native_callback::<AddTwoInts::Service, _>(
    "sum", QosProfile::services_default(),
    |request, response, _info| {
        response.sum = request.a + request.b;
        Ok(())
    },
)?;
```

The native response buffer is initialized once and retained across requests. For dynamic fields,
use a response message's `try_copy_to_native(response)` to reuse initialized sequence/string
capacity. Every request take still receives an independently initialized native message: this
does not rely on an RMW safely reusing populated request storage. The borrowed request cannot
escape or transfer its native allocation/deallocator. The response remains owned; a handler may
replace it with another valid owned native container if it needs a different ownership policy.
Handlers must write every field required for the current reply, since prior response values
persist. The typed adapter replaces all generated fields through fallible native copying.

The returned `CallbackService` token offers actual endpoint QoS, resolved service name and native
introspection configuration on Iron and newer. Service event publication retains standard
request identity, timestamps and request/response contents. Dropping the token stops subsequent
takes, queues targeted cleanup and wakes the current native executor. The already-running
handler may finish its taken request. `Node::destroy_callback_service` immediately finalizes the
endpoint and releases captured state. Foreign-node tokens schedule their owning node's cleanup.

`CallbackServiceOptions::max_requests_per_spin` sets the throughput/fairness budget; the default is
16, matching ordinary service batching, and zero is rejected before endpoint creation. DDS QoS
history is configured separately through the service's `QosProfile`. These callbacks are `Send`
because nodes can move to another executor thread, but each node runs them synchronously and
serially. Do bounded work, and do not recursively spin or wait for executor-dependent results.
Use the ordinary `Service<T>` request stream for deferred replies, runtime I/O and asynchronous
workflow composition.

A callback error returns from the active spin without implicitly removing the endpoint. Its
taken request gets no response; the client can use its existing timeout/cancellation API. Panics
unwind normally through Rust dispatch, never across a native callback boundary. Request storage
is retired on success, errors and unwinding; later requests can be handled when spinning resumes.
A failed response-buffer allocation leaves no native service or registry entry. Partial native
response updates follow ordinary fallible message-copy semantics and can be overwritten on retry.

[Independent ready dispatch](DISPATCH_ERRORS.md) preserves other ready endpoint/action work
before returning the first error. The [recovery example](../r2r/examples/dispatch_recovery.rs)
retains one caller-owned application future across a reported callback error.

The implementation is isolated in `services/callback` and `nodes/service_callbacks`, with nearby
native storage, identity, fairness, outcome and ownership tests. Nodes without callback services
allocate no callback registry or extra native guard. The ordinary deferred service path retains
its existing channel and response ownership behavior.

```sh
cargo run -p r2r --example service_callback
cargo test -p r2r --lib callback_service -- --test-threads=1
cargo test -p r2r --lib --release callback_service -- --test-threads=1
```

Native tests verify zero owned request decoding, complete dynamic responses through
128/16/0/32/128-value updates, stable response/string buffers, native allocation/destruction,
failure rollback, exact request sequence identity, service event payloads, callback errors/panic,
cross-node action/service readiness and current-executor cleanup wakes. Fewer payload copies and
per-request Rust objects are structural properties; end-to-end speed claims require the isolated
application benchmark matrix.
