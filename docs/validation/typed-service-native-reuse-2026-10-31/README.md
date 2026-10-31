# Typed service native reuse validation

The Rust-style `Service<T>` stream and `ServiceRequest<T>::respond` APIs remain unchanged. Each
typed service now owns one native request-take slot and one native response-preparation slot.
Request decoding occurs in the existing detached delivery closure, after releasing the service
mutex, and returns its native lease before the owned Rust request enters the channel.

Response buffers are shared by every request and request clone from the service. A fallible copy
error retires its native container without consuming the request's once-only response opportunity;
an existing clone can retry. Concurrent requests or responses check out independent storage while
the shared slot is occupied.

A real `AddTwoInts` oracle validates three exact roundtrips, request-take reuse, response reuse,
injected response conversion failure, clone retry and deterministic destruction of both buffers.
Jazzy runs it in debug/release under Cyclone DDS and Fast DDS; Humble/Fast DDS and Kilted/both RMWs
run it in debug. Focused and workspace warnings-denied Clippy plus the true no-ROS gate are required.
All pass; the complete Jazzy release suite reports 822 passed with 20 manual stages ignored under
each RMW.

The deterministic five-repetition stage models request decode and response preparation for 10,000
`SetBool` calls per path, using a 4096-byte response string:

| Native storage | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh request + response | 40,000 | 30,000 | 41,310,000 |
| Reused request + response | 0 | 0 | 0 |

Every repetition has identical allocation counts. Reuse removes four allocation calls, three free
calls and 4,131 requested bytes per service call. The stage excludes RCL, DDS and channel delivery;
its elapsed diagnostics are not timing-qualified.

```sh
bash docs/validation/typed-service-native-reuse-2026-10-31/run-jazzy.bash
bash docs/validation/typed-service-native-reuse-2026-10-31/run-compat.bash
bash docs/validation/typed-service-native-reuse-2026-10-31/run-no-ros.bash
python3 docs/validation/typed-service-native-reuse-2026-10-31/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
