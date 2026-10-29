# Native type-description discovery and source fidelity

r2r now serves the standard `~/get_type_description` endpoint on Jazzy and newer and exposes
checked async query clients. Node options/parameter overrides or explicit startup select the
additional endpoint. Native cache responses run in the existing Node/shared/background executor,
with at most sixteen requests handled per dispatch and no user request channel. Generated
message/service/action hashes are available independently from payload construction.

Clients validate the returned type name and canonical schema RIHS hash before exposing a
successful response. Native schemas follow endpoint cache lifetime: shared types stay available
until their last publisher/subscription/service/action registration is removed. Source inclusion,
invalid/missing identities, node namespace/remapping and stop/restart preserve their normal
native service semantics. Explicit service/client QoS supports larger query windows; the forty
concurrent request oracle uses matching KeepLast(64) histories. Default KeepLast(10) is a finite
history even with reliable transport, so the larger unconfigured batch remains a rejected boundary.

## Exact source boundary

Direct generated C metadata for std_msgs/msg/UInt64 on this Jazzy installation reports a 263-byte
source whose last byte is NUL, while the installed 263-byte source ends in newline. RCL cache
conversion can remove that NUL and return 262 bytes. Both ordinary rclcpp and r2r native servers
use this C cache. The initial strict source-file comparison and subsequent cross-language source
comparison failed; logs and native bytes remain under `rejected/`.

r2r repairs only a source prefix matching the complete installed file up to missing trailing
whitespace. Native-server responses and checked-client results share this recovery module;
missing or semantically different installed files leave the native source unchanged. Source file
contents and lookup keys are cached, and owned Rust source buffers reuse spare capacity. The
immutable generated cache is preserved. Source fidelity is a separate recorded gate; it cannot
be inferred from matching canonical structure hashes, which exclude comments and whitespace.

Independent Rust-server/C++-client and C++-server/Rust-client processes verify the complete UInt64
structure, recomputed canonical RIHS, source inclusion/omission and exact source bytes under both
Jazzy RMWs. The server stays alive until client completion. Build manifests bind all workspace
sources, generated snapshots, scripts and binaries; the runner rejects build drift before launch
and the verifier checks the matrix and recorded exact-source gate.

```sh
python3 tools/ros_oracles/type_descriptions.py verify --require-current-sources \
  --require-exact-sources \
  docs/interop/type-descriptions-2026-10-30/cyclonedds.json \
  docs/interop/type-descriptions-2026-10-30/fastdds.json
```

## Local acceptance

Twelve focused cases cover RIHS parsing/version boundaries, generated identities, source recovery
counterexamples and buffer reuse, live message/service/action schemas, shared type registration,
invalid/missing hashes, source omission, remapping/options/stop/restart, same-name forged schema
rejection, background forty-query progress and timeout/foreign-node ownership. The final debug
workspace and both optimized Jazzy RMW suites pass 641 main-library tests with eight manual
measurements ignored; seventeen Rustdoc examples and warning-denied all-target Clippy pass.

Humble/Fast DDS verifies the unsupported service/hash boundary; Kilted/both RMWs exercise the
native schema API, service tests, examples and Clippy. The Jazzy documentation snapshot adds
`type_description_interfaces` while preserving all prior packages. Candidate validation and
transactional installation pass, and a true no-ROS container checks all targets, documentation,
reflection functions, schema client methods and ownership compile-fail examples.

A full optimized suite exposed an existing asynchronous per-node graph discovery race. Its tests
now register a graph waiter before querying and wait for visibility before retaining exact
names/types/enclave assertions. The failure and subsequent native graph gates are archived.

CI requires independent native type-description endpoints for Jazzy, Kilted, Lyrical and Rolling.
Humble/Iron startup remains explicitly unsupported; Iron generated hash support has no new local
runtime evidence. Local external C++ evidence covers Jazzy only. Missing installed sources,
networked/multithreaded schema applications, malformed remote schema resource bounds and
representative performance comparisons remain wider validation scope. This slice establishes
functional/runtime discovery capability and source fidelity, not a throughput leadership claim.

## Saved validation identity

`compatibility.json` records 196 current source/snapshot/gate inputs and 10 compressed logs.
Humble/Fast DDS passes 3 type-description boundaries, 53 service cases and 7 graph cases; Kilted
under both RMWs passes 12 type-description, 64 service and 7 graph cases per environment. Examples
and all-target Clippy pass. The true no-ROS gate verifies 36 pages/functions and 72 methods,
including all four reflection/hash functions, node configuration and checked client APIs.

```sh
python3 tools/bench_ros/verify_pubsub_pipeline.py --compatibility-manifest \
  docs/interop/type-descriptions-2026-10-30/compatibility.json
```

The initial exact-source failure, direct native C metadata and default-history large-window
stall are preserved. Green final evidence covers the final-source implementation; rejected
reports and partial runs are not used to claim this validation. Performance comparisons remain
pending a source-identical rebuild and isolated run.
