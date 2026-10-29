# Documentation and compilation without ROS

The `doc-only` feature uses a checked-in ROS **Jazzy** snapshot. It provides the
same Rust message representations and API conditions as the generated Jazzy
bindings, including service/action events, lifecycle nodes/publishers/loans,
managed ROS clocks and the shared executor. Its purpose is documentation and
type checking; running ROS applications uses an ordinary native build.

The snapshot distribution is defined in
[`documentation_distro.txt`](../r2r_common/documentation_distro.txt).
`r2r/bindings/_r2r_generated_cfg.txt` records the available interfaces.
Documentation restores those conditions from the snapshot instead of depending
on `ROS_DISTRO` or discovering packages on the build host. The snapshot contains
24 standard/example/test packages, including the previously absent
`example_interfaces`, `std_srvs` and `service_msgs`. Existing documented packages
remain available. Generated string fields now match native `RosString` storage
and generated services expose their associated `Event` types.

Run the complete documentation gate:

```sh
python3 tests/check_documentation.py
```

It removes ROS discovery, middleware and library-path variables for its child
processes, then runs snapshot transaction tests, workspace compilation with all
targets, common-library tests, the two pure message-path generator tests, two compile-fail
`NativeMessageRef` ownership doctests and Rustdoc. It checks 46 public documentation artifacts and 150 specific methods,
including lifecycle native/loaned publication and destruction, configurable
managed publishers/timers, service introspection, simulated-time controls and
owned background task completion/cancellation/errors, bounded parallel Node groups and their
ownership compile-fail examples, action feedback overflow policy and
typed and runtime-typed batched action-server acceptance/execution/terminal completion, and scoped
native subscription callback ownership/options, borrowed-native publication native QoS event streams, type-description schema/hash/source queries and cancellable ROS-clock sleeps.
The main library now uses the default Rustdoc test setting, so ordinary workspace tests also
execute its API examples and ownership compile-fail cases. The no-ROS gate also runs the pipeline harness counterexamples, including raw-sample integrity, source drift,
finite performance ratios and preflight/runtime isolation requirements.

The `docs_no_ros` CI job runs this same gate in Ubuntu 24.04 with the
repository-pinned Rust 1.85 toolchain and no ROS installation. The refreshed
snapshot therefore covers examples, benches and test compilation as well as
library documentation.

For a quick compiler-only check:

```sh
cargo check --workspace --all-targets --features doc-only
```

## Refreshing the snapshot

Source Jazzy and the packages listed in
[`documentation_packages.txt`](../tools/documentation_packages.txt), then run:

```sh
tools/refresh_bindings.bash
```

The script generates all four binding crates into a separate candidate root.
It rejects missing packages and candidates that remove previously documented
modules, validates the candidate through the complete no-ROS documentation
gate, then replaces the checked-in directories. Partial replacement failures
restore every previous binding directory. Failed generation/validation keeps
the previous snapshot and leaves the candidate available for diagnosis.

No workspace-wide `cargo clean` or upfront binding deletion is needed. Existing
Cargo artifacts remain available. Package/prefix output and the generated file
list are sorted, so identical inputs produce identical snapshots; independent
local regeneration produced the same SHA-256 for all 36 binding files.

`R2R_BINDINGS_DIR` can select a separate snapshot root for build tooling; use an
absolute path with the layout `<root>/<crate>/bindings`. Native builds still
generate bindings from their sourced distribution. The Jazzy documentation
snapshot does not change Humble/Iron or newer native API conditions.
