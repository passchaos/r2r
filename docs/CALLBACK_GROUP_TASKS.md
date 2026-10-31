# Runtime-agnostic callback-group tasks

`CallbackGroup::spawn_task` and `ReentrantCallbackGroup::spawn_task` retain any `Send + 'static`
future returning `r2r::Result<()>` without choosing an async runtime. The exclusive form uses one
mutual-exclusion token:

```rust,no_run
# use futures::StreamExt;
# use r2r::example_interfaces::action::Fibonacci;
# fn install(
#   group: &mut r2r::CallbackGroup,
#   mut requests: r2r::ActionServer<Fibonacci::Action>,
# ) -> r2r::CallbackGroupTask {
group.spawn_task(async move {
    while let Some(request) = requests.next().await {
        let (mut goal, _cancels) = request.accept_deferred()?;
        goal.execute()?;
        goal.succeed(Fibonacci::Result::default())?;
    }
    Ok(())
})
# }
```

The task is polled with the same scheduler token as grouped subscription, service and timer
handlers. Synchronous work in one task cannot overlap another handler in that group. Awaiting an
action request, client result/feedback, runtime I/O or another Rust future yields without blocking
the native Node wait job. This gives typed action client/server workflows an explicit group without
adding a second async runtime or action-specific threads.

A bounded reentrant group assigns each task to the lane with the fewest retained registrations:

```rust,no_run
# fn install() -> r2r::Result<()> {
let mut group = r2r::ReentrantCallbackGroup::new(4)?;
for connection in 0..8 {
    group.spawn_task(async move {
        process_connection(connection).await
    }).detach();
}
# fn process_connection(_: usize) -> impl std::future::Future<Output = r2r::Result<()>> {
#     std::future::ready(Ok(()))
# }
# Ok(())
# }
```

One task stays in one stable lane across stop/restart. Work in that lane is serial, while distinct
lanes may run concurrently up to `group.concurrency()` and the executor worker budget. A future is
never polled concurrently, so it needs `Send` but not `Sync`; shared synchronous handlers retain
their existing `Fn + Send + Sync` requirement. Registration balancing is deterministic and does not
migrate a live future or spawn a thread.

## Ownership and cancellation

`CallbackGroupTask` is an RAII cancellation handle. `cancel()` and handle drop wake the group;
the application future and its captured endpoint streams are dropped on the group worker at the
next poll. `is_cancelled()` reports the request and `is_finished()` reports actual retirement.
`detach()` transfers the lifetime entirely to the group when individual cancellation is not
needed.

Stopping or cancelling `CallbackExecutor` pauses group tasks without dropping their futures.
Recovered groups retain await state and captures; after `reset_cancel()` and restart, pending tasks
are explicitly reawakened and continue. A returned task error follows the existing group failure
path and stops peer workers. Panics remain scheduler panics and never cross an FFI callback.

Task completion removes it from `CallbackGroup::len()` or its selected reentrant lane. A group
containing only completed tasks is empty and can be reused. A task that blocks inside `poll` blocks
its assigned lane, just as a synchronous callback does; normal await points release the lane.

Run [`callback_group_action`](../r2r/examples/callback_group_action.rs) for a complete action
server/client feedback/result workflow under mutual exclusion, or
[`reentrant_group_action`](../r2r/examples/reentrant_group_action.rs) for the same workflow in two
bounded lanes.
