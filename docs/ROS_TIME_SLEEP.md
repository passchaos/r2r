# Await ROS-clock deadlines

`Node::sleep_until_ros(deadline_ns)` and `Node::sleep_for_ros(duration)` return an owned `RosSleep`
future. They use the node's ROS clock, including signed simulated timestamps. Await them while
driving the owning Node/shared/background executor. Independent runtime futures and ordinary
timers continue progressing in the same executor.

```rust
let delay = node.sleep_for_ros(std::time::Duration::from_secs(1))?;
node.spin_until_future_complete_with_options(
    delay,
    std::time::Duration::from_secs(10),
    r2r::FutureExecutionOptions::wake_driven(),
)?;
```

The outer execution timeout uses steady time. The sleep itself uses ROS time. Paused simulated
time can keep the sleep pending indefinitely; wall-clock time does not consume its relative
duration. Backwards jumps preserve the absolute target, and reaching that target completes the
wait. A ROS-time activation/deactivation interrupts existing waits with `RosTimeSourceChanged`,
matching the rclcpp clock sleep's unsuccessful return on a time-source switch. Register a new
wait if the application wants to use the new clock domain. Zero/past deadlines complete without
creating a scheduler, timer or jump callback. Oversized relative durations and signed deadline
overflow return an argument error.

Dropping a `RosSleep` removes only its deadline immediately. `cancel()` explicitly cancels that
wait and its future returns `RosSleepCancelled`; it returns false after completion or interruption
has committed. Reused slots have generation keys, so cancellation through an old wait cannot
remove a new deadline. The future is Send and can outlive the node. Node destruction returns
`RCL_RET_NODE_INVALID`, and context shutdown returns `RCL_RET_NOT_INIT`. Completion, cancellation
and shutdown notifications run after releasing the deadline queue, clock and context locks.

Every node with waits owns at most one native timer and one jump callback. An indexed minimum
heap handles insertion/removal in O(log n) with a reusable slot array and no cancellation
tombstones. System-backed ROS time arms a steady native wake timer for the earliest ROS deadline.
Native wake periods are capped at one day, then rechecked against the original absolute ROS
target; this avoids native period overflow for targets near i64::MAX. Simulated time
cancels that timer and uses native clock/jump notifications. Preparing and dispatching each handle
at most 64 completions per pass; continuation remains runnable until due work is exhausted.
Stable waits avoid extra clock queries during unrelated traffic. The native timer is omitted
from waits when the queue is empty, and retained storage can be reused on later sleeps.

The native guard binding follows the current executor. Clock changes, head cancellation and
capacity changes notify the current wait; returned nodes rebind before their next spin. Removing
the scheduler's native timer serializes finalization with its owned native wake clock.
Nodes that never use these APIs allocate no sleep scheduler or additional native timer.

Run the example:

```sh
cargo run -p r2r --example ros_sleep
cargo test -p r2r --lib ros_sleep -- --test-threads=1
cargo test -p r2r --lib --release ros_sleep -- --test-threads=1
```

The example drives simulated time through 1, 2, 1 and 3 seconds using a wall timer and completes
one absolute ROS sleep after the backwards jump. Native tests cover real `/clock` delivery,
paused/negative/backwards time, clock-source interruption, current background and returned-node
guard bindings, independent cancellation, shutdown lock reentry, 512 simultaneous waits sharing
one native timer and bounded completion passes. A sorted oracle checks indexed heap mutation,
and 100000 canceled far-future waits keep one reusable slot and no dead entries. These establish
time/cancellation semantics and entity scaling; they are not application latency or throughput
leadership evidence. The queue and native timer modules can evolve independently for wider
scaling or native reactor integration without changing the public future contract.
