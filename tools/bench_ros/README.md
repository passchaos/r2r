# Rust/rclcpp ROS message comparison

This harness compares equivalent stages of `std_msgs/msg/Int32MultiArray` handling. Each
independent executable constructs the same layout and integer payload and verifies the complete
decoded message. The runner additionally requires byte-for-byte identical CDR fixtures and
operation checksums, the same RMW, input size and iteration count. Failed or malformed results
stop the run.

The [end-to-end pub/sub harness](TRANSPORT.md) compares separate ping/echo processes and
includes both cross-language directions, individual RTT percentiles and raw sample artifacts.
The [service harness](SERVICE.md) measures native `AddTwoInts` roundtrips with both single-request
and bounded-concurrency windows using the same archive/verification infrastructure.
The [action harness](ACTION.md) validates deferred acceptance, observed feedback, successful and
canceled terminal results, and records complete lifecycle and protocol-stage samples.

The stages are:

* `copy_reuse`: copy a borrowed source into an initialized destination whose storage can be
  reused. Rust uses `WrappedTypesupport::try_copy_to_native`; C++ uses message assignment.
* `serialize_owned`: serialize a borrowed message and return a new owned byte buffer. Both
  reuse the middleware's output buffer, then copy to a fresh `Vec<u8>` or `std::vector<uint8_t>`.
* `serialize_reuse`: use `MessageSerializer<T>` to reuse native fields and consume its borrowed
  bytes without an application copy. C++ consumes its reusable `SerializedMessage` buffer.
  A historical Rust baseline without this API uses `to_serialized_bytes()`, including its native
  allocation and owned-result copy, then consumes the same bytes. This measures the improvement
  available through the new API rather than a change to the old API's ownership semantics.
* `serialize_owned_reuse`: use the new serializer but explicitly copy the result to fresh owned
  bytes; C++ likewise copies its reusable output to `std::vector<uint8_t>`.
* `deserialize_owned`: deserialize identical bytes into a freshly owned message. Rust includes
  its native-to-Rust conversion; C++ uses `rclcpp::Serialization` directly.

These are message-stage measurements. They do not measure end-to-end DDS delivery, executor
scheduling, service or action latency, or intra-process transport. Source representations differ
between languages; conversion work is included where the application API requires it.

## Build and run

Run from the repository root after sourcing a supported ROS environment with `rclcpp`,
`std_msgs`, libclang, a C++17 compiler, CMake, Python 3 and the repository-pinned Rust toolchain:

```sh
python3 tools/bench_ros/build.py --baseline-ref a6de146
python3 tools/bench_ros/run.py \
  --rust baseline=target/ros-comparison/r2r-baseline \
  --rust candidate=target/ros-comparison/r2r-candidate \
  --cpp target/ros-comparison/rclcpp-comparison \
  --cpu 30 --rmw rmw_cyclonedds_cpp \
  --sizes 0,32,4096 --iterations 100000 --minimum-run-ms 200 --repetitions 5 \
  --build-manifest target/ros-comparison/build-manifest.json \
  --max-regression 1.05 \
  --output target/ros-comparison/results.json
```

Choose an available, otherwise idle CPU. Stop competing compilation and testing before measuring.
The builder uses separate Cargo targets for the baseline and candidate, the same current benchmark
source and lockfile, release optimization, and an independently compiled rclcpp binary. It records
source hashes and the baseline revision. To compare only the current library with C++, omit
`--baseline-ref` and the baseline `--rust` argument.

The runner alternates binary order between repetitions. It calibrates a common iteration count
for each stage/input pair so the fastest implementation runs for approximately the requested
minimum duration; `--iterations` is a lower bound. The report retains every raw result, binary
hashes, compiler and ROS package versions, CPU affinity, build settings, and median ratios.
Values below 1 in `relative_to_rclcpp` indicate less time per operation than rclcpp.
`--max-regression` rejects a candidate whose median exceeds the baseline by the configured ratio
in any measured stage. It writes the raw report before reporting a failed gate.
Use `python3 tools/bench_ros/verify_codec.py <report.json>` to recheck archived workload identity,
raw records, summary medians and gate consistency. `--require-performance-pass` additionally
requires a recorded passing regression gate.

Select extra API modes with `--modes serialize_owned,serialize_reuse,serialize_owned_reuse`.
The builder records whether the serializer API exists in each checkout and uses a benchmark cfg
to select the historical fallback without adding this feature to a baseline library. Default
modes preserve the original copy/owned-serialization/owned-deserialization workload.

`batch_ns_per_op_p50/p95/p99` describe operation time averaged over batches of up to 1000 calls.
They are not individual-message tail-latency measurements. `native_copy_pointer_changes` only
observes pointer changes; an allocator can reuse an address, so it is not an allocation counter.
Reproducibility records establish the workload and environment, not performance on other hardware
or middleware. Keep reports scoped to the stages actually measured.
