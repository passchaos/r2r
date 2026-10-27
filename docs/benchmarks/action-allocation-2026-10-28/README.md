# Action allocation, scheduling and cohort measurement evidence

Action benchmarks now record measured-window allocation calls, requested bytes, application
poll/spin counts, Rust future wakes and process CPU/context-switch counters. Independent
cohorts retain the same complete protocol and native result-retention settings, with fresh
action/ready endpoints on a shared node/context/executor. This closes a missing measurement
boundary; it does not establish general Rust/C++ superiority.

A proposed allocation-free native status iterator was tested and removed. Its short-cohort
tail gate failed. Same-binary A/A controls failed too, including after cohort-level interleaving,
so the current host/short-cohort setup cannot reliably qualify a 5% tail change. Production
keeps `b416c0f`/`d45e261` routing. Failed controls and the exact candidate patch remain archived.

## Counted boundaries

The optional [Linux/glibc probe](../../../tools/bench_ros/probe/probe.c) forwards malloc, calloc,
realloc, free, aligned_alloc, memalign and posix_memalign to the next implementation. It records
outer allocation-function calls across all process threads while the measured-window epoch
is active. Free(NULL) is separate. Requested bytes include allocation attempts, including
failed attempts; they are not live bytes, allocator capacity, copied bytes or unique objects.
Pool suballocations and unwrapped entry points such as valloc/pvalloc are outside this count.

The epoch disables new writers and drains committed writers before a snapshot. Writers delayed
across stop/reset/start cannot contaminate a later frame. Counters saturate and flag overflow;
overflowed reports fail verification. Native getrusage deltas cover user/system CPU time and
voluntary/involuntary context switches. Context switches are scheduling observations, not a
count of ROS wake events. Max RSS is the process-lifetime high water, including middleware,
libraries and harness bookkeeping; it is not isolated framework heap usage.

Rust measured futures use a counting Waker, with polling/spin-loop behavior retained. C++
std::future signaling is not instrumented, so its future-wake field is null. Both languages
record application future polls and native spin calls. Those API observations differ in shape;
they identify work rather than serving as a language-performance score.

Each frame excludes endpoint construction, discovery, warmup, controls and teardown. It
includes measured lifecycle construction, processing, validation and observer cleanup within
the loop. Middleware background work recorded during that window is included. Instrumented
latency is retained for integrity but cannot be used as performance evidence or pass a latency
regression gate. The runner rejects combining --probe with --max-regression.

Dynamic TLS access in an allocation hook initially recursed while ROS dlopened its RMW library.
The preload probe uses initial-exec TLS. Native oracles cover dynamic library/TLS loading,
content after realloc, calloc zeroing, alignment, invalid alignment, errno, allocation failure,
overflow, four-thread accounting, nested frames, reset and a deliberately delayed writer
crossing epochs. These are reproducible via:

```sh
python3 tools/bench_ros/probe/check.py --output-dir target/probe-validation
```

## Baseline counts

The library is `b416c0f`; the current harness is compiled independently for Rust and rclcpp.
Both RMWs run three repetitions of success and cancellation in all four client/server language
directions. Long runs measure 1000 goals after 50 warmups in one retained cohort. Short runs
measure 20 fresh cohorts of ten goals after five warmups each, giving 200 measured goals per
repetition and at most 15 retained goals per server instance. Both use the
[complete action protocol](../../../tools/bench_ros/ACTION.md), ten-second native retention,
reliable volatile KeepLast(16), status defaults and a 100 ms native wait cap.

The table shows median repetition counts per measured goal for same-language pairs. Counts
are allocation-function attempts; requested bytes are bytes per goal.

| RMW | Scenario | Rust client calls | C++ client calls | Rust client bytes | C++ client bytes | Rust server calls | C++ server calls |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| CycloneDDS | Success | 88.04 | 110.07 | 151004 | 123152 | 255.03 | 265.97 |
| CycloneDDS | Cancel | 103.82 | 122.09 | 201376 | 163911 | 277.03 | 273.99 |
| Fast DDS | Success | 144.62 | 157.63 | 96800 | 68708 | 121.08 | 121.27 |
| Fast DDS | Cancel | 166.31 | 173.61 | 122482 | 84837 | 146.38 | 130.62 |

The Rust client's roughly 28 KB/37 KB extra requested bytes per long-cohort success/cancel goal
appear under both RMWs. Native status decoding still collects every retained UUID/status into
an update Vec; that was selected as the next allocation target. This is a source-backed
hypothesis, not allocation-stack attribution. Short-cohort requested-byte gaps are much smaller.
The [normalized summary](summary.json) retains long/short, client/server, CPU, scheduling,
application and RSS observations; raw reports retain both mixed directions and every frame.

* [CycloneDDS long counts](counts-long-cyclonedds.json) and [samples](counts-long-cyclonedds.samples.json.gz)
* [Fast DDS long counts](counts-long-fastdds.json) and [samples](counts-long-fastdds.samples.json.gz)
* [CycloneDDS short counts](counts-short-cyclonedds.json) and [samples](counts-short-cyclonedds.samples.json.gz)
* [Fast DDS short counts](counts-short-fastdds.json) and [samples](counts-short-fastdds.samples.json.gz)

Together these contain 96 endpoint-pair records and 57,600 RTT samples, plus full stage samples,
allocation/resource frames, executable/probe/source hashes and package/compiler versions.

## Rejected native iterator and measurement controls

The candidate validates native sequence bounds and alignment before routing and borrows the
initialized status records. Typed/untyped routing shares one helper; public goal tracking,
live results, feedback-only observation and detached notification semantics remain intact.
It passed 14 optimized status cases, including native-path observer lifetime and invalid
storage boundaries. The [stage report](iteration-stage.json) consumed every UUID/status with
an exact checksum and retained five repetitions:

| Statuses | Update Vec and consumption | Native iteration and consumption | Less time |
| ---: | ---: | ---: | ---: |
| 1 | 18.37 ns | 4.77 ns | 74.0% |
| 128 | 275.42 ns | 203.70 ns | 26.0% |
| 1024 | 1972.90 ns | 1557.79 ns | 21.0% |

The candidate [blockwise short A/B](rejected-blockwise-ab.json) failed the 1.05 median gate;
success/cancellation p99 ratios were 1.544/2.237. Each repetition contains 100 fresh cohorts
of ten measured/five warmup goals, giving 1000 RTT samples while retaining the original
15-goal short boundary. Baseline and candidate ran in long separate blocks, reversing order
between five repetitions. Its [samples](rejected-blockwise-ab.samples.json.gz) and
[runtime patch](rejected-native-iteration.patch.gz) are preserved.

The subsequent [blockwise A/A](blockwise-aa.json) ran the exact baseline executable under both
Rust labels. It also failed: success p99 ratio 1.058. Its [samples](blockwise-aa.samples.json.gz)
show that identical code can cross the 5% limit on this setup. An alternative
[cohort-interleaved A/A](interleaved-aa.json) kept both endpoint pairs resident, waited for all
nodes to initialize, alternated execution after every cohort, destroyed inactive action/ready
endpoints and delayed reporting until every measured window ended. The pairs used separate
DDS domains, and their executable hashes are identical. That control still failed with success
p99 ratio 1.062; its [samples](interleaved-aa.samples.json.gz) remain available.

These A/A failures show insufficient measurement precision; they do not prove that all of the
candidate's tail loss is noise. The candidate was not promoted, and its three runtime source
files were restored byte for byte. More effective isolation, settled middleware conditions or
greater statistical power are needed before a future 5% short-tail claim can be accepted.

## New cohort and integrity contracts

The Rust/C++ harness accepts an optional cohort count. Every cohort uses a distinct endpoint
name and a global goal-sequence offset. Each ends only after observed feedback and all terminal
results, followed by two rejected control goals. Multi-cohort clients observe both rejections.
Native default retention remains ten seconds; finalizing a completed cohort's endpoints clears
its server storage. Only one cohort's endpoints are active during an interleaved window.

Raw RTT/stage arrays are concatenated before percentiles. Completion rate uses the sum of the
measured windows. Per-cohort checksums, counts, sequence offsets and elapsed time are retained;
aggregate counts and total checksum must match. Whole cohort-loop wall time is reported
separately and includes setup/warmup/teardown and any control waiting. This distinction prevents
the effective measured-window rate from being confused with complete endpoint-churn throughput.

--action-stability is an explicit Rust baseline/candidate boundary gate. It requires at least
1000 measured samples, five repetitions and at most 15 goals per cohort. Its two-pair inventory
cannot masquerade as full interoperability: the verifier recognizes its scope and validates the
exact inventory. The default full matrix retains all four or seven language pairs. The current
cohort-stability setup remains unqualified because the same-binary control fails.

## Environment and verification

The host is a Core i9-13900K; client and middleware threads use CPU 24, server and middleware
threads CPU 25. ROS 2 Jazzy, Rust 1.85 release, GCC 13.3 -O3 -DNDEBUG and default ISA targeting
match. Count runs use domains 180–183. The rejected A/B uses 186, blockwise A/A 190 and
interleaved A/A 191/192. No task-owned compilation/tests overlap measurement. Other shared-host
jobs have unrestricted affinity and remain a limitation, particularly for tail precision.

Verify raw data, frames, matrix inventory, counts, percentiles, rates, sample digests and recorded
gate outcomes with:

```sh
python3 tools/bench_ros/verify_transport.py \
  docs/benchmarks/action-allocation-2026-10-28/counts-long-cyclonedds.json \
  docs/benchmarks/action-allocation-2026-10-28/counts-long-fastdds.json \
  docs/benchmarks/action-allocation-2026-10-28/counts-short-cyclonedds.json \
  docs/benchmarks/action-allocation-2026-10-28/counts-short-fastdds.json \
  docs/benchmarks/action-allocation-2026-10-28/rejected-blockwise-ab.json \
  docs/benchmarks/action-allocation-2026-10-28/blockwise-aa.json \
  docs/benchmarks/action-allocation-2026-10-28/interleaved-aa.json
```

All three tail-control reports must fail --require-performance-pass. Count reports have no
latency performance gate. Probe/driver oracles and runner checks are independent of application
latency qualification; larger feedback, concurrent goals, allocation-stack attribution and
multithread scaling remain separate work toward the broader goal.

Restoration checks confirm that all three action-client runtime files match `b416c0f` byte for
byte. The complete optimized Jazzy library passes 409 tests with two manual benchmarks ignored;
all-target Clippy and doc-only compilation pass. Native Humble/Kilted build the new Rust/C++
harness and preload probe and each pass all four language combinations for success/cancellation
with two fresh cohorts, in both plain and counted modes. Those container runs are compatibility
checks rather than cross-distribution performance evidence. The runner/controller suite passes
32 cases; native probe oracles pass for normal and delayed-writer builds.
