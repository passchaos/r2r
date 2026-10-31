# Native action feedback delivery

Generated typed action clients can opt into feedback backed directly by the ROS C message taken
from `rcl_action_take_feedback`:

```rust,no_run
# use futures::StreamExt;
# use r2r::example_interfaces::action::Fibonacci;
# async fn observe(client: &r2r::ActionClient<Fibonacci::Action>) -> r2r::Result<()> {
let (goal, result, mut feedback) = client
    .send_goal_request_native_feedback(Fibonacci::Goal { order: 16_384 })?
    .await?;
let sample = feedback
    .next()
    .await
    .ok_or(r2r::Error::RCL_RET_ACTION_CLIENT_INVALID)?;
assert_eq!(sample.goal_id(), goal.uuid);
assert_eq!(sample.feedback.sequence.size, 16_384);
drop(sample); // returns the native container to the client reuse slot
let _ = result.await?;
# Ok(())
# }
```

`ActionNativeFeedback<T>` owns a RAII lease, not an `&T` whose lifetime crosses a stream poll.
It dereferences read-only to the generated `T::FeedbackMessage` C struct and can also provide a
`NativeMessageRef`. No owned `T::Feedback`, dynamic Rust sequence or intermediate erased payload
is constructed on the generated fast path.

The ordinary `send_goal_request`, `ActionFeedbackStream<T>` and owned feedback semantics are
unchanged. `send_goal_request_native_feedback_with_options` retains the same result/feedback
tracking controls, and the two native timeout variants bound goal acceptance just like their
owned counterparts. Custom unsafe `WrappedActionTypeSupport` implementations must provide a
native UUID accessor and report native-delivery support; otherwise registration returns
`RCL_RET_UNSUPPORTED` before a goal request is sent.

## Ownership, buffering and shutdown

Each client keeps one reusable native feedback container. A delivered item holds that container
until drop. If several items are buffered or retained by the application, RCL takes use additional
containers; when leases return, one stays cached and displaced storage is finalized outside the
pool lock.

`FeedbackOverflowPolicy::Backpressure` remains lossless and pauses further native feedback ingress
when any goal cannot accept its next sample. `Coalesce` retains already-buffered items plus the
newest pending native lease per goal; replacing a pending item drops and recycles the superseded
lease without decoding it. Owned and native streams have independent internal queues but share the
same configured capacity and policy.

Dropping the native stream schedules the same bounded maintenance cleanup as an owned stream.
Rejected goals, result-request failures, terminal status without a live result observer and result
completion all remove and close the appropriate observer after releasing the action-client lock.
A native item already received by application code remains valid until that item itself is dropped.

The runnable [`action_native_feedback`](../r2r/examples/action_native_feedback.rs) example uses a
16,384-element feedback sequence without owned delivery. Nearby unit tests cover lease reuse,
Coalesce replacement, cancellation and terminal closure; the real-RCL oracle sends a 4096-element
sample and passes under both Jazzy CycloneDDS and Fast DDS. Application-level allocation and
latency comparison against rclcpp remains a separate isolated benchmark requirement.

A Linux/glibc allocation stage exercises the final typed completion, channel send/receive and
payload validation with the same reusable preinitialized 4096-element native message. Across five
repetitions of 10,000 deliveries, owned delivery records exactly 50,000 allocations/frees and
165.60 MB requested, while native delivery records 30,000 and 1.68 MB. That is two fewer
allocation/free calls per message and 163.92 MB (98.99%) fewer requested bytes at this boundary.
The allocation counts are qualified deterministic evidence; recorded stage time is explicitly not
used as latency evidence.
