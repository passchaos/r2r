R2R - Easy to use, runtime-agnostic, async rust bindings for ROS2.
====================

Easy to use bindings for ROS2 that do *not* require hooking in to the ROS2 build infrastructure -- `cargo build` is all you need. Convenience Rust types are created by calling into the c introspection libraries. This circumvents the ROS2 .msg/.idl pipeline by relying on already generated C code. By default, the behavior is to build bindings to the RCL and all message types that can be found in the currently sourced ros environment, but it is possible to be more specific to save on build time (see note below).

When integration with the colcon build system is desired, a CMakeLists.txt file can be used to limit the generation of bindings to only include specific (idl) dependencies. This is done through additional environment variables. A minimal example of the colcon integration is available here: <https://github.com/m-dahl/r2r_minimal_node>.

This library differ a bit in style from rclpy and rclcpp as it eliminates all synchronous callbacks in favor of rust futures and streams. Coupled with the rust await syntax, this makes it very pleasant to work with ROS services and actions, even in a single threaded setup (see service.rs example). The library purposefully does not chose an async runtime -- this means that the user needs to take care of any task spawning. This also limits the API to what futures-rs provides.

Documentation can be found on docs.rs: <https://docs.rs/r2r/latest/r2r>.

For documentation and type checking without ROS, `cargo check --workspace --all-targets --features doc-only` uses the checked-in Jazzy bindings, including lifecycle, service events and example packages. Run `python3 tests/check_documentation.py` for the complete compiler and public-documentation gate. [Documentation builds](docs/DOCUMENTATION_BUILDS.md) describes the snapshot and refresh workflow.

The ongoing development goal, evidence requirements, and known capability, usability and performance gaps are tracked in [the development roadmap](docs/ROADMAP.md).

For managed nodes, `LifecycleNode::create_publisher` gates typed, reusable native and middleware-loaned publication on activation. Endpoint queries and async waits remain available while inactive. Run `cargo run -p r2r --example lifecycle_publication` for a complete service-driven workflow through the shared executor; [lifecycle publication](docs/LIFECYCLE_PUBLICATION.md) describes ownership, state changes and API boundaries.

Managed ROS/wall timers pause outside the active lifecycle state and restart with a fresh deadline on activation. Queued ticks from an earlier activation are discarded; manual cancellation remains in effect until reset. Run `cargo run -p r2r --example lifecycle_timer` for a native-service-controlled sensor workflow. [Lifecycle timers](docs/LIFECYCLE_TIMERS.md) explains timing, buffering and ownership.

All ordinary, synchronous callback, callback-group and lifecycle timer handles expose lossless asynchronous reset events. Late observers receive resets accumulated before registration, fast resets coalesce without loss, and timer destruction closes the stream after pending delivery. Jazzy/Kilted use the native RCL callback; r2r supplies the same behavior on Humble, where rclcpp has no reset callback API. Run `cargo run -p r2r --example timer_reset_events`; [the reset-event contract](docs/TIMER_RESET_EVENTS.md) describes observer transfer, lifecycle restarts and the no-user-code FFI boundary.

To compose ROS with Tokio or another independently driven runtime, `Executor::spawn()` returns an awaitable native execution task. `task.stop().await` recovers the executor and nodes; dropping a pending task requests cancellation. Run `cargo run -p r2r --example tokio_executor` for a current-thread TCP/ROS workflow with explicit stop and node reuse. [Background execution](docs/BACKGROUND_EXECUTION.md) describes ownership, late observers and cleanup.

To isolate long synchronous callbacks, `ParallelExecutor::new(context, workers)` assigns independent Node groups to a fixed native worker budget. `add_node_to_worker` controls placement; Nodes within a group dispatch serially. Run `cargo run -p r2r --example parallel_executor` for control RPC progress during a blocked callback followed by stop and Node reuse. [Parallel execution](docs/PARALLEL_EXECUTION.md) describes cancellation, failure cleanup and the remaining same-Node concurrency scope.

For independent callbacks on the same Node, `CallbackExecutor` runs mutually exclusive `CallbackGroup` and bounded `ReentrantCallbackGroup` handlers with native waiting within one total worker budget. Typed/native subscriptions, services and wall/ROS timers register directly in groups; managed timers retain lifecycle generations after Node transfer. Run `cargo run -p r2r --example callback_groups` for native forwarding and control RPCs during a blocked group, followed by stop/reuse. Run `cargo run -p r2r --example reentrant_callbacks` to verify concurrent requests to one native service with shared `Fn + Send + Sync` work and independent response buffers. [Callback groups](docs/CALLBACK_GROUPS.md) describes input bounds, native lifetimes and remaining scheduling/performance scope.

`CallbackGroup::spawn_task` retains runtime-agnostic futures under the same mutual-exclusion token, while `ReentrantCallbackGroup::spawn_task` assigns each future to a stable bounded lane. Action server streams, client result/feedback futures and external I/O can therefore share exclusive or concurrent group policy without an action-specific thread. The RAII task handle supports cancellation or detachment, while executor stop/reset preserves pending future state. Run `cargo run -p r2r --example callback_group_action` or `cargo run -p r2r --example reentrant_group_action`; [group tasks](docs/CALLBACK_GROUP_TASKS.md) defines ownership, lane and failure behavior.

For native entity scaling work, `cargo bench -p r2r --bench spin_idle_action_servers` covers idle action endpoints, mixed subscription/service/client/action/timer wait sets and bulk availability/matching waiter cleanup across 0–128 entities. [The scaling protocol](docs/ENTITY_SCALING.md) keeps startup, graph convergence and non-isolated diagnostics separate from steady spin evidence.

Subscriptions can apply middleware-side SQL-like filters during native creation and replace them at runtime through `ContentFilterOptions`, avoiding native takes, decoding and Rust queue traffic for rejected samples. Existing stream/callback/group option structs remain `Copy`; their builder returns an owned configuration only when filter strings are present. Typed/native/untyped/raw streams, scoped native callbacks and exclusive/reentrant grouped subscriptions share the same query/set/clear contract. Unsupported or silently ignored RMW options become `RCL_RET_UNSUPPORTED` without local fallback. Run `cargo run -p r2r --example content_filtered_subscription`; [the filter contract](docs/CONTENT_FILTERED_SUBSCRIPTIONS.md) describes ownership, retained startup history, dynamic updates and middleware boundaries.

Large native subscription payloads can use `subscribe_native_reusable` to receive RAII `NativeSubscriptionSample` items through an async stream. Dropping a sample returns its independently owned native container to a one-slot pool; retaining several samples allocates distinct storage, and samples remain valid after endpoint teardown. A deterministic 4096-element stage removes two allocation/free calls per delivery and about 99.59% of requested bytes at the measured container/fill/channel boundary. Run `cargo run -p r2r --example reusable_native_subscription`; [the ownership contract](docs/REUSABLE_NATIVE_SUBSCRIPTIONS.md) separates guaranteed outer reuse from RMW-specific nested-buffer behavior.

Applications wanting ordinary safe Rust values can use `subscribe_reusable`. Generated messages update existing primitive/string/wstring/nested storage and return the decoded value to a one-slot pool on sample drop; handwritten message implementations retain a compatible replacement default. The 4096-element typed stage removes one allocation/free and about 99.51% of requested bytes per delivery at the native-to-Rust decode/channel boundary. Run `cargo run -p r2r --example reusable_typed_subscription`; [the typed reuse contract](docs/REUSABLE_TYPED_SUBSCRIPTIONS.md) defines retained-sample and custom-type behavior.

The ordinary Rust-style `subscribe<T>` path also reuses its internal native decode container transparently while continuing to return independent owned Rust messages. A separate 4096-element stage removes two native-container allocation/free calls and about 49.98% of total requested bytes per message; the remaining owned payload allocation preserves the existing public ownership model.

Queued subscription bursts can use `subscribe_batches` or `subscribe_native_batches` to acquire up to a caller-selected number of consecutive messages with one `rcl_take_sequence` operation and deliver them as one async stream item. Typed batches preserve each generated Rust message's dynamic-field capacity; retained batches own independent storage and remain valid after endpoint teardown. Native sequence arrays are pooled, while generated native containers are safely reinitialized at stable outer addresses between takes instead of relying on unsafe RMW-specific nested-storage reuse. Successful sparse takes refresh only the previously returned prefix rather than the configured maximum; runtime errors retain conservative full-capacity recovery. Run `cargo run -p r2r --example batch_subscription`; [the batch contract](docs/BATCH_SUBSCRIPTIONS.md) describes ordering, backpressure, ownership and the current performance-evidence boundary.

Dropped endpoint handles and canceled final operations wake the background native executor for targeted cleanup. Delayed action cancellation decisions and full cancel streams also notify native maintenance when work becomes ready. [Background cleanup](docs/BACKGROUND_CLEANUP.md) describes the resource and cancellation contracts.

Canceled action observers continue cleanup across sixteen-entry maintenance batches. The current batch reports unexamined work directly to native waiting, so large result/cancel/feedback-observer backlogs can drain without unrelated ROS traffic. [Cancellation backlog cleanup](docs/ACTION_CANCELLATION_BACKLOG.md) describes batching, live-observer checks and retry behavior.

Action feedback retries follow channel capacity notifications. Blocked goals leave the runnable queue; consuming a sample wakes retry delivery without another ROS message. Run `cargo run -p r2r --example action_feedback_backpressure` for a slow-consumer workflow with ordered feedback and a terminal result. [Feedback progress](docs/ACTION_FEEDBACK_PROGRESS.md) describes bounded retries and ownership.

For progress snapshots, select `ActionClientOptions::feedback_overflow_policy(FeedbackOverflowPolicy::Coalesce)`. Already-buffered samples remain ordered, each goal retains its newest pending item, and other goals continue receiving feedback. Superseded native samples are discarded before owned Rust/JSON conversion; only the latest pending item is decoded when capacity returns. Run `cargo run -p r2r --example action_feedback_coalesce` for a two-goal workflow with bounded retention and checked terminal results. The default `Backpressure` policy retains its existing lossless behavior.

Generated action clients inspect the native feedback goal UUID before constructing the Rust payload. Goals sent with `ActionGoalOptions::track_feedback(false)` therefore discard their feedback without payload conversion, while observed feedback enters its typed channel without an intermediate erased allocation. Custom action wrappers retain compatible full decoding unless they opt into the unsafe native identity accessor.

Large-feedback consumers can call `send_goal_request_native_feedback` to receive an `ActionNativeFeedback` RAII item backed directly by the taken ROS C message. The item stays valid across stream polls, returns its container on drop and follows the existing Backpressure/Coalesce, cancellation and terminal-close contracts without constructing an owned Rust payload. Run `cargo run -p r2r --example action_native_feedback`; [native action feedback](docs/NATIVE_ACTION_FEEDBACK.md) describes custom-type support and retention costs.

Action clients also retain one outer native feedback message between takes. Its RAII lease returns storage after post-lock conversion or every error/closed/unobserved path, avoiding repeated outer message construction without assuming that middleware preserves dynamic payload buffers.

The typed per-goal lifecycle also reuses GetResult request preparation across `ActionClient` clones
and keeps one SendGoal response container on the native endpoint. Response completions run outside
the client lock and return each lease before the next bounded take, so steady accepted goals remove
two native allocation/free calls without changing goal/result futures or the public Rust API.

Action servers continue large cancellation delivery and decision batches through ready queues. Full streams wait for capacity, and cached notifications follow the node after executor transfer. Run `cargo run -p r2r --example action_cancel_group` for a forty-goal `cancel_all()` workflow with partial acceptance and checked terminal results. [Server cancellation progress](docs/ACTION_SERVER_CANCELLATION_PROGRESS.md) describes batching, ownership and buffering.

Cancellation maintenance follows ready server identities, so blocked servers avoid repeated polling during unrelated ROS traffic. External decisions wake the existing foreground, shared or background executor guard, including after node transfer. [Action-server scheduling](docs/ACTION_SERVER_SCHEDULING.md) describes the bounded queue and saved correctness/performance evidence.

The ordinary Rust-style `Publisher<T>::publish(&T)` now transparently reuses one native preparation container across the publisher and its clones. Concurrent publishes still receive independent storage; a failed fallible copy retires that container before the next call. A deterministic 4096-element stage removes two allocation/free calls and 32,824 requested bytes per message at the Rust-to-native preparation boundary. For explicit storage control, keep a `NativeMsg<T>`, update it with `try_copy_to_native`, and use `publish_native`. Primitive and nested-message sequences retain capacity across smaller updates, and string fields reuse existing storage when it fits. Run `cargo run -p r2r --example native_message_reuse` for a complete example. [The Rust/rclcpp comparison harness](tools/bench_ros/README.md) measures the copy and codec stages with identical message fixtures.

For forwarding or filtering without owned delivery, `Node::subscribe_native_callback` supplies a lifetime-bound `NativeMessageRef<'_, T>` during one node dispatch. The callback can inspect native fields and call `Publisher::publish_native_ref` directly, avoiding the native-to-Rust deep copy and futures-channel send while preserving independent native storage for every RMW take. Run `cargo run -p r2r --example native_callback_forward`; [the callback contract](docs/NATIVE_SUBSCRIPTION_CALLBACKS.md) covers ownership, errors and per-spin fairness.

Short synchronous services can register `create_service_callback` directly; `create_service_native_callback` receives a scoped native request and reusable native response buffer. Handlers run during node dispatch without internal service/channel locks, and the ownership token supports native QoS, introspection and deterministic teardown. Run `cargo run -p r2r --example service_callback`; [the service callback contract](docs/SERVICE_CALLBACKS.md) covers lifetime, fairness and errors.

For simulation-aware delays, `node.sleep_for_ros(duration)` and `node.sleep_until_ros(deadline_ns)` return cancellable futures that share one native timer. Paused simulated time keeps them pending; backwards jumps preserve the absolute target, and time-source switches report an explicit error. Run `cargo run -p r2r --example ros_sleep`; [the time contract](docs/ROS_TIME_SLEEP.md) covers cancellation, shutdown and executor transfer.

Unix applications can opt into rclcpp-style graceful process termination with `context.install_signal_handler()?`. SIGINT and SIGTERM are forwarded from an async-signal-safe handler to the normal `Context::shutdown` path, interrupting empty native waits and returning executor ownership. Run `cargo run -p r2r --example graceful_shutdown`; [the shutdown contract](docs/GRACEFUL_SHUTDOWN.md) covers process-global ownership, safety and context retargeting.

For runtime schema discovery on Jazzy and newer, start the standard `~/get_type_description` service with `NodeOptions::start_type_description_service(true)` or `Node::start_type_description_service()`. `TypeDescriptionClient` asynchronously checks the returned type name and canonical RIHS hash before exposing schema/source data. Run `cargo run -p r2r --example type_description`; [the contract](docs/TYPE_DESCRIPTIONS.md) covers endpoint ownership, QoS, distribution boundaries and sources.

Native QoS diagnostics compose with application futures through `publisher.qos_events(&mut node, kind)` and `subscription.qos_events(&mut node, kind)`. A bounded `QosEventStream` waits on deadline, liveliness, incompatible QoS and other supported status events through the same native executor. Run `cargo run -p r2r --example qos_event_diagnostics`; [the event contract](docs/QOS_EVENT_STREAMS.md) covers observer ownership, backpressure and endpoint cleanup.

For repeated serialization, `MessageSerializer::<T>::new()?` keeps native message storage and returns borrowed ROS wire bytes from `serialize(&message)?`. Pass the slice directly to `publish_raw`, or call `to_vec()` when separate ownership is needed. Run `cargo run -p r2r --example reusable_serialization` for a complete typed receive/raw publish example. The selected RMW may resize the serializer's output buffer; returned bytes remain valid for their Rust borrow.

Live parameters share one transactional path for local calls and the standard ROS services. `add_pre_set_parameters_callback` can derive or normalize a batch, `add_on_set_parameters_callback` rejects it before state changes, and `add_post_set_parameters_callback` observes only committed values after internal locks are released. Registrations are scoped by RAII handles and recursive same-node mutation returns an error instead of deadlocking. Run `cargo run -p r2r --example parameter_callbacks`; [the callback contract](docs/PARAMETER_CALLBACKS.md) covers ordering, atomicity, declarations, events and descriptor constraints.

`#[derive(RosParams)]` fields can declare `read_only`, integer/floating ranges, steps and additional constraints through `#[ros_param(...)]`. CLI/YAML overrides are validated before the user struct is mutated, and the same descriptors drive local calls and standard ROS parameter services. [Derived parameter descriptors](docs/DERIVED_PARAMETER_DESCRIPTORS.md) documents the syntax and transaction boundary.

[The end-to-end pub/sub comparison](tools/bench_ros/TRANSPORT.md) verifies independent Rust/rclcpp processes in both language directions and measures individual roundtrip latency under matching transport settings.

Run `cargo run -p r2r --example service_pipeline` to process service calls with bounded futures-rs concurrency and a runtime-agnostic deadline. [The service comparison](tools/bench_ros/SERVICE.md) validates native Rust/rclcpp requests and responses in both language directions, including one and 16 requests in flight.

Typed `Client<T>::request` and `request_with_info` transparently reuse one native request-preparation container across cloned handles while keeping conversion outside the client mutex. Concurrent requests check out independent storage, and a failed fallible copy retires its container before any RCL request is registered. A deterministic request with one 4096-byte string removes four allocation calls, three frees and 4,146 requested bytes per request after warmup.

The typed client endpoint also reuses one native response-take container across plain and metadata-bearing futures. Decode remains outside the client mutex and produces independent owned Rust values; explicit endpoint destruction releases the cache even when public client clones remain. For a 4096-byte string response this removes three allocation calls, two frees and 4,130 requested bytes while retaining the required owned Rust allocation.

Typed `Service<T>` streams now reuse separate native request-take and response-preparation containers without changing owned Rust requests or asynchronous `respond` semantics. Request clones share the response cache and once-only response state; conversion failure retires the container without consuming the response opportunity. A deterministic 4096-byte response stage removes four allocation calls, three frees and 4,131 requested bytes per service call after warmup.

Run `cargo run -p r2r --example action_lifecycle` for deterministic deferred execution, feedback, success and cancellation with a wall-clock deadline. [The action comparison](tools/bench_ros/ACTION.md) checks both language directions and records lifecycle, acceptance and cancellation timings under an explicit feedback-confirmation protocol.

Typed action client goal sends transparently reuse one native SendGoal request container across cloned handles while preserving conversion-before-lock and all existing futures/options. Copy failure retires the container before RCL or goal-state registration. For the generated Fibonacci request this removes one allocation/free and 20 requested bytes per goal after warmup.

Typed action clients also retain one endpoint-owned native status array across lifecycle updates. Direct native status decode remains outside the client mutex; a 128-goal stage removes two allocations/frees and 3,608 requested bytes per status message while retaining the decoded update Vec.

Typed action result futures reuse one endpoint-owned native GetResult response container while continuing to deliver independent owned Rust results. Detached decode and goal cleanup ordering remain unchanged; a 4096-element Fibonacci result removes two allocations/frees and 16,416 requested bytes while retaining the required owned Vec allocation.

Servers handling concurrent goal windows can use `ActionServerGoalRequest::accept_batch`, `ActionServerGoal::execute_batch` and the terminal `*_batch` methods to prepare work before locking and publish one reliable status snapshot per chosen slice. The same six methods are available on `ActionServerGoalRequestUntyped` and `ActionServerGoalUntyped`; mixed runtime action types, foreign servers, duplicate UUIDs and malformed JSON results fail before native result preparation. Results, feedback and late-result retention remain per goal. Run `cargo run -p r2r --example action_goal_batch` for the typed workflow or `cargo run -p r2r --example action_goal_batch_untyped` for a typed client interoperating with a runtime-typed JSON server. Choose slice size with the action status QoS and result-retention window in mind; a larger slice reduces status traffic, while a smaller slice bounds each snapshot burst across RMW implementations.

On ROS 2 Iron and newer, typed and untyped services and clients expose `configure_introspection(&mut node, state, event_qos)`. Choose `ServiceIntrospectionState::Metadata` for identities and timestamps or `Contents` to include payloads; `Off` disables publication. Subscribe to the generated service `Event` type on `<service_name>/_service_event`. Run `cargo run -p r2r --example service_introspection` for a complete request, response and event stream example.

[Service introspection](docs/SERVICE_INTROSPECTION.md) explains cross-language tracing, native identities and simulated clocks. The independent Rust/rclcpp workflow verifies typed/untyped endpoints, mode transitions and complete event contents under both Jazzy and Kilted RMW implementations.

Generated service bindings provide the associated `Event` type automatically. Manual implementations of `WrappedServiceTypeSupport` on Iron and newer must also define `type Event` to match their native service event type.

Development focuses on complete application workflows, safe and convenient Rust APIs, and measured performance against native ROS implementations. The API may change as capability gaps are closed. Feature requests and contributions are welcome; the roadmap records current evidence and remaining work.

How to use
--------------------
Supported ROS 2 distributions start at **Humble**: Humble, Iron, Jazzy, Kilted, Lyrical and Rolling. Distribution-specific APIs are enabled only where ROS provides them.

1. Make sure you have libclang installed. (e.g. libclang-dev on ubuntu)
2. Depend on this package in Cargo.toml: `r2r = "0.9.7"`
3. You need to source your ROS2 installation before building/running.
4. The bindings will rebuild automatically if/when you source your workspace(s).
5. If you make changes to existing message types, run `cargo clean -p r2r_msg_gen` to force recompilation of the rust message types on the next build.

Examples of how to use the crate are included in examples/
```
. /opt/ros/humble/setup.sh
cargo build
cargo run --example subscriber
# In other shell
ros2 topic pub /topic std_msgs/msg/String "data: 'Hello, world'"
```

A note on build times
--------------------
Since the default behavior is to build all sourced message types, build time can quickly become a problem for larger workspaces. To avoid building everything, it is possible to declare only the message packages needed using the environment variable `IDL_PACKAGE_FILTER`. Setting this can be done in `.cargo/config.toml` for convenience, e.g. <https://github.com/m-dahl/r2r_minimal_node/blob/master/r2r_minimal_node/.cargo/config.toml>. Note there is no automatic dependency resolution done for nested message types, so all used message packages need to be explicitly included.

What works?
--------------------
- Up to date with ROS2 ~Dashing~ ~Eloquent~ Foxy Galactic Humble Iron Jazzy Lyrical
- Building Rust types
- Publish/subscribe
- Services
- Actions
- Parameter handling
- Simulated time (make sure `rosgraph_msgs` is sourced when building to enable)
- Runs on Linux, OSX, and Windows.

Changelog
--------------------
#### [Unreleased]

#### [0.9.7] - 2026-09-13
- Bump bindgen version to fix errors with LLVM 22 and later <https://github.com/sequenceplanner/r2r/pull/133>.

#### [0.9.6] - 2025-06-04
- rcl_timer_init wrapper to compile for Foxy Galactic Humble Iron Jazzy and Lyrical

#### [0.9.5] - 2025-04-22
- Add `/set_parameters_atomically` <https://github.com/sequenceplanner/r2r/pull/120>, <https://github.com/sequenceplanner/r2r/pull/121>. Fixes `ros2 param ...` on Jazzy.
- Update bindgen 0.63.0 -> 0.71.1 <https://github.com/sequenceplanner/r2r/pull/116>. (Issues may persist until next bindgen version, see PR).

#### [0.9.4] - 2024-11-21
- Fix cargo syntax for older rust versions < 1.77 <https://github.com/sequenceplanner/r2r/commit/74ad4410c79b1be7e42eb1822a291639e3c40ec4>
- Fix message generation for WChar idl type <https://github.com/sequenceplanner/r2r/commit/94db799db282c8b1e0222f1699e6a6420b5fd544>

#### [0.9.3] - 2024-11-09
- Expose WrongParameterType <https://github.com/sequenceplanner/r2r/pull/111>
- Add functions to destroy a publisher <https://github.com/sequenceplanner/r2r/pull/110>

#### [0.9.2] - 2024-10-13
- Fix cfg warnings <https://github.com/sequenceplanner/r2r/commit/4cb5bd362e81154924cc7dc21f8edbf470c4604a>
- More convinent one time parameter access <https://github.com/sequenceplanner/r2r/pull/107>

#### [0.9.1] - 2024-10-12
- Minor code fixes: <https://github.com/sequenceplanner/r2r/pull/105>, <https://github.com/sequenceplanner/r2r/pull/106>
- Update for ros2 jazzy <https://github.com/sequenceplanner/r2r/pull/100>

#### [0.9.0] - 2024-05-17
- Fix unsafe precondition(s) violated with rust 1.78 <https://github.com/sequenceplanner/r2r/issues/96>. There may be more of these to fix, please report if you encounter.
- Expose QoS settings for service clients and servers <https://github.com/sequenceplanner/r2r/pull/95>. Note, slight API change!
- Make the order of parameters deterministic <https://github.com/sequenceplanner/r2r/pull/94>
- Add a r2r_info example to print environmental information <https://github.com/sequenceplanner/r2r/pull/92>
- Code cleanups <https://github.com/sequenceplanner/r2r/pull/90>, <https://github.com/sequenceplanner/r2r/pull/91>
- Remove is_available's node mut-ref dependency <https://github.com/sequenceplanner/r2r/pull/89>. Note, slight API change!
- Support simulated time <https://github.com/sequenceplanner/r2r/pull/88>

#### [0.8.4] - 2024-03-19
- Fix QoS for rolling <https://github.com/sequenceplanner/r2r/pull/87>
- Update for ros2 iron <https://github.com/sequenceplanner/r2r/pull/84>

#### [0.8.3] - 2024-01-14
- Add `get_publishers_info_by_topic` <https://github.com/sequenceplanner/r2r/pull/80>
- Raw publishers <https://github.com/sequenceplanner/r2r/pull/76>
- Add `rcl_publisher_get_subscription_count` related methods <https://github.com/sequenceplanner/r2r/pull/75>
- From serialized bytes for `WrappedNativeMsgUntyped` <https://github.com/sequenceplanner/r2r/commit/d5f2ef0eac7072c66fe8866a3dbbfc2326b13804>
- Message (de-)serialization helpers <https://github.com/sequenceplanner/r2r/pull/74>.
- Raw message subscribers. <https://github.com/sequenceplanner/r2r/pull/73>

#### [0.8.2] - 2023-12-11
- Fix include path regression on linux. <https://github.com/sequenceplanner/r2r/pull/71>

#### [0.8.1] - 2023-11-30
- Fix regression when building with colcon/cmake. <https://github.com/sequenceplanner/r2r/commit/7fc96e3eb2fd9f7f272258e07204041aeecfba76>

#### [0.8.0] - 2023-10-05
- Windows support! <https://github.com/sequenceplanner/r2r/pull/66>
- Derive macro for ros parameters. <https://github.com/sequenceplanner/r2r/pull/65> and <https://github.com/sequenceplanner/r2r/pull/68>. NOTE: Breaks old parameters API, see commit message for details <https://github.com/sequenceplanner/r2r/commit/00d7a3db0b48c27a61ce153b084eadeef799765a>.
- Implement rcl_interfaces::srv::ListParameters service. <https://github.com/sequenceplanner/r2r/pull/64>
- Eliminate compiler warnings about virtual workspace's resolver. <https://github.com/sequenceplanner/r2r/pull/63>

#### [0.7.5] - 2023-06-27
- Fix issue with snake case conversion for idl header files. <https://github.com/sequenceplanner/r2r/pull/61>
- Fix build issue with race condition between header file generation and calling bindgen. <https://github.com/sequenceplanner/r2r/commit/f5f41baa2554b24813cc2f212ecf2e45476063e1>

#### [0.7.4] - 2023-06-26
- Refactor code generation using syn and quote and improve build times <https://github.com/sequenceplanner/r2r/pull/58>
- Replace (e)println with log <https://github.com/sequenceplanner/r2r/pull/51>

#### [0.7.3] - 2023-06-20
- Fix mistake in docs generation.

#### [0.7.2] - 2023-06-20
- Various CI improvements: <https://github.com/sequenceplanner/r2r/pull/54>, <https://github.com/sequenceplanner/r2r/pull/56>
- Fix build problem at docs.rs <https://github.com/sequenceplanner/r2r/pull/56>
- Clarify compilation flags related to build time in README.md as suggested in <https://github.com/sequenceplanner/r2r/issues/53>

#### [0.7.1] - 2023-05-21
- Add associated constants to generated message types. <https://github.com/sequenceplanner/r2r/pull/46>
- Log loaned message error only once. <https://github.com/sequenceplanner/r2r/pull/44>
- Update r2r_cargo.cmake to latest version (see [here](https://github.com/m-dahl/r2r_minimal_node/commit/897774868edaa97e0272fffc253402d6474aaeb7))

#### [0.7.0] - 2023-03-21
- Use non-mangled names for serde serialization. <https://github.com/sequenceplanner/r2r/pull/40>
- Avoid segfault when rcl_init fails. <https://github.com/sequenceplanner/r2r/pull/41>
- Loaned message support. (Changes `publish_native` api). <https://github.com/sequenceplanner/r2r/pull/42>

#### [0.6.7] - 2023-02-20
- Fix undeclared type on Foxy. <https://github.com/sequenceplanner/r2r/pull/39>

TODO
--------------------
- Documentation is lacking. (For now, look at the examples.)


LICENSING
--------------------
All code is under the MIT licence unless another license is specified in the source files. Some parts from the rclrust (https://github.com/rclrust/rclrust) package are included in this repo and these are licensed under Apache 2.0.


***
<!--
    ROSIN acknowledgement from the ROSIN press kit
    @ https://github.com/rosin-project/press_kit
-->

<a href="http://rosin-project.eu">
  <img src="http://rosin-project.eu/wp-content/uploads/rosin_ack_logo_wide.png"
       alt="rosin_logo" height="60" >
</a></br>

Supported by ROSIN - ROS-Industrial Quality-Assured Robot Software Components.
More information: <a href="http://rosin-project.eu">rosin-project.eu</a>

<img src="http://rosin-project.eu/wp-content/uploads/rosin_eu_flag.jpg"
     alt="eu_flag" height="45" align="left" >

This project has received funding from the European Union’s Horizon 2020
research and innovation programme under grant agreement no. 732287.
