# Transactional live-parameter validation

r2r's local parameter API and standard ROS parameter services use the same
transaction engine. Scoped pre-set callbacks transform a batch, on-set callbacks
reject before any mutation, and post-set callbacks observe a successful commit
after internal locks release. Duplicate names normalize deterministically;
descriptor constraints, derived/runtime storage, simulated time and local/ROS
events remain one atomic boundary.

The focused Jazzy gate covers 65 parameter-related cases. These include callback
ordering and RAII removal, transformed batches, empty/invalid transformations,
service/local parity, derived/runtime commits, strict declaration policy,
descriptor ranges/read-only/dynamic typing, event publication, override parsing,
remote clients, timeout ownership and recursive-mutation errors. The
`parameter_callbacks` example compiles with warnings denied.

The current no-ROS gate compiles all targets and verifies 67 public API artifacts plus
276 required methods. It explicitly checks the three callback handle pages and
the Node declaration, atomic mutation, descriptor and pre/on/post registration
methods.

## Reproduction

Both compilation paths bind Cargo/rustc to one allowed CPU and cap memory at
10 GiB or less:

```sh
bash docs/validation/parameter-callbacks-2026-10-30/run-jazzy.bash
bash docs/validation/lazy-typesupport-2026-10-30/run-compat.bash no-ros
```

Run the interactive standard-service example with:

```sh
cargo run -p r2r --example parameter_callbacks -- --ros-args -r __ns:=/demo
ros2 param set /demo/parameter_callbacks gain 4
ros2 param get /demo/parameter_callbacks effective_gain
```

`results.json` binds parameter source and compressed-log identities; the shared
no-ROS count also includes the subsequently added Context signal-shutdown page
and its shutdown Future.
The first focused
test attempt inherited an unrelated missing CycloneDDS configuration path from
the host; clearing external DDS profile variables produced the recorded passing
run and is enforced by the reproduction script.
