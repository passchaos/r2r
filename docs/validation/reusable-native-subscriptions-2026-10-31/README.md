# Reusable native subscription validation

`Node::subscribe_native_reusable` delivers `NativeSubscriptionSample<T>` items through the normal
async subscription stream. Every sample owns a generated native container and returns it to one
per-subscription cache on drop. Existing typed/raw/native subscription APIs remain unchanged.

The real `UInt64MultiArray` oracle delivers 4096, 128, 64 and 32 elements. It requires the first
container address to reappear after drop, distinct addresses while two samples are retained, the
most recently returned address on the next take, exact payload preservation, and valid sample
access after deterministic endpoint destruction. The shared action native-feedback lease test also
passes after extracting the common pool implementation.

Jazzy runs the real oracle in debug and release under Cyclone DDS and Fast DDS. Humble/Fast DDS and
Kilted/both RMWs run it in debug. Every environment runs the complete example, the shared action
lease regression and focused warnings-denied Clippy. Workspace all-target Clippy and the true
no-ROS gate pass; the latter verifies 84 public pages and 341 methods.

The deterministic five-repetition allocation stage compares 10,000 fresh and reusable deliveries
of one 4096-element generated message through channel send/receive and payload validation:

| Delivery | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh | 40,000 | 40,000 | 329,440,000 |
| Reusable | 20,000 | 20,000 | 1,360,000 |

Every repetition has the same counts. The reusable path removes two allocations/frees per sample
and 328.08 MB (about 99.59%) of requested bytes. The preload probe passes its allocation, content,
alignment, failure, epoch, thread and reset oracles. Elapsed values are diagnostic only; this stage
does not establish DDS or rclcpp latency leadership.

```sh
bash docs/validation/reusable-native-subscriptions-2026-10-31/run-jazzy.bash
bash docs/validation/reusable-native-subscriptions-2026-10-31/run-compat.bash
bash docs/validation/reusable-native-subscriptions-2026-10-31/run-no-ros.bash
python3 docs/validation/reusable-native-subscriptions-2026-10-31/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
