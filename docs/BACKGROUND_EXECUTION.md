# Async runtime integration through an owned native executor task

[Background cleanup](BACKGROUND_CLEANUP.md) describes automatic native endpoint
release after public handles are dropped, final-operation cancellation and
action-server decision/backpressure notifications.

`Executor::spawn()` starts one native wait thread for all owned nodes and returns
`ExecutorTask`. The task implements `Future<Output = Result<Executor, ExecutorTaskError>>`, so any
Rust runtime can await completion. ROS subscriptions/services remain streams;
clients remain futures. Application code and the runtime's TCP/timer/I/O reactor
run normally while the native thread handles the shared ROS wait and dispatch.
The library depends on futures and the standard library, without a Tokio runtime
dependency. Tokio's network features are used only by examples and tests.

```rust
let mut executor = r2r::Executor::new(context);
let node_id = executor.add_node(node)?;
let task = executor.spawn()?;

// Await ROS futures/streams together with the application's runtime I/O here.
let mut executor = task.stop().await?;
executor.reset_cancel();
let node = executor.remove_node(node_id)?;
```

Configure entities before spawning. The task exclusively owns its nodes while
running; it exposes cancellation and completion, not `node_mut`. Endpoint
handles keep their ordinary thread-safety rules. Non-Send application futures
can stay on a current-thread runtime because they never move to the ROS thread.
Inline lifecycle/parameter callbacks still run during native dispatch on that
thread; application stream processing can run on runtime tasks.

Service/action availability, publisher matching and ACK observers registered
after the native wait has started notify its existing guard condition. Active
observer queues are scheduled before the wake, so the notification cannot race
ahead of registration. Canceled observers wake the same guard to prune their
queues; pre-spawn observers have their cancellation wake observes the shared state on transfer.
Weak wake references do not keep the executor/context alive. No generic periodic
fallback is added. Pending ACK observers retain the native executor's existing
bounded query cadence.

| Operation | Ownership and completion |
| --- | --- |
| `task.handle()` | Clone a weak `ExecutorHandle` for independent cancellation |
| `task.cancel()` | Request cancellation without waiting; repeated requests coalesce |
| `task.stop().await` | Cancel and recover the stopped executor, with cancellation still set |
| `task.await` | Wait for prior cancellation, context shutdown or native failure |
| Dropping a pending task | Request cancellation; release abandoned nodes when native dispatch returns |
| Native failure/callback panic | Release nodes before returning the native error or `ExecutorTaskError::ThreadPanicked` |

Cancellation is cooperative. A running callback must return before stopping can
finish. Drop does not synchronously join the worker, so it cannot block a runtime
thread behind that callback. Applications needing confirmed cleanup should await
`stop` (or retain and cancel/await the task). Once completion returns an executor,
native spin has ended and ownership is safe to reuse; unrelated thread-local
teardown may still finish afterwards. `is_finished` reports actual worker-thread
completion. Thread creation failures report `ExecutorTaskError::ThreadSpawn`; because spawn
consumes its executor, failure releases those owned nodes. Panic reporting needs
unwinding enabled; a process configured with `panic=abort` still aborts.

Context shutdown completes the task with its stopped executor; the shutdown
context cannot be restarted. Ordinary cancellation preserves a valid context
and permits reset, entity edits, node removal and another spawn. Dropping an
unobserved completed task releases the executor stored in its completion inbox.

Run the complete current-thread Tokio network workflow:

```sh
env -u CYCLONEDDS_URI cargo run -p r2r --example tokio_executor
```

It verifies 32 TCP-to-ROS service-to-pub/sub-to-TCP roundtrips, reliable ACKs,
explicit stop and a second service exchange after node reuse. The local TCP
listener uses an OS-assigned port and bounded workflow/stop deadlines.

Focused gates:

```sh
env -u CYCLONEDDS_URI cargo test -p r2r --lib executor::background:: -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib availability:: -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --test context_shutdown shutdown_completes_background_task_and_rejects_new_spawn -- --test-threads=1
python3 tests/lifecycle_publication_api.py
python3 tests/check_documentation.py
```

Background-specific errors stay separate from the common ROS error representation,
and observer notification uses the existing cancellation allocation instead of
expanding each default observer queue.

The native execution and the application's runtime scheduling remain distinct
ownership boundaries. This bridge supplies safe composition today while leaving
native worker/reactor scheduling replaceable; it does not impose a callback
executor or per-node thread as the application API.
