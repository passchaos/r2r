# Lifecycle-managed native timers

`LifecycleNode::create_timer` and `create_wall_timer` return `LifecycleTimer`.
The ROS-clock variant follows managed/simulated ROS time; the wall variant uses
a monotonic steady clock. Both native timers start paused unless created on an
already active node with autostart enabled.

| State/control | Native schedule and Rust ticks |
| --- | --- |
| Unconfigured, inactive or finalized | The native timer is canceled before the next native wait; `tick` remains pending |
| Successful activation | Start a fresh full period from the current native clock time |
| Deactivation/error recovery outside active | Gate delivery immediately, cancel before the next wait and discard queued old ticks |
| Failed deactivation returning active | Preserve the active native deadline |
| `cancel(&mut node)` | Cancel immediately; manual pause survives later node activation |
| `reset(&mut node)` | Discard queued ticks and enable a fresh period while active; remain canceled while inactive |

`TimerOptions::autostart(false)` requires an explicit reset before active-phase
ticks. Channel capacity uses the ordinary timer's bounded queue. Period changes
and native clock queries retain the ordinary timer API. Invalid/foreign node
controls report timer invalidity before changing the timer's requested state.
Deadline queries report cancellation while a new activation awaits native
reconciliation.

Reset changes the next deadline, but RCL's elapsed-time metadata is measured
since the previous actual timer call. It can include paused time. The tests
compare this behavior directly with an ordinary RCL timer; timing metadata and
clock-domain identities remain native.

Each activation/control change carries a generation. A deactivate/reactivate
pair issued without an intervening spin still invalidates old queued ticks.
Delivery waits until that generation has been applied. Cancellation precedes
queue draining, so the native producer cannot refill the queue during reset.
A tick that already observed activation may finish during concurrent
deactivation; already-delivered ticks are not retracted.

The public handle owns its inbox, while the node and lifecycle runtime hold weak
registrations. Dropping the handle schedules ordinary timer cleanup. `destroy`
finalizes the native timer immediately. Destroying the node wakes even an
inactive tick observer with `RCL_RET_TIMER_INVALID`. Ownership transfer through
`LifecycleNode::into_node` and the shared executor preserves lifecycle control.

Run the complete service-driven sensor example:

```sh
env -u CYCLONEDDS_URI cargo run -p r2r --example lifecycle_timer
```

It validates sixteen timer-driven publications across two active phases and
three paused phases using native lifecycle services and one shared executor.

Ordinary `Node` timers keep their native tick representation and channel path.
The node allocates its managed registry only when a managed timer is created;
native reconciliation locks an inbox only when its generation changes. No
additional timer, worker thread, periodic query or native guard is introduced.
The private state/inbox boundary can evolve independently of the public API.

The lifecycle factories now return a managed type instead of the previously
inherited ordinary timer. Use `Node::create_timer(&mut managed, period)` or the
corresponding wall/options method when an explicitly unmanaged timer is needed.
Call `timer.destroy(&mut node)` for managed teardown. The wrapper does not expose
an ordinary `Timer` through dereferencing or fields.

Focused gates:

```sh
env -u CYCLONEDDS_URI cargo test -p r2r --lib lifecycle::timer:: -- --test-threads=1
env -u CYCLONEDDS_URI cargo test -p r2r --lib timer -- --test-threads=1
python3 tests/lifecycle_publication_api.py
python3 tests/check_documentation.py
```
