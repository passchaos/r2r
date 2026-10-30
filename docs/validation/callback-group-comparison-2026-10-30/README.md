# Callback group application comparison evidence

The [workload](../../../tools/bench_ros/CALLBACK_GROUPS.md) compares one Node and
one native service in independent Rust/rclcpp application binaries. It includes
explicit same-handler overlap before warmup, exactly-once input acceptance,
client/server native identity matching, independently checked computation results,
request/handler interval bounds, and complete-process resource measurements.

The smoke matrix contains five cases: exclusive 1 worker/window 1/empty body;
reentrant 1 worker/window 4/empty body; exclusive budget 4/window 8/16384 operations;
reentrant 2 workers/window 4/empty body; and reentrant 4 workers/window 8/16384
operations. Each uses 64 measured plus 16 warmup requests, with two interleaved
repetitions of both languages. Twenty application processes produce 1280 checked
measured request traces per environment. Timing in this protocol matrix is
unqualified for performance promotion.

An initial C++ four-worker run timed out waiting for a response. The captured failure output is retained; that early version did not persist
partial numeric records before the exception. Future process failures retain
partial records and offending stdout/stderr. Diagnostic state was added for outstanding request IDs,
server take/finish and executor exceptions. Twenty larger runs (5120 traces) and
200 short-lived runs (12800 traces) did not reproduce it. Its cause remains
unknown; those passes do not erase the failure or prove wider rclcpp reliability.

Resource calibration found that some ru_maxrss values shared a pre-exec process
peak. The accepted harness instead samples application VmRSS/VmHWM before
teardown and output allocation, retaining raw ru_maxrss for context. Server
atomic telemetry orders were also matched before final measurements. Build input
changes are rejected before accepting executable/source provenance.

Dynamic ELF dependency inspection of the current Jazzy Rust application found
86 direct needed libraries, including 74 interface libraries. The rclcpp binary
had 9 direct dependencies and one interface library. The Rust binary includes
message packages unused by this typed service workload. This identifies default
runtime linking as a concrete memory/startup audit target; dependency counts
alone do not quantify its memory or timing cost.

Reproduce protocol/compatibility checks, then run performance independently:

```sh
bash docs/validation/callback-group-comparison-2026-10-30/run-jazzy.bash
bash docs/validation/callback-group-comparison-2026-10-30/run-compat.bash
bash docs/validation/callback-group-comparison-2026-10-30/run-performance.bash
```

| Native environment | Applications | Measured request traces | Benchmark Clippy |
| --- | ---: | ---: | --- |
| Jazzy / CycloneDDS | 20 | 1280 | Pass |
| Jazzy / Fast DDS | 20 | 1280 | Pass |
| Humble / Fast DDS | 20 | 1280 | Pass |
| Kilted / CycloneDDS | 20 | 1280 | Pass |
| Kilted / Fast DDS | 20 | 1280 | Pass |

All 94 performance-tool unit tests pass, including 18 callback/build-specific cases and protocol, resource, isolation
and source-integrity counterexamples. The probe passes both normal and epoch/dynamic-TLS oracles.
Jazzy all-target workspace Clippy passes with warnings denied. The true no-ROS
all-target/documentation gate includes the new benchmark and verifier cases,
and verifies 59 public objects and 257 methods. The library runtime was not
changed by this slice; runtime/full-suite acceptance for the reentrant feature
remains in the [preceding validation](../reentrant-groups-2026-10-30/README.md).

The corrected four-worker diagnostic has 200 process runs/12800 request traces;
a separate larger matrix has 20 runs/5120 traces. The original C++ timeout cause
is still unproven. Protocol passes establish these checked runs, rather than
universal baseline reliability.

The final performance preflight failed before measurement. 5 samples reported
load1 4.48–5.73, with two to five external Zig processes. No Rust/C++
performance gate ran, and thresholds remain unchanged. Complete observations
are in [performance-isolation-failure.json](performance-isolation-failure.json).

Current smoke diagnostics show fewer requested allocations for Rust, while its
application VmHWM remains higher in the matched four-worker cases. These short,
externally contended resource/timing records are retained without performance
promotion. They direct the next investigation toward unnecessary default
interface linking, with [dynamic-link-audit.json](dynamic-link-audit.json) as
concrete evidence. Full default typed/untyped capability must survive any fix.

[results.json](results.json) records exact counts and artifact hashes;
[source-manifest.json](source-manifest.json) records build/run inputs. Protocol
reports retain compressed individual traces and executable/probe, compiler,
package and CPU provenance. Iron, Lyrical, Rolling, wider payloads, other ROS
entities and networked-host application comparisons remain open.
