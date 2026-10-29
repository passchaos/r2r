# Batched action goal state changes

ROS action status messages are complete arrays of every retained goal. Calling the ordinary
accept, execute or terminal method for every goal publishes another complete reliable array each
time. A concurrent window can therefore spend more work on repeated status snapshots than on its
feedback or result payloads.

r2r exposes explicit typed batching without changing any existing single-goal behavior:

```rust
let accepted = r2r::ActionServerGoalRequest::accept_batch(requests)?;
let (mut goals, _cancel_streams): (Vec<_>, Vec<_>) = accepted.into_iter().unzip();

// Publish feedback only after clients have installed their observers.
for goal in &goals {
    goal.publish_feedback(make_feedback(&goal.goal))?;
}

let results = goals.iter().map(|goal| make_result(&goal.goal)).collect();
r2r::ActionServerGoal::succeed_batch(&mut goals, results)?;
```

`accept_batch` accepts and starts all requests, then publishes one Executing snapshot.
`accept_deferred_batch` publishes one Accepted snapshot; `execute_batch` later publishes one
Executing snapshot. `succeed_batch`, `abort_batch` and `cancel_batch` prepare every result before
locking the server, transition the group, retain and deliver every per-goal result, publish one
terminal snapshot and notify the native expiry timer once.

Every slice must contain unique UUIDs from one server, and terminal result counts must match the
goal count. Empty slices are no-ops. If a native transition fails after a prefix changed state,
the changed prefix still gets a final status snapshot before the first error is returned. Result,
status and expiry-notification errors retain first-error reporting while later required cleanup is
still attempted. Existing `accept`, `accept_deferred`, `execute`, `succeed`, `abort` and `cancel`
keep their source and runtime behavior.

## Choosing a slice size

Batch size controls reliable status traffic; it is not a hidden middleware switch. Larger slices
publish fewer complete arrays, but result retention also controls how large each array becomes.
Applications should benchmark their goal window, status QoS and retention policy under the RMWs
they deploy.

The qualified cross-RMW configuration uses eight active goals, slices of two and one-second result
retention. It passes a preselected 5% baseline gate under CycloneDDS and Fast DDS. With the same
4096-element, four-feedback-per-goal workload, complete Rust lifecycle p50/p95/p99 is 20–29% lower
than same-run rclcpp and throughput is 39–42% higher. Ten-second retention also passes under Fast
DDS, but the same slice has a CycloneDDS feedback-p99 regression and is retained as a rejected
boundary rather than generalized.

Run the complete local example without a hand-written spin loop:

```sh
cargo run -p r2r --example action_goal_batch
```

The [saved benchmark evidence](benchmarks/action-goal-batching-2026-10-29/README.md) includes raw
stage/lifecycle samples, all Rust/C++ directions, independent baseline gates and configuration
counterexamples.
