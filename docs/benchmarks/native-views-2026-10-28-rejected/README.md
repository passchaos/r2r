# Rejected borrowed native receive and forwarding experiment

The candidate is removed from the active library. Its complete CycloneDDS A/B failed the
preselected 1.05 median regression limit: zero-payload p99 increased 6.0%, and 4096-payload p99
increased 27.8%. Large-payload p50 gains and decode-stage improvements do not override this
failure. The delivered change retains the comparison harness, raw evidence and a symmetric
same-binary calibration gate.

## Prototype and correctness

`subscribe_view<T>` returned independently owned native messages with immutable generated
accessors for scalar, sequence, fixed-array, nested-message, string and wide-string fields.
Slices borrowed native payloads; nested/message/string iterators used inline generic projections.
Owners, views and iterators supported shared/scoped thread reads. `publish_view` forwarded the
same immutable native storage while views remained live. Native allocation, deserialization
and DDS serialization still occurred; this was an explicit API choice rather than replacing
ordinary typed subscriptions.

The [archived patch](candidate.patch.gz) applies cleanly to `37c7f92` and contains the complete
prototype, nearby tests, image example and comparison tooling. Native pointer identity and
complete owned-message oracles cover primitive/fixed/nested/string data, Unicode errors,
malformed bounds, backpressure, subsequent-take independence, node destruction and thread
transfer/shared release. Six view cases passed under both Jazzy RMWs and native Humble/Kilted;
the final immutable-forwarding/stale-publisher extension passed the Jazzy/Fast DDS specialty.
Five Rust compile contracts verified working access plus four prohibited ownership/mutation
operations. The prototype's saved doc-only view implementations remained unfinished.

## Decode-stage evidence

The [stage records](decode-stage.json) compare owned conversion/destruction and borrowed reads
in one optimized binary on CPU 24. Each native `Int32MultiArray` fixture has a complete oracle
before timing; both timed paths consume the same length/first/middle/last checksum. Five
alternating-order repetitions use 500000 iterations. Fixture creation and full validation are
excluded. This stage isolates conversion and does not establish allocation counts or transport
performance.

| Native i32 values | Owned conversion, ns | Borrowed read, ns | Less time |
| ---: | ---: | ---: | ---: |
| 1 | 22.06 | 1.71 | 92.2% |
| 33 | 25.26 | 1.71 | 93.2% |
| 4097 | 330.83 | 1.72 | 99.5% |

## Complete transport gate

Separate historical Rust, candidate Rust and rclcpp executables run the same ping/echo protocol.
Rust views receive the native payload and forward through immutable native publication;
historical Rust uses owned typed receive/publication. The seven pairs include baseline/baseline,
candidate/candidate, C++/C++, and both Rust/C++ directions for each Rust version. Every sequence,
layout and payload value is validated, and an explicit completion message confirms delivery.

Five alternating-order repetitions measure 5000 roundtrips after 100 warmups for 0, 32, 4096
and 65536 extra u64 values. Reliable volatile KeepLast(16), one request in flight, a 100 ms spin
cap, disabled rosout/parameter endpoints and localhost DDS remain equal. RTT ends before
initiator payload validation; completion rate includes that validation and all measured work.
It is stop-and-wait performance, not pipelined saturation throughput.

Microseconds below are medians of repetition percentiles:

| Payload u64 values | Baseline p50 | Candidate p50 | C++ p50 | Baseline p99 | Candidate p99 | C++ p99 |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 41.401 | 42.003 | 43.535 | 48.456 | 51.361 | 54.786 |
| 32 | 42.444 | 42.301 | 44.680 | 50.666 | 52.106 | 53.708 |
| 4096 | 98.991 | 91.485 | 95.437 | 221.531 | 283.175 | 840.076 |
| 65536 | 920.105 | 846.399 | 867.378 | 1066.568 | 978.093 | 1003.312 |

The 4096/65536 p50 improves 7.6%/8.0%. The 65536 completion rate remains about 883/s for both
Rust versions versus about 924/s for C++; median RTT alone would overstate throughput. Some
mixed-pair repetitions have large tails, including a p99 around 194 ms. All repetitions and
samples remain in the report; none are removed as outliers.

The host is an i9-13900K with ping/RMW threads on CPU 24 and echo/RMW threads on CPU 25. Jazzy,
Rust 1.85 release and GCC 13.3 `-O3 -DNDEBUG` use default ISA targeting. Domain 227 isolates the
run. No task-owned build or test overlaps formal measurement; unrelated unrestricted Zig
work is a shared-host limitation. Exact binary/source hashes, ROS package versions and build
manifests accompany the evidence. Fast DDS protocol smoke and native Humble/Kilted checks are
correctness evidence; its full performance matrix was not run after the earlier gate failure.

## Same-binary calibration

The failed dimensions then run with the identical candidate executable in both Rust slots,
five repetitions and 5000 samples per pair in domain 231. The original one-way regression gate
passes. Symmetric calibration exposes apparent improvements as instability too:

| Payload | First slot p50, us | Second slot p50, us | First slot p99, us | Second slot p99, us |
| ---: | ---: | ---: | ---: | ---: |
| 0 | 41.692 | 41.239 | 58.779 | 49.300 |
| 4096 | 82.404 | 81.674 | 111.859 | 111.347 |

The zero-payload p99 differs by a maximum bidirectional ratio of 1.192 and rate by 1.084,
failing symmetric 5% calibration. The 4096 control is close, but this does not overturn the
failed independent-version A/B. The symmetric gate is computed from the preserved records;
the original collection metadata/source hashes remain intact.

## Evidence and replay

* [Failed A/B](cyclonedds.json): 140 records and [700000 RTT samples](cyclonedds.samples.json.gz).
* [A/A calibration](aa-cyclonedds.json): 70 records and [350000 RTT samples](aa-cyclonedds.samples.json.gz).
* [Prototype patch](candidate.patch.gz), with digest and base revision verified by the A/B verifier.

```sh
python3 tools/bench_ros/verify_transport.py \
  docs/benchmarks/native-views-2026-10-28-rejected/cyclonedds.json \
  docs/benchmarks/native-views-2026-10-28-rejected/aa-cyclonedds.json
```

Integrity passes; `--require-performance-pass` rejects both the failed A/B and failed symmetric
calibration. In an isolated `37c7f92` checkout, decompress/apply the prototype patch, then build
with `tools/bench_ros/build.py --workload native_views --baseline-ref 37c7f92`. Run
`transport.py --native-views` for the prototype, or add `--baseline-native-views
--same-binary-control` with identical Rust executable paths for symmetric calibration. Without
the prototype, the retained harness uses typed receive; declare `--rust-receive-path owned`.

Next work must stabilize short-payload transport measurement and account for native/Rust
allocation and buffer lifetime across the complete path before promoting borrowed receive or
forwarding as a performance optimization.
