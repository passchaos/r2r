# Driving multiple ROS nodes

`Executor` owns nodes from one `Context` and waits for all their native ROS entities in one
reusable RCL wait set. Empty nodes do not add sequential wait deadlines. Ready subscriptions,
timers, service clients/servers and action clients/servers are dispatched after the same native
wait, and a Rust future can drive the complete application without an additional runtime.

```rust
let context = r2r::Context::create()?;
let mut server_node = r2r::Node::create(context.clone(), "server", "")?;
let mut client_node = r2r::Node::create(context.clone(), "client", "")?;
// Configure services, clients, subscriptions and timers on their owning nodes.
let mut executor = r2r::Executor::new(context);
let server_id = executor.add_node(server_node)?;
let client_id = executor.add_node(client_node)?;
let result = executor.spin_until_future_complete_with_options(
    application_future,
    std::time::Duration::from_secs(5),
    r2r::FutureExecutionOptions::wake_driven(),
)?;
```

The [runnable pipeline](../r2r/examples/multi_node_pipeline.rs) puts a timer/publisher, compute
service and requesting/subscribing application on three nodes. It validates 8192 payload values
and 64 service responses with at most 16 requests in flight, then destroys its timer/endpoints
and removes the nodes. One native wait drives the workflow.

## Ownership and configuration

`add_node` moves a node into the executor, excluding concurrent waits for the same entities.
All nodes must share the executor context. `ExecutorNodeId` remains stable when other nodes
are removed; IDs from removed nodes or another executor report `RCL_RET_NODE_INVALID`.
`node_mut(id)` allows configuration between spins, and `remove_node(id)` returns ownership for
independent spinning or destruction. Adding/removing nodes clears inactive native wait-set
references before ownership can leave the executor. The wait set is finalized before the
remaining nodes.

Futures and their application state remain on the calling thread and can be non-Send. An
external channel or independently running runtime can wake the native wait. A synchronous
executor call does not run another runtime's I/O reactor; independently driven sources must
make progress themselves. Entity handles/streams can be configured before moving the nodes
into the executor and then composed in the application future.

`spin_once(timeout)` waits once for all nodes and reports native errors. `Context::shutdown`
interrupts even an empty executor's native wait. Ready futures complete with zero timeout and
without creating native storage; pending zero-timeout work reports `RCL_RET_TIMEOUT`.
Completion, timeout and unwinding disarm the Rust waker. Old wakers have weak native references
and do not retain the executor/context or complete later work.

## Cancellation and reuse

`executor.handle()` creates a cloneable, thread-safe `ExecutorHandle`. The executor allocates
the shared cancellation state only when a handle is requested. Handles hold weak references:
they do not keep the executor, its nodes or its context alive. `handle.cancel()` returns
`Ok(true)` when the executor still exists and `Ok(false)` after it has been dropped.

Cancellation is persistent until the owner calls `reset_cancel()` between runs. It interrupts
an active native wait through the existing cached guard condition, without shutting down the
context, adding a polling thread or creating another native timer/guard. Repeated requests
coalesce to one native wake until reset. `spin_once` and pending future execution report
`Error::ExecutorCancelled`; continuous `spin()` returns `Ok(())` on cancellation or normal
context shutdown. It drives ROS events, while application futures still need a caller or an
independently running Rust executor.

```rust
let control = executor.handle();
let worker = std::thread::spawn(move || {
    executor.spin()?;
    Ok::<_, r2r::Error>(executor)
});
// Drive the application's streams and futures while the worker handles ROS events.
control.cancel()?;
let mut executor = worker.join().expect("ROS worker panicked")?;
executor.reset_cancel();
// The same nodes, context and native storage can now drive another operation.
```

A request made before execution prevents even a ready future's first poll and does not create
native wait storage. A future that completes during a poll can win a simultaneous cancellation
request after the check immediately before that poll. Reset and new requests should be
coordinated by the application; a native wake already in flight can be spurious after reset,
and pending work remains reusable. Native triggers temporarily retain the guard storage while
calling RCL, so a concurrent executor drop cannot invalidate it.

The [runnable control example](../r2r/examples/executor_control.rs) drives ROS on a background
thread and composes service and subscription streams with `futures::executor::block_on`. It
validates 8192 payload values and 32 requests, cancels and joins the worker, then resets and
reuses the same nodes for another sum request.

## Waiting and dispatch

The node implementation separates pre-wait maintenance, native entity registration and ready
event dispatch. Single-node and multi-node execution share those responsibilities and the same
atomic future-wake implementation. The executor records actual ordinary-entity offsets for
each node, then appends all action entities. Native action handles retain the returned native
indices. A callback failure in one node is retained while other ready nodes are dispatched.

Graph waits and matching register only the interested node's native graph guard. Generic
future polling is disabled by `wake_driven`; live reliable ACK observers still cap the shared
wait at 10 ms because RCL provides an ACK query without a wait-set event. Completed local
action-cancel decisions request a zero wait. No polling thread or extra native timer is added.

Native wait-set allocation is reused while capacities remain equal. Topology and entity
changes update capacities on the next spin; node IDs use a stable map and swap removal.
Dispatch is currently on one thread with the existing bounded entity batches. The separated
registration/readiness boundary allows future work on scheduling policy, concurrent workers
and runtime reactors without making sequential node polling the executor's permanent design.

## Local verification

The [nearby tests](../r2r/src/nodes/executor/tests.rs) exercise shared native waiting, non-Send
external wakes, zero deadlines, stable removal IDs, dropped endpoints, stale wakers, ACK-query
cadence and shutdown. [Complete workflows](../r2r/src/nodes/executor/tests/workflows.rs) cover
ordinary entity offsets with decoys on other nodes and action success/cancellation after
ordinary entities on a later node. Native wait return codes and requested wait deadlines
establish interruption without relying only on elapsed-time thresholds.
The [cancellation tests](../r2r/src/nodes/executor/tests/cancellation.rs) cover requests before
the first spin/during initial polling, native wait interruption, concurrent wake coalescing,
node/guard reuse, expired weak handles and repeated cancellation after reset.

```sh
env -u CYCLONEDDS_URI cargo test -p r2r --lib nodes::executor -- --test-threads=1
env -u CYCLONEDDS_URI RMW_IMPLEMENTATION=rmw_fastrtps_cpp \
  cargo test -p r2r --lib nodes::executor -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib future_executor -- --test-threads=1
env -u CYCLONEDDS_URI cargo run -p r2r --example multi_node_pipeline
env -u CYCLONEDDS_URI cargo run -p r2r --example executor_control
```

The service-application performance harness compares historical Rust nonblocking multi-node
polling, the shared native wait and rclcpp `SingleThreadedExecutor`. It records request RTTs,
completed throughput and process CPU time with fixed node counts, request windows, QoS and
CPU affinity. Application-level performance and compatibility are separate acceptance gates.
