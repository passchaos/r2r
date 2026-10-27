# Reusable borrowed-byte serialization

`MessageSerializer<T>` adds a reusable encoding path whose output is a borrowed byte slice.
The measured 4096-integer fixture uses 16.2% less time than the historical owned-byte API with
CycloneDDS and 42.7% less time with Fast DDS. Fast DDS smaller fixtures improve by 39.6–41.4%;
CycloneDDS smaller fixtures are 3.1–3.4% slower. This is an API choice, not a replacement for
`to_serialized_bytes()` or a claim about end-to-end transport performance.

## API and storage

```rust
let mut serializer = r2r::MessageSerializer::<MyMessage>::new()?;
let bytes = serializer.serialize(&message)?;
publisher.publish_raw(bytes)?;
```

The native message container is created once and generated copies reuse native sequence/string
capacity. Returned bytes borrow serializer storage, avoiding a fresh application `Vec<u8>`
allocation and copy. Rust prevents another serialization while that borrow remains used.
Call `to_vec()` when independent ownership is necessary; native field reuse still applies.

RMW may resize or replace serialized storage, including shrinking an initially larger buffer.
`with_capacity()` sets an initial allocation and `capacity()` reports its actual current size.
Backing addresses are not fixed. Errors expose no result, and a valid input can be retried.
No middleware deserialization state is reused. The old owned-byte method preserves its behavior
through a shared codec helper.

## Measurement design

The [codec harness](../../tools/bench_ros/README.md) uses independent historical Rust, candidate
Rust and rclcpp executables, the same benchmark source and lockfile, and complete decoded-message
plus byte-exact CDR fixture oracles. The message has one dimension labelled `samples` and 0, 32
or 4096 deterministic signed integers. Each stage/input pair has five alternating repetitions
and a common iteration count calibrated to approximately 200 ms or more. CPU 24 is fixed on an
i9-13900K; Rust 1.85 release and GCC 13.3 `-O3 -DNDEBUG` use default ISA targeting. No task-owned
build or test overlaps measurement; unrelated shared-host activity on other CPUs is a limitation.

Historical library `097c734` lacks `MessageSerializer`, so new benchmark modes use its public
`to_serialized_bytes()` and consume the same result. The builder records a cfg that selects the
new API only where it exists. rclcpp consumes reusable `SerializedMessage` bytes directly in
`serialize_reuse`; `serialize_owned_reuse` makes a fresh vector copy in both languages. C/C++
type-support implementations differ, and required representation conversions remain measured.

## Borrowed-result median time

Time is nanoseconds per operation; lower is better. Historical Rust still returns owned bytes
in this mode because its API cannot borrow output.

| RMW | Integers | Historical Rust | MessageSerializer | rclcpp reused output | Change vs historical Rust |
| --- | ---: | ---: | ---: | ---: | ---: |
| CycloneDDS | 0 | 9,531.94 | 9,830.81 | 10,185.90 | 3.1% slower |
| CycloneDDS | 32 | 9,552.58 | 9,877.82 | 10,148.70 | 3.4% slower |
| CycloneDDS | 4096 | 12,799.09 | 10,725.04 | 10,903.40 | 16.2% less time |
| Fast DDS | 0 | 1,374.35 | 829.64 | 1,382.94 | 39.6% less time |
| Fast DDS | 32 | 1,464.18 | 857.68 | 1,399.39 | 41.4% less time |
| Fast DDS | 4096 | 3,466.30 | 1,986.42 | 2,078.14 | 42.7% less time |

When copying the new borrowed output to a fresh `Vec`, Fast DDS time decreases by 34.3%, 34.5%
and 17.6% respectively. CycloneDDS small fixtures increase by 2.9–3.8%, while the large owned-byte
result improves by 1.1%. The original copy, owned serialization and owned deserialization modes
were measured too. All 15 stage/input groups in each RMW report pass the preselected 1.05 median
regression limit; the maximum observed ratio is approximately 1.046. Unrelated-path variation
does not establish a general improvement or regression.

## Evidence and verification

* [CycloneDDS raw report](reusable-serialization-2026-10-28-cyclonedds.json)
* [Fast DDS raw report](reusable-serialization-2026-10-28-fastdds.json)
* [Runnable serialized publish/typed receive example](../../r2r/examples/reusable_serialization.rs)

Each report contains 225 records, compiler/package versions, binary/source hashes, API-detection
cfgs and the gate outcome. Verify matrix completeness, timing calculations, wire fixtures,
checksums, medians and the passing performance gate with:

```sh
python3 tools/bench_ros/verify_codec.py --require-performance-pass \
  docs/benchmarks/reusable-serialization-2026-10-28-cyclonedds.json \
  docs/benchmarks/reusable-serialization-2026-10-28-fastdds.json
```

Six API tests cover full codec oracles, native storage reuse, empty/shrinking/growing input,
owned copies surviving destruction, input/allocation errors, independent buffers and cross-thread
movement. Unicode byte comparisons use aligned fixtures because CDR padding is unspecified.
Embedded-NUL tests preserve native contents and compare decoded values against the selected RMW's
existing string codec semantics. The example validates 4096 → 1024 → 0 → 4096 integers under both
RMW implementations.

Measurement is Jazzy-only. Humble/Kilted have API/native-codec gates; Jazzy has workspace, optimized
library, Clippy and doc-only checks. Raw-publish latency, services/actions, allocation counts,
pipeline throughput and receive-copy elimination remain separate work toward the broader goal.
