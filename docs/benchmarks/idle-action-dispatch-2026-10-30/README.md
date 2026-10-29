# Idle action dispatch candidate

The current runtime performs an extra empty completion operation for every action client and
server after sampling readiness. A client with all five flags false still clones its Arc and
reacquires its mutex. A server without an expired goal still enters the completion helper.
These costs grow with registered endpoints even when an unrelated event triggered the wait.

The candidate skips the client helper before any Arc clone or lock when its readiness snapshot
is empty, and invokes expiration completion only when expiration is ready. Native readiness
sampling, each active channel's sixteen-take budget, post-lock delivery and queued cancellation
maintenance remain on their existing paths. No per-node scheduling state is added.

The candidate is **not promoted**. Both performance isolation preflights failed, so timed
same-binary calibration, independent qualified A/B and the ordinary action performance gate
were not run. The primary runtime was restored to parent `5ea838b`. Candidate source and tools
remain in the isolated `target/idle-action-prototype` worktree and the archived patch.

## Correctness evidence

The first locked-endpoint regression failed after a started worker attempted the empty client
helper and waited for a mutex held by unrelated work. The test releases and joins the worker
before reporting a failure. A real native guard-only readiness fixture also failed with one
extra client and server completion call per endpoint.

The candidate passes three dedicated cases under both Jazzy RMWs: empty snapshot lock avoidance,
every individually-ready client channel's exact delivery, and actual idle snapshots with zero,
one and four typed/untyped endpoint pairs. All six ready-dispatch/error/budget/completion cases
and three existing cancellation-maintenance scheduling cases pass in each RMW. Module/test
Clippy passes. The helper call counters exist only in native test builds and are excluded from
production and doc-only binaries.

The stage uses actual RCL registration and one guard-only native wait. It verifies that every
action data/expiration flag is false before measuring repeated dispatch of that stable snapshot.
It includes native readiness queries; it excludes registration, discovery, wait latency and
application traffic. TLS completion counters are present identically in the stage binaries.
Their overhead is part of this instrumented stage, so the stage cannot establish an uninstrumented
application latency improvement.

## Independent stage records

The baseline is `5ea838b`, adapted with the same nearby test harness and test-only counters.
The optimization is not copied into the baseline. Independent release test binaries use CPU 24,
0/1/8/32/64 endpoint pairs, 200 warmup passes, 2000 measured passes and three alternating
baseline/candidate repetitions. Endpoint pairs alternate typed and runtime-typed APIs.

Each Jazzy RMW retains 30 records and 60000 raw dispatch samples. At 64 pairs, each baseline
record enters the empty client and server helpers 128000 times apiece; the candidate enters
neither. The 60 records and 120000 total samples pass native identity, counts, affinities,
percentiles, summary and final candidate input verification.

Timing fields remain **unqualified stage data**. Reports have `scope=protocol_only`, no formal
performance gate and no passing isolation claim. They must not be used as a Rust/C++ speed
comparison or an application regression pass.

The first isolation attempt found competing Zig compilation and load above the fixed 2.0
ceiling. The later attempt followed a verified external-state change: compilers had stopped.
The longer preflight still did not obtain three consecutive idle samples below the ceiling.
Both complete observation records are archived. The threshold was not relaxed.

Eight verifier tests reject missing/duplicate frames, incorrect helper counts, wrong native
affinity, source/timing-summary drift, falsely promoted same-binary data, failed/contended
isolation and missing or mismatched calibration. A formal A/B must reference a verified passing
same-binary calibration with identical candidate identity and measured configuration.

## Resume from the exact candidate

The current isolated worktree matches all 168 compiled candidate input identities. To recreate
it from the repository root, choose a fresh worktree path:

```sh
git worktree add --detach target/idle-action-replay 5ea838b
gzip -dc docs/benchmarks/idle-action-dispatch-2026-10-30/candidate.patch.gz | \
  git -C target/idle-action-replay apply
cp Cargo.lock target/idle-action-replay/Cargo.lock
```

`Cargo.lock` is ignored in this checkout and is copied explicitly, as it was for both builds.
The exact lockfile is also retained as `Cargo.lock.gz` in this archive.
From that recreated worktree, source Jazzy and use the included runner:

```sh
python3 tools/bench_ros/idle_dispatch.py build --baseline-ref 5ea838b \
  --output-dir target/idle-action-stage
python3 tools/bench_ros/idle_dispatch.py run --binaries target/idle-action-stage \
  --cpu 24 --endpoints 0,1,8,32,64 --iterations 20000 --warmup 1000 --repetitions 7 \
  --rmw rmw_cyclonedds_cpp --calibration \
  --output target/idle-action-stage/calibration.json
python3 tools/bench_ros/idle_dispatch.py run --binaries target/idle-action-stage \
  --cpu 24 --endpoints 0,1,8,32,64 --iterations 20000 --warmup 1000 --repetitions 7 \
  --rmw rmw_cyclonedds_cpp --performance \
  --calibration-report target/idle-action-stage/calibration.json \
  --output target/idle-action-stage/performance.json
```

Repeat for Fast DDS. After qualified stage results, build uninstrumented parent/candidate
action application binaries and retain calibrated success/cancellation performance evidence,
then run the affected compatibility and full library gates before promotion. The current
candidate has not completed those later gates.

`evidence.json` binds the candidate patch, source inputs and compressed logs/reports/samples.
Decompress JSON reports beside their original sample filenames to rerun the included verifier.
