# Default typed publisher native reuse validation

The ordinary Rust-style `Publisher<T>::publish(&T)` API remains unchanged. Publisher handles and
their clones now share a one-slot native preparation cache. A publish checks out the cached
container, updates its generated fields, calls `rcl_publish`, then returns the container. Concurrent
publishes never alias: while one lease is active, another publish creates independent storage and
the most recently returned container becomes the cache.

A real `UInt64MultiArray` oracle publishes 4096, 64 and 16 elements through the default typed API,
receives exact ordinary Rust messages, and observes one native allocation across the first two
publishes and a cloned publisher handle. An injected `try_copy_to_native` failure destroys the
checked-out container rather than caching partially updated state; the next successful publish
creates clean storage and endpoint destruction plus final clone drop destroys it exactly once.

Jazzy runs the oracle in debug and release under Cyclone DDS and Fast DDS. Humble/Fast DDS and
Kilted/both RMWs run it in debug. Every environment runs warnings-denied focused Clippy. Workspace
all-target Clippy and the true no-ROS gate pass. The complete Jazzy release library suite passes
822 cases with 20 manual stages ignored under each RMW.

The deterministic five-repetition stage prepares 10,000 4096-element messages per path and
validates native sequence size and storage:

| Native preparation | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh container | 20,000 | 20,000 | 328,240,000 |
| Reused container | 0 | 0 | 0 |

Every repetition has identical allocation counts. Reuse removes two allocation/free calls and
32,824 requested bytes per message at the Rust-to-native preparation boundary. The stage excludes
RCL/DDS and its elapsed diagnostics are not timing-qualified.

```sh
bash docs/validation/typed-publisher-native-reuse-2026-10-31/run-jazzy.bash
bash docs/validation/typed-publisher-native-reuse-2026-10-31/run-compat.bash
bash docs/validation/typed-publisher-native-reuse-2026-10-31/run-no-ros.bash
python3 docs/validation/typed-publisher-native-reuse-2026-10-31/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
