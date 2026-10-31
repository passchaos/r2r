# Same-Node callback groups with owned native execution

CallbackExecutor drives owned Nodes, mutually exclusive groups and bounded reentrant groups
within one total worker budget. The native wait is one owned job in that pool,
and ready group jobs can run on other workers. A long handler in one group does
not occupy a different group's worker. The native job can migrate between
workers while retaining exclusive Node and wait-set ownership.

Configure group factories before transferring Nodes/groups, or after stopping:

```rust
let context = r2r::Context::create()?;
let mut node = r2r::Node::create(context.clone(), "application", "")?;
let mut perception = r2r::CallbackGroup::new();
let mut control = r2r::CallbackGroup::new();
let timer = node.create_wall_timer_callback_in_group(
    &mut perception, std::time::Duration::from_millis(10), || Ok(()),
)?;
// Register subscriptions and services with &mut control.
let mut executor = r2r::CallbackExecutor::new(context, 2)?;
let node_id = executor.add_node(node)?;
let control_id = executor.add_group(control)?;
executor.add_group(perception)?;
let task = executor.spawn()?;
// Await application I/O and endpoint futures on the chosen Rust runtime.
let mut executor = task.stop().await?;
executor.reset_cancel();
let (node, control) = executor.node_and_group_mut(node_id, control_id)?;
```

The [current-thread example](../r2r/examples/callback_groups.rs) checks 32 exact
native forwards and control RPC responses on one Node while a separate group's
timer handler is explicitly blocked. It releases that handler, stops the pool,
recovers state and verifies another response after restart. Two workers include
native waiting; no extra wait thread or per-callback thread is created.

## Native input and callback APIs

subscribe_callback_in_group decodes typed input on the callback worker.
subscribe_native_callback_in_group retains an independently initialized native
take container and supplies NativeMessageRef only during invocation. Native
forwarding can publish_native_ref without an owned Rust payload. A scoped
borrow cannot escape its invocation, even across worker execution.

CallbackQueueOptions separates ring capacity from the native per-source batch.
The default is 16 waiting inputs and a 16-input snapshot limit. The running
handler's input is separately owned. GroupedSubscriptionOptions additionally
retains local-publication and network-flow settings from native subscription
options; both configured batch limits apply. DDS QoS/history remains independent.
Full input rings are omitted from native waiting and resume through the current
owner's cached guard when capacity returns. No unread sample is taken solely to
discard it under pressure.

create_service_callback_in_group adapts typed requests/replies on the worker.
create_service_native_callback_in_group supplies a scoped request, request
identity and one reusable native response buffer. Request take, response send,
query/configuration and finalization share a short native operation mutex.
Application code holds that mutex for no part of its invocation. Dropping a
service token closes intake and wakes cleanup; a selected handler can finish
owned input, but a finalized endpoint cannot receive a late native response.
Iron-and-newer native service introspection and actual endpoint QoS remain
available.

Wall/ROS group timers support autostart, one-shot policy, period queries/exchange,
cancel/reset and native timing metadata where the SDK provides TimerCallInfo.
For mutually exclusive groups, only one invocation can be queued or running for a timer. Native advancement
occurs on the worker just before the user handler, so expected/actual timestamps
describe invocation advancement. In-flight timers are omitted from waiting
until completion. Controls wake the current owner, and finalization is
synchronized with short native operations outside user code.

LifecycleNode overrides these factories. The native timer resource retains its
activation/manual generation after into_node transfer. Activation resets a
fresh native deadline; old queued selections cannot consume a new activation's
readiness. One-shot completion/manual cancel remain paused until reset. A
callback that already acquires an active native permit may finish during a
concurrent deactivation.

## Runtime-agnostic group tasks

`CallbackGroup::spawn_task` retains arbitrary `Send` futures under the same mutually exclusive
scheduler token. `ReentrantCallbackGroup::spawn_task` places each future in the least-populated
stable lane, bounding concurrent polls by the group's configured concurrency without requiring the
future to be `Sync`. This is the action/integration path: an action server request stream and client
goal/result/feedback futures can be moved into group tasks while the native Node job continues to
own RCL wait and take dispatch. Await points yield normally; synchronous application work follows
the chosen exclusive or per-lane ordering. Task handles provide RAII cancellation or explicit
detach, and stop/reset preserves pending future state. See
[callback-group tasks](CALLBACK_GROUP_TASKS.md), `callback_group_action` and
`reentrant_group_action`.

## Bounded reentrant groups

ReentrantCallbackGroup::new(concurrency) admits up to that many simultaneous
invocations across all its sources. Its factories require Fn + Send + Sync;
mutable local FnMut state belongs in CallbackGroup. Native wait-set ownership
stays exclusive. Each lane owns a receiver, invocation inputs and any service
reply buffer. The shared application handler runs without an internal handler
mutex. The group limit and total worker budget both bound actual concurrency.
Completion order can differ from input order.

```rust
let mut group = r2r::ReentrantCallbackGroup::new(4)?;
let subscription = node.subscribe_reentrant_callback::<r2r::std_msgs::msg::UInt64, _>(
    &mut group, "input", r2r::QosProfile::default(), |message, _| {
        println!("{}", message.data);
        Ok(())
    },
)?;
let group_id = executor.add_reentrant_group(group)?;
// After stopping and resetting cancellation:
let (node, group) = executor.node_and_reentrant_group_mut(node_id, group_id)?;
```

The typed/native subscription factories are subscribe_reentrant_callback and
subscribe_native_reentrant_callback, each with an options variant. Both reuse
the same single-producer bounded source ring. Dropping one retired receiver
leaves its peers available; the last closes and drains input. Close wakes all
receivers, and capacity recovery wakes the current native owner.

create_service_reentrant_callback and create_service_native_reentrant_callback
retain one reusable response container per lane. Short take/send/finalization
operations synchronize on the native endpoint; user work on different requests
can overlap. Native request identities and response buffers remain independent.

create_wall_timer_reentrant_callback and create_timer_reentrant_callback provide
startup, one-shot, and supported timestamp variants. A timer holds one pending
selection until a lane advances it under the native/clock locks, then releases
selection before user work. The next deadline can therefore enter another lane
while the first body is still running. One-shot cancellation precedes release.
Manual cancel/reset, stale lifecycle generations, ROS time and finalization
retain the exclusive timer's controls. LifecycleNode explicitly overrides every
reentrant timer factory and retains generation policy after into_node.

The [reentrant example](../r2r/examples/reentrant_callbacks.rs) waits until one
handler is blocked, verifies a second request to the same native service completes,
then releases the first and checks stopped group reuse. Three workers include
native waiting and two invocation lanes. No per-invocation thread is created.

## Ownership, completion and errors

Node/group IDs remain stable after stop. Live sources must belong to groups and
Nodes owned by the same CallbackExecutor; spawn rejects missing membership.
Node/group configuration is exclusively owned by the running task.

handle() is cloneable and weak. It does not retain Nodes, handlers or Context.
Requests persist until reset_cancel on the stopped owner. Task worker_count
reports launched OS threads; the executor reports the configured budget.
Empty groups do not increase the launched count. At least the native job runs
so an empty executor can observe cancellation/context shutdown.

Native/callback errors and panics cancel peers directly, including a worker in
rcl_wait, before aggregate task polling. Error completion collects workers and
releases recovered ownership before returning the lowest failing worker index.
Normal stop returns the executor with live queued inputs and handler state.
Dropping a pending task cancels and detaches without joining on the runtime.
Cancellation is cooperative: the handler currently executing must return.

Group jobs and the native job transfer owned values through separate slots.
Ready notifications coalesce in preallocated rings; per-event scheduling does
not allocate a new job box or waker. Native message take allocations and user
work remain explicit. Ordinary stream, scoped inline callback and serial
Executor APIs retain their existing execution models.

## Validation and remaining scope

Nearby tests cover native same-Node progress, group exclusion, source capacity
recovery, close/publication retirement, native sequence/storage identity,
handler reentry, send/finalization races, timestamp advancement, lifecycle
generations, stop/reuse, weak expiry and panic/drop cleanup.

```sh
cargo test --locked -p r2r --lib nodes::callback_executor::tests -- --test-threads=1
cargo test --locked -p r2r --lib --release nodes::callback_executor::tests -- --test-threads=1
cargo run --locked --release -p r2r --example callback_groups
cargo run --locked --release -p r2r --example reentrant_callbacks
cargo test --locked -p r2r --doc CallbackExecutor
cargo test --locked -p r2r --doc subscribe_native_callback_in_group
```

Check nonzero matching counts and use a valid sourced RMW configuration.
Live configuration, a thread-free runtime reactor and broader
application/performance matrices remain open. Mutually exclusive group capability
and structural ownership/routing properties do not establish a throughput,
latency or resource advantage over C++. CPU-pinned independent interleaved
Rust/rclcpp measurements remain required by the full goal.
