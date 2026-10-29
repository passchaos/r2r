# Native action-feedback identity routing

Generated action support now reads `goal_id.uuid` from the initialized native
feedback message before constructing the Rust feedback payload. Built-in typed
and untyped clients use that identity to choose one of three paths after the
client lock is released:

- feedback for an unobserved goal drops the native message without decoding it;
- feedback for an observed goal decodes once and enters the typed channel
  directly, without an intermediate `Box<dyn Any>`;
- custom action support whose new accessor returns the default `None` retains
  complete decoding and the existing erased routing behavior.

The accessor is part of the existing unsafe action-support contract: a supplied
identity must agree with the subsequently destructured goal ID and may inspect
only initialized native identity fields. A disagreement is rejected with
`RCL_RET_ACTION_CLIENT_INVALID`. Closed feedback channels are detected before
decoding; closure during decoding cannot restore a sender. This keeps native
layout knowledge in generated bindings and leaves a replaceable private boundary
for future retained native storage or borrowed payload views.

## Correctness and conversion oracle

Generated typed and untyped accessors match complete conversion for feedback
vectors of length 0, 1, 32 and 65536 and three UUID patterns. A real untyped RCL
test publishes eight feedback messages containing 4096 elements for an
unobserved goal, requires all eight native messages to be taken, and makes the
decoder panic if invoked. The same test retains a second observed goal and
requires its feedback to arrive. Replacing the accessor with `None` requires
exactly eight legacy decodes instead. Nearby completion tests cover single
decode/delivery, pre-decode and during-decode closure, destruction outside the
client lock and UUID disagreement.

The final Jazzy feedback filter executes 55 cases under each RMW with three
manual measurements ignored. The raw logs are retained under [logs](logs/).

## Fixed-CPU conversion/allocation stage

The [stage report](stage/report.json) contains twenty records: five repetitions
of observed/unobserved legacy and direct completion, 100000 messages per record,
pinned to CPU 24. The Linux/glibc probe and current optimized test binary are
identified by SHA-256. Every direct record removes exactly one allocation per
message relative to erased routing. Unobserved direct feedback performs zero
decodes.

| Observer | Path | Allocations/message | Decodes/100000 | Median ns/message |
| --- | --- | ---: | ---: | ---: |
| absent | erased legacy | 3 | 100000 | 199.969 |
| absent | native identity | 2 | 0 | 131.826 |
| present | erased legacy | 6 | 100000 | 475.643 |
| present | native identity | 5 | 100000 | 406.343 |

Absolute allocation and timing counts include this empty-Fibonacci-payload test
fixture and completion/channel machinery. The stable one-allocation delta and
decode counters are the asserted result; this stage excludes RCL take, DDS,
large-payload conversion, application concurrency and transport tails.

## Ordinary action regression gate

Independent optimized candidate and `e73254f` baseline binaries use identical
confirmed-feedback action inputs, native QoS, result retention and harness
sources. Ping and echo run as separate processes on CPUs 24/25. Both success and
cancellation execute 1000 measured goals after 50 warmups for five alternating
repetitions. The unchanged same-language maximum ratio is 1.05 for median
p50/p95/p99 and inverse throughput.

| RMW | Scenario | p50 ratio | p95 ratio | p99 ratio | Inverse throughput |
| --- | --- | ---: | ---: | ---: | ---: |
| CycloneDDS | Success | 0.988435 | 0.997592 | 0.999809 | 0.994041 |
| CycloneDDS | Cancellation | 1.001056 | 0.997188 | 1.003773 | 1.000139 |
| Fast DDS | Success | 1.000807 | 1.006958 | 1.021861 | 1.000289 |
| Fast DDS | Cancellation | 0.996492 | 0.986999 | 1.002200 | 0.999634 |

Both [CycloneDDS](action/cyclonedds.json) and [Fast DDS](action/fastdds.json)
pass, each with 70 records and 70000 individual RTT samples. Their compressed
sample artifacts and the [source/build manifest](action/build-manifest.json) are
saved beside the reports. All 78 candidate source hashes match the delivered
source. Candidate, baseline and C++ binary hashes are recorded in each report.

The four mixed-direction p99 ratios relative to matching baseline-language
pairs range from 0.990567 to 1.009499 in these final runs; their largest metric
ratio is 1.026087. They are outside the preselected same-language gate and no
new same-binary A/A calibration was run for this slice. The action workload
always observes one small feedback item, so it protects the normal path from a
regression but does not measure the unobserved large-payload saving. These data
do not establish universal tail improvement or overall rclcpp performance
leadership.

## Validation and retained failures

The final native Jazzy workspace gate passes. Its main r2r library result is 586
passed and six ignored; the remaining workspace integration, package and doc
tests also pass. Complete optimized r2r library suites report the same 586/6
under CycloneDDS and Fast DDS. Formatting and warning-denied all-target Clippy
pass.

Humble/Fast DDS passes 55 feedback cases with three manual measurements ignored
and all-target Clippy. A newly built Kilted image from the archived
[Dockerfile](compat/Dockerfile.kilted) explicitly contains both RMW packages;
CycloneDDS and Fast DDS each pass the same 55/3 boundary, followed by all-target
Clippy. The no-ROS image uses a fresh target directory, passes four
snapshot transaction tests, compiles every workspace target, passes six common
and two generator tests, builds Rustdoc and verifies fifteen public API pages
and twenty-nine methods.

Four unsuccessful validation attempts remain under [logs/rejected](logs/rejected/):

- initial Humble and Kilted Clippy loaded the shared allocation probe module
  twice; the final test-only module is defined once and reused;
- `cargo test --workspace --all-targets` executed a harness-free benchmark
  without its required runtime arguments after all library/integration tests
  passed; the valid test gate is `cargo test --workspace`, while all-target
  compilation is covered by Clippy;
- the first Humble rerun used an old mixed-ownership Cargo target and failed
  before compilation could write a fingerprint; the final run uses a new
  user-owned target directory.

[validation.json](validation.json) records final results, image/source/binary
identities and SHA-256 for every archived artifact. Iron, Lyrical and Rolling
have no new local runtime evidence for this slice.

## Reproduction and remaining boundary

```sh
python3 tools/bench_ros/build.py --workload action --baseline-ref e73254f \
  --output-dir target/native-feedback-ab
python3 tools/bench_ros/verify_transport.py --require-performance-pass \
  docs/benchmarks/native-feedback-identity-2026-10-29/action/cyclonedds.json \
  docs/benchmarks/native-feedback-identity-2026-10-29/action/fastdds.json
```

The generated accessor removes payload construction only when no feedback
observer exists. Observed feedback still constructs its owned Rust payload, and
each RCL take still creates native message storage. Retained/reused native
feedback buffers, borrowed delivery, larger and bursty payloads, concurrent
multi-goal fanout, networked hosts and calibrated complete Rust/rclcpp scaling
remain work toward the broader usability, capability and performance goal.
