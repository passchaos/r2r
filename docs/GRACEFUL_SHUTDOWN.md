# Process signals and graceful shutdown

r2r keeps signal handling explicit so libraries embedding ROS do not silently
replace a process-global resource. Applications that want rclcpp-style Ctrl-C
behavior can install it on their shared context:

```rust
let context = r2r::Context::create()?;
context.install_signal_handler()?;

let node = r2r::Node::create(context.clone(), "application", "")?;
let mut executor = r2r::Executor::new(context.clone());
executor.add_node(node)?;

let stopped = futures::executor::block_on(executor.spawn()?)?;
assert!(!context.is_valid());
```

Applications without an Executor can await the same event without polling or a
runtime-specific signal crate:

```rust
let shutdown = context.wait_for_shutdown();
futures::executor::block_on(shutdown);
```

`ContextShutdown` is a fused, runtime-agnostic Future. Its registration is weak,
so it does not retain the Context; dropping it cancels only that observation.
Manual shutdown, an installed process signal and implicit finalization of the
last Context owner all complete it. A future created after shutdown is ready
immediately.

On Unix, the process handler subscribes to SIGINT and SIGTERM. Calling
`install_signal_handler` again is harmless and retargets it when a finalized
context is replaced by a fresh one. The handler remains installed for the
process lifetime, matching the process-level behavior expected by ROS command
line applications.

## Safety and wake behavior

The POSIX handler does not call RCL, lock a mutex or allocate. It writes through
`signal-hook`'s async-signal-safe channel; a dedicated ordinary thread calls the
existing idempotent `Context::shutdown` path. Shutdown then:

- triggers registered native guards while the context is still valid;
- wakes empty Node, Executor, ParallelExecutor and CallbackExecutor waits;
- completes background executor tasks and returns their owned nodes/groups;
- invalidates every context clone and rejects later native spins/spawns;
- defers native resource finalization until owners and the final context clone
  are dropped.

Signal disposition is process-global. Install the handler early and coordinate
with any other signal-handling library. The underlying registry chains handlers
where possible, but applications should not assume independent libraries can
all own default signal behavior.

Run the complete example and interrupt it from another shell:

```sh
cargo run -p r2r --example graceful_shutdown
kill -INT <pid>   # or press Ctrl-C
```

The native integration tests raise real SIGINT and SIGTERM signals, verify an
empty background native wait is interrupted, recovers the executor's Node and
then prove that the process handler retargets a newly created context. They also
cover ready-after-shutdown, dropped observations and last-owner finalization.
