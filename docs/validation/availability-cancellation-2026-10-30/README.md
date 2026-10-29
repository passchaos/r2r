# Availability cancellation validation

This slice fixes native progress and endpoint lifetime after availability/matching cancellation.
The [contract](../../AVAILABILITY_CANCELLATION.md) describes bounded cleanup, current-guard
binding and ACK lifetime behavior. Rust 1.85.0 is used throughout.

`results.json` retains the changed Rust source digests, deleted module path, parent commit,
compressed-log digests, exact test summaries and raw preparation-stage samples. The native
binding helper moved from `background_notifications` to `observer_notifications`; its nearby
tests cover foreground/shared use as well as background binding.

## Reproductions

The two original background tests failed against the pre-fix runtime with two-second deadlines:
512 canceled availability observers stopped clearing while one healthy observer remained, and
an already-dropped native client remained registered after cancellation. The original focused
summary was `0 passed; 2 failed; 678 filtered out`.

A separate foreground Node regression failed because canceling an availability future during
the native wait did not trigger the guard. That original test was subsequently moved into the
nearby binding test module and expanded to Node/shared executors, factories before/after
binding, node transfer and every individual cancellation source. The archived foreground
baseline reflects the initial single regression, not the expanded final test inventory.

`background-baseline.log.gz` and `foreground-baseline.log.gz` retain those failures. The
foreground reproduction ran with the bounded-backlog fix already present and the old native
wake bindings still in place. Its production binding behavior was unchanged from the parent
commit; it is not a clean full-parent source snapshot.

## Validation boundaries

Jazzy workspace validation includes 681 main-library cases and 18 Rustdoc examples. Both
Jazzy RMWs pass the complete optimized 681-case library suite, the 32-roundtrip current-thread
Tokio workflow with node reuse, and warning-denied all-target Clippy. Eight manual measurement
tests are intentionally ignored in each complete library run.

Distribution checks use native Humble/Fast DDS and Kilted/CycloneDDS plus Fast DDS containers.
Their availability, binding, executor and callback-service filters execute nonzero relevant
cases, followed by the optimized Tokio workflow and warning-denied all-target Clippy. A
container with no `/opt/ros` checks all targets, saved bindings and public documentation.
The exact per-filter counts and summaries are retained in `results.json`. Iron, Lyrical and
Rolling remain supported CI entries without new local runtime evidence in this slice.

| Native environment | Availability | Binding | Executor | Callback services |
| --- | ---: | ---: | ---: | ---: |
| Humble / Fast DDS | 25 | 4 | 79 | 11 |
| Kilted / CycloneDDS | 25 | 4 | 80 | 12 |
| Kilted / Fast DDS | 25 | 4 | 80 | 12 |

`ack-lifetime.log.gz` records the focused lifetime contract after the old next-spin destruction
assertion was replaced with same-pass completed-result delivery and pending-ACK retention
checks. The completed ACK future remains readable after its native publisher is destroyed.

The preparation stage runs 32 parked action servers with 200 warmup and 2000 measured passes
under each Jazzy RMW. Native maintenance poll counts remain zero. CPU affinity is unrestricted
and other host compilation was active: the raw samples check this preparation path and do not
establish a throughput/latency improvement or pass a formal performance gate. Independent,
CPU-pinned, interleaved A/B and representative Rust/C++ resource/scaling evidence remain open.

## Reproduction

From the repository root on the recorded native host:

```sh
bash docs/validation/availability-cancellation-2026-10-30/run-availability-final-jazzy.bash
bash docs/validation/availability-cancellation-2026-10-30/run-availability-compat.bash
```

The compatibility script records its native images, source/cache mounts, target directories,
domain and middleware choices. Equivalent sourced environments can run the same cargo gates
directly. The fixed container/cache paths are specific to the recorded host.
