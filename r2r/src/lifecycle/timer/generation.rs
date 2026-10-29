//! Shared activation/reset generations for native stream and callback timers.

use std::sync::atomic::{AtomicU64, Ordering};

const NODE_ACTIVE: u64 = 1;
const MANUALLY_ENABLED: u64 = 2;
const FLAGS: u64 = NODE_ACTIVE | MANUALLY_ENABLED;
const GENERATION: u64 = 4;

pub(crate) struct Generation {
    desired: AtomicU64,
    applied: AtomicU64,
}
impl Generation {
    pub(crate) fn new(active: bool, autostart: bool) -> Self {
        Self {
            desired: AtomicU64::new(u64::from(active) | (u64::from(autostart) << 1)),
            applied: AtomicU64::new(u64::MAX),
        }
    }
    fn update(&self, flag: u64, enabled: bool, force: bool) -> bool {
        self.desired
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                let flags = if enabled { old | flag } else { old & !flag };
                if old == flags && !force {
                    None
                } else {
                    Some((old.wrapping_add(GENERATION) & !FLAGS) | (flags & FLAGS))
                }
            })
            .is_ok()
    }
    pub(crate) fn set_active(&self, active: bool) -> bool {
        self.update(NODE_ACTIVE, active, false)
    }
    pub(crate) fn set_manual(&self, enabled: bool, force: bool) {
        self.update(MANUALLY_ENABLED, enabled, force);
    }
    pub(crate) fn current(&self) -> u64 {
        self.desired.load(Ordering::Acquire)
    }
    pub(crate) fn matches(&self, word: u64) -> bool {
        self.applied.load(Ordering::Acquire) == word
    }
    pub(crate) fn mark_applied(&self, word: u64) {
        self.applied.store(word, Ordering::Release);
    }
    pub(crate) fn requested_enabled(word: u64) -> bool {
        word & FLAGS == FLAGS
    }
    pub(crate) fn enabled(&self) -> bool {
        let word = self.current();
        Self::requested_enabled(word) && self.matches(word)
    }
}
