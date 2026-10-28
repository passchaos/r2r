//! Weak lifecycle registrations, independent of endpoint ownership.

use std::sync::atomic::Ordering;
use std::sync::Weak;

use parking_lot::Mutex;

use super::publisher::WeakActivation;
use super::timer::state::LifecycleTimerState;

enum ManagedEntity {
    Publisher(WeakActivation),
    Timer(Weak<LifecycleTimerState>),
}

#[derive(Default)]
pub(crate) struct ManagedEntities {
    entities: Mutex<Vec<ManagedEntity>>,
}

impl ManagedEntities {
    pub fn register(&self, entity: WeakActivation) {
        self.entities.lock().push(ManagedEntity::Publisher(entity));
    }

    pub fn register_timer(&self, timer: Weak<LifecycleTimerState>) {
        self.entities.lock().push(ManagedEntity::Timer(timer));
    }

    pub fn set_enabled(&self, enabled: bool) {
        // Waking a timer may synchronously call user code. Detach the registry
        // so reentrant registration never runs under its lock; retain capacity.
        let mut entities = std::mem::take(&mut *self.entities.lock());
        entities.retain(|entity| match entity {
            ManagedEntity::Publisher(weak) => match weak.upgrade() {
                Some(activation) => {
                    activation.store(enabled, Ordering::SeqCst);
                    true
                }
                None => false,
            },
            ManagedEntity::Timer(weak) => match weak.upgrade() {
                Some(timer) => {
                    timer.set_node_active(enabled);
                    true
                }
                None => false,
            },
        });
        let mut registered = self.entities.lock();
        entities.append(&mut registered);
        *registered = entities;
    }
}
