# Same-Node callback group application comparison

This workload compares independent r2r and rclcpp binaries, each containing one
Node, one AddTwoInts service, one client and a fixed native/callback worker pool.
The application controller stays outside both pools. Exclusive mode launches
min(worker budget, 2) workers; reentrant mode launches the budget, and body
concurrency is bounded by that budget and the request window. rclcpp uses
MultiThreadedExecutor and the corresponding callback group. r2r uses
CallbackExecutor and native scoped service callbacks. Intra-process shortcuts,
rosout, parameter services and type-description services are disabled.

Requests carry a unique ordinal and an operation count. Both implementations
run the same wrapping u64 xorshift/add dependency chain and return its positive
i64 result. The controller precomputes expected values before timing. A separate
Python integer reference checks every measured response. This is a compute
workload encoded in AddTwoInts fields, rather than the normal a+b application.

Eligible reentrant runs first send two special requests that rendezvous inside
the same handler. The report must contain two invocations and a simultaneous
active peak of two. This challenge precedes warmup and measurement. Normal
requests use preallocated server rows and result buffers, complete out of order,
and retain client and server native request IDs. Every request is accepted
exactly once. Trace intervals independently verify the request window and
handler concurrency bound.

The measured window starts before submitting its first request and ends after
receiving and validating all results. RTT starts before request creation/send and
ends when the application obtains the response; window queueing is included.
Server entry/finish use the same process monotonic clock. Request/result checking
is identical across implementations, and server telemetry uses matching atomic
memory orders. Formatting, sorting, setup, discovery, warmup, teardown and the
independent reference calculation are outside the measured window.

The optional Linux/glibc probe counts allocations and requested bytes across all
process threads, with CPU and context-switch deltas. Threads, VmRSS and VmHWM are
sampled after measurement and before pool teardown/output construction. VmHWM
is the application's address-space high-water mark; the gate uses it rather
than ru_maxrss, which can retain a larger inherited pre-exec process peak. Raw
probe ru_maxrss remains in each record for diagnosis. Memory high-water values
include setup and warmup, rather than being a window-local allocation delta.

```sh
source /opt/ros/jazzy/setup.bash
python3 tools/bench_ros/build.py --workload callback_groups --with-probe \
    --output-dir target/callback-group-comparison
python3 tools/bench_ros/callback_groups.py \
    --rust target/callback-group-comparison/r2r-candidate \
    --cpp target/callback-group-comparison/rclcpp-comparison \
    --probe target/callback-group-comparison/libr2r_bench_probe.so \
    --build-manifest target/callback-group-comparison/build-manifest.json \
    --cpus 24,25,26,27 --iterations 64 --warmup 16 --repetitions 2 \
    --output target/callback-group-protocol.json
python3 tools/bench_ros/callback_groups.py --verify target/callback-group-protocol.json
```

Cases use kind:workers:window:operations. Default cases cover exclusive and
reentrant execution with empty and CPU-consuming bodies. Choose CPUs available
on the host; the orchestrator reserves other CPUs. Rust/C++ order alternates
between repetitions. All native/RMW workers inherit the same allowed CPU set.
This is one complete process per application, rather than independently pinned
client/server endpoints. Other payloads, pub/sub, actions, timers and networked
hosts need their own comparisons.

Formal runs require --probe, --max-rclcpp-ratio, at least three independently
spaced idle samples, load1 at most 2.0, and continuous monitoring for build/GPU
competition. The gate covers RTT p50/p95/p99, inverse throughput, CPU/request,
allocations/request, requested bytes/request, VmHWM and process thread count.
No threshold is automatically widened. Failed protocol output and partial records
are retained separately; failed isolation writes measurement_started=false.
Timing and resource diagnostics from a smoke run do not establish a performance
advantage.

The builder checks source hashes before/after compilation and records executable
and probe hashes. The runner rejects mismatched build inputs/binaries and checks
sources again after running. Reports retain compressed per-request traces,
package/compiler/CPU metadata, exact execution order and resource samples. The
verifier recomputes summaries and gates, checks actual isolation evidence, and
rejects duplicate/missing requests, wrong response values or IDs, false concurrency,
source changes and fabricated passing flags. The documentation gate runs these
counterexamples without ROS.
