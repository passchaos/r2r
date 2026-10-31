# Timer reset event streams

`Timer::observe_resets`, `CallbackTimer::observe_resets` and
`GroupedTimer::observe_resets` expose native timer restarts as a runtime-agnostic
`TimerResetEvents` stream. `LifecycleTimer` and `LifecycleCallbackTimer` delegate
the same API, so lifecycle activation restarts are visible beside explicit
application resets.

Each `TimerReset` contains the number of resets coalesced since the previous
item and the total since native timer creation. Registration is lossless: the
first item includes resets that happened before observation. If an observer is
dropped, a later observer resumes from its consumed cursor and receives resets
that occurred between observers. One observer is allowed at a time; a second
registration returns `Error::TimerResetAlreadyObserved` without replacing the
live stream.

Jazzy and Kilted use `rcl_timer_set_on_reset_callback`, including RCL's native
late-registration counter. Humble predates that API in both RCL and rclcpp, so
r2r records successful calls at its centralized native reset boundary. The
public behavior is identical across the required distributions. Explicit reset,
lifecycle activation/reactivation and callback-group generation reconciliation
are counted. Timer initialization, cancellation and period changes are not
resets.

The native callback never invokes a user closure. It performs an atomic count
update and wakes the stream; a panic from a custom executor waker is caught
before returning across the C ABI. Ordinary and synchronous callback timers
serialize registration through their owning `&mut Node`. Grouped timer controls
use their native operation lock and can be observed after the Node moves into a
callback executor.

Reset state is allocated lazily on the first observer. Applications that never
call `observe_resets` incur no additional heap allocation per timer; Humble keeps
only an inline pre-observation counter, while newer RCL versions retain that
counter in the native timer implementation.

Pending resets are delivered before endpoint destruction closes the stream.
Dropping the stream does not destroy or pause its timer, and dropping the timer
does not invalidate already-delivered `TimerReset` values.

Run the finite example:

```sh
cargo run -p r2r --example timer_reset_events
```

It resets an initially paused timer twice before observation, verifies one
coalesced historical event, observes a later reset, and verifies deterministic
stream closure after native timer destruction.
