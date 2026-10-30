# Callback group execution architecture

The remaining execution goal includes real same-Node callback concurrency,
mutually exclusive and reentrant groups, convenient native factories, complete
shutdown/ownership recovery, and measured scaling against equivalent rclcpp
applications. The implementation includes native grouped subscriptions, services, timers and
owned task completion. [Recorded validation](../validation/callback-groups-2026-10-30/README.md)
covers the predecessor exclusive groups. Bounded reentrant groups add shared
handlers and independent invocation lanes. Independent performance comparisons
remain required.

## Ownership and scheduling

A fixed worker pool shares a single native Executor job and callback group
jobs. The native job owns the entire Executor value, including every Node and
the reusable native wait set. A worker takes that owned value from its slot,
releases the slot mutex, waits/dispatches, and returns ownership before making
the native job runnable again. Other workers execute only independently owned
callback inputs. No two workers borrow the Node or register the same entity in
simultaneous wait sets.

Group jobs use the same ownership transfer. A mutually exclusive group has one
slot, so its handlers can migrate between workers while retaining serial
execution. Slot state transitions are idle, queued, running, notified while
running, and finished. Notifications during execution coalesce into one future
ready token. The ready ring has one slot per job; it cannot grow with message
volume. Wakers and boxed job values are allocated during scheduler construction.
Invocation creates no job box or replacement waker allocation.

A reentrant group expands to a fixed number of owned lane slots. Each lane
holds independent receiver futures around a shared Fn + Send + Sync callback.
The group cancellation flag and source associations remain in retained group
metadata. Recovery refills lanes at their original indices and reconstructs
mixed exclusive/reentrant groups in their original order. Actual workers are
bounded by the configured budget and the number of nonempty executable lanes.
A source uses one native producer and a multi-consumer ArrayQueue; receiver
wakers are allocated once, and last-receiver retirement closes the source.
The single-consumer path keeps its inline AtomicWaker.

Callback wakers are primed before native waiting, including the one-worker
configuration. Ready publication unparks registered workers with persistent
thread tokens and interrupts the native wait through its cached observer guard.
This lets the native wait thread execute ready callback work when the other
workers are occupied. The ready ring and source rings are bounded ArrayQueues;
neither queue holds a lock during application code or payload retirement.
Slot mutexes cover ownership transfer only.

The wake set is private. Current broadcast unparking is correct even if
publication races an empty check or precedes parking. Allocation/resource and
scaling evidence must guide any replacement with targeted idle-worker wakeups;
an unsynchronized idle counter must not reintroduce a lost-wake window.

## Native input boundaries

Native sources retain sole RCL endpoint ownership in the Node. Subscription
sources take independently initialized `NativeMsg` containers into a fixed
source ring. A worker constructs `NativeMessageRef` only for the invocation,
then releases the owned container. Typed adapters decode on the callback worker.
The native path must preserve zero owned Rust decoding and must not assume
middleware reuse of populated dynamic buffers.

Source capacity is checked before a native take. Full or in-flight sources
must be omitted from native waiting until capacity returns, with a persistent
capacity-wait handshake and the current owner's guard. Returning capacity
must not leave an unread DDS endpoint spinning indefinitely. A rejected owned
input is retained or returned intact; buffer pressure never silently discards
a taken sample.

Service requests retain native sequence/writer metadata and independently
owned request storage. Each exclusive handler or reentrant lane retains its own native response buffer.
Response send and endpoint finalization need a shared, synchronized native
lifetime boundary; application handlers run outside that boundary's mutex.
Node destruction closes pending inputs and prevents use of a finalized native
service. Existing native metadata initialization and distribution conditions
remain required.

Timer work needs a stable native lifetime, at most one selected invocation,
and safe clock/advancement synchronization. Reentrant lanes release selection
after synchronized advancement, before entering shared user work; exclusive
handlers release after their body. One-shot cancellation occurs before release. Native advancement and timestamp
metadata must correspond to the selected invocation. Managed timers retain
their generation permit before advancement and user invocation, rejecting
obsolete activation without consuming one-shot readiness. Endpoint finalization
must wait for short native operations and release locks before user code.
Audit the native SDK thread-safety contracts before implementing shared timer
or service storage; native structs need stable interior-mutable storage where
RCL can concurrently read them.

Factory preparation must be transactional: invalid capacity or native creation
failure leaves no group registration or endpoint. Native sources and group
membership need stable identities, and the executor must reject sources whose
group/Node ownership is missing. Endpoint queries, graph/matching observers,
QoS events, cleanup wakes and removed-node IDs retain their existing contracts.

## Completion and acceptance

The owned public task must cancel peers directly after errors/panic, wake a
native wait before application polling, collect every worker, and recover all
Node/group state on normal stop. Drop detaches without blocking the runtime.
Control handles remain weak, cancellation persists until reset, and queued
inputs/handler state remain valid for stopped reuse. Cancellation remains
cooperative for the handler already executing.

The final 30-case Jazzy focused gate covers input bounds/retirement, current
owner wakes, same-Node progress, group exclusion, ownership/reuse, service
sequence/storage identity and finalization, invocation timestamps and lifecycle
epochs. Native Humble passes 29 cases and Kilted passes 30 under both RMWs.
The complete example checks 32 native forwards and RPC responses during a blocked
handler with stopped reuse. Full Jazzy suites pass 752 library cases; the no-ROS
gate verifies 58 public documentation artifacts and 226 methods.

Details, raw logs and source hashes are in the recorded validation. The predecessor isolation
prerequisite failed external contention before measurement.
live configuration, a native runtime reactor and allocation/resource scaling
with CPU-pinned interleaved Rust/rclcpp application comparisons remain required
by the complete goal.

The [reentrant validation](../validation/reentrant-groups-2026-10-30/README.md)
adds nineteen cases and retains all predecessor gates. Final Jazzy module gates
execute 49 cases in debug/release under each RMW; workspace and optimized full
library suites execute 771 cases. Humble executes 47 module/139 executor cases,
and Kilted executes 49/142 under both RMWs. The true no-ROS gate verifies 59
public documentation artifacts and 257 methods. Independent source/SDK hashes
and compressed logs accompany these results.
