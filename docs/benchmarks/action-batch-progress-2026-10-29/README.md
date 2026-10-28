# Action batch progress prototype: not promoted

The prototype was removed from production after its final Fast DDS action
comparison failed the preselected 5% p99 regression limit. The production
runtime remains at `df69693`. The correctness gaps below remain open; the
prototype and native counterexamples are retained for the next implementation.

## Reproduced progress gaps and prototype behavior

A 40-entry canceled-result queue failed the continuation oracle: one bounded
pass removed sixteen entries, left twenty-four queued and did not schedule
another pass. Pending feedback still registered a noop sender waker, so
capacity recovery could also lose native progress.

The prototype introduced finite readiness epochs for feedback, cancel request
delivery and nested cancel decisions. It kept unexamined work scheduled across
bounded batches, stopped self-notifications after one complete blocked scan,
and resumed on channel/decision readiness. Cached readiness wakers were disabled
when their node was transferred. Large synchronous cancel requests enabled a
separate scan path; ordinary small requests retained direct scanning.

Correctness evidence included:

* Forty ready decisions finishing in three bounded passes.
* Forty requests with twenty unresolved decisions each finishing a finite sweep,
  parking, then preserving all identities and decisions on readiness.
* Forty backpressured feedback channels parking after a bounded sweep and
  delivering the ready tail after capacity recovery.
* No feedback queue lock during readiness notification, and stale-waker disabling.
* Typed/untyped 40-goal `cancel_all`, exact UUID sets and all canceled result
  payloads under native Jazzy CycloneDDS/Fast DDS, Humble and Kilted.
* Same-thread cancellation and observer cleanup across batch boundaries.

The result-service QoS explicitly retained 64 entries for forty concurrent
results. A first CycloneDDS test with the smaller default queue timed out during
the result burst; increasing the explicit workload queue capacity retained the
full result oracle. That fixture change does not establish a default QoS policy.

`candidate.patch.gz` contains the final runtime prototype, nearby tests and the
complete `action_cancel_many` example against `df69693`. Its uncompressed patch
hash and exact source hashes are in `candidate-source.json`. Native test logs
are compressed alongside the reports. A successful test is not a substitute
for the required performance gate.

## Final fixed-parameter comparison

The safe `df69693` baseline and final candidate were independent optimized
binaries. CPUs 24/25, DDS domain 122, QoS, input and success/cancellation protocol
were fixed. Seven endpoint combinations include both baseline/C++ and candidate/
C++ directions as well as baseline/baseline, candidate/candidate and C++/C++.
Five interleaved repetitions used 1000 measured goals and 50 warmup goals per
scenario. Each final report retains 70 records and 70000 RTT samples.

| RMW | Scenario | p50 ratio | p95 ratio | p99 ratio | Inverse throughput | Gate |
| --- | --- | --- | --- | --- | --- | --- |
| CycloneDDS | Success | 0.9953 | 0.9900 | 0.9926 | 1.0012 | Pass |
| CycloneDDS | Cancellation | 0.9980 | 0.9851 | 0.9360 | 0.9900 | Pass |
| Fast DDS | Success | 0.9957 | 1.0076 | 1.0898 | 0.9996 | Fail |
| Fast DDS | Cancellation | 0.9894 | 1.0005 | 1.0593 | 0.9974 | Fail |

Every report passes protocol/integrity verification; Fast DDS does not pass
`--require-performance-pass`. No performance gain or regression pass is claimed
for the removed prototype.

## Earlier runs and calibration

`first-candidate/` preserves the earlier implementation's reports. Its final
long-cohort CycloneDDS cancellation p99 failed by 7.4%. Same-binary A/A then
failed symmetric cancellation p99 by 40.7%. A calibration using twenty fresh
cohorts of 100 measured/20 warmup goals passed a 5% symmetric A/A check, but the
subsequent A/B failed cancellation p99 by 13.4%. That calibration pass therefore
did not reliably establish subsequent tail precision.

After separating small-request scanning from the new scan-state path, the final
fixed-parameter comparison passed CycloneDDS but failed Fast DDS as recorded
above. All failures and anomalous samples remain in the archives. The threshold
was not relaxed, and no earlier passing result was used to approve final code.

Next work needs a lower-overhead continuation/ready-queue design and matched
tail calibration before promotion. In particular, neither unconditional
self-waking nor increasing the batch limit closes the bounded-work/liveness
contract without weakening fairness or CPU usage. Native multigoal throughput,
allocation accounting and wider RMW/hardware/distribution evidence remain open.

```sh
python3 tools/bench_ros/verify_transport.py docs/benchmarks/action-batch-progress-2026-10-29/isolated-cyclonedds.json
python3 tools/bench_ros/verify_transport.py docs/benchmarks/action-batch-progress-2026-10-29/isolated-fastdds.json
```

The supplemental source identity inventories the new scan module explicitly;
the original build manifest remains unchanged. Archived tests added after
benchmark compilation do not change the runtime being measured.
