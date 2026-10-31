# Lazy native type-support loading

r2r now resolves generated ROS interface operations when an interface is first
used instead of adding every generated package to the executable's direct ELF
dependencies. Loaded libraries and operation pointers remain process-owned, so
type-support metadata and matching message finalizers cannot outlive their
library.

This closes the concrete dependency and resident-memory gap found by the
[callback-group application comparison](../callback-group-comparison-2026-10-30/README.md).
It does not remove typed or runtime-typed interfaces from the generated Rust API.
Publisher, subscription, client, service, action, serialization, loaned-message,
introspection, lifecycle, clock and type-description factories use fallible type-
support resolution before registering native ownership. A missing library or
symbol is returned as `Error::NativeTypeSupport` with both the logical interface
and loader error instead of becoming a null dereference or a partially registered
endpoint. Existing custom `Wrapped*TypeSupport` implementations retain default
fallible adapters over their original required methods.

## Dependency and resource result

The same final-source Jazzy callback-group application changes from 86 to 12
direct `DT_NEEDED` entries. Its 74 directly needed generated interface libraries
fall to zero; the remaining entries are the RCL/RMW/runtime and system libraries.
In a restricted-library smoke, the candidate starts and completes the exact
typed service protocol while the baseline loader exits before `main` on the first
unavailable, unused interface package. This demonstrates removal of the eager
runtime requirement, not support for executing an interface whose own libraries
are absent.

Four interleaved diagnostic repetitions used the same binary, workload, CPU set,
RMW and allocation probe on both sides. These samples are explicitly not timing-
qualified:

| Callback-group case | Eager-link median VmHWM | Lazy-load median VmHWM | Reduction |
| --- | ---: | ---: | ---: |
| Exclusive, budget 2/window 4, empty body | 19,454 KiB | 14,652 KiB | 24.7% |
| Reentrant, 4 workers/window 8, 16,384 operations | 19,500 KiB | 14,724 KiB | 24.5% |

Every run retains exact response, identity, concurrency and trace validation.
The result establishes lower application peak resident memory for these two
same-input diagnostics. It is not a startup-latency, throughput, wider-hardware
or universal RSS claim.

## Stability boundaries

The loader opens outside its cache mutex, tolerates concurrent first use and
retains only the winning handle. Failed opens are not cached, so a corrected
runtime search path can be retried. Tests compile and load a real fixture shared
library, exercise concurrent native allocation/finalization, and prove metadata
remains valid after the local resolver is retired. Generated owned-message
factories resolve the finalizer before allocation, validate null type-support
handles and propagate missing-library failures before endpoint registration.

The no-ROS `doc-only` path continues to use saved bindings and never calls the
runtime resolver. This boundary has a workspace all-target compilation gate; it
caught and fixed an initial unconditional type-description finalizer lookup.

## Reproduction

Run the native Jazzy and supported-distribution gates separately because they
share Cargo and ROS resources. Both scripts pin compilation to one allowed CPU,
set Cargo and build-script parallelism to one, and cap virtual/container memory
at 10 GiB or less. Set `R2R_BUILD_CPU` to select another allowed CPU:

```sh
bash docs/validation/lazy-typesupport-2026-10-30/run-jazzy.bash
bash docs/validation/lazy-typesupport-2026-10-30/run-compat.bash
```

Pass `humble`, `kilted` or `no-ros` to `run-compat.bash` to rerun one
compatibility cell after a transient container failure; the default `all` runs
all three sequentially.

After building the callback-group baseline/candidate and allocation probe, the
bounded resource diagnostic is:

```sh
python3 docs/validation/lazy-typesupport-2026-10-30/run-stage.py
```

[`results.json`](results.json) binds dependency counts, the restricted-library
outcome, raw VmHWM samples, the final-source build manifest and artifact
identities. The [protocol report](candidate-protocol.json) and compressed traces,
[resource samples](resource-stage.json.gz), [direct dependencies](dependencies.json),
[restricted-library result](restricted-library.json) and
[single-core Jazzy gate](logs/final-jazzy.log.gz),
[Humble gate](logs/final-humble.log.gz) and
[Kilted gate](logs/final-kilted.log.gz), plus the
[no-ROS documentation gate](logs/final-no-ros.log.gz), are retained beside it.
Startup timing and broader application/resource matrices remain open; the
required native release matrix is Humble, Jazzy and Kilted.
