# Endpoint construction and destruction resource scaling (2026-11-01)

The entity-scaling matrix previously measured idle spin and listed endpoint construction/destruction
probes as remaining work. This stage creates 1/8/32/128 subscriptions, 1/8/32 services, 1/8/32
clients, 1/8/32 timers and mixed 8×/32× sets on a fresh node, then drops them and spins cleanup.
The LD_PRELOAD probe records process-wide allocations, frees, requested bytes, CPU and peak RSS for
each phase; each case is repeated five times.

Construction allocations (median, CycloneDDS / Fast DDS):

| Case | Entities | CycloneDDS allocations | Fast DDS allocations | Requested bytes (Cyclone / Fast) |
| --- | ---: | ---: | ---: | ---: |
| subscriptions | 1 | 177 | 692 | 22,992 / 141,100 |
| subscriptions | 8 | 928 | 4,461 | 107,805 / 686,058 |
| subscriptions | 32 | 3,609 | 17,963 | 445,271 / 2,809,376 |
| subscriptions | 128 | 17,346 | 71,950 | 2,227,121 / 11,933,949 |
| services | 32 | 8,564 | 24,204 | 1,133,804 / 4,301,357 |
| clients | 32 | 8,468 | 24,172 | 1,127,212 / 4,275,065 |
| mixed 8×4 | 32 | 5,357 | 16,929 | 681,047 / 2,807,455 |
| mixed 32×4 | 128 | 24,912 | 67,062 | 3,314,969 / 12,709,537 |

Destruction allocates and frees a smaller amount while the dropped endpoints are finalized across
bounded spins. Peak RSS stays constant at about 51.5 MiB for every case and both RMWs, so creating
up to 128 endpoints does not grow process memory beyond the endpoint storage itself. Fast DDS
performs roughly three to four times more construction allocations than CycloneDDS, and the
single-entity cases include participant/discovery startup noise. CPU values are diagnostic only.

The gate requires complete case/phase coverage, bounded per-endpoint allocations, bounded and stable
RSS, and five repetitions.

```sh
bash docs/validation/endpoint-construction-resource-2026-11-01/run-jazzy.bash
bash docs/validation/endpoint-construction-resource-2026-11-01/run-no-ros.bash
python3 docs/validation/endpoint-construction-resource-2026-11-01/verify.py
```

All compilation used one pinned CPU, one Cargo job and a 10 GiB memory ceiling.
