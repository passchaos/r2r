# Optimized runtime validation

The required native release matrix for the project goal is Humble, Jazzy and
Kilted. Mentions below of unavailable Iron, Lyrical or Rolling evidence document
historical breadth only; those releases are not acceptance blockers.

## [Timer reset event streams](TIMER_RESET_EVENTS.md) (2026-10-31)

Ordinary `Timer`, synchronous `CallbackTimer`, exclusive/reentrant `GroupedTimer` and both lifecycle
wrappers expose one lossless runtime-agnostic reset-event stream. The first item includes resets
before registration, rapid resets coalesce with exact item/total counts, dropping an observer
preserves its consumed cursor for a replacement, and native destruction closes the stream only
after pending delivery. Lifecycle activation/reactivation and grouped generation reconciliation use
the same native reset boundary as explicit application calls.

Jazzy and Kilted use `rcl_timer_set_on_reset_callback`; Humble lacks that symbol in both RCL and
rclcpp, so r2r supplies the identical contract at its centralized successful-reset boundary. The C
trampoline performs only atomic accounting and a caught stream wake—no user closure or panic can
cross the ABI. Reset state is lazy, leaving timers that are never observed with zero additional heap
allocations.

Five focused reset/lifecycle/lazy-allocation cases plus one callback/grouped ownership case pass in
debug and release under both Jazzy RMWs. Humble/Fast DDS and Kilted/both RMWs pass the same six debug
cases, finite example and focused warning-denied Clippy. The complete Jazzy release library passes
815 cases with thirteen manual stages ignored under both RMWs; workspace tests and all-target Clippy
pass. The true no-ROS gate compiles all targets and verifies 84 public pages and 341 methods.
[Recorded evidence](validation/timer-reset-events-2026-10-31/README.md) binds final sources and logs.

## [Batched subscription streams](BATCH_SUBSCRIPTIONS.md) (2026-10-31)

`subscribe_batches` and `subscribe_native_batches` expose the Humble-through-Kilted
`rcl_take_sequence` API as ordinary async streams. Each nonempty item owns one ordered native batch
and matching RMW metadata. Typed batches recursively update generated Rust storage; a dropped batch
reuses all Vec/string/nested capacities, while retained peers and endpoint-outliving batches remain
independent. Channel capacity counts batches and dispatch admits one sequence take per subscription
per spin pass.

The first real CycloneDDS reuse test found that supplying the same dynamic generated C contents
to a second sequence take can finalize invalid nested storage. The safe implementation now recycles
RMW sequence arrays and failure-safely reinitializes generated native messages at stable outer
addresses before reuse; typed Rust capacity reuse remains intact. Focused burst cases cover zero-size rejection before endpoint creation, exact
four-message order, metadata alignment, consecutive sequence takes, retained peers, shrinking
4096-to-64-element typed storage and endpoint teardown. Both Jazzy RMWs pass the three focused cases
and the finite example. Creation-time content-filter coverage includes both new factories: Fast DDS
delivers only retained matching data, while CycloneDDS preserves the explicit unsupported boundary.
Humble/Fast DDS and Kilted/both RMWs pass the same focused cases and example. Isolated rclcpp
performance remains required before broader performance promotion.

A deterministic four-message reset stage compares the previous replacement boundary with in-place
generated reset. Per 10,000 batches, allocations/frees fall from 90,000 to 40,000 and requested
bytes fall by 3.2 MB. This removes five allocation/free calls per batch; diagnostic elapsed values
are not promoted as timing evidence.

Successful sequence takes now remember their actual length and refresh only that prefix; non-idle
errors still refresh the full capacity because RMW leaves it in an unknown valid state. A second
10,000-iteration stage models capacity 16 after one taken message: native reset transactions fall
from 160,000 to 10,000 while both paths refill the same 4096-element payload. The removed 150,000
init/fini calls are deterministic work accounting; elapsed values remain diagnostic.
Typed batch decode also removes a duplicate per-message `MessageInfo` construction: payload update
now uses a message-only iterator and converts each aligned metadata entry once. A thread-local test
counter fixes this invariant without adding production instrumentation.

The independent pipeline harness now selects `batch_typed` without changing its per-message RTT,
window, fanout or payload oracles. Jazzy/both RMWs each pass three cases in all four Rust/C++
directions, producing 12 records and 1200 samples against the complete current-source manifest.
The strict 1.0 rclcpp command stopped at the isolation preflight before measurement; these reports
are correctness/provenance evidence, not a performance claim. [Recorded pipeline evidence](validation/batch-typed-pipeline-2026-10-31/README.md)
retains both reports and the structured zero-measurement failure.

## Default typed publisher native preparation reuse (2026-10-31)

The ordinary Rust-style `Publisher<T>::publish(&T)` API now shares one native preparation cache
across cloned publisher handles. Checkout and return hold only the cache lock; overlapping calls
allocate independent containers, preserving concurrent conversion before the existing serialized
RCL call. A failed `try_copy_to_native` retires the checked-out native value, and the next publish
creates clean storage.

A real 4096→64→16-element RCL workflow verifies exact owned subscriber payloads, one container
across successful calls and clones, failure retirement, recovery, endpoint destruction and final
clone cleanup under Humble/Fast DDS, Jazzy/both RMWs and Kilted/both RMWs. Five deterministic
10,000-message repetitions reduce fresh native preparation from 20,000 allocations/frees and
328.24 MB requested bytes to zero after warmup: two allocation/free calls and 32,824 bytes removed
per message. The stage excludes RCL/DDS and does not promote its elapsed diagnostics.

## Typed client request native preparation reuse (2026-10-31)

`Client<T>::request` and `request_with_info` now share one native request cache across cloned Rust
handles. Conversion stays before the client mutex, overlapping requests use independent leases,
and a fallible copy error retires its container before `rcl_send_request` or response registration.

A real `AddTwoInts` workflow verifies exact responses, clone reuse, injected conversion failure,
recovery and deterministic endpoint/final-clone cleanup under the required Humble/Jazzy/Kilted RMW
matrix. Five deterministic 10,000-request repetitions use a `DescribeParameters` request with one
4096-byte name: fresh preparation records 40,000 allocations, 30,000 frees and 41.46 MB requested
bytes, while the warmed reusable path records zero for all three. This removes four allocations,
three frees and 4,146 requested bytes per request at a boundary that excludes discovery/RCL/DDS.

## Typed client response native reuse (2026-10-31)

Every typed client endpoint now retains one native response-take container shared by plain and
metadata-bearing futures. The native lease is held through the detached conversion closure, so
owned decode remains outside the client mutex, then returns before future delivery. The cache is
endpoint-owned and is released by explicit client destruction even if a stale Rust clone remains.

A real `AddTwoInts` workflow verifies exact plain and `ServiceResponseInfo` results, one native
container across both paths, positive sequence metadata, endpoint destruction and stale-clone
rejection across the required three-release RMW matrix. Five 10,000-response repetitions use a
4096-byte `SetBool` string: fresh fill/decode records 40,000 allocations, 30,000 frees and 82.26 MB
requested bytes versus reusable 10,000, 10,000 and 40.96 MB. Native reuse removes three
allocations, two frees and 4,130 bytes per response; the remaining allocation preserves owned Rust
output. The boundary excludes RCL/DDS.

## Typed service request/response native reuse (2026-10-31)

Each typed `Service<T>` now owns separate native request-take and response-preparation caches.
Request decoding stays in the detached delivery closure outside the service mutex and returns the
native lease before channel delivery. All requests and their clones share the response cache and
once-only response state; copy failure retires storage without preventing a clone retry.

A real `AddTwoInts` workflow verifies three exact roundtrips, request/response reuse, injected
response-copy failure, retry and deterministic service teardown under Humble/Fast DDS, Jazzy/both
RMWs and Kilted/both RMWs. Five 10,000-call repetitions model `SetBool` request decode plus a
4096-byte response: fresh storage records 40,000 allocations, 30,000 frees and 41.31 MB requested
bytes; warmed reuse records zero. This removes four allocations, three frees and 4,131 bytes per
call at a boundary excluding RCL/DDS/channel delivery.

## Typed action goal-request native reuse (2026-10-31)

Typed `ActionClient<T>` handles and clones now share one native SendGoal request slot. Goal-message
construction and copy remain before the action-client mutex; concurrent goal sends use independent
leases. Copy failure retires storage before `rcl_action_send_goal_request` and before any goal,
feedback or result state is registered.

A Fibonacci-compatible custom action client interoperates with the standard server and verifies
reuse across rejected goals and clones, injected copy failure, later recovery, endpoint destruction
and final clone cleanup under Humble/Fast DDS, Jazzy/both RMWs and Kilted/both RMWs. Five
10,000-goal repetitions reduce native preparation from 10,000 allocation/frees and 200 KB requested
bytes to zero. This fixed per-goal stage excludes RCL/DDS and does not promote elapsed diagnostics.

## Typed action status native reuse (2026-11-01)

Typed action clients now retain one endpoint-owned native `GoalStatusArray` container. The lease is
held through the existing detached direct-native decode, then returned before applying updates and
running goal cleanup. Public goal handles and status behavior remain unchanged.

A real Fibonacci workflow completes two accepted goals and verifies distinct successful status
takes, stable cached native address, succeeded results and stale-clone rejection across the required
three-release RMW matrix. Five 10,000-message repetitions fill/decode 128 statuses: fresh storage
records 30,000 allocations/frees and 57.84 MB requested bytes versus reusable 10,000 and 21.76 MB.
Reuse removes two allocations/frees and 3,608 bytes per status message; the remaining allocation is
the decoded update Vec. The stage excludes RCL/DDS.

## Typed action result-response native reuse (2026-11-01)

Typed action clients now retain one endpoint-owned native GetResult response container. The lease
crosses detached result decode and goal cleanup, then returns after constructing the independent
owned Rust result. Public result futures and goal status semantics remain unchanged.

A real Fibonacci workflow completes 4096- and 64-element results, requires one stable native
address, retains the first owned Vec while decoding the second, and rejects a stale clone after
endpoint destruction across the required three-release RMW matrix. Five 10,000-result repetitions
record fresh 30,000 allocations/frees and 328 MB requested bytes versus reused 10,000 and 163.84 MB.
Reuse removes two allocations/frees and 16,416 bytes per result; the remaining allocation is the
owned Rust result Vec. The boundary excludes RCL/DDS.

## Typed action result-request and goal-response native reuse (2026-11-01)

Typed `ActionClient<T>` clones now share one GetResult request-preparation slot, and the native
endpoint retains one SendGoal response container. Result-request copy failure retires the slot
before RCL send. Each response completion runs after releasing the client lock and before the next
bounded response take, returning its native lease before an idle probe or another response.

A real Fibonacci workflow verifies accepted goals across clones, stable request/response addresses,
injected result-request copy failure, successful retry and endpoint/last-handle cleanup. The
Humble/Fast DDS, Jazzy/both-RMW and Kilted/both-RMW matrix passes; complete Jazzy release suites each
report 825 passed with 22 manual stages ignored. Five 10,000-goal repetitions reduce the combined
native lifecycle boundary from 20,000 allocations/frees and 280 KB requested bytes to zero. The
stage excludes RCL/DDS and does not qualify elapsed diagnostics.

## [Reusable typed subscription samples](REUSABLE_TYPED_SUBSCRIPTIONS.md) (2026-10-31)

`WrappedTypesupport::update_from_native` gives handwritten messages a source-compatible replacement
default while generated messages recursively reuse primitive Vec, RosString/RosWString, fixed-array
and nested-message storage. `subscribe_reusable` couples one native take container and one decoded
Rust value pool to safe `ReusableSubscriptionSample<T>` delivery; retained samples stay independent
and valid after endpoint teardown. The default Rust-style `subscribe<T>` API remains unchanged and
now transparently returns its native decode container before delivering each independently owned
Rust message.

Standard-message oracles preserve primitive, nested-message and nested-string addresses across
shrinking updates; test_msgs-enabled environments additionally cover string/nested and wstring
sequence branches. Real 4096/128/64/32-element typed streams pass under the required three-release
RMW matrix. Five deterministic 10,000-delivery repetitions record fresh 30,000 allocations/frees
and 329.20 MB requested bytes versus reusable 20,000 and 1.60 MB: one allocation/free and about
99.51% requested bytes removed per sample at the native-to-Rust decode/channel boundary. The
default typed-path stage separately records fresh-native 50,000 allocations/frees and 656.72 MB
versus reused-native 30,000 and 328.48 MB per 10,000 owned messages, removing two native-container
allocation/free calls and about 49.98% of total requested bytes. Its real-RCL oracle retains both a
4096-element and a later 64-element result while observing one native-container construction. The
complete Jazzy release suite passes 822 cases with 20 manual stages ignored under each RMW. The
84-page/341-method no-ROS and workspace all-target Clippy gates pass; both stages exclude DDS/RCL
timing, so rclcpp latency remains an isolated follow-up requirement.

## [Reusable native subscription samples](REUSABLE_NATIVE_SUBSCRIPTIONS.md) (2026-10-31)

`subscribe_native_reusable` delivers async RAII `NativeSubscriptionSample<T>` items backed by a
one-slot native container pool. A retained sample owns its storage exclusively; concurrent samples
allocate distinct containers, the most recently returned value becomes the cache, and samples stay
valid after endpoint teardown. Existing `subscribe_native` behavior is unchanged. The shared lease
implementation also retains the previously validated native action-feedback semantics.

Real 4096/128/64/32-element `UInt64MultiArray` cases pass under Jazzy/both RMWs,
Humble/Fast DDS and Kilted/both RMWs, proving outer address reuse, retained-sample independence and
endpoint-outliving access. Five deterministic 10,000-delivery allocation repetitions record fresh
40,000 allocations/frees and 329.44 MB requested bytes versus reusable 20,000 and 1.36 MB: two
allocation/free calls and about 99.59% requested bytes removed per sample at the no-RCL/DDS fill/
channel boundary. Focused/workspace Clippy and the 84-page/341-method no-ROS gate pass. End-to-end
rclcpp latency remains a separate isolated requirement.

## [Content-filtered subscriptions](CONTENT_FILTERED_SUBSCRIPTIONS.md) (2026-10-31)

`Subscription<T>`, `NativeCallbackSubscription` and `GroupedSubscription` now expose one
middleware-side apply/query/replace/clear contract across typed/native/untyped/raw streams, scoped
native callbacks and exclusive/reentrant groups. Foreign Node ownership, interior NUL strings and
the native 100-parameter bound fail before mutation; unsupported RMWs return
`RCL_RET_UNSUPPORTED` without a local decode-and-filter fallback.

Six focused cases, including typed/native batch creation filtering, and a finite
creation/apply/replace/clear example pass in debug and release under both Jazzy RMWs,
Humble/Fast DDS and Kilted/both RMWs. Fast DDS filters transient-local history
published before subscription creation and performs exact native `Int32` runtime replacement in all
three distributions. Cyclone DDS silently ignores the creation option natively; r2r detects the
disabled CFT state, finalizes the endpoint and reports unsupported under Jazzy and Kilted. Focused
and workspace all-target Clippy pass with warnings denied; the no-ROS gate verifies 84 public pages
and 341 methods. [Recorded evidence](validation/content-filtered-subscriptions-2026-10-31/README.md)
binds final sources, compressed logs and the per-RMW support matrix.

## [Transactional live parameters](PARAMETER_CALLBACKS.md) (2026-10-30)

Local and standard ROS parameter-service updates share ordered pre/on/post
callbacks and one atomic transaction core. Pre callbacks transform the final
batch, validation callbacks reject before any derived/runtime value changes, and
post callbacks observe only committed values after internal locks are released.
RAII handles unregister one callback; same-node recursive mutation returns an
explicit error. Declaration, undeclaration, duplicate normalization, descriptors,
events and simulated-time changes retain documented phase boundaries. The
`parameter_callbacks` example exposes the same path to `ros2 param set`.
[Recorded gates](validation/parameter-callbacks-2026-10-30/README.md) retain the
65 focused native cases, example/Clippy result and the current 67-page/276-method
no-ROS public API contract.

## [Graceful process shutdown](GRACEFUL_SHUTDOWN.md) (2026-10-30)

`Context::install_signal_handler` provides explicit rclcpp-style SIGINT/SIGTERM
shutdown without performing RCL calls, locking or allocation inside the POSIX
handler. A dedicated process thread retargets the current shared context and
uses the existing idempotent shutdown/guard path. Native tests raise real
signals, interrupt an empty background executor, recover its Node and verify a
fresh context receives a later signal. `Context::wait_for_shutdown` adds a fused,
weakly registered Future for manual, signal and final-owner shutdown without
polling or retaining Context. The runnable example and no-ROS public API gate
cover the user-facing path. Humble/Fast DDS and Kilted/both RMWs pass
the same nine cases plus example Clippy. Complete optimized Jazzy suites pass
773/8 under both RMWs. The first broad CycloneDDS run also exposed and repaired
an action-QoS test's immediate asynchronous graph-cache query; it now uses the
existing bounded graph-change protocol and both complete reruns pass.

## [Native entity scaling](ENTITY_SCALING.md) (2026-10-30)

The former fixed 128-action-endpoint benchmark now defines 35 zero-to-128
entity cases. It covers idle action clients/servers, a mixed
subscription/service/client/action/timer wait set and service/action/publisher/
subscription waiter registration-cancellation cleanup. The optimized benchmark
constructs every topology and passes warning-denied Clippy under the single-core,
10 GiB gate. Host load exceeded 20, so no timing is promoted and the production
Vec+identity-index+swap-remove registries remain unchanged pending isolated
dual-RMW curves and process-wide allocation/RSS evidence.

## Lazy native type-support loading (2026-10-30)

Generated interface functions now resolve their package's generator, C type-
support or introspection library on first use and retain successful handles for
the process lifetime. Safe endpoint and owned-message factories propagate a
new interface-specific loader error before native registration; owned allocation
resolves its matching finalizer first. Custom type-support implementations remain
source-compatible through default fallible adapters, and runtime-typed message,
service and action maps preserve their complete generated surface.

The final-source Jazzy callback-group binary drops from 86 to 12 direct ELF
dependencies and from 74 to zero directly needed interface libraries. A restricted-
library run completes the typed service protocol where the eager baseline fails
in the loader on an unused interface package. Four interleaved same-input resource
runs record median application VmHWM reductions from 19,454 to 14,652 KiB and from
19,500 to 14,724 KiB in the two selected exclusive/reentrant cases (24.5–24.7%).
These bounded diagnostics are not timing-qualified or a universal RSS claim.

Real shared-library fixture tests cover retry, concurrent first lookup, native
allocation/finalization and retained metadata. Missing-support endpoint tests
verify errors before publisher or callback-group registration. Jazzy checks cover
typed/untyped messages, services, actions, type descriptions and the complete
optimized suite under both RMWs; the workspace, warning-denied Clippy and true
no-ROS all-target gate remain required. The no-ROS gate found and fixed an initial
unconditional type-description finalizer lookup and now verifies 73 public API
artifacts plus 317 required methods. Humble/Fast DDS and Kilted/both-RMW
compatibility gates retain generated, typed/untyped and callback coverage.

[Complete lazy-loading evidence](validation/lazy-typesupport-2026-10-30/README.md)
records the exact boundaries, artifacts and reproduction commands. Startup timing
and wider application/resource comparisons remain open.

## Native delivered action feedback (2026-10-30)

Generated typed clients can opt into `ActionNativeFeedbackStream`, whose items own the native
message lease taken by RCL and recycle it on drop without constructing `T::Feedback`. The ordinary
owned stream is unchanged. Independent native queues preserve Backpressure/Coalesce semantics;
goal rejection, cancellation, terminal status and result completion close both registration kinds
through post-lock cleanup. Custom action wrappers without a native identity accessor return
`RCL_RET_UNSUPPORTED` before sending a goal.

Jazzy/both RMWs pass 86 focused action-client cases with seven manual stages ignored, plus a real
4096-element RCL workflow and a 16K-element example. Humble/Fast DDS and Kilted/both RMWs pass 11
focused cases, the real workflow, example and warnings-denied focused Clippy. Workspace all-target
Clippy passes, and the current no-ROS gate verifies 84 pages/341 methods. Final-source dual-RMW four-
direction and same-binary owned/native protocol smokes pass source-aware verification. The formal
five-repetition owned/native and rclcpp gate stopped before measurement at load1 80.47 with five
Zig processes, so no latency or throughput claim is promoted.

[Recorded native-feedback evidence](validation/native-action-feedback-2026-10-30/README.md)
retains exact source, binary, report, sample and compressed-log identities.

The final 4096-element typed delivery boundary has five deterministic 10,000-message allocation
repetitions. Owned delivery records 50,000 allocations/frees and 165.60 MB requested; native
delivery records 30,000 and 1.68 MB. This qualifies the allocation/copy boundary only. Its elapsed
samples remain diagnostic and cannot replace the isolation-gated application comparison.

## Derived parameter descriptors (2026-10-30)

`#[derive(RosParams)]` leaf fields accept declarative read-only, additional-constraint and
integer/floating range metadata. The macro rejects unknown, duplicate, incomplete and conflicting
options. Built-in leaf implementations fill the ROS name/type while explicit Node descriptors
retain precedence; the new trait hook has a default so hand-written implementations remain source
compatible.

Handler setup registers descriptors and validates matching CLI/YAML overrides before mutating the
provider. The real-RCL test checks valid override application, invalid override rollback, local
updates, read-only/range/step rejection, exact `describe_parameters` contents and per-entry
`set_parameters` results. Jazzy debug and release run 75 parameter-related cases under both RMWs;
Humble/Fast DDS and Kilted/both RMWs run the focused service case, macro parser suite, nested
example and warnings-denied Clippy. The current no-ROS all-target gate verifies 84 pages and 341 methods.

[Recorded derive-descriptor evidence](validation/derived-parameter-descriptors-2026-10-30/README.md)
binds final sources and compressed logs.

## Runtime-agnostic callback-group tasks (2026-10-30)

`CallbackGroup::spawn_task` retains arbitrary `Send` futures under the exclusive group scheduler
token. `ReentrantCallbackGroup::spawn_task` deterministically chooses the least-populated stable
lane, so typed action server/client and external futures share the configured group-wide bound
without requiring `Sync`, migration or an action-specific thread. The native executor retains RCL
wait/take ownership. The RAII task handle supports explicit cancellation or detach; worker-side
retirement drops captured endpoints. Executor cancellation pauses rather than drops tasks, and
`reset_cancel` explicitly reawakens their FuturesUnordered child wakers before restart.

Eight task ownership/balancing/error/bookkeeping cases, two bounded overlap/panic cases and four real
exclusive/reentrant action/stop-reset cases pass in debug and release under both Jazzy RMWs.
Humble/Fast DDS and Kilted/both RMWs pass the same focused cases, both complete examples and
warnings-denied Clippy. The no-ROS gate compiles all targets, executes both non-Send ownership
counterexamples and verifies 84 pages/341 methods. Isolated rclcpp action-group performance remains
separate work.

[Recorded callback-group task evidence](validation/callback-group-tasks-2026-10-30/README.md)
binds sources and compressed logs.

A final-source cold-process protocol now flushes a readiness marker after native
self-discovery and separately records exec-to-ready, main-to-ready, one exact
exchange and exec-to-exit. Its four-process smoke passes provenance, response and
trace verification; the host had active competing compilers, so these timings are
protocol evidence only. Formal startup comparison still requires thirty
interleaved runs and the unchanged continuous-isolation gate under each Jazzy RMW.

## Callback group application comparison (2026-10-30)

[Native callback-group applications](../tools/bench_ros/CALLBACK_GROUPS.md) now
compare one service/client Node in independent Rust/rclcpp binaries with identical
worker budgets, request windows, compute inputs and per-request oracles. An
explicit overlap challenge precedes warmup. Captured client/server request IDs,
independent returned-value checking and stage intervals enforce actual work and
concurrency. The optional probe measures complete-process allocations/CPU; memory
uses application VmHWM rather than a potentially inherited ru_maxrss floor.

All five Jazzy/Humble/Kilted RMW environments pass 20 checked application runs
and 1280 request traces each, plus benchmark Clippy. Ninety-four performance-tool
cases and both probe oracles pass; Jazzy all-target Clippy and true no-ROS
all-target/public-documentation gates pass. An original C++ four-worker timeout
remains unexplained after larger and repeated-lifecycle diagnostics; its captured
failure is retained. Final isolation failed external compilation before A/B,
so smoke records establish no C++ performance advantage.

[Complete comparison evidence](validation/callback-group-comparison-2026-10-30/README.md)
records corrected memory semantics, raw traces, final source/binary hashes and
the ELF dependency audit that motivated the completed lazy-loading slice above.

## Bounded reentrant callback groups (2026-10-30)

[Reentrant groups](CALLBACK_GROUPS.md#bounded-reentrant-groups) allow shared
Fn + Send + Sync subscription/service/timer bodies to overlap within a group-wide
invocation bound. One native owner remains responsible for Node/wait sets.
Every lane owns its inputs and reusable service response buffer. Timer selection
is released after synchronized advancement, with one-shot and lifecycle
cancellation applied before another lane can select the next deadline.

The nineteen new cases cover same-source worker concurrency, independent native
reply storage and exact request identities, finalization during two blocked
handlers, bounded timer overlap, native timestamps, stale lifecycle selections,
cancel/restart, close/publication retirement, mixed group recovery and panic
cleanup. The final Jazzy module gate executes 49 cases under both RMWs in debug
and release; workspace and both optimized full suites pass 771 main-library cases
with eight existing manual measurements ignored. All 27 Rustdoc cases, both
complete examples and warning-denied all-target Clippy pass.

Native Humble/Fast DDS passes 47 module/139 executor cases; Kilted/both RMWs pass
49/142. Examples, public compiler boundaries and Clippy pass. A true no-ROS gate
compiles all targets and verifies 59 public documentation objects and 257 methods.
That environment supports compilation through saved declarations; linking the
complete unit-test binary still requires ROS native symbols. Native-only helper
warnings in doc-only test compilation do not affect the native Clippy gates.

[Reentrant evidence](validation/reentrant-groups-2026-10-30/README.md) retains exact
counts, source/SDK hashes and raw compressed logs. Independent isolated C++
performance comparisons remain required for the full goal.

## Same-Node callback groups (2026-10-30)

[Callback groups](CALLBACK_GROUPS.md) run scoped native subscriptions, typed/native
services and wall/ROS timers across mutually exclusive group jobs within one
total native/callback worker budget. One job retains exclusive native Node/wait
ownership. Stop returns handler/Node state; failures/panic cancel peers before
application polling. Lifecycle generation policy remains on timer storage.

Jazzy passes 30 focused cases under each RMW in debug and release, the complete
two-worker forwarding/RPC example, 752 main-library cases in the workspace and
both optimized suites, 25 Rustdoc cases and all-target warning-denied Clippy.
Native Humble/Fast DDS passes 29 optimized focused and 121 executor cases;
Kilted/CycloneDDS plus Fast DDS each pass 30/123 cases. Examples, ownership
Rustdoc and Clippy pass in those native environments. The no-ROS gate verifies
58 public documentation artifacts and 226 methods.

[Recorded evidence](validation/callback-groups-2026-10-30/README.md) includes native
sequence/storage, finalized-service send rejection, exact invocation timestamps,
lifecycle selection epochs and input retirement/backpressure oracles. The
performance isolation prerequisite failed external contention before measurement;
At that predecessor checkpoint, reentrant groups and isolated representative C++ comparisons remained open.

r2r validates runtime behavior with debug assertions disabled as well as with the default
test profile. The Jazzy quality job runs the complete library suite with `cargo test --release`.
Version-specific and middleware integration gates still cover the supported Humble-and-newer
matrix.

## Parallel native Node groups (2026-10-30)

[`ParallelExecutor`](PARALLEL_EXECUTION.md) composes owned background executors
over a fixed, explicitly configured native worker budget. Each group owns its
Nodes, wait set and wake guard exclusively. Long synchronous callbacks remain
serial within a group while other groups make native progress. Stop returns
stable Node IDs and placement; errors/panics cancel peers before application
polling and collect their cleanup before returning a configured worker index.

Thirteen focused tests establish native progress/thread identities, distinct
reused wait/guard storage, stable ownership, pre-start cancellation, skipped
empty groups, response identities, error/drop/panic cleanup, empty/populated
shutdown and standard lifecycle services plus native simulated-time deadlines.
The current-thread example validates 32 control RPCs during a blocked callback
and subsequent Node reuse. [Recorded gates](validation/parallel-executor-2026-10-30/README.md)
track debug/release, native distribution/RMW, public Rustdoc ownership examples,
no-ROS documentation and necessary full-library/workspace validation separately
from the remaining isolated multithreaded performance matrix.

Final Jazzy debug workspace and both optimized RMW library suites pass 722
main-library tests each, with eight existing manual measurements ignored and
22 Rustdoc cases passing. Native Humble/Fast DDS and Kilted/CycloneDDS plus
Fast DDS each pass 13 optimized parallel cases, 92/93 executor-family cases,
the complete example, ownership Rustdoc and all-target Clippy. The no-ROS gate
verifies 46 public documentation artifacts and 150 methods. Iron, Lyrical and
Rolling remain supported CI entries without new local runtime evidence here.

## Release-only registration failure fixed on 2026-10-27

The first optimized, independent-process pub/sub roundtrip probe failed while querying the
subscription's matched publishers. Entity-map registrations had been written inside
`debug_assert!`, so Cargo's default release profile removed the mutations along with the
assertion. Debug tests passed because those expressions were evaluated there.

The correction evaluates state mutations before debug assertions in all fourteen affected
locations:

* Subscription registration/rebuild, service, timer, publisher and action-server registration.
* Removal of the active parameter-transaction marker on guard destruction and initialization
  of provider-owned parameter names.
* Typed and untyped action reverse-map cleanup when goal responses or result requests finish.

The assertions still check the previous/removed values in debug builds. Their absence cannot
change registration, cleanup or provider behavior in optimized builds. The fix preserves the
existing lookup structures and synchronization.

## Gates

With a sourced ROS environment, run the smallest directly relevant gates before broader tests:

```sh
cargo test -p r2r --lib --release indexes -- --test-threads=1
cargo test -p r2r --lib --release parameter_transaction_guard -- --test-threads=1
cargo test -p r2r --lib --release provider -- --test-threads=1
cargo test -p r2r --lib --release -- --test-threads=1
```

Filtered tests must execute nonzero relevant cases. The new typed/untyped action tests require
both reverse indexes to be empty after out-of-order response/request cleanup. Existing entity
tests cover registration, targeted destruction, swap-removal, index rebuilding and automatic
cleanup. Parameter tests verify repeated operations after guard destruction and provider-backed
reads. Full optimized Jazzy library validation now passes 445 tests, with two manual benchmarks ignored.
Humble and Kilted each pass nine optimized index cases and the parameter-transaction gate.

The [future-execution pub/sub boundary](FUTURE_EXECUTION.md) adds eight cases for graph-driven
matching and bounded pending ACK queries. All 18 executor cases pass under both Jazzy RMWs and
native Humble/Kilted; the complete payload/timer/ACK example runs in each environment. Release
validation exposed two discovery tests querying local graph caches before their asynchronous
update. The [graph tests](../r2r/src/nodes/tests/graph.rs) now register a graph waiter before
checking visibility and use a two-second overall deadline. Exact type, endpoint, QoS and count
assertions remain after discovery. These three cases plus graph broadcast/cancellation pass
under both Jazzy RMWs and native Humble/Kilted.

The [multi-node execution gates](MULTI_NODE_EXECUTION.md) add eleven cases for one shared native
wait, disjoint ordinary/action readiness spans, node removal and endpoint cleanup, native ACK
cadence, external/non-Send futures and shutdown. They pass under both Jazzy RMWs and native
Humble/Kilted. The refactored single-node path retains all eighteen future-execution cases and
the complete optimized library suite.

The [end-to-end roundtrip harness](../tools/bench_ros/TRANSPORT.md) exercises native DDS transport
through optimized application binaries, including both Rust/C++ directions. This boundary is
useful for catching profile-dependent failures that compilation or debug-only unit tests miss.

Keep state-changing expressions outside every `debug_assert!`, `debug_assert_eq!` and
`debug_assert_ne!`. Assertions should inspect results already produced by normal execution.

## Native lifecycle (managed node) support added on 2026-10-28

The lifecycle feature ships in two layers: a map-only safe wrapper over
`rcl_lifecycle_state_machine_t` (`LifecycleStateMachine`, no node/DDS entities)
and `LifecycleNode`, which owns a regular `Node` plus one machine and serves
the five standard lifecycle services and the `~/transition_event` publisher
as native r2r entities. Transition semantics match `rclcpp_lifecycle`:
external command edge, user callback on the intermediate state, generic
`transition_success`/`transition_failure`/`transition_error` edge, plus the
error-processing callback; label-based requests take precedence over ids.

Targeted gates (debug and `--release`, nonzero matched cases each):

```sh
cargo test -p r2r --lib lifecycle -- --test-threads=1
cargo test -p r2r --lib --release lifecycle -- --test-threads=1
```

29 lifecycle cases execute: 13 state-machine cases (full 11-state/25-edge graph
oracle taken from the installed Jazzy rcl via a C probe, happy path,
failure/error/shutdown edges, invalid triggers), 12 managed-node cases over
native r2r service clients/subscribers including the unified `Executor`, and
4 `LifecyclePublisher` cases (gating until active, manual toggle, dead-entity
pruning, service-driven activation over DDS). At that checkpoint, full debug
and release suites passed 488 tests each. `LifecycleNode::create_publisher` returns a gated
publisher that is activated on reaching `active` and deactivated on leaving it
(weak registry prunes dropped publishers on each transition).
External interoperability was verified with `ros2 lifecycle get/list/set`
against a running r2r node: configure, activate, an invalid command rejected
with the state unchanged, deactivate, cleanup and shutdown all returned the
standard `unconfigured/inactive/active/finalized` states.

Note: the local Jazzy shell exports a stale `CYCLONEDDS_URI=/nav_env/...`;
DDS-backed gates run with that variable unset. Map-only state-machine cases
need no DDS at all.

## Distribution boundaries after node module extraction (2026-10-28)

Native Humble validation found two extraction regressions: the `nodes` module
re-exported Jazzy's `TimerCallInfo` without its Humble/Iron exclusion, and the
action-name resolver became private to the action-client module even though
the action-server factory also uses it. The export now follows the type's
distribution condition and the resolver is visible inside `crate::nodes`.
The test-only graph string helper is also imported only where its caller exists.
Humble passes the action-name case, seven timer cases and library/test/example
Clippy with warnings denied. The native Jazzy workspace suite passes with these
boundaries restored. Iron uses the same guarded APIs; its local runtime matrix
remains unverified.

## Lifecycle publication boundary (2026-10-28)

A new native-message regression case failed against `9780902`: an
unconfigured managed publisher delivered a message through `publish_native`.
The wrapper's unrestricted `Deref<Target = Publisher<T>>` also allowed safe
cloning/coercion to the ordinary publisher and unguarded loan publication.
[Lifecycle publication](LIFECYCLE_PUBLICATION.md) now explicitly gates typed,
native and loaned publication, delegates endpoint queries/waits, and exposes
native loan fields without exposing the underlying publisher or loan object.

Nine publisher cases cover typed and native delivery/drop behavior, preserved
native storage, shared clone activation, failure/error recovery, creation while
active, stale native endpoints, the configurable managed factory, immediate
destruction and foreign-node isolation, loan ownership/errors, returning gated
and unpublished loans, and activation changes between borrowing and publishing.
The complete lifecycle filter executes 33 cases. The native compiler gate
executes seven public API checks: a usable example, rejected publisher
coercion/clone/private-field escapes, rejected loan coercion/private-field
escapes, and rejection of a loan outliving its node.

| Environment | Runtime evidence | Loan path |
| --- | --- | --- |
| Jazzy / CycloneDDS | Workspace debug suite and full optimized library suite: 493 r2r cases pass in each, two manual benchmarks ignored; optimized complete example | Unsupported loaning and foreign-node errors verified |
| Jazzy / Fast DDS | 33 lifecycle cases in debug and release; optimized complete example | Actual loan publish, gated return, manual activation changes and reuse verified |
| Humble / native image default RMW | 33 lifecycle cases, action-name case, seven timer cases; compiler gate, complete example, library/test/example Clippy | Unsupported loaning and foreign-node errors verified |
| Kilted / native image default RMW | 33 optimized lifecycle cases; compiler gate, optimized complete example, library/test/example Clippy | Actual loan publish, gated return, manual activation changes and reuse verified |

The example transfers the lifecycle node into the shared executor and changes
state through native services. It checks exact typed/native/loan payloads,
reliable ACK progress and bounded absence of extra publications in all five
phases. Iron, Lyrical and Rolling are still outside this local runtime evidence.
The compiler gate is registered in Jazzy quality CI. These results establish
the publication contract and retained native ownership, not a transport
performance advantage over C++.

Native Jazzy also passes all-target workspace Clippy with warnings denied,
formatting and the doc-only workspace compilation gate. The existing
`lifecycle_node` example now names its transition function-pointer type so it
passes the same Clippy gate without suppressing the complexity lint.

## Documentation snapshot and optional native metadata (2026-10-28)

The [documentation gate](DOCUMENTATION_BUILDS.md) now compiles all workspace
targets and verifies ten API pages and fourteen methods in Ubuntu 24.04 with
no `/opt/ros` installation. Its explicit Jazzy snapshot contains 23 packages,
saved interface availability, lifecycle and service/action events. Four
snapshot transaction cases verify preparation failure, partial-install rollback,
complete replacement and retention of previously documented packages. All 36
binding files have identical SHA-256 after independent regeneration.

Native Kilted validation exposed an unrelated unsafe metadata assumption:
Fast DDS does not fill response writer-GUID bytes. [Native metadata oracles](NATIVE_SERVICE_METADATA.md)
with three initial byte patterns verify that omission on Jazzy/Humble/Kilted and
full response-identity writes under Jazzy/CycloneDDS. Service metadata storage
now starts initialized; action clients read only their mandatory sequence field.
The typed/untyped integration case compares returned identities with a direct
RCL exchange through the same client instead of assuming a nonzero identity.

Final native debug/release Jazzy suites pass 495 library cases under both RMWs,
with two manual benchmarks ignored. The full Jazzy workspace suite also passes.
Native Humble and Kilted library suites pass 486 and 499 cases respectively,
with two ignored in each; their full-target workspace Clippy gates pass.
Jazzy full-target Clippy, formatting, two metadata counterexamples, 146 action
cases and 55 service cases pass. Humble's now-version-specific `MaybeUninit`
import remains available only where its native client-GID query needs it.

[The corrected service comparison](benchmarks/native-metadata-service-2026-10-28/README.md)
retains fixed CPUs 24/25, two windows, all four Rust/C++ endpoint combinations,
80 records and 400000 raw RTT samples. It compares the corrected application
with C++; the implementation that read uninitialized metadata is not used as
a performance baseline.

## Lifecycle-managed timers (2026-10-28)

[Lifecycle ROS/wall timers](LIFECYCLE_TIMERS.md) now pause the native producer
outside active, restart from the current clock on activation and invalidate
buffered ticks with a generation even when deactivate/reactivate occur between
spins. Manual cancel/reset, explicit destruction, foreign-node rejection and
native timing queries are preserved. Node destruction wakes paused observers.
No additional thread, timer, periodic query or native guard is introduced;
ordinary timer message storage and delivery stay on their existing path.

Fourteen focused Jazzy cases cover native inactive suppression, fresh deadlines,
stale queues, manual pause/startup, failed activation/deactivation, error/shutdown,
drop cleanup, foreign controls, native elapsed/tick metadata, native `/clock`,
backward ROS time and service-driven shared execution. Native elapsed values are
checked against an ordinary timer. The complete sensor example validates 16
publications over two active phases and three paused phases. Humble/Kilted
optimized focused suites execute 13/14 cases; examples and full-target Clippy
pass in both distributions. Humble lacks native `tick_with_info`.

The optimized Jazzy library suite and debug workspace suite each pass 509
library cases, with two manual benchmarks ignored. The Fast DDS optimized
lifecycle filter executes 48 cases. Nine public API compilation boundaries
check managed timer factories/controls and reject ordinary-timer escapes. The
no-ROS container compiles all targets and checks 11 API artifacts/23 methods.

[Default-path performance evidence](benchmarks/lifecycle-timers-2026-10-28/README.md)
compares a safe `88f1ac7` baseline, candidate and rclcpp under both Jazzy RMWs.
The original 5% p50/p95/p99/inverse-throughput gates pass in all window-1/16
cells: 140 records and 700000 raw RTT samples. This service workload measures
the optional managed registry's effect on ordinary nodes; managed-timer scaling
remains a separate performance requirement.

## Owned background executor and runtime I/O (2026-10-29)

[`Executor::spawn`](BACKGROUND_EXECUTION.md) returns an awaitable owned task
backed by one shared native wait thread. Stop/cancellation returns node ownership
without shutting down the context. Drop requests cancellation without joining
on the runtime thread; native errors and inline callback panics release nodes
before completion. Background-specific errors use `ExecutorTaskError`, preserving
the ordinary ROS error representation.

A late-observer regression case first timed out after discovery was complete and
the native wait had started. Background startup now installs a weak notification
waker in the existing observer-cancellation allocation. Service/action matching
and publisher matching/ACK queues are scheduled before triggering the existing
native guard. Cancellation uses the same state, including observers registered
before spawn. The default observer queue layout is retained.

Twelve background cases cover ownership/reuse, cancellation before guard
registration, drop cleanup, pending callbacks, failure/panic teardown, unobserved
completion, late typed/untyped/action/matching/ACK observers, current-thread TCP
I/O and non-Send application futures. Seven observer cases retain bounded
cancellation and registration semantics. The separate context-shutdown case
checks native wake, task completion and rejection of spawn after shutdown.
The complete current-thread Tokio example verifies 32 TCP/ROS-service/pub/sub
roundtrips, ACK progress, explicit stop and a second exchange after reuse.

The Jazzy debug workspace and optimized library suites execute 522 library
cases, with two manual benchmarks ignored. Fast DDS executes all 12 optimized
background cases. Native Humble/Kilted each pass the 12 optimized background
cases, seven observer cases, context shutdown, optimized example and full-target
Clippy. Ten public compilation boundaries include task ownership/Send/result
types. No-ROS compilation covers all targets and checks 13 API artifacts and
28 methods, including `ExecutorTaskError`.

[Performance reports](benchmarks/background-executor-2026-10-29/README.md) retain
the initial failures and same-binary calibration runs. Final default same-thread
and optional background A/B reports under both Jazzy RMWs record passing original
5% p50/p95/p99/CPU/inverse-throughput gates: 432 records and 12960000 RTT samples.
Default processes use CPU 24; all compared background processes and RMW threads
use the same physical CPU set 24/25. The background baseline is the safe manual
native-thread API at `67ea778`.

The earlier background A/A calibration had a 5.2% tail difference after longer
warmup/sample cohorts. The final candidate has no passing matched same-binary
calibration, so passing A/B reports remain limited evidence and do not establish
calibrated tail precision or superiority over C++. Native-thread
handoff costs, scheduling, allocation/resource evidence and reactor integration
without a separate wait thread remain explicit work. The default fast path is
preserved; the concrete improvement is safe async-runtime I/O composition.

## Notification-driven background cleanup (2026-10-29)

An idle native worker regression test first failed because public endpoint Drop
only queued a cleanup identity. [Background cleanup](BACKGROUND_CLEANUP.md) now
notifies the existing native guard after the first entry in each cleanup queue,
with the queue lock released. The shared queue allocation retains an optional
weak background wake; public endpoint/node queue pointers retain their layout.
Batch notifications coalesce until drain, and extracted nodes unbind the old
executor wake. No polling timer or new native guard is added.

Outstanding client responses still defer endpoint removal. Canceling the final
typed/untyped response now wakes its owner. Action maintenance retries a dropped
client after result/feedback cancellation clears the last tracked operation.
Action-server decision futures and full cancel-stream readiness use the same
background native waker, retaining bounded scans and protocol semantics.

Seven live-worker cases verify ordinary entity destruction, pending client
responses, typed/untyped action endpoints and observers, delayed cancel decisions
and node transfer. Native endpoint ownership is observed before stopping the
task. Nearby notification cases check unlock-before-wake and burst coalescing;
two action cases check decision and backpressure wakeup. The action-server
module suite executes 38 cases. Humble/Kilted and both Jazzy RMWs cover the
targeted native boundaries; optimized gates use assertions-disabled builds.

Final debug workspace and optimized Jazzy library suites each execute 532
library cases, with two manual benchmarks ignored. Both native Humble/Kilted
optimized cleanup, notification and action-server gates pass, as do their
full-target Clippy checks. The ROS-free all-target/documentation gate remains
green with the same 13 API artifacts and 28 methods.

The [default executor A/B](benchmarks/background-cleanup-2026-10-29/README.md)
uses the safe `6f97881` baseline, CPU 24, nodes 1/8 and windows 1/16, 30000
requests after 5000 warmup and nine interleaved repetitions. Both RMWs pass the
original 5% latency/throughput/CPU gates: 216 application records and 6480000 RTT
samples. It verifies the default path's performance boundary, not a cleanup
throughput improvement. Background tail precision and wider scaling remain open.

## Unpromoted action batch-progress prototype (2026-10-29)

The [archived experiment](benchmarks/action-batch-progress-2026-10-29/README.md)
reproduced a missing continuation after sixteen of forty canceled-result entries.
Its native typed/untyped 40-goal cancellation/decision/result oracles and bounded
blocked-feedback/decision scans passed, including same-thread and background
execution. It did not pass final performance acceptance: Fast DDS success and
cancellation p99 were 8.98% and 5.93% above the safe `df69693` baseline. The
final CycloneDDS comparison passed the unchanged 5% limit.

Both final reports retain all protocol checks, 140 total records and 140000 RTT
samples. Earlier long-cohort/short-cohort failures and same-binary calibration
are preserved. A short-cohort A/A pass did not predict the following A/B tail
stability. The full source patch, test logs and source identities are archived;
the prototype was removed from production. These open liveness/performance
requirements are not recorded as achieved.

## Canceled-observer backlog continuation (2026-10-29)

[Cancellation cleanup](ACTION_CANCELLATION_BACKLOG.md) now re-arms the existing
action-client maintenance notifier if a bounded drain leaves unexamined entries.
The drain returns that progress bit through the existing maintenance operation
to native waiting. It adds no queue layout, epoch, timer, guard or queue-length
query, and keeps the sixteen-entry limit. The bit is captured before detached
feedback-sender retries are requeued; retries alone do not make that pass busy.

Six directly relevant cases cover queue boundaries 0/1/16/17/40/128, unlock and
notification ordering, final/disabled notifications, live result/cancel senders,
stale identifiers, detached feedback retry, typed/untyped native forty-observer
cleanup and same-thread nonblocking continuation. Goal handles stay alive while
the native cases wait for cleanup so status-drop or result events cannot conceal
lost continuation. All six execute in optimized native Humble and Kilted; their
full-target Clippy gates pass. The ROS-free all-target/documentation gate remains
green at 13 API artifacts/28 methods.

[The normal-action A/B](benchmarks/cancellation-continuation-2026-10-29/README.md)
uses a safe `f3bc7ad` baseline, independent release binaries, CPUs 24/25 and the
same success/cancellation oracles. Both final RMW reports pass the original 5%
p50/p95/p99/throughput regression gates: 140 records and 140000 RTT samples.
Initial failures are retained. The first design added queue-length queries; the
final design derives progress while draining, without those extra queries.
Prior action A/A tail limitations are not treated as calibrated improvements.

Jazzy's debug workspace and optimized library suite each pass 538 library cases,
with two manual benchmarks ignored. The first release run exposed a pre-existing
graph test that queried local discovery before the DDS cache update. That test
now lives with the graph cases and registers a graph waiter before querying,
using the existing two-second discovery deadline. Relative/private topic types
and exact publisher/subscription/service/client counts remain asserted in an
isolated namespace. Its optimized gate passes under both Jazzy RMWs; only test
code changed after the performance measurements.

This is a promoted fix for canceled-observer cleanup only. Pending feedback
delivery and large server cancel delivery/decision batches remain open and the
earlier broad prototype remains unpromoted.

## Action feedback capacity recovery (2026-10-29)

[Feedback progress](ACTION_FEEDBACK_PROGRESS.md) replaces noop sender wakes and
blocked-goal rotation with goal-specific capacity notifications. Full senders
leave the runnable queue; consuming a sample queues one retry and notifies
native maintenance after releasing the queue lock. Remaining runnable work
continues through the existing maintenance progress bit in bounded
sixteen-candidate passes. Forty blocked goals eventually produce no further
self-notifications. The normal successful-send path creates no readiness object.

Eight new cases cover capacity recovery, forty-goal bounded parking and exact
ready UUID selection, detached sender restoration, stale weak registration
ownership, unlock-before-notify and repeated wake coalescing, typed/untyped
native delivery, and same-thread progress. The background cases run four
full/ready cycles, verify eight ordered samples without extra ROS input, and
check that the native guard's trigger count stays stable while blocked. The
complete current-thread Tokio example verifies slow-consumer feedback,
successful result and explicit executor stop under both Jazzy RMWs.

Jazzy's complete debug workspace suite passes, including 546 library cases with
two manual benchmarks ignored. Full optimized library suites execute the same
546 cases under CycloneDDS and Fast DDS. Formatting and all-target workspace
Clippy with warnings denied pass.

Native Humble and Kilted each execute 33 optimized feedback cases, including
all eight new cases, and run the complete optimized slow-consumer example.
Their all-target workspace Clippy gates pass. The no-ROS Ubuntu container
compiles all workspace targets, passes the binding snapshot/generator checks,
and verifies 13 API artifacts and 28 documented methods. Iron, Lyrical and
Rolling retain their supported CI entries without new local runtime evidence.

[The ordinary action A/B](benchmarks/feedback-capacity-2026-10-29/README.md)
compares safe `2c2ff89` with independent optimized candidate binaries on CPUs
24/25, identical inputs and both Jazzy RMWs. Both reports pass the unchanged
5% Rust/Rust versus baseline/baseline p50/p95/p99/inverse-throughput gates and all protocol checks: 140 records
and 140000 raw RTT samples. One additional lock/coalescing test was appended
after build; the saved identity record verifies that production source is
byte-identical to the measured prefix. Prior action same-binary tail drift
still limits claims of calibrated improvements. The report also retains an
unresolved 14.0% CycloneDDS C++/Rust cancellation p99 increase in a mixed-endpoint
control; the preselected same-language gate does not cover that comparison.

This closes feedback capacity notification and bounded retry progress. Native
feedback ingress/fanout, larger payloads and large server cancel delivery or
decision batches remain separate requirements. The broader rejected prototype
remains unpromoted, and passing this ordinary-path gate does not establish
multigoal throughput leadership over C++.

## Indexed action-server cancellation work (2026-10-29)

[Server cancellation progress](ACTION_SERVER_CANCELLATION_PROGRESS.md) replaces
large request/decision rotation with ready indices. A native forty-goal
counterexample originally entered a 250 ms wait after processing only sixteen
ready decisions. Pending registration and delivery now continue in bounded
passes; full channels park until capacity recovery. Request-slot generations
reject stale notifications and weak node notification ownership follows executor
transfer, including ordinary small cancel streams. Bulk state is boxed and
allocated on demand; an ordinary successful single-goal operation does not
allocate bulk readiness state.

Seven nearby cases cover 0/1/16/17/40/128 request and decision boundaries,
all-ready/all-blocked work, exact ready selection, slot reuse, abandoned
decisions, weak current ownership, wake-after-unlock and removed-goal sender
restoration. All 38 original action-server cases pass. Typed/untyped native
cases issue two forty-goal requests into zero-capacity cancel channels, verify
all-blocked quiescence, recover only the final goal, transfer the blocked node,
then check rejected/partially accepted responses, exact UUID sets and all
terminal results. A two-goal case covers small-stream transfer; the same-thread
counterexample now requires a zero wait. These boundaries pass under both Jazzy
RMWs. Result endpoints use KeepLast(64) for the forty-result burst; the earlier
default-depth terminal-result timeout is retained as a diagnostic.

The complete `action_cancel_group` application uses public `cancel_all()`,
accepts twenty cancellations and rejects twenty, verifies forty terminal results
and explicitly stops its current-thread Tokio/native executor. It runs under
both Jazzy RMWs. The complete debug workspace suite passes 557 library cases,
with two manual benchmarks ignored. Formatting and all-target workspace Clippy
with warnings denied pass.

[Ordinary action A/B](benchmarks/server-cancellation-2026-10-29/README.md) uses
safe `b15e12b` and independent optimized binaries, fixed CPUs 24/25 and identical
inputs/QoS. Both original Rust/Rust versus baseline/baseline 5% latency and
inverse-throughput gates pass: 140 application records and 140000 raw RTT
samples, with all protocol checks. Source inventories include all extracted
server modules and node preparation/factory code. The mixed-endpoint controls
retain unresolved p99 increases, including 11.6% for Fast DDS C++/Rust success;
those comparisons are outside the preselected gate and have no matched passing
calibration. Passing the gate does not establish calibrated tail gains or C++
performance leadership.

Complete optimized Jazzy library suites execute 557 cases under CycloneDDS and
Fast DDS, with two manual benchmarks ignored. These full suites include the
new cancellation boundaries and the existing lifecycle, ownership, background
I/O and cleanup contracts.

Native Humble and Kilted each pass 45 optimized action-server module cases,
four native scheduling/transfer cases and the same-thread fanout counterexample,
plus the optimized complete application and all-target workspace Clippy. The
no-ROS Ubuntu container compiles all workspace targets, passes snapshot and
generator checks, and verifies 13 public API artifacts and 28 methods. Iron,
Lyrical and Rolling retain supported CI entries without new local runtime
evidence for this slice.

## Ready action-server scheduling (2026-10-29)

[Action-server scheduling](ACTION_SERVER_SCHEDULING.md) replaces the node's
pending-server set with ready UUID notifications in its existing action-server
notification allocation. Small queues deduplicate without a HashSet; large
queues use an optional boxed index. Each preparation handles at most sixteen
servers and requests continuation for remaining work. Identity wakers follow
the current node binding; the existing native guard caches a weak observer
waker shared by foreground, direct shared waits and background execution.

The native twenty-server counterexample records eighty maintenance calls over
four unrelated spins on the baseline and zero on the candidate. Readiness of
only the final server causes exactly one maintenance call. A background case
handles thirty-two ordinary service requests with zero blocked-server polls,
queues readiness while unbound, transfers the node and verifies exact selective
maintenance plus every cancellation response and terminal result. Foreground
and direct shared wait tests make decisions from another thread without other
ROS traffic. The injected baseline foreground test fails with
`Err(RCL_RET_TIMEOUT)`; the final candidate passes. Queue boundaries cover
0/1/16/17/40/128 identities, coalescing, cleanup, weak ownership and wake after
unlock. The cached observer test checks reuse, inactive guards and destruction.

[Saved independent A/B evidence](benchmarks/server-scheduling-2026-10-29/README.md)
retains initial, final and same-binary control reports, individual samples,
source/binary manifests and validation logs. Preparation-stage p50 with
thirty-two parked servers falls from 4027/4018 ns to 316/315 ns under
CycloneDDS/Fast DDS; both zero-server controls and all original 5% gates pass.
This optimized library-test stage includes TLS instrumentation and excludes
native waiting, dispatch and application throughput.

The initial ordinary CycloneDDS action cancellation p99 ratio was 1.085293 and
failed the unchanged 1.05 limit. Its complete source patch and reports remain
archived. Removing redundant waker clones and avoiding the small-queue HashSet
produces a final candidate whose two complete ordinary action matrices pass:
140 records and 140000 individual RTT samples. A same-baseline-binary
CycloneDDS cancellation control passes the symmetric 5% calibration, within
that scenario only. Mixed endpoint controls are retained; none has a final p99
increase above 5% relative to its matching baseline pair. These results do not
establish universally calibrated tail improvements or C++ leadership.

The complete debug workspace suite passes, including 564 library cases with
three manual benchmarks ignored. Complete optimized Jazzy library suites pass
the same 564 cases under both RMWs. Formatting and all-target workspace Clippy
with warnings denied pass. Source inventories for both measured candidates
match the delivered Rust source byte for byte.

Native Humble and Kilted each pass the three scheduling tests (one manual
benchmark ignored), the background scheduling/transfer case, cached observer
case, all 47 action-server module cases, four native cancellation boundaries
and 19 future-execution cases. Both run the complete optimized forty-goal
`action_cancel_group` application and all-target workspace Clippy. The no-ROS
Ubuntu container compiles all workspace targets, passes six snapshot and two
generator cases, and verifies 13 API artifacts and 28 documented methods.
Iron, Lyrical and Rolling retain supported CI entries without new local runtime
evidence. Native wait-set construction/dispatch scaling, complete mixed-load
benchmarks, allocation/resource accounting and representative C++ comparisons
remain additional requirements toward the long-term goal.

## External service introspection (2026-10-29)

[Service introspection](SERVICE_INTROSPECTION.md) now has independently launched
typed/untyped Rust clients against rclcpp services and typed/untyped Rust
services against rclcpp clients. The native endpoints remain alive through
`Off -> Metadata -> Contents -> Off -> Contents`. Each phase performs three
ordered, distinct sum calls. Both subscribers must observe the complete native
event set, exact payloads appropriate to the mode, correlated native request
sequences and the frozen simulated timestamp 12.000000034. Client GID and
server-observed writer GUID are checked against their corresponding origin,
preserving middleware-specific encodings. Disabled phases require zero event
publishers and zero received events after 200 ms of additional spinning.

[Four saved matrices](interop/service-introspection-2026-10-29/README.md) pass
under Jazzy/Kilted and CycloneDDS/Fast DDS: sixteen endpoint pairs, 160 endpoint
phases, 240 real calls and 1152 event observations. The original four nearby
native introspection cases pass under all four environments, including foreign
ownership and retained history for late subscribers. The final source and binary
inventories match all four final validated candidates. All calls/events and compiler/package
versions remain in the reports.

The initial Fast DDS untyped-client run timed out before its first phase
completed because the new harness submitted before service discovery. It is
preserved with its source and manifest; the final harness uses the existing
public `wait_for_service()` in both representations. Kilted's initial validation
image lacked CycloneDDS and failed the native package check. A separate derived
validation image installs the missing RMW, and the original failure/image-build
logs remain archived. Neither failure was bypassed or treated as a passing cell.

Five adversarial validator tests reject missing/duplicate events, wrong native
identities, timestamp/payload corruption, disabled publication, missing reenable
events, incomplete matrices and native call mismatches. Formatting, all-target
workspace Clippy, the complete Jazzy debug workspace and complete optimized
library suite pass (564 library cases, three manual benchmarks ignored).
Humble compiles the new example and explicitly returns exit 2 for unsupported
service introspection; the runner rejects that distribution before building.
Humble's support-boundary test and all-target Clippy pass. The no-ROS container
compiles every workspace target, passes six snapshot/two generator cases and
verifies 13 public API artifacts and 28 methods.

CI now installs both DDS implementations and requires the independent matrix
on Iron/Jazzy/Kilted/Lyrical/Rolling, uploading reports and endpoint logs. Iron,
Lyrical and Rolling have no new local runtime evidence; their live CI outcomes
remain unverified here. This slice adds diagnostics/interoperability evidence
and a runnable Rust workflow without changing production hot paths. It makes
no new latency/throughput claim. Broader payloads, event QoS combinations,
distributed diagnostic applications and the remaining runtime matrix remain
requirements toward the full goal.

## Explicit per-goal feedback coalescing (2026-10-29)

`ActionClientOptions::feedback_overflow_policy(FeedbackOverflowPolicy::Coalesce)`
adds bounded snapshot delivery for typed/untyped action clients. It retains
already-queued samples in order and one newest pending item per goal, permitting
shared native ingress to continue while a goal consumer is blocked. Default
Backpressure retains its prior behavior. Native DDS QoS remains independent.
[Feedback contracts](ACTION_FEEDBACK_PROGRESS.md) document intermediate-item
omission, reserved channel capacity, observer cleanup and replaceable native
decoding/storage boundaries.

The original two-goal counterexample timed out after 250 ms while waiting for
the healthy goal behind a blocked stream. Typed/untyped foreground tests now
compare both policies using the same thirty-two-sample native input: default
delivery checks every sample, while Coalesce checks the queued prefix and newest
pending sample after observing the other goal first. Background tests stop and
unbind the node, recover capacity without triggering the old guard, transfer
ownership and require pending delivery without another ROS message. Both exact
result payloads and stream closure remain required. Native feedback history is
KeepLast(64), preventing middleware truncation from weakening these oracles.

Six nearby policy/layout/count cases cover thousands of replacements, capacities
0/1/8, reentrant arrivals during detached successful/failed sends, payload
retirement after releasing client/channel/readiness locks, stale weak wakes and
checked count boundaries. Policy uses existing count storage and channel padding;
legacy queue and typed/JSON channel header sizes remain unchanged for the pinned
toolchain. Delivery construction now defers channel work until the client lock
is released, and both normal delivery and retries share sender restoration.

[Independent default-path A/B](benchmarks/feedback-overflow-2026-10-29/README.md)
compares safe `b7af300` with independent optimized candidate binaries on CPUs
24/25, identical inputs/QoS and both Jazzy RMWs. The unchanged same-language
1.05 p50/p95/p99/inverse-throughput gates pass in success and cancellation:
140 application records and 140000 individual RTT samples. All seven language
pairs and protocol counts remain checked. Mixed-direction p99 stays within 5%
of matching baseline pairs. Source inventories are byte-identical to measured
candidates. This does not establish calibrated tail improvement or C++ leadership.
The separate fixed-CPU local stage characterizes two million u32 completions per
run; it excludes ROS ingress, native conversion, transport and tail distributions.

The complete Jazzy workspace and both complete optimized library suites pass
574 cases with four manual benchmarks ignored. Both RMWs pass 43 relevant
feedback cases, including the four new native cases, and run the complete
optimized `action_feedback_coalesce` application. Formatting and all-target
workspace Clippy pass. Native Humble/Fast DDS and Kilted/both RMWs each pass
the 43 feedback cases, optimized application and all-target Clippy. The no-ROS
container compiles all targets, passes six snapshot/two generator cases and
verifies 15 API artifacts and 29 methods. Iron, Lyrical and Rolling retain
supported CI entries without new local runtime evidence.

This closes explicit snapshot overflow semantics and per-goal progress under
bounded retention. Eager native feedback conversion, allocation/resource
accounting, larger payloads, fanout and full multi-goal Rust/C++ application
scaling remain additional requirements toward the original goal.

## Borrowed feedback route ownership (2026-10-29)

Native feedback dispatch borrows a caller-owned erased payload slot from
typed/untyped client routing. Matching values move into the existing delivery
completion; unknown goals and rejected types retain their original Box until
the client guard is released. Two injected native-client tests originally
destroyed a rejected payload under that guard and failed a destructor reentry
oracle. Both now require exactly one destruction after unlock. Three adjacent
cases cover original allocation identity, wrong-type/empty-slot behavior and
successful delivery. The original owned hook remains available through a
default borrowed adapter; in-built clients share one private routing helper.

[Saved routing evidence](benchmarks/feedback-routing-2026-10-29/README.md) retains
the original failure, candidate patch, resource measurements, calibration and
final comparisons. The initial CycloneDDS cancellation p99/inverse-throughput
ratios 1.769215/1.052906 failed the unchanged 1.05 gate. Promotion stopped while
same-baseline cancellation A/A and resource probes were run. Allocation, polling
and CPU counts were similar; this did not explain the large tail difference.
Disassembly found an additional non-inlined forwarding call. Inlining removes
that call, and final independent binaries pass the original same-language
success/cancellation p50/p95/p99/inverse-throughput gates under both Jazzy RMWs:
140 records and 140000 raw RTT samples. This does not attribute the initial
77% tail change to the forwarding call or establish general C++ superiority.

Mixed success p99 controls retain unresolved 6.346% CycloneDDS Rust/C++ and
11.020% Fast DDS C++/Rust increases relative to corresponding baseline-language
pairs, outside the preselected same-language gate. Success A/A under both RMWs
passes same-language calibration; the Fast DDS identical-binary C++/Rust p99
comparison still differs by 7.479%, outside symmetric 5%. Qualified mixed-tail
precision remains open. The underlying records and derived control ratios are
retained rather than treating every passing same-language gate as a full matrix
performance claim.

An optimized CPU-24 allocation stage repeats twenty million unknown-goal/type
rejection routes over ten records. Original input Boxes remain unchanged, with
zero measured allocation/free calls and requested bytes. The native Linux/glibc
probe's content/alignment/failure/epoch/thread/reset and dynamic-TLS oracles pass.
The stage excludes native decoding, input allocation and complete ROS transport.

Jazzy workspace and both complete optimized library suites pass 579 cases,
with five manual measurements ignored. Both RMWs pass 48 feedback cases,
including legacy-adapter, backpressure/coalescing, transfer and new ownership
boundaries. Formatting and all-target workspace Clippy pass. Native Humble/Fast
DDS and Kilted/both RMWs pass the same 48 cases and all-target Clippy. The no-ROS
container compiles every target and verifies 15 API artifacts/29 methods.
Delivered code remains byte-identical to final measured source inventories.
Iron, Lyrical and Rolling have no new local runtime evidence. Eager feedback
conversion, native buffer reuse, multigoal allocation/scaling and qualified
Rust/C++ application tails remain requirements toward the full goal.

## Native feedback identity routing (2026-10-29)

Generated action bindings now read native feedback `goal_id.uuid` before full
payload conversion. Built-in typed/untyped clients capture the matching typed
feedback channel under the client lock, then unlock before conversion and
delivery. An unobserved goal drops its native packet without conversion; an
observed goal decodes once and bypasses the intermediate erased Box. Closed
channels skip conversion, identity disagreement is rejected and custom unsafe
action support retains the old complete-decode path through a default `None`
accessor.

Seven direct correctness cases cover typed/untyped native identity against full
conversion from empty through 65536-element feedback, no-observer destruction,
one-decode direct delivery, closure before/during decode, UUID disagreement and
legacy fallback. The untyped RCL counterexample takes eight 4096-element samples
with zero decodes for an unobserved goal, including while another goal remains
observed; disabling the accessor requires eight legacy decodes.

The [fixed-CPU stage and complete reports](benchmarks/native-feedback-identity-2026-10-29/README.md)
use the final source and optimized binary. Across five 100000-message repetitions,
native identity removes exactly one allocation per message for both observed and
unobserved completion. The direct unobserved path records zero decodes. This
empty-payload local boundary excludes RCL take, DDS and application scaling.

Independent optimized `e73254f` baseline/candidate ordinary action matrices
retain identical sources, inputs, native QoS, CPUs 24/25 and the original 1.05
same-language p50/p95/p99/inverse-throughput limit. Both Jazzy RMWs pass success
and cancellation: 140 records and 140000 individual RTT samples. The maximum
candidate/baseline gated ratio is Fast DDS success p99 at 1.021861. Final mixed
direction ratios stay within 2.61%, but they are outside this gate and have no
new same-binary calibration. The workload observes one small feedback item, so
it is regression evidence rather than an end-to-end measurement of skipped
large-payload conversion or general rclcpp leadership.

The final Jazzy workspace passes; its main library has 586 passed and six
ignored cases, with all integration, package and doc tests also green. Complete
optimized main-library suites pass the same 586/6 under CycloneDDS and Fast DDS.
Each RMW executes 55 feedback cases with three manual measurements ignored.
Formatting and warning-denied all-target Clippy pass. Humble/Fast DDS and
Kilted/both RMWs each pass the same 55/3 feedback boundary and all-target Clippy.
The Kilted image explicitly installs both RMW packages. A fresh no-ROS target
passes snapshot transactions, all-target compilation, six common and two
generator tests, Rustdoc and fifteen-page/twenty-nine-method API verification.

Initial Humble/Kilted Clippy failures from loading the allocation-probe source as
two modules, an invalid all-target runtime invocation of a harness-free benchmark,
and an old target-directory permission failure are preserved with the final
logs. They were fixed by sharing one test module, using the workspace test gate
plus all-target Clippy, and allocating fresh user-owned compatibility targets.
All 78 candidate source hashes match the archived final build manifest. Iron,
Lyrical and Rolling have no new local runtime evidence. Retained native feedback
storage, borrowed delivery, bursty large payloads and qualified multi-goal
Rust/rclcpp scaling remain open.

## Reusable outer native feedback storage (2026-10-29)

Every typed/untyped action client now owns a bounded one-slot native feedback
pool. `rcl_action_take_feedback` checks out its message and a RAII lease returns
it after the completion releases the client lock. Unobserved/closed channels,
successful delivery, legacy fallback, conversion errors and UUID mismatches all
return storage. Creating an empty slot and destroying a displaced message happen
outside the pool lock, with destructor-reentry oracles. A lease retains the pool
if the action client is destroyed during completion. Untyped conversion borrows
the uniquely owned native message and `WrappedNativeMsgUntyped` follows the
existing unsafe typesupport contract for cross-thread ownership.

Real RCL tests publish sequence lengths 8192 through zero. Both Jazzy RMWs take
all eight messages with the same outer native address, preserve zero decode for
unobserved goals, execute eight legacy fallback decodes and retain another
observed goal. The initial stronger oracle required the nested sequence data
address to remain unchanged; both RMWs failed it. Those failures are retained,
and the promoted contract makes no dynamic payload-capacity reuse promise.

The [saved stage and complete matrices](benchmarks/native-feedback-buffer-2026-10-29/README.md)
use final source-identical binaries. Five CPU-24 runs of 100000 empty-feedback
completions reduce measured allocations/frees from 300000/300000 to
200000/200000, requested bytes from 20.8 MB to 16.8 MB and median local time
from 208.953 to 141.841 ns/message. The boundary excludes RCL, DDS and payload
conversion.

Process-wide probes over the complete action lifecycle also show lower client
allocation totals. Per 1000 measured goals, CycloneDDS median client allocation
entry points fall by 2006 success and 1985 cancellation; Fast DDS falls by 2485
and 2156. Client free calls fall by 3006/2985 and 2485/2156 respectively. Server
medians differ by at most 344 calls. These are implementation-dependent observed
process totals, not fixed per-message ABI claims; instrumented timing is not used
as latency evidence.

Independent optimized `3f877ec` baseline/candidate ordinary action matrices on
CPUs 24/25 retain the original 1.05 same-language p50/p95/p99/inverse-throughput
limit. CycloneDDS and Fast DDS both pass success and cancellation: 140 records
and 140000 raw RTT samples. The largest gated ratio is Fast DDS success p99 at
1.015166. Mixed directions remain within 1.36% of their corresponding baseline
pairs, outside the preselected gate and without a new same-binary calibration.
No universal tail or rclcpp leadership conclusion is made.

The complete Jazzy workspace passes with 589 main-library cases and seven
manual measurements ignored; both complete optimized RMW suites pass the same
589/7. Each RMW passes 58 feedback cases with four ignored measurements.
Formatting and warning-denied all-target Clippy pass. Fresh Humble/Fast DDS and
Kilted/both-RMW targets pass 58/4 per environment plus all-target Clippy. A
fresh no-ROS target passes four snapshot transactions, all-target compilation,
six common/two generator tests, Rustdoc and fifteen-page/twenty-nine-method API
checks. All 78 measured candidate source hashes match delivered code. Iron,
Lyrical and Rolling have no new local runtime evidence. Dynamic payload reuse,
borrowed feedback, burst/fanout and qualified concurrent Rust/rclcpp scaling
remain open.

## Deferred latest-only Coalesce decoding (2026-10-29)

Built-in typed/untyped action clients now keep a native lease as the pending
`FeedbackOverflowPolicy::Coalesce` item. A newer native sample replaces the old
lease without constructing its owned Rust/JSON payload. Capacity recovery takes
the newest item, decodes once outside client/channel/retry locks and uses the
existing sender restoration path. Default Backpressure remains eager, ordered
and lossless; custom action support without a native UUID accessor retains the
compatible eager route.

Pending feedback is internally `Ready(T)` or a boxed deferred decoder. The
channel grows by at most two machine words per tracked goal (u32 is 40 -> 56
bytes on the pinned target); the retry queue size is unchanged. Replacement and
closure drop captured native storage after unlocking. A detached-sender RAII
guard restores sender ownership on decode `Err` and panic unwind, while UUID
mismatch remains an action-client error.

A real two-goal untyped RCL oracle sends one queued 4096-element sample, eight
large updates and one healthy-goal sample. It observes the healthy goal first,
then the blocked goal's original and latest values, exact results and stream
closure. Only three decodes execute for the three delivered payloads; seven
superseded large samples are never converted. CycloneDDS and Fast DDS both pass.
Typed/untyped foreground overflow, background executor transfer, capacity wakes
and Backpressure cases remain green.

The [saved fixed-CPU stage](benchmarks/native-feedback-deferred-2026-10-29/README.md)
runs five repetitions of 10000 full-channel arrivals with 4096 i32 values. The
former eager path performs 10000 arrival decodes, 30000 allocations/frees and
165.12 MB requested bytes. Deferred ingress performs zero superseded decodes,
20000 allocations/frees and 1.6 MB requested bytes; the latest is decoded once
after measurement. Median stage time is 845.208 versus 489.608 ns/arrival. This
excludes RCL/DDS and is not a complete application-latency claim.

Independent optimized `42c862b` baseline/candidate ordinary action matrices
protect default Backpressure on CPUs 24/25 with the unchanged 1.05
p50/p95/p99/inverse-throughput gate. Both Jazzy RMWs pass success and
cancellation: 140 records and 140000 RTT samples. The largest gated ratio is
Fast DDS cancellation p99 at 1.026240. Mixed directions stay within 2.1% of
matching baseline-language pairs, outside the gate and without new A/A
calibration. No general rclcpp leadership claim is made.

The complete Jazzy workspace passes with 594 main-library cases and eight
manual measurements ignored; complete optimized suites pass 594/8 under both
RMWs. Each RMW passes 63 feedback cases with five ignored measurements.
Formatting and warning-denied all-target Clippy pass. Fresh Humble/Fast DDS and
Kilted/both-RMW targets pass 63/5 per environment plus all-target Clippy. A
fresh no-ROS target passes snapshot transactions, all-target compilation, six
common/two generator cases, Rustdoc and fifteen-page/twenty-nine-method API
checks. All 78 measured source hashes match delivered code. Initial test tuple
field compile failures and the rejected legacy 40-byte channel assertion are
retained. Iron, Lyrical and Rolling have no new local runtime evidence. Borrowed
delivered payloads, dynamic RMW storage reuse and full burst/fanout scaling
remain open.

## Concurrent large-payload action feedback harness (2026-10-29)

The new independent-process action-feedback workload keeps eight goals active, publishes
four round-robin 4096-element feedback messages per goal, validates every payload element,
acknowledges complete feedback consumption and checks every terminal result. Rust/Rust,
C++/C++, Rust/C++ and C++/Rust smoke matrices pass under both Jazzy RMWs. Its offline
verifier recomputes endpoint counts/checksums, lifecycle and feedback-stage percentiles,
rates, matrix completeness, compressed-sample identity and recorded regression gates.

Three-repetition, 1000-batch evidence shows current `0a5f829` r2r ahead of same-run rclcpp
under CycloneDDS: feedback p50/p95/p99 are 1.1–3.5% lower, complete lifecycle percentiles
are 1.2–3.6% lower and feedback-message rate is 3.7% higher. Instrumented 4096×8×4 runs
also record fewer Rust client/server allocations and requested bytes than rclcpp. These are
workload-specific leadership and resource results, not a universal claim.

Fast DDS is an explicit failing boundary. Current r2r feedback p50 is lower, but periodic
Rust-server/status-reader stalls raise p99 to 2.17 seconds versus rclcpp's 15.2 ms and reduce
throughput by 66%. Cross-language rows and raw sample positions reproduce the issue. A keyed
status-interest candidate, a sixteen-snapshot drain and a two-snapshot drain all fail the
preselected 1.05 p50/p95/p99/inverse-throughput gate; production is unchanged. The rejected
reports and profile are retained under
[action feedback scaling evidence](benchmarks/action-feedback-scaling-2026-10-29/README.md).
This paragraph records the earlier per-goal path. The batched action slice immediately below
subsequently attributed the stall, introduced the explicit status transaction and passed both
RMW gates; the rejected candidates above were never promoted.

## Batched action goal state snapshots (2026-10-29)

Typed and runtime-typed action servers now accept, defer/execute and terminally complete explicit goal slices while
publishing one reliable complete status array per slice. Requests/results are prepared before the
server lock; slices reject foreign servers, duplicate UUIDs and result-count mismatches. Changed
prefixes still publish after a later transition failure, and result/status/expiry cleanup preserves
first-error reporting. Runtime-typed slices additionally reject mixed generated action types and
decode all JSON results before native preparation or state transitions. Existing single-goal methods
are unchanged.

A three-goal real-RCL oracle verifies Executing state, 128-element feedback, exact results and
terminal stream closure under both Jazzy RMWs. The four-goal wake-driven example passes under
Jazzy/both RMWs, Humble/Fast DDS and Kilted/both RMWs. Humble and Kilted also pass the four focused
optimized cases and all-target warning-denied Clippy. The runtime-typed wake-driven example uses a
typed client against a JSON server. No-ROS validation checks 19 API pages and 41 methods, including
all twelve typed/runtime-typed batch entry points.

The runtime-typed extension has two real-RCL cases under each Jazzy RMW: erased-type mismatch and
empty/count boundaries, plus deferred execute, JSON failure rollback, 128-element feedback,
success/abort/cancel and stream closure. Humble/Fast DDS and Kilted/both RMWs run the same cases and
four-goal example with all-target warning-denied Clippy. The complete Jazzy main library passes
600/8 in debug and release under both RMWs.

Independent `dc4e1c4` baseline/candidate matrices use 1000 measured eight-goal batches, five
repetitions, 4096-element feedback, four feedback messages per goal, status slice two and one-second
result retention. Clients observe all Executing states before isolated feedback timing. Both RMWs
pass the unchanged 1.05 p50/p95/p99/feedback/inverse-throughput gate. Ordinary single-goal
success/cancellation matrices also pass under both RMWs: 140 records and 140000 RTT samples.

The full four-direction matrices retain actual status/feedback competition. Rust/Rust complete
lifecycle p50/p95/p99 is 20–29% lower than rclcpp/rclcpp and completion rate is 39% higher under
Fast DDS, 42% higher under CycloneDDS. Ten-second retention with the same slice passes Fast DDS
but fails CycloneDDS feedback p99 by 25%; it is archived as a rejected configuration boundary.
See [batched action evidence](benchmarks/action-goal-batching-2026-10-29/README.md).

An independent `fe938e4` same-batched-path guard protects the typed hot path after the runtime
adapter refactor. Each RMW report contains 10 records and 10000 raw batch samples with identical
4096×8×4 input, slice two, one-second retention and CPUs 24/25. Both pass the unchanged 1.05 gate.
CycloneDDS lifecycle p99 is 1.7% lower and batch rate 3.0% higher; Fast DDS lifecycle p50/p95/p99
are 0.5%/1.3%/0.4% lower and batch rate is 0.4% higher. See
[runtime-typed batch evidence](benchmarks/action-goal-batching-untyped-2026-10-29/README.md).

## Scoped native subscription callbacks and pipelined pub/sub (2026-10-29)

`Node::subscribe_native_callback` adds an opt-in, scoped native receive path for forwarding,
filtering and native codecs. Each take still owns an independent native container; the callback
receives only a lifetime-bound, read-only `NativeMessageRef<'_, T>` with private provenance, so
neither the native C struct contents nor the owning wrapper/deallocator can escape
dispatch. `Publisher::publish_native_ref` forwards it directly. This skips the native-to-owned Rust deep copy and
futures-channel send without reviving the rejected populated-container reuse contract. A public
ownership token exposes topic/QoS/matching helpers and deterministic destruction; per-spin drain
limits make throughput/fairness explicit. Existing stream, raw, untyped and loaned APIs are
unchanged.

Seven real-RCL cases under each Jazzy RMW cover native forwarding, exact payload delivery, option
validation, callback-error propagation without implicit teardown, endpoint identity and explicit
teardown. Allocation-failure injection preserves the unread sample. Instrumented native wrappers
reject Rust decoding and count exact container retirement after success, errors and panic with
4096/7/0/128-element payloads. A seven-message burst uses at most two callbacks per spin while
a ready ordinary stream receives its complete batch. Dropped ownership tokens release captures,
fail pending publisher waits and preserve sibling subscriptions. The wake-driven example passes under Jazzy/both RMWs,
Humble/Fast DDS and Kilted/both RMWs. Jazzy runs all 17 Rustdoc examples, including two
compile-fail ownership cases; the no-ROS gate independently runs those two cases and checks 24 API
pages and 54 methods.

The independent pipeline harness validates `elements × window × fanout`, every fanout copy,
ordered responses, complete payloads, window maxima and final controls. Reports retain raw RTT,
ping publish, echo collect and echo publish samples. The selected cases are 0×1×1, 4096×16×1 and
4096×8×4 with 1000 measured messages, 50 warmups and five interleaved repetitions on CPUs 24/25.
Pre-final runs passed the unchanged 1.05 p50/p95/p99/inverse-throughput candidate/baseline gate,
but their candidate manifest differs from the final ownership implementation, and
the isolation-aware runner and qualification checks have changed too. They are retained as
superseded evidence pending an isolated final-source rerun, not promoted as release evidence.

In those pre-final same-run observations, r2r lowers large-pipeline p50/p95/p99 by 9%/16%/5% and raises rate 16%
under CycloneDDS; Fast DDS improves 14%/11%/7% and 18%. Fanout-four improves p50/p95/p99 by
23%/28%/29% with 32% more throughput under CycloneDDS, and 30%/32%/34% with 39% more throughput
under Fast DDS. These comparisons require final-source reproduction before any leadership claim.
A 4096×16×4 CycloneDDS calibration has 100–198 ms tails in identical Rust builds and remains a
rejected saturation boundary. A final-source rerun made under concurrent Zig load is also retained
as rejected, contended evidence. See
[pipeline/fanout evidence](benchmarks/pubsub-pipeline-2026-10-29/README.md).


The 2026-10-30 boundary audit keeps callback code and seven real-middleware cases in a separate
`subscribers/native_callback` module. Jazzy full workspace and both optimized RMW suites pass
607 main-library cases with eight manual measurements ignored, plus warning-denied all-target
Clippy. Humble/Fast DDS and Kilted/both RMWs pass seven callback and nine lifecycle-publication
cases, the example and all-target Clippy. Default Rustdoc execution is restored; all seventeen
examples pass. A true no-ROS container verifies all targets, snapshot transactions, twelve harness
counterexamples, common/generator tests, two ownership compile-fail cases and 24 API pages/54
methods. Final four-direction protocol smoke passes under both RMWs with a complete 165-input
source manifest. The formal performance preflight failed due to concurrent Zig compilation;
final-source performance promotion remains pending. Source/log identities and all reports live in
the [callback evidence archive](benchmarks/pubsub-pipeline-2026-10-29/README.md).

## Native QoS event streams (2026-10-30)

`Publisher`, runtime-typed publishers, ordinary/raw/untyped subscription streams, scoped native
callback tokens and `LifecyclePublisher` now register `QosEventStream` observers. Native event
readiness joins Node/shared/background executor wait sets. One taken status is retained; a full
slot leaves subsequent changes in middleware counters until consumption wakes the current
executor. Duplicate observers and manual takes are rejected while a stream owns the native
counter. Endpoint destruction finalizes events first and produces a final endpoint-invalid error
after any already-taken status. Node teardown and channel notifications release locks before
waking application observers.

A CycloneDDS counterexample exposed cached event status conditions surviving a changed wait-set
layout: a full event stream caused repeated immediate native returns. The portable cache boundary
now rebuilds only when an event-bearing populated layout changes, preserving ordinary/action
indices; stable layouts reuse native wait storage. Event layouts retain native endpoint identity
and type as well as pointer identity so allocator address reuse cannot hide a semantic change.
Nodes without observers retain zero event-array capacity and allocate no event registry or guard.

Twenty-two focused Jazzy cases pass under both RMWs. They cover incompatible QoS, deadline and
liveliness expiry, connect/disconnect counts, bounded counter recovery without idle spinning,
manual/duplicate/foreign ownership, automatic and explicit endpoint teardown, context/waker lock
reentry, runtime-type/native/lifecycle endpoint adapters, same-count membership changes with
complete action results, background capacity wakes after executor transfer, stable layout reuse
and every event-kind registration/unsupported rollback. The full Jazzy workspace and both full
optimized RMW suites pass 629 main-library cases with eight manual measurements ignored, plus
seventeen Rustdoc examples and warning-denied all-target Clippy.

Independent Rust/rclcpp processes observe exactly one offered and one requested reliability
incompatibility under each Jazzy RMW. Both processes retain entities until both observations and
finish controls complete. Build manifests bind source and binary identities; offline verification
rejects missing endpoints, incorrect counters/policies and current-source drift. CI now requires
this oracle for every Humble-and-newer distribution and archives both RMW reports. Local
Iron/Lyrical/Rolling runtime evidence remains unavailable. Formal throughput/tail comparisons
remain pending the isolated application matrix; native event correctness is not a performance
leadership claim. See [the stream contract](QOS_EVENT_STREAMS.md) for API and capacity semantics.

Final compatibility after native-layout identity hardening passes 21 QoS cases on Humble/Fast DDS
(the matching event is unavailable there), 22 on Kilted under each RMW, and 71 executor-related
cases per environment. Each runs the diagnostic example and warning-denied all-target Clippy.
The true no-ROS gate passes all targets, four snapshot transactions, twelve pipeline and one QoS
oracle counterexample test, six common/two generator tests, two ownership compile-fail doctests,
and 28 public API pages with 60 required methods. Source-addressed reports and compressed logs
are in [native QoS evidence](interop/qos-event-streams-2026-10-30/README.md).

## Native type-description discovery (2026-10-30)

Jazzy and newer nodes can start the standard `~/get_type_description` endpoint through explicit
Node options, the matching CLI/YAML override or `start_type_description_service`. It answers
directly from RCL's live type cache in bounded sixteen-request native dispatch passes. A checked
async `TypeDescriptionClient` verifies type name and canonical RIHS schema hash before returning
the native response. Message/service/action hash helpers read generated type support without
constructing payloads. Explicit server/client QoS supports larger concurrent histories; the
forty-request boundary uses matching reliable KeepLast(64), since default KeepLast(10) cannot
preserve a larger unattended batch merely by being reliable.

Response ownership is explicit: RCL initializes the response itself, so r2r finalizes any prior
initialized fields before invoking the native handler and retains only the outer RAII container.
Repeated source/no-source requests, missing/invalid hashes and endpoint removal preserve cleanup
and later query progress. Shared types remain discoverable until their last endpoint is removed.
Stopping/restarting the standard service preserves unrelated endpoint IDs and native remapping.

The exact-source C++ oracle initially failed: generated C metadata stored a NUL in place of the
installed `.msg` file's trailing newline, and RCL cache conversion removed that byte. Both Rust
and standard rclcpp services use this cache. Server-side and checked-client recovery now compare
installed resources against the complete received source prefix and append only missing trailing
whitespace; changed semantic contents are never substituted. Cached files/lookup keys and spare
owned buffer capacity avoid repeated lookup allocations. Both Jazzy RMWs pass schema/hash/source
checks in independent Rust-server/C++-client and C++-server/Rust-client directions, including an
unchanged strict byte-for-byte source-file gate. The original failure and direct native metadata
bytes remain archived. Missing resources preserve native metadata and do not imply exact fidelity.

Twelve focused cases cover RIHS parsing/version boundaries, source recovery counterexamples,
shared message/service/action schemas, invalid/missing identities, source omission, native
registration lifetime, remapping/options/stop/restart, forged same-name schema rejection,
background forty-query progress and timeout/foreign ownership. Final debug workspace and both
optimized Jazzy RMW suites pass 641 main-library cases with eight manual measurements ignored.
A preexisting per-node graph query race was exposed by the optimized suite; its two tests now
register graph interest and wait for cache visibility before retaining the original name/type/
enclave assertions. They live beside the other graph tests and pass both Jazzy RMWs.

Humble/Fast DDS passes three unsupported/hash boundary cases, 53 service cases, the boundary
example and all-target Clippy. Kilted under both RMWs passes twelve type-description and 64
service cases, the schema example and all-target Clippy. The documentation snapshot now contains
24 packages; candidate validation and transactional replacement pass. A true no-ROS gate checks
all targets, snapshot/harness/common/generator tests, two ownership compile-fail cases and 36
public pages/functions with 72 specific methods. Independent C++ interoperability is required
by CI for Jazzy/Kilted/Lyrical/Rolling. Local Iron/Lyrical/Rolling runtime evidence and broader
network/multithread performance comparisons remain open.

## Aggregated ROS-clock sleeps (2026-10-30)

`Node::sleep_until_ros` and `sleep_for_ros` return Send futures with independent cancellation and
signed ROS-clock targets. Each node lazily owns one native wake timer and one jump callback,
with an indexed minimum heap and reusable generation slots. Cancellation removes the deadline
immediately in O(log n); there are no far-future tombstones. Past/zero targets create no scheduler.
Simulated time pauses and backwards jumps preserve the absolute deadline. Clock-source switches
interrupt only earlier registrations; new-domain waits survive prior interruption cleanup.

Native wakes use a steady clock while readiness compares the ROS clock. Native periods are
capped at one day and rechecked against the original target, avoiding RCL's unchecked
now+period arithmetic near i64::MAX. A maximum signed simulated-time target is covered directly.
Simulated time cancels the native wake timer and uses clock notifications. At most 64 due or
interrupted waits complete per preparation/dispatch pass, with runnable continuation. Idle
queues omit the canceled native timer from wait sets. Stable registrations avoid unrelated
clock queries or full deadline scans. Native guard bindings follow Node/shared/background
executor ownership and returned-node reuse.

Completion, explicit cancellation and node destruction notify after releasing deadline, clock
and context locks. Context shutdown now separates native guard triggers from post-shutdown
application observer wakes, with those wakes occurring after context unlock. The timer's owned
native clock serializes its finalizer. Weak callback/context registration prevents reference
cycles, and scheduler creation rolls back safely if shutdown wins registration.

Seventeen focused cases cover sorted indexed-heap oracles, 100000 canceled far-future slot reuse,
past deadlines/overflow, exact system deadline readiness, root cancellation/rearm, node/context
cleanup and lock reentry, signed/paused/backwards simulated time, source changes, 512 concurrent
waits sharing one native timer with bounded passes, ordinary wall timer progress, current
background/returned-node native guards, actual /clock delivery and the i64::MAX target. Both
Jazzy RMWs pass optimized cases. Final debug and both optimized main-library suites pass
658 tests with eight manual measurements ignored; workspace/Rustdoc and all-target Clippy
pass. The runnable example verifies 1/2/1/3 second simulated updates and independent cancellation.

Independent Rust and rclcpp processes follow the same controlled native /clock trace: pending
through pause and backward movement, complete at the original absolute deadline, then cancel
another pending wait. Both Jazzy RMWs pass the exact command/time/state oracle with source and
binary manifests. These are semantic and native entity-scaling results, not application
latency/throughput leadership evidence. Humble and Kilted also execute the focused sleeps,
shutdown cases, example and Clippy. Iron/Lyrical/Rolling runtime and broader networked/concurrent
clock applications remain pending the supported distribution matrix.

## Typed and scoped native service callbacks (2026-10-30)

Short synchronous services now register typed or lifetime-scoped native handlers directly through
Node/shared/background native execution. A lazy callback registry owns stable native endpoints;
user handlers, payload retirement and captured-state destruction run without service/channel/
handler locks. Native requests are independently initialized for each RMW take. One response
container is reused, and explicit per-endpoint drain bounds retain fairness alongside ordinary
request streams, other callbacks and action endpoints. The ownership token carries actual QoS,
resolved names, native introspection and targeted immediate/drop cleanup with current-executor
wakes. Callback errors or panic consume only the taken request and leave the endpoint registered.

Twelve Jazzy cases verify exact replies and request sequence identities, error/panic recovery,
foreign-node/capture cleanup, max-two dispatch passes alongside healthy callback and deferred
services, shared-node service/action offsets and current background cleanup guards. Native type-
support wrappers reject owned request decoding, count every independent request retirement and
observe one native response create/destroy with stable dynamic buffers across 128/16/0/32/128
Unicode replies. Response allocation failure leaves no endpoint. Native introspection checks
complete request/response event payloads and sequence numbers. A Rustdoc compile-fail case proves
that the native request cannot escape the handler.

The workspace stage and final double-RMW optimized library suites pass 670 main-library cases
with eight manual measurements ignored; eighteen Rustdoc examples, callback example and all-
target warning-denied Clippy pass. Final Humble/Fast DDS gates execute eleven callback and 64
service cases, Kilted/both RMWs twelve callback and 76 service cases per environment, with examples
and Clippy. A true no-ROS gate checks all targets, snapshot/oracle/common/generator tests, three
ownership compile-fail cases and 39 public artifacts with 86 methods.

The independent native-callback/27f2130-stream/rclcpp service protocol has explicit receive-path
identities, exact sequence/operand/sum and final stop/confirmation oracles. Both Jazzy RMWs pass
seven language combinations at request windows one and sixteen: 28 records and 2800 raw RTT
samples. Reports bind final source and independent binary identities. These busy-host short
runs are protocol-only; isolated representative performance and resource evidence remains open.
See [service callback evidence](interop/service-callbacks-2026-10-30/README.md).

## Availability cancellation and current native wakes (2026-10-30)

[Availability cancellation](AVAILABILITY_CANCELLATION.md) closes two background regressions:
512 canceled observers stranded a bounded cleanup backlog, and cancellation left an already-
dropped client registered. A separate foreground Node regression demonstrated no native guard
notification when an availability observer was canceled during wake-driven execution.

One bounded state snapshot reports live observers and unexamined cancellation work. Targeted
interest remains active and native waits use zero timeout while cleanup remains. Maintenance
retries dropped service/action clients and publishers after their last availability/ACK work
is removed. Successful ACK results remain readable after same-pass publisher destruction;
incomplete ACK work still retains its endpoint. Publisher matching and ACK snapshots avoid
redundant state queries and retain independent graph/query policies.

Native observer binding covers foreground Node and shared/background executors, graph and
subscription matching, factories after guard initialization and transferred-node reuse.
Existing queue wake identity skips repeat entity scans; the implementation adds no node state,
eager guard, timer or thread. Eleven new cases combine exact 16-token bounds, 512 cancellations
per endpoint queue, six healthy peer waits, native ownership release, ACK cadence restoration,
each cancellation source, actual foreground wait interruption and a binding-access probe.

Final Jazzy workspace validation passes 681 main-library cases and 18 Rustdoc examples. Both
Jazzy RMWs pass complete optimized 681-case suites, the optimized 32-roundtrip TCP/service/
pubsub Tokio workflow with node reuse and all-target warning-denied Clippy. Eight manual
measurement cases remain ignored. The 32-server preparation stage executes 2000 measured
passes per RMW with zero native maintenance polls; busy-host unrestricted-affinity samples
are path checks and do not establish a formal performance result.

Native Humble/Fast DDS and Kilted/both-RMW scoped runtime, optimized Tokio workflow and
all-target Clippy checks accompany a true no-ROS all-target/documentation gate. Exact executed
counts, source identities, original failures, complete logs and stage samples are recorded in
[availability cancellation evidence](validation/availability-cancellation-2026-10-30/README.md).
Iron, Lyrical and Rolling have no new local runtime evidence in this slice. Calibrated
independent A/B and representative C++ throughput/resource/scaling comparisons remain open.

## Independent ready dispatch after operational errors (2026-10-30)

Two native readiness fixtures failed against `07d936c`: a subscription error skipped a ready
timer, and a service callback error skipped a ready action request. [Dispatch error handling](DISPATCH_ERRORS.md)
now attempts other ready entity families and action channels before returning the first
operational error. Each failing drain stops at its existing channel boundary while independent
channels retain their own budgets. Ordinary/action dispatch have separate nearby modules and
one stack-local error outcome; delivery/conversion/retirement retain existing post-lock paths.

Six nearby cases use actual jointly-ready RCL masks, exact service/response sequence identities,
independent endpoint allocation failure and action-channel error/budget/completion probes.
The first shared-reply allocation fixture failed Fast DDS because its reader could be ready
for another client's response. Installed 8.4.4 listener behavior and its tagged take/GUID source
explain that boundary. The final test uses independent reply topics and verifies native slot
identities, exact values and retained native ownership; the initial failure remains archived.

The optimized [recovery example](../r2r/examples/dispatch_recovery.rs) reports one callback error
while preserving a caller-owned pinned application future, then verifies its healthy native
service response identity and wall timer. Native error and background-task failure contracts
remain explicit. Jazzy workspace passes 687 main-library cases and 18 Rustdoc examples; both
RMWs pass six release dispatch cases, complete optimized 687-case suites, both recovery/Tokio
workflows. Jazzy all-target warning-denied Clippy passes. Eight manual measurement cases remain ignored.

Humble/Fast DDS and Kilted/both-RMW scoped dispatch, callback, executor and action gates,
optimized recovery examples and all-target Clippy accompany a true no-ROS all-target/39-public-
artifact documentation check. Exact counts and source/log identities are retained in
[ready-dispatch evidence](validation/ready-dispatch-2026-10-30/README.md).

Independent parent/candidate/rclcpp native callback pipeline binaries pass seven combinations
and three configurations per Jazzy RMW: 42 records and 4200 raw RTT samples with current-source,
receive-path, payload/sequence/count/stop checks. Fixed-CPU short runs are protocol-only because
the performance isolation preflight failed on competing Zig builds and load. Formal A/B,
saturated fanout, broad concurrency/scaling and performance-leadership evidence remain open.

## Native synchronous timer callbacks (2026-10-30)

[Timer callbacks](TIMER_CALLBACKS.md) add synchronous wall/ROS handlers with explicit native
autostart, one-shot and period controls. Each pinned allocation contains RCL timer storage,
handler state and optional steady-clock ownership. A lazy registry keeps ordinary timer layout
and tick channels intact, adds a separate native readiness span and inherits the current weak
guard for background and returned-node cleanup. Plain handlers advance RCL without constructing
elapsed metadata or sending a tick. Native time advances before Rust callback execution.

Eleven Jazzy cases check exact controls and owner isolation, native address stability, failure/
drop/destruction retirement outside context and clock locks, stale native readiness suppression,
paused/backwards ROS deadlines, native timestamps, error/panic rearming, mixed timer/sleep/service/
action offsets and live current-guard cleanup. A compile-fail case rejects a non-Send handler.
The example verifies 32 exact callback publications, native manual startup, one-shot ROS timing
and endpoint destruction. Newer call-info factories follow the same Humble/Iron exclusions as
existing TimerCallInfo support.

Final Jazzy workspace and both-RMW optimized library gates pass 698 main-library cases, with
eight manual measurements ignored. Nineteen Rustdoc examples, dual-RMW optimized callback and
Tokio workflows, and all-target warning-denied Clippy pass. Native Humble/Fast DDS and Kilted/
both-RMW callback/timer/executor gates, optimized examples and all-target Clippy accompany a
true no-ROS all-target/documentation gate expanded to 41 artifacts and 109 methods. Exact
executed counts, source identities and full logs are retained in
[timer callback evidence](validation/timer-callbacks-2026-10-30/README.md).

Dedicated lifecycle callback wrappers, concurrent callback scheduling and isolated representative
Rust/C++ performance/resource comparisons remain open. This slice establishes native callback
capability and ownership behavior, without a throughput-leadership claim.

## Lifecycle-native callback timers (2026-10-30)

The inherited callback factory previously initialized inactive LifecycleNode timers as running;
the native pause counterexample failed against `423d21f`. [Managed callback timers](LIFECYCLE_CALLBACK_TIMERS.md)
now shadow these factories with weak Node/runtime registration, shared activation/reset generations
and a permit acquired before native time advancement. Stale readiness cannot consume one-shot
state. Completion/error/panic consumes one-shot manual enable before user code, and explicit
reset rearms it. Failed deactivation retaining active state preserves its existing deadline.

Control and native lifecycle state hold no user callback lock. The permit keeps control lifetime
through invocation; external activation signals the current weak native owner even with no tick
future. Existing stream timers share the generation utility without changing their inbox boundary.
Public compilation rejects managed-to-ordinary callback and private-state escapes.

An initial signed-time timing test failed because installed RCL 9.2.11 explicitly rejects native
calls at negative clock time before invocation. Tagged native source and original error logs are
retained. Paired managed/regular tests verify that exact error and preserved one-shot state, then
recover after reset; positive-domain backwards reactivation retains exact current SDK timestamps.

Eleven Jazzy managed callback cases, fourteen existing stream cases and the twelve-case public
compiler gate pass. The native service-driven example validates sixteen callback publications
through two active/three paused phases. Final Jazzy workspace and both optimized RMW library
suites pass 709 main-library cases plus nineteen Rustdoc examples, with eight manual measurements
ignored. Optimized lifecycle callback/Tokio workflows and all-target warning-denied Clippy pass.
Native Humble/Fast DDS and Kilted/both-RMW scoped runtime, optimized examples, compiler gates and
Clippy accompany true no-ROS all-target and 42-artifact/131-method documentation checks.
See [complete lifecycle callback evidence](validation/lifecycle-callbacks-2026-10-30/README.md).

Concurrent callback scheduling and isolated representative performance/resource comparisons
remain separate requirements for the full Rust/C++ leadership goal.
