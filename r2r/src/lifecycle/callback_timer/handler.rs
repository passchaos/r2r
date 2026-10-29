use super::state::CallbackTimerState;
use crate::nodes::timer_callbacks::native::Handler;
use crate::{ClockType, Result};
use std::sync::{Arc, Weak};

pub(super) struct Managed<H> {
    pub(super) handler: H,
    pub(super) state: Weak<CallbackTimerState>,
    pub(super) oneshot: bool,
}
impl<H: Handler> Handler for Managed<H> {
    type Tick = H::Tick;
    type Permit = (Arc<CallbackTimerState>, H::Permit);
    fn permit(&self) -> Option<Self::Permit> {
        let state = self.state.upgrade()?;
        if !state.enabled() {
            return None;
        }
        Some((state, self.handler.permit()?))
    }
    fn advance(timer: *mut r2r_rcl::rcl_timer_t, clock: ClockType) -> Result<Self::Tick> {
        H::advance(timer, clock)
    }
    fn invoke(&mut self, tick: Self::Tick, (state, permit): Self::Permit) -> Result<()> {
        if self.oneshot {
            state.consume_oneshot();
        }
        self.handler.invoke(tick, permit)
    }
}
