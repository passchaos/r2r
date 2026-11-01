# Node blocking spin and shared-context conveniences (2026-11-01)

`Node::spin()` blocks until the shared context is shut down and propagates wait-set errors;
`spin_with_timeout` selects the idle wait bound. A shutdown that races with the native wait is
treated as a normal termination. `Node::context`, `Node::is_valid` and `Node::shutdown` expose the
shared context for signal-driven or multi-node applications.

Three focused cases verify that a background context shutdown ends a blocking spin, that two nodes sharing a
context terminate together, and that the context accessor/shutdown forward to the same context. They pass on Jazzy (both RMWs, debug
and release), Humble/Fast DDS and Kilted/both RMWs with warning-denied Clippy. The complete Jazzy
release library suite passes 838 cases with 25 manual stages ignored under both RMWs. The no-ROS
gate compiles all targets and verifies 84 public pages and 341 methods.

```sh
python3 docs/validation/node-spin-context-2026-11-01/verify.py
```

All compilation used one pinned CPU, one Cargo job and a 10 GiB memory ceiling. This record is a
per-run snapshot and does not pin current sources.
