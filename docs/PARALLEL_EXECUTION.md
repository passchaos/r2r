# Bounded native execution across independent Node groups

`ParallelExecutor` keeps a fixed worker budget and gives each nonempty group
one native thread with exclusive Node and reusable RCL wait-set ownership.
A long synchronous callback in one group does not occupy another group's
worker. Nodes assigned to the same group dispatch serially. This makes
placement an explicit application decision: isolate slow perception or
blocking device callbacks from control, services and diagnostics.

```rust
let context = r2r::Context::create()?;
let perception = r2r::Node::create(context.clone(), "perception", "")?;
let control = r2r::Node::create(context.clone(), "control", "")?;
// Configure endpoints and callbacks before transferring the Nodes.
let mut executor = r2r::ParallelExecutor::new(context, 2)?;
let perception_id = executor.add_node_to_worker(perception, 0)?;
executor.add_node_to_worker(control, 1)?;
let task = executor.spawn()?;
// Await application streams/futures on the chosen Rust runtime.
let mut executor = task.stop().await?;
executor.reset_cancel();
let perception = executor.remove_node(perception_id)?;
```

The [current-thread Tokio example](../r2r/examples/parallel_executor.rs)
checks 32 exact control RPC responses while another group's callback is
held at an explicit barrier. It releases the callback, stops all workers,
recovers three Nodes, resets cancellation, verifies a new request and removes
the endpoints/Nodes. The barrier proves independent progress; it does not
measure transport latency or claim a throughput advantage over C++.

## Placement and ownership

`new(context, workers)` rejects zero workers and an invalid Context. `add_node`
places Nodes round-robin after each successful insertion. `add_node_to_worker`
uses a zero-based group index without advancing round-robin placement; invalid
indices fail. All Nodes must share the Context. Stable `ExecutorNodeId` values
survive other removals and stop/restart. `node_mut` and `remove_node` are
available only when the application owns the stopped executor.

The configured budget is `ParallelExecutor::worker_count()`. Empty groups do
not start threads. `ParallelExecutorTask::worker_count()` reports the number
actually launched. A completely empty pool starts one native wait worker so
Context shutdown and cancellation can complete it. No per-Node/per-spin thread
or generic periodic spin timer is introduced.

Each group reuses the [serial Executor](MULTI_NODE_EXECUTION.md) and its
[background task](BACKGROUND_EXECUTION.md), including action offsets, native
QoS events, graph/matching observers, reliable ACK cadence and current-owner
cleanup wakes. The same Node/entity never appears in simultaneous native wait
sets. There is no new unsafe pointer sharing, shared dispatch lock or global
ready queue on ordinary execution paths.

## Completion, cancellation and failures

The task implements `Future<Output = Result<ParallelExecutor,
ParallelExecutorTaskError>>`. Normal cancellation and Context shutdown return
all stopped groups, preserving configured placement. Cancellation remains set
until `reset_cancel()` between runs. Application streams/futures stay on their
chosen runtime; their surrounding application state can be non-Send.

`handle()` returns a cloneable aggregate weak handle. Clones share one immutable
slice of worker handles and do not retain Nodes, Contexts or native threads.
`cancel()` attempts every group, including groups not started yet; it returns
whether any configured worker still exists, or the first native wake error.
`is_cancelled()` observes whether any live worker has a pending request.
Persistent per-worker controls prevent an early failure from losing a peer's
cancellation before that peer registers its native guard.

On a native dispatch error or synchronous callback panic, the failing native
thread requests peer cancellation before sending its completion. This happens
even when the application never polls the aggregate task. Awaited failure
collects every worker and drops recovered healthy groups before reporting the
lowest failing configured worker index. `Worker` preserves the underlying
`ExecutorTaskError`; `Cancellation` represents an explicit stop-wake failure.
Thread/startup failure cancels already-started groups and returns immediately;
their asynchronous cleanup still requires running callbacks to return.

Dropping a pending task cancels workers and detaches their threads without
blocking the application. `is_finished()` checks actual native thread teardown
without polling completion. Cancellation is cooperative: a callback that never
returns can keep its own worker alive. Other workers can finish independently.

## Scope and validation

This is explicit concurrency across independent Node groups. Same-Node callback
groups, work stealing, dynamic running placement and a native runtime reactor
remain separate capability gaps. Future scheduling work can replace the grouping
layer while preserving exclusive native wait ownership and the existing bounded
dispatch/notification boundaries. `Executor` remains the single-thread choice.

Nearby [regressions](../r2r/src/nodes/parallel_executor/tests.rs) make a native
subscription ready only after another callback is confirmed blocked, then
check progress and actual thread identity. The same-group case must wait until
release and run on the same thread. Additional tests cover stable IDs, skipped
empty groups, pre-start cancellation, weak expiry/reuse, 32 cross-worker native
RPC identities, simultaneous errors, panic/drop cleanup, shutdown and native
lifecycle service plus `/clock` activation/backward-time deadlines.
Native storage identities establish three disjoint wait sets/guards and unchanged
cached storage across stop/restart. Shutdown covers both the empty worker and
three idle populated groups.

```sh
cargo test --locked -p r2r --lib parallel_node_executor_ -- --test-threads=1
cargo test --locked -p r2r --lib --release parallel_node_executor_ -- --test-threads=1
cargo run --locked --release -p r2r --example parallel_executor
cargo test --locked -p r2r --doc ParallelExecutor
```

Run DDS tests in a sourced ROS environment with valid RMW configuration and
check nonzero filtered counts. [Recorded validation](validation/parallel-executor-2026-10-30/README.md)
tracks profiles, distributions and the full necessary gates. Independent-binary
CPU-pinned scaling, allocation/resource measurements, Rust/rclcpp multithreaded
application comparisons and the wider supported-distribution runtime matrix
remain required evidence for the overall performance goal.
