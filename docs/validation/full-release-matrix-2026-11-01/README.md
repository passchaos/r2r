# Cross-release full library suite (2026-11-01)

The complete `r2r` library test suite ran in release mode for every required native environment
at commit `d0a8325`:

| Distribution | RMW | Passed | Ignored | Failed |
| --- | --- | ---: | ---: | ---: |
| Jazzy | `rmw_cyclonedds_cpp` | 828 | 24 | 0 |
| Jazzy | `rmw_fastrtps_cpp` | 828 | 24 | 0 |
| Kilted | `rmw_cyclonedds_cpp` | 834 | 24 | 0 |
| Kilted | `rmw_fastrtps_cpp` | 834 | 24 | 0 |
| Humble | `rmw_fastrtps_cpp` | 804 | 24 | 0 |

Jazzy ran natively; Humble and Kilted ran in their release-validation containers with one pinned
CPU, one Cargo job, one test thread and a 10 GiB memory ceiling. The 24 ignored cases are manual
allocation-probe stages. Humble's lower count is the expected exclusion of Iron-and-newer
service-event and introspection tests; Kilted's higher count is its newer binding surface.

The logs are archived under `logs/` with digests in `results.json`. This record is a per-commit
snapshot: it is not re-run against later source changes and does not pin current sources.

```sh
python3 docs/validation/full-release-matrix-2026-11-01/verify.py
```
