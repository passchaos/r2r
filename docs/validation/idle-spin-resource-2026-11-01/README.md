# Idle-spin allocation, CPU and RSS scaling (2026-11-01)

The entity-scaling matrix previously measured wall-clock spin cost without process-wide resource
probes. This stage uses the LD_PRELOAD allocation probe to measure 10,000 zero-timeout idle spins
for 0/1/8/32/128 subscriptions and for mixed 8×/32× subscription/service/client/timer sets on one
node. Each case is repeated five times.

| Case | CycloneDDS allocations | CycloneDDS user CPU | Fast DDS allocations | Fast DDS user CPU |
| --- | ---: | ---: | ---: | ---: |
| 0 entities | 0 | 4.0 ms | 0 | 4.1 ms |
| 1 subscription | 0 | 16.0 ms | 10,000 | 21.4 ms |
| 8 subscriptions | 0 | 18.7 ms | 40,000 | 32.5 ms |
| 32 subscriptions | 0 | 25.9 ms | 60,000 | 60.3 ms |
| 128 subscriptions | 0 | 28.2 ms | 80,000 | 232.1 ms |
| mixed 8×4 | 0 | 22.1 ms | 60,000 | 78.0 ms |
| mixed 32×4 | 0 | 40.6 ms | 80,000 | 225.6 ms |

CycloneDDS performs no heap work during idle waits, so the process-wide probe observes exactly
zero allocations, frees and requested bytes across every entity count. Fast DDS allocates inside
`rmw_wait`; the recorded cost is middleware-internal and scales with the number of entities (up to
eight allocations per spin at 128 subscriptions), which r2r cannot remove without bypassing the
RMW. Peak RSS stays constant at about 51.5 MiB for every case in both runs, so r2r's wait-set and
registry reuse do not grow process memory with entity count.

CPU values are diagnostic rather than timing-qualified: the host was contended. Allocation counts
and RSS are deterministic. The gate requires zero CycloneDDS allocations, bounded Fast DDS
allocations, bounded/stable RSS and complete case coverage.

```sh
bash docs/validation/idle-spin-resource-2026-11-01/run-jazzy.bash
bash docs/validation/idle-spin-resource-2026-11-01/run-no-ros.bash
python3 docs/validation/idle-spin-resource-2026-11-01/verify.py
```

All compilation used one pinned CPU, one Cargo job and a 10 GiB memory ceiling.
