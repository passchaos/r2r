# Lifecycle-managed callback timers

`LifecycleNode::create_timer_callback` and `create_wall_timer_callback` return
`LifecycleCallbackTimer`. Their options and native timing factories mirror regular callback
timers, while native activation follows lifecycle and manual controls. No tick channel is
created. The [service-driven example](../r2r/examples/lifecycle_callback_timer.rs) checks sixteen
publications across two active phases and three paused phases on one shared executor.

| Event | Callback and native schedule |
| --- | --- |
| Unconfigured/inactive/finalized | Native timer canceled; no user callback starts |
| Successful activation | Start a fresh full native period if manually enabled |
| Deactivation/reactivation between spins | Old generation cannot consume ready tick; restart the period |
| Failed deactivation remaining active | Preserve the current native deadline |
| `cancel(&mut node)` | Pause immediately and retain manual pause across activation |
| `reset(&mut node)` | Enable manually; restart while active, remain paused while inactive |
| One-shot callback starts | Cancel natively and consume manual enable before user code; explicit reset rearms |

Stream and callback timers share generation logic. A managed callback acquires an enabled,
applied generation permit before native advancement. Stale readiness therefore cannot consume
the timer's one-shot schedule. The permit retains control lifetime but holds no control mutex
while invoking user code. A callback that already acquired the active permit may finish during
concurrent deactivation; completed work is not retracted.

Ownership and native controls remain scoped to the owning `Node`. Foreign operations fail before
changing logical or native state. Dropping the managed handle drops its regular ownership token
and queues current-owner cleanup. Explicit `destroy(&mut node)` removes the native endpoint.
No dereferencing, coercion or exposed field yields an ordinary callback timer.

Weak registrations in both the owning Node and lifecycle runtime preserve activation through
`into_node`, shared execution and background execution. External activation changes notify the
current weak native guard even when no application tick future exists; a paused native wait can
therefore apply the new generation and resume. Ordinary stream storage and its polling contract
retain their existing boundaries.

## Time and failures

Native expected/actual call metadata uses supported SDK `TimerCallInfo` APIs. Positive-domain
backwards clock jumps followed by reactivation begin at the new clock plus a full period.
The installed RCL 9.2.11 rejects native timer calls when the clock returns a negative timepoint,
with `RCL_RET_ERROR` at `timer.c:290`. A paired regular/managed native callback test checks that
error, zero invocation and retained one-shot state, then verifies reset/recovery after time
returns to the supported domain. Signed timing queries remain separate from native-call support.
The initial negative-success test and the exact SDK source reference are retained in the evidence.

Native advancement and one-shot consumption precede the handler. Handler errors or panics do
not silently rearm a one-shot on the next activation. Existing independent-ready error dispatch
and normal Rust unwind behavior remain in effect. User callbacks and captured-state destruction
hold no lifecycle registry, timer-control or context lock.

## Validation

```sh
cargo test --locked -p r2r --lib lifecycle_callback_timer_ -- --test-threads=1
cargo test --locked -p r2r --lib --release lifecycle_callback_timer_ -- --test-threads=1
cargo test --locked -p r2r --lib lifecycle::timer:: -- --test-threads=1
cargo run --locked --release -p r2r --example lifecycle_callback_timer
python3 tests/lifecycle_publication_api.py
```

The counterexamples cover inactive native startup, activation/reset generations, manual pause,
one-shot sticky completion, settled transition outcomes, stale-ready native advancement, user
reentry/retirement, panic, native timing and external activation without a tick future. The
compiler gate rejects managed-to-ordinary timer and field escapes. Version-specific timing
factories follow regular callback timer support. Distribution/source evidence is tracked in
[release validation](RELEASE_VALIDATION.md). Concurrent callback workers and representative
isolated Rust/C++ performance comparisons remain open.
