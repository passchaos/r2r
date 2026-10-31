# Typed action-server feedback native reuse validation

Each typed action server now retains one native feedback container and reuses it for every
`publish_feedback` call across goals. Feedback conversion still happens before the native server
lock is acquired, so the shared slot never extends a server critical section. The one-slot cache
reuses the native outer message and the nested feedback sequence capacity; a fallible conversion
retires its partially updated container instead of exposing it to a later publication.

A real Fibonacci client drives two accepted goals with four and three feedback publications
through a tracked server. It requires one native feedback allocation across both goals and verifies
that explicit server destruction releases the cached container.

The five-repetition no-RCL/DDS stage fills 10,000 feedback messages per path:

| Native feedback storage | Allocations | Frees | Requested bytes |
| --- | ---: | ---: | ---: |
| Fresh | 20,000 | 20,000 | 1,680,000 |
| Reused | 0 | 0 | 0 |

Every repetition has identical allocation counts. Reuse removes two allocation/free calls and 168
requested bytes per feedback publication. Elapsed diagnostics are retained but are not
timing-qualified.

```sh
bash docs/validation/typed-action-server-feedback-native-reuse-2026-11-01/run-jazzy.bash
bash docs/validation/typed-action-server-feedback-native-reuse-2026-11-01/run-compat.bash
bash docs/validation/typed-action-server-feedback-native-reuse-2026-11-01/run-no-ros.bash
python3 docs/validation/typed-action-server-feedback-native-reuse-2026-11-01/verify.py
```

All compilation is pinned to one allowed CPU with one Cargo/build-script job and a 10 GiB
virtual/container memory ceiling.
