# Typed client response native reuse validation

Typed client response futures remain ordinary owned Rust values. Each `TypedClient<T>` endpoint now
owns one native response-take slot. The native lease crosses the existing detached conversion
closure, so `T::Response::from_native` still runs after releasing the client mutex; the lease is
returned immediately after decode and never escapes through the future.

The cache belongs to the native client endpoint rather than public client handles. Explicit client
destruction therefore releases it even when a cloned Rust handle remains. Plain `request` and
`request_with_info` responses share the same slot, while metadata remains copied into an owned
`ServiceResponseInfo`.

A real `AddTwoInts` oracle validates exact plain and metadata-bearing responses, one native
response allocation across both paths, positive sequence metadata, endpoint-owned destruction and
stale-clone rejection. Jazzy runs it in debug/release under Cyclone DDS and Fast DDS;
Humble/Fast DDS and Kilted/both RMWs run it in debug.
Focused/workspace Clippy and the true no-ROS gate pass; the complete Jazzy release suite reports
822 passed with 20 manual stages ignored under each RMW.

The deterministic five-repetition stage fills and decodes 10,000 `SetBool` responses per path,
each containing a 4096-byte string:

| Native response storage | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh | 40,000 | 30,000 | 82,260,000 |
| Reused | 10,000 | 10,000 | 40,960,000 |

Every repetition has identical allocation counts. Native reuse removes three allocation calls,
two free calls and 4,130 requested bytes per response. The remaining allocation is the independent
owned Rust string required by the public API. The stage excludes RCL/DDS and its elapsed values are
not timing-qualified.

```sh
bash docs/validation/typed-client-response-native-reuse-2026-10-31/run-jazzy.bash
bash docs/validation/typed-client-response-native-reuse-2026-10-31/run-compat.bash
bash docs/validation/typed-client-response-native-reuse-2026-10-31/run-no-ros.bash
python3 docs/validation/typed-client-response-native-reuse-2026-10-31/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
