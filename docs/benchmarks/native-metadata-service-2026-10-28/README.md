# Service transport after native metadata initialization

The Rust application includes the service metadata safety correction: complete
native metadata starts initialized, and action replies read only their required
sequence number. The comparison uses the unchanged independent rclcpp
application and the complete payload/sequence/checksum/completion oracles.

Both Jazzy RMWs cover Rust/Rust, C++/C++, Rust/C++ and C++/Rust, windows 1/16,
5000 measured requests, 100 warmup requests and five repetitions. CPUs 24/25 are
distinct physical cores; DDS domain 122 and identical reliable/volatile service
QoS are fixed. Language-pair order alternates between repetitions. Builds and
tests completed before these measurements. Each report contains 40 records and
200000 raw RTT samples; both archived reports pass `verify_transport.py`.

Values below are the report's medians across repetitions. Each numeric pair is
Rust/Rust followed by C++/C++.

| RMW | Window | RTT p50, us | RTT p99, us | Requests/s | Rust throughput advantage |
| --- | --- | --- | --- | --- | --- |
| cyclonedds | 1 | 44.08 / 47.01 | 60.79 / 61.42 | 21,173 / 20,748 | 2.1% |
| cyclonedds | 16 | 301.29 / 374.23 | 385.87 / 464.60 | 52,867 / 42,938 | 23.1% |
| fastdds | 1 | 47.86 / 51.20 | 66.96 / 71.11 | 20,137 / 18,918 | 6.4% |
| fastdds | 16 | 288.46 / 431.33 | 504.50 / 691.80 | 52,727 / 36,168 | 45.8% |

At window 16, current Rust throughput is 23.1% above C++ with CycloneDDS and
45.8% above C++ with Fast DDS. The Rust p50/p99 values are lower in all four
same-language cells. Cross-language endpoints also satisfy the complete
transport oracle.

These results compare the corrected implementation with C++; they do not use
the implementation that read uninitialized metadata as a performance baseline.
No candidate/previous-Rust regression pass is claimed, and zero initialization
itself is not presented as an optimization. The source manifest includes
`r2r/src/native_metadata.rs` alongside endpoint/benchmark sources. Raw compressed
samples and binary/source/package/compiler/affinity settings remain in the
reports.

Evidence remains limited to this fixed-size, single-threaded service workload
and the measured hardware/distributions. Dynamic payloads,
metadata-observer costs, concurrent-goal actions, allocation accounting, fanout
and multithreaded scaling remain separate evidence requirements.

```sh
python3 tools/bench_ros/verify_transport.py docs/benchmarks/native-metadata-service-2026-10-28/cyclonedds.json
python3 tools/bench_ros/verify_transport.py docs/benchmarks/native-metadata-service-2026-10-28/fastdds.json
```
