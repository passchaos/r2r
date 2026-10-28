# Rust ROS 2 client library comparison (2026-10-28)

Surveyed the four practical Rust options: `r2r` (this repo),
`rclrs`/`ros2_rust` (the official ROS 2 working-group client),
`safe_drive` (Tier IV), and `ros2-client` (Atostek). Facts below come from
each project's current README/source and crates.io metadata, not claims of
universal superiority.

## Middleware strategy: the fundamental fork

| Library | Transport | RMW choice | Consequence |
| --- | --- | --- | --- |
| `r2r` | Links **rcl/rmw** via bindgen | Any installed RMW (Cyclone, Fast, Zenoh...) | Exact ROS semantics, including features that never leave rcl; RMW ABI and distribution coupling. |
| `rclrs` | Links **rcl/rmw** (the reference bindings) | Any RMW | Same strategic choice as r2r; this is the project the ROS 2 organization endorses. |
| `safe_drive` | Links **rmw directly**, bypassing rcl | CycloneDDS/FastDDS | More control of the wait/dispatch path; formal TLA+ spec of its scheduler; thinner dependency but more protocol surface to maintain. |
| `ros2-client` | **Pure Rust** (RustDDS) + optional Zenoh backend | None; interoperates by wire protocol | No ROS install needed for the DDS path; most portable and embeddable; must independently re-implement every ROS behavior (parameters, actions, lifecycle, discovery, security). |

This choice dominates everything else. rcl-linking gives correctness and
feature coverage for free; pure-Rust gives independence and a smaller trust
base but permanently chases protocol parity.

## Execution model

| Library | Primary model | Threading stance |
| --- | --- | --- |
| `rclrs` | **Callbacks** registered on entities, driven by an `Executor`; a `Worker` pattern for shared state; async variants exist | Callbacks are the idiom, mirroring rclcpp |
| `safe_drive` | Both **callbacks and async/await**; a starvation-free, TLA+-verified single-threaded scheduler | Formal no-starvation/no-deadlock guarantees |
| `ros2-client` | **async/await only** ("there are no callbacks"), plus mio polling | Tokio-style; `.spin()` only services background work |
| `r2r` | **Futures/streams** on typed/untyped entities; `Node::spin_once` and a multi-node `Executor`; external Rust futures wake the native wait via a guard condition | No callback registration: subscriptions/services are streams, clients are futures |

r2r's stream/future model is the most idiomatic-Rust of the four and is also
the architectural basis of the multi-node executor: one native wait covers
many nodes with no per-node polling, measured ahead of rclcpp in the
[multi-node benchmark](benchmarks/multi-node-executor-2026-10-28/README.md).
rclrs deliberately optimizes for familiarity over idiomatic Rust.

## Feature coverage (documented support)

| Area | r2r | rclrs | safe_drive | ros2-client |
| --- | --- | --- | --- | --- |
| Pub/sub, QoS, loaned messages | yes | yes (incl. loaned) | yes, zero-copy + custom allocator | yes |
| Services/clients | typed + untyped + raw | yes (async variants) | yes | yes |
| Actions | yes | yes | yes | yes (async required) |
| Parameters | full services + typed `RosParams` derive + parameter clients | mandatory/optional/read-only | basic | local + remote parameter services |
| Timers/clocks/sim time | yes | repeating/one-shot/inert | yes | ROS/steady/sim |
| Graph queries/endpoints/QoS events | broad, incl. type hashes and network-flow endpoints | topic/node discovery | limited | discovery events (async) |
| Lifecycle | **yes** (native services + managed publisher, 2026-10-28) | no documented support | no | no |
| Service introspection events | Iron+ | no | no | no |
| Dynamic/untyped messaging | yes (JSON + raw serialized) | yes (runtime introspection) | no | Serde codegen |
| Multi-node shared executor | yes, measured | node per executor | per-context selector | per-context background tasks |
| ROS 2 Security | inherited via RMW | inherited via RMW | not documented | experimental |
| Zenoh RMW | inherited | inherited | no | native backend option |

Where a library links rcl, lifecycle, security and new RMW features are
inherited. r2r additionally implements features rcl itself does not expose
over the FFI boundary (untyped services, future-driven wakes). ros2-client
must build each of these itself and is visibly still catching up (no
lifecycle; security experimental; codegen experimental).

## Maturity and ecosystem signals

| Signal | r2r | rclrs | safe_drive | ros2-client |
| --- | --- | --- | --- | --- |
| crates.io version / updated | 0.9.7 / 2026-09 | 0.8.0 / 2026-09 | 0.4.3 / 2024-08 | 0.12.1 / 2026-10 |
| All-time downloads | ~527k | ~93k | ~27k (older momentum) | ~293k |
| Maintenance | active, weekly commits | active (official WG) | stalled at the repo level (Jazzy PR landed, but release cadence stopped) | active |
| Stability contract | 0.9 API, broad real use | explicit "no stability guarantees yet" | 0.4 | frequent API breakage (0.11/0.12 notes) |
| Distribution support | Humble..Rolling with cfg boundaries | distribution via colcon | Humble/Jazzy | Galactic..Lyrical by feature flags, lightly tested |

## Which design is "best" — by what goal

1. **Shipping ROS applications today: r2r or rclrs.** Both bind rcl, so they
   inherit semantics instead of re-implementing them. Between the two, r2r
   is more mature (downloads, API stability, breadth: lifecycle, untyped
   APIs, service introspection, typed parameter derive), has the strongest
   measured execution story (shared-wait multi-node executor beating rclcpp
   throughput), and a more idiomatic-Rust API. rclrs is the strategic long
   bet for teams that require the official ROS 2 project's imprimatur and
   prefer the callback style; it is still pre-stability.
2. **Safety-critical verification: safe_drive.** The TLA+ scheduler proof
   (starvation/deadlock freedom) is unique and valuable. But the project is
   effectively unmaintained as a release and its feature surface is
   narrower; the verified-executor ideas are worth studying and borrowing,
   not betting new products on.
3. **No-ROS-dependency/embedding and non-DDS transports: ros2-client.**
   Pure Rust + Zenoh is a genuinely different deployment profile (small
   images, no sourced ROS). It pays for that with protocol parity debt and
   API churn; use when the middleware-independence requirement is real, not
   by default.

## What this means for r2r's roadmap

The architecture comparison validates r2r's two defining decisions and
points at where to keep extending the lead:

- **Keep the rcl binding, keep moving dispatch into native r2r code.**
  r2r already sits between rclrs (fully rcl-driven execution) and safe_drive
  (rmw-level control): entities are native r2r types on one shared rcl wait.
  The measured multi-node advantage and the cancellation/ACK work depend on
  this; do not regress toward per-node callback executors.
- **Lean into the idiomatic-Rust differentiator.** Streams/futures plus the
  unified executor are the thing rclrs deliberately does not offer. Examples
  and docs should foreground that shape; lifecycle already follows it
  (services are inline `Service_` impls, not hidden rcl callbacks).
- **Adopt the ideas competitors got right:** rclrs's documented
  callback+async duality and worker pattern; safe_drive's formal scheduling
  arguments (a written starvation/liveness argument for the shared wait
  would strengthen the multi-node claim); ros2-client's Zenoh interop test
  (r2r inherits `rmw_zenoh` for free but has no evidence matrix for it).
- **Close real gaps relative to the ROS world, not relative to other Rust
  clients:** lifecycle-managed timers, the Humble/Kilted runtime matrix,
  allocation accounting on hot paths, and an entity-scaling benchmark before
  any registry/slotmap change. No competing Rust library does these today,
  so completing them is direct differentiation rather than catch-up.

## Should r2r rewrite rcl/rmw away? (2026-10-28 decision: no)

Measured dependency surface: r2r calls about 213 distinct `rcl_*`, 51
`rcl_action_*`, 20 `rmw_*` and a dozen `rcl_lifecycle_*` entry points (librcl
is 284 KB / 286 exported symbols, librmw 44 KB — thin C libraries, not the
rclcpp C++ layer r2r already bypasses for execution).

A full rewrite would mean independently re-implementing and maintaining
interoperable semantics for: the action protocol (five hidden services plus
feedback/status topics, UUID and result-retention conventions), the parameter
system (CLI/YAML parsing, seven services, events, descriptor constraints),
name/remapping expansion with per-distribution corner cases, QoS
compatibility arbitration (an rcl responsibility, not automatic in DDS),
SROS2 security, content-filtered topics, REP-2011 type-hash negotiation,
service introspection events, loaned messages, rosout, graph caches and
RMW-specific GID encodings — then proving all of it across six distributions
and multiple RMWs that today work by inheritance.

Three independent data points point the same way: `ros2-client` (pure
RustDDS, years active) still has no lifecycle, experimental security/codegen
and repeated protocol-driven API breakage; `safe_drive` (rmw-level bypass
with the TLA+ scheduler proof) stopped releasing in 2024; and `rclrs`, the
official ROS 2 Rust client, still chooses to link rcl after facing the same
trade-off. Against all three goals the rewrite is neutral-to-negative:
features accelerate through rcl (the lifecycle slice landed in a day on
`rcl_lifecycle` and Zenoh/security are inherited), the measured performance
advantage comes from replacing the rclcpp *execution model* with one shared
native wait — already done — while a thin non-virtual rcl FFI boundary is
nanoseconds and has never shown up as a hotspot, and the rcl dependency is
free for the target users, who already have ROS installed.

Reconsider only if one of these becomes real: a strategic rcl-free
deployment requirement (statically linked production images with no ROS
install), profiling with interleaved A/B evidence that rcl itself (not
rclcpp) is the bottleneck, or upstream rcl maintenance stopping. The cheap
ways to capture the benefits without a rewrite are incremental bindgen
caching, refreshed doc-only binding snapshots, the no-colcon cargo path, and
an `rmw_zenoh` interoperability matrix (already inherited, currently
unevidenced). If middleware independence is ever genuinely required it should
arrive as an optional compile-time backend behind a trait, not as a rewrite
of the default.
