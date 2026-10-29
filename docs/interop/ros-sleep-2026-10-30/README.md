# Aggregated ROS-clock sleep semantics

This slice supplies owned, independently cancellable absolute/relative ROS-clock waits. It closes
an application API gap: ordinary wall sleeps cannot model paused or backwards simulated time.
Native Node/shared/background execution drives pending sleeps without an application spin loop
or a native timer per wait. A node lazily owns one steady native wake timer and one ROS clock
jump callback, with an indexed minimum heap and generation-protected reusable slots.

## Acceptance boundaries

Past/zero deadlines complete without scheduler/entity allocation. System-backed ROS time uses
the native wake timer but verifies exact readiness against the ROS clock. Simulated time pauses
and backwards jumps preserve the absolute target; forward clock delivery completes only due
waits. Changing between simulated and system-backed ROS time interrupts existing registrations
with a distinct source-change error. New registrations after the switch survive cleanup of older
interrupted waits. Signed targets and relative overflow are explicit.

Cancellation removes only its indexed deadline immediately in O(log n), without lazy tombstones.
100000 repeated far-future registrations/cancellations reuse one slot. A sorted oracle exercises
10000 mixed insertion/removal/reuse steps. 512 simultaneous deadlines use one native timer and
at most 64 completions per preparation/dispatch pass, with runnable continuation. Ordinary wall
timers continue progressing while ROS sleeps remain pending. Empty queues omit the canceled
wake timer from native waits. Stable sleep registrations do not add clock queries to unrelated
traffic.

The native wake clock is steady and independent from the managed ROS clock. Periods are bounded
and exact ROS targets are rechecked after wakeup. This prevents RCL's unchecked now+period
arithmetic from overflowing when simulated time reaches i64::MAX; that signed boundary has its
own native test. Native timer finalization is serialized with its owned clock.

Background and returned-node operations follow current native guard bindings. Completion,
cancellation and node destruction run application wakes outside queue/clock/context locks.
Context shutdown triggers native guards while valid, then invokes application observers after
unlock. Weak shutdown/jump registrations exclude context ownership cycles. The public future can
outlive its node and returns an explicit node-invalid or context-shutdown error.

## Independent C++ trace

Independent r2r and rclcpp processes consume one exact command/time/state trace. Native `/clock`
updates advance to 100, start a wait for 200, move through 150, pause, move backwards to 50,
advance to 199 and then 200. Both remain pending until 200, then complete. A subsequent wait for
300 is explicitly canceled. Each process reports native ROS times and state after every command;
offline rejection tests catch premature completion, changed clock values and missing cancellation.
Both Jazzy RMWs pass with source/binary manifests. This is semantic comparison evidence, not an
application timing or throughput benchmark. C++ uses its existing blocking clock sleep thread;
Rust drives the owned future through the ordinary native executor.

```sh
python3 tools/ros_oracles/ros_sleep.py verify --require-current-sources \
  docs/interop/ros-sleep-2026-10-30/cyclonedds.json \
  docs/interop/ros-sleep-2026-10-30/fastdds.json
```

## Verification scope

Seventeen focused cases cover heap, cancellation, system/paused/signed/backwards time, maximum
targets, source changes, bounded cohorts, normal timer progress, actual `/clock`, current-executor
transfer and lock-reentry shutdown. Final Jazzy debug and both optimized main-library suites pass
658 tests with eight manual measurements ignored. Humble/Fast DDS and Kilted/both RMWs pass
17 sleep and five shutdown cases, the runnable example and all-target warning-denied Clippy.
The true no-ROS gate validates all targets, common/generator and oracle tests, two ownership
compile-fail cases and 37 public artifacts with 76 required methods. CI now runs the native
Rust/rclcpp trace on the supported Humble-and-newer matrix.

Iron/Lyrical/Rolling runtime evidence, external C++ traces outside Jazzy, system-clock correction
behavior, adversarial concurrent clock writers, large interrupted cohorts and network/multithread
application performance remain wider evidence scope. The replaceable heap/native wake boundary
preserves the path to larger scaling and reactor integration without changing future semantics.

## Artifact identity

The current-source manifest covers 199 workspace/snapshot/harness/gate inputs and 10 compressed
verification logs. Both native trace reports carry independently built binary hashes and the
complete build source manifest. The final workspace includes library, integration and Rustdoc
checks; both optimized RMW library suites execute all 658 cases. The rejected first sorted-oracle
fixture failed on an empty oracle after its first cancellation; its correction leaves the
randomized independent sorted comparison and 100000-slot-reuse gate intact.

```sh
python3 tools/bench_ros/verify_pubsub_pipeline.py --compatibility-manifest \
  docs/interop/ros-sleep-2026-10-30/compatibility.json
```

No latency/throughput gate is declared in these semantic reports. Existing application
performance matrices need a final-source rebuild and isolated comparison before promotion.
