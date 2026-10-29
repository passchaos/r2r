# Independent ready dispatch after operational errors

This slice completes independent work from the current native readiness snapshot before
reporting its first operational error. See the [dispatch contract](../../DISPATCH_ERRORS.md)
and [retained-future example](../../../r2r/examples/dispatch_recovery.rs).

The original native fixtures failed against parent `07d936c`: a subscription error skipped a
jointly-ready timer, and a service callback error skipped a jointly-ready action request.
The original summary was `0 passed; 2 failed; 689 filtered out`. The native fixture was later
expanded with a ready service response and exact request/response sequence checks.

Final native tests use real `rcl_wait` readiness masks and invoke one dispatch. Healthy
callbacks, timer delivery, ordinary requests, client responses and action requests must be
visible without a later spin. A separate response allocator fails before its RMW take while
a healthy client's exact response is delivered. Both endpoints and the unread failed response
remain owned. Action-channel tests inject multiple failures, retain the first error, verify
exact later response values and sixteen-take limits, skip unready sources and acquire the
actual client mutex during every completion. Six cases execute under each recorded RMW.

## Native readiness boundary found during validation

The first allocation fixture put both clients on one reply topic. It passed CycloneDDS and
failed Fast DDS because readiness alone did not imply a response for the selected client.
`shared-reply-fixture.log.gz` retains that failure.

The installed `ros-jazzy-rmw-fastrtps-shared-cpp` package is
`8.4.4-1noble.20260911.095823`. Its installed client listener counts unread responses for
readiness. The matching [8.4.4 response implementation](https://github.com/ros2/rmw_fastrtps/blob/8.4.4/rmw_fastrtps_shared_cpp/src/rmw_response.cpp#L69)
takes at most one sample, checks its related writer GUID against this client and sets `taken`
only on a match. A foreign client's response can therefore produce an empty take even when
the reader was ready. The final allocator oracle uses distinct service reply topics and checks
both native slot identities before dispatch; exact response and sequence assertions remain.
The source reference and digest are recorded in `results.json`.

## Profiles and distribution checks

Jazzy workspace validation passes 687 main-library cases and 18 Rustdoc examples. Both Jazzy
RMWs pass all six optimized dispatch cases, the complete optimized 687-case library suite,
the retained-future recovery example and the optimized 32-roundtrip current-thread Tokio
workflow with node reuse. All-target warning-denied Clippy passes. Eight manual measurement
cases remain ignored in complete library runs.

Native Humble/Fast DDS and Kilted/both-RMW checks run dispatch, callback, executor and action
client/server filters, the optimized recovery example and all-target Clippy. A container with
no `/opt/ros` checks all targets and 39 public documentation artifacts. Exact nonzero filter
counts and complete compressed logs are in `results.json`. Iron, Lyrical and Rolling remain
supported CI entries without new local runtime evidence in this slice.

| Native environment | Dispatch | Executor | Callback | Action client | Action server |
| --- | ---: | ---: | ---: | ---: | ---: |
| Humble / Fast DDS | 6 | 79 | 33 | 98 | 55 |
| Kilted / CycloneDDS | 6 | 80 | 34 | 98 | 55 |
| Kilted / Fast DDS | 6 | 80 | 34 | 98 | 55 |

Each action-client filter separately ignores six manual measurements. The executed counts
above are the matching top-level test summaries, with subprocess cases included in their
parent suite rather than counted again.

## Representative transport and performance scope

Independent release binaries contain parent `07d936c`, the final candidate and rclcpp. The
native callback pub/sub workload fixes CPUs 24/25, 100 measured/10 warmup messages, one
repetition and configurations `elements:window:fanout` of `0:1:1`, `4096:16:1` and `4096:8:4`.
Both RMWs pass seven endpoint language/library combinations: 42 records and 4200 raw RTT
samples. Counts, payload/sequence/stop checks, actual receive paths and final candidate source
identities pass the existing verifier.

The performance isolation preflight did not pass. It sampled without an r2r build/test process
running and found competing Zig builds and load above the fixed 2.0 ceiling. This short matrix
is protocol evidence. It has no formal regression or Rust/rclcpp performance gate; its timing
fields cannot establish performance leadership. Calibrated interleaved A/B, the retained
saturated window/fanout boundary and wider resource/scaling evidence remain open.

`results.json` records source and log digests, the preflight observations and protocol artifact
digests. JSON protocol/build artifacts are gzip compressed; raw samples retain their original
compressed filenames. Decompress the JSON reports into one directory beside the sample files
before running the existing verifier.

## Reproduction

From the repository root:

```sh
python3 tools/bench_ros/build.py --workload pubsub_pipeline --baseline-ref 07d936c \
  --output-dir target/ready-dispatch-comparison
bash docs/validation/ready-dispatch-2026-10-30/run-ready-dispatch-protocol.bash
bash docs/validation/ready-dispatch-2026-10-30/run-ready-dispatch-final-jazzy.bash
bash docs/validation/ready-dispatch-2026-10-30/run-ready-dispatch-compat.bash
```

The scripts record native images, source/cache mounts, RMWs, domains and targeted gates. The
compatibility cache/mount paths are specific to the recorded host; equivalent native sourced
environments can run the same cargo commands directly.
