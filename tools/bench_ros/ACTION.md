# Confirmed action lifecycle comparison

Independent Rust/rclcpp processes execute native Fibonacci action goals through acceptance,
feedback and terminal results. Scenario 0 succeeds; scenario 1 waits for the client to consume
feedback, accepts cancellation and returns a Canceled result. Every goal carries an increasing
`order`; the exact three-element feedback/result fixture is checked, along with one acceptance,
feedback and result per goal and the correct terminal state. Both client/server language
directions use the same oracle and protocol.

```sh
python3 tools/bench_ros/build.py --workload action --output-dir target/ros-action
python3 tools/bench_ros/transport.py --workload action \
  --rust target/ros-action/r2r-candidate \
  --cpp target/ros-action/rclcpp-comparison \
  --cpu-ping 24 --cpu-echo 25 --domain-id 134 --rmw rmw_cyclonedds_cpp \
  --scenarios 0,1 --iterations 1000 --warmup 50 --repetitions 5 \
  --build-manifest target/ros-action/build-manifest.json \
  --output target/ros-action/cyclonedds.json
python3 tools/bench_ros/verify_transport.py target/ros-action/cyclonedds.json
```

Use two idle distinct CPUs, an unused DDS domain and no competing builds/tests. Change the
RMW to `rmw_fastrtps_cpp` for the second matrix. Raw reports retain each endpoint's lifecycle
counts, complete RTT samples, acceptance/cancellation stage samples, compiler/package versions,
binary/source hashes and a deterministic gzip artifact. The offline validator checks every
count, sample, stage percentile, throughput calculation and combination/repetition.

## Measurement boundary and handshake

The comparison deliberately guarantees observed feedback; it is not a bare goal/result benchmark.
C++ can discard feedback arriving before its goal-response handling installs the accepted goal.
Both servers therefore accept goals **deferred**, and both clients send an auxiliary `AddTwoInts`
request on `<action-name>/ready` after acceptance and result-observer setup. Phase 0 confirms
that observation is ready, then the server executes and publishes exactly one feedback. For
success, the client sends phase 1 after feedback consumption, then the server completes the
result. For cancellation, the cancel request itself confirms feedback consumption instead.
These identical readiness/feedback handshakes are part of the lifecycle RTT in both languages.

Lifecycle RTT starts before goal submission and ends at terminal result retrieval. Acceptance
time ends after accepted-goal retrieval and result-request submission, excluding phase 0.
Cancellation time starts at the cancel request and ends at accepted-cancel response retrieval.
Server validation and all handshakes are included in lifecycle time. One goal is in flight.
The completed-goals rate includes application validation between calls. Discovery, warmup and
teardown are excluded. Rejected control goals -1 and -2 confirm all measured results were
retrieved before server exit.

Goal/result/cancel services and feedback use reliable volatile KeepLast(16). Status topics retain
native ROS defaults; result retention is 10 seconds. Both applications use one executor/spin
thread with a 100 ms blocking wait cap and have no rosout or parameter endpoints. The auxiliary
readiness service uses matching ServicesQoS KeepLast(16).

Results remain retained for the same 10-second interval in both implementations. Consecutive
goals therefore create a cohort of native action states and status messages; this lifecycle
workload measures that cohort's cost too. Interpret its percentiles over the entire measured
run, not as a constant cost for an isolated goal. Faster completion can leave more retained
goals during the same retention window, which is part of the configured protocol behavior.

## Baseline comparison

`build.py --baseline-ref <revision>` and `transport.py --baseline-rust <binary>` add the three
baseline combinations. The same harness source is compiled against the historical library.
For the cancel-progress change, scenario 1 alone can compare the former timeout-sized delay
without extending unrelated success workloads. A passing `--max-regression` applies to the
measured scenarios only; it does not imply unmeasured workloads passed. The raw artifact stores
the gate result and the verifier can require a passing recorded gate.

The runnable `cargo run -p r2r --example action_lifecycle` demonstrates deterministic deferred
execution, observer readiness, feedback, success and cancellation using futures-rs. It completes
with a wall-clock deadline and does not require an async runtime or random outcomes.

This fixture is a protocol/observability workload with small payloads. Bare acceptance-only,
fire-and-forget, larger feedback streams, multi-goal concurrency, native/loaned paths, networked
hosts and multithread scaling are separate measurements still needed for the broader goal.
