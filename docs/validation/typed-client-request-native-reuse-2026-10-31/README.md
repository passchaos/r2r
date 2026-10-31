# Typed client request native reuse validation

The Rust-style `Client<T>::request(&Request)` and `request_with_info` APIs remain unchanged. A
client and its clones now share a one-slot native request-preparation cache. Conversion still
happens before the existing client mutex; overlapping calls therefore check out independent native
containers rather than extending the mutex critical section.

A real `AddTwoInts` oracle sends requests through an ordinary typed service, validates exact
responses, and observes one native request allocation across successful calls and a cloned client.
An injected `try_copy_to_native` failure retires the checked-out container before any RCL request is
registered; the next call creates clean storage and completes normally. Explicit endpoint
destruction plus final clone drop destroys the retained container exactly once.

Jazzy runs the oracle in debug and release under Cyclone DDS and Fast DDS. Humble/Fast DDS and
Kilted/both RMWs run it in debug. Every environment runs warnings-denied focused Clippy. Workspace
all-target Clippy and the true no-ROS gate pass. The complete Jazzy release library suite passes
822 cases with 20 manual stages ignored under each RMW.

The deterministic five-repetition stage prepares 10,000
`rcl_interfaces/srv/DescribeParameters` requests per path, each containing one 4096-byte name:

| Native preparation | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh container | 40,000 | 30,000 | 41,460,000 |
| Reused container | 0 | 0 | 0 |

Every repetition has identical allocation counts. Reuse removes four allocation calls, three free
calls and 4,146 requested bytes per request. The stage excludes service discovery, RCL and DDS; its
elapsed diagnostics are not timing-qualified.

```sh
bash docs/validation/typed-client-request-native-reuse-2026-10-31/run-jazzy.bash
bash docs/validation/typed-client-request-native-reuse-2026-10-31/run-compat.bash
bash docs/validation/typed-client-request-native-reuse-2026-10-31/run-no-ros.bash
python3 docs/validation/typed-client-request-native-reuse-2026-10-31/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
