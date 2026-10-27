# Independent-process ROS service roundtrip

This workload sends `example_interfaces/srv/AddTwoInts` requests through the real ROS service
protocol, with one or a bounded number of requests in flight. It covers Rust/Rust, C++/C++,
Rust client/C++ server and C++ client/Rust server under identical QoS and CPU conditions.
Request `a` is the monotonically increasing sequence; `b` is a deterministic signed operand.
Servers verify every sequence and operand and respond with the exact sum. Clients verify every
response, message count and sequence checksum. Native type support and service transport remain
part of the measurement.

```sh
python3 tools/bench_ros/build.py --workload service --output-dir target/ros-service
python3 tools/bench_ros/transport.py --workload service \
  --rust target/ros-service/r2r-candidate \
  --cpp target/ros-service/rclcpp-comparison \
  --cpu-ping 24 --cpu-echo 25 --domain-id 122 --rmw rmw_cyclonedds_cpp \
  --windows 1,16 --iterations 5000 --warmup 100 --repetitions 5 \
  --build-manifest target/ros-service/build-manifest.json \
  --output target/ros-service/cyclonedds.json
python3 tools/bench_ros/verify_transport.py target/ros-service/cyclonedds.json
```

Choose two otherwise idle distinct CPUs and an unused DDS domain, finish builds and tests before
measurement, and run the Fast DDS matrix with `--rmw rmw_fastrtps_cpp`. The runner reverses
language combination order each repetition, isolates topics/service names and removes inherited
middleware profiles. The report records binary and library source hashes, package/compiler
versions, affinity/domain settings, endpoint results and compressed raw RTT samples.

Both applications have a single-threaded execution loop, a 100 ms blocking spin wait cap,
`ServicesQoS` with reliable volatile KeepLast(max(16, window)), no rosout, and no parameter
services or event publisher. Rust uses `Client::request` futures and a `Service` stream. C++
uses `async_send_request` and a standard service callback. Results are observed in submission
order in both languages; RTT includes queueing within the bounded window. Per-request watchdogs
fail missing responses after 10 seconds. Warmup drains completely before measured requests start.

RTT starts before the async request call and ends on response retrieval, before client-side sum
validation. Server-side validation and response creation are included. Completed requests per
second includes request submission, allocation and client validation, excluding discovery,
warmup and teardown. Window 1 is stop-and-wait latency; larger windows are bounded-concurrency
throughput/queueing scenarios. Summed per-request latency can exceed wall time when requests
overlap, and the offline validator uses that workload's timing boundary.

After measured responses complete, a stop request and response are exchanged, followed by an
explicit client confirmation. Servers validate both control requests. Client-side pending
confirmation state is removed; it does not enter measured counts. A valid exit therefore
establishes complete data-response delivery in addition to the sum oracle.

The shared transport tool also supports `--baseline-rust` and the seven-combination regression
matrix. It validates the complete window/language/repetition matrix, counts, checksums, individual
RTT percentiles, throughput and summary medians. `--max-regression` applies to same-language
candidate/baseline medians; `--require-performance-pass` requires a recorded passing gate when
verifying such a report. No regression pass is inferred from a correctness-only comparison.

This fixed-size service workload does not cover large service payloads, action workflows,
multithreaded executors, timeouts under load, networked machines or saturation at unlimited
concurrency. It establishes a reproducible first service comparison; those scenarios remain
separate work toward the overall Rust/C++ goal.
