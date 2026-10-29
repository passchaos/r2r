//! Indexed minimum deadlines with immediate cancellation and reusable generations.

use crate::{Error, Result};
use futures::channel::oneshot;

const VACANT: usize = usize::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Key {
    slot: usize,
    generation: u64,
}

pub(in crate::nodes) struct Entry {
    pub(super) deadline: i64,
    key: Key,
    pub(in crate::nodes) sender: oneshot::Sender<Result<()>>,
}

struct Slot {
    position: usize,
    generation: u64,
    next_free: usize,
}

pub(super) struct Deadlines {
    heap: Vec<Entry>,
    slots: Vec<Slot>,
    free_head: usize,
}

impl Default for Deadlines {
    fn default() -> Self {
        Self {
            heap: Vec::new(),
            slots: Vec::new(),
            free_head: VACANT,
        }
    }
}

impl Deadlines {
    pub(super) fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.heap.len()
    }
    pub(super) fn first(&self) -> Option<i64> {
        self.heap.first().map(|entry| entry.deadline)
    }

    pub(super) fn insert(
        &mut self, deadline: i64, sender: oneshot::Sender<Result<()>>,
    ) -> Result<Key> {
        self.heap
            .try_reserve(1)
            .map_err(|_| Error::RCL_RET_BAD_ALLOC)?;
        while self.free_head != VACANT && self.slots[self.free_head].generation == u64::MAX {
            // A generation is never recycled after overflow, so stale future keys cannot alias.
            self.free_head = self.slots[self.free_head].next_free;
        }
        let slot = if self.free_head == VACANT {
            self.slots
                .try_reserve(1)
                .map_err(|_| Error::RCL_RET_BAD_ALLOC)?;
            let index = self.slots.len();
            self.slots.push(Slot {
                position: VACANT,
                generation: 0,
                next_free: VACANT,
            });
            index
        } else {
            let index = self.free_head;
            self.free_head = self.slots[index].next_free;
            self.slots[index].generation += 1;
            index
        };
        let key = Key {
            slot,
            generation: self.slots[slot].generation,
        };
        let position = self.heap.len();
        self.slots[slot].position = position;
        self.heap.push(Entry {
            deadline,
            key,
            sender,
        });
        self.up(position);
        Ok(key)
    }

    pub(super) fn remove(&mut self, key: Key) -> Option<Entry> {
        let slot = self.slots.get(key.slot)?;
        if slot.generation != key.generation || slot.position == VACANT {
            return None;
        }
        let position = slot.position;
        let removed = self.heap.swap_remove(position);
        self.slots[key.slot].position = VACANT;
        self.slots[key.slot].next_free = self.free_head;
        self.free_head = key.slot;
        if position < self.heap.len() {
            self.slots[self.heap[position].key.slot].position = position;
            if position > 0 && self.less(position, (position - 1) / 2) {
                self.up(position);
            } else {
                self.down(position);
            }
        }
        Some(removed)
    }

    pub(super) fn pop_due(&mut self, now: i64) -> Option<Entry> {
        let first = self.heap.first()?;
        if first.deadline > now {
            return None;
        }
        self.remove(first.key)
    }

    pub(super) fn drain_into(&mut self, output: &mut std::collections::VecDeque<Entry>) {
        for entry in self.heap.drain(..) {
            let slot = &mut self.slots[entry.key.slot];
            slot.position = VACANT;
            slot.next_free = self.free_head;
            self.free_head = entry.key.slot;
            output.push_back(entry);
        }
    }

    fn less(&self, a: usize, b: usize) -> bool {
        (self.heap[a].deadline, self.heap[a].key.slot)
            < (self.heap[b].deadline, self.heap[b].key.slot)
    }
    fn swap(&mut self, a: usize, b: usize) {
        self.heap.swap(a, b);
        self.slots[self.heap[a].key.slot].position = a;
        self.slots[self.heap[b].key.slot].position = b;
    }
    fn up(&mut self, mut position: usize) {
        while position > 0 {
            let parent = (position - 1) / 2;
            if !self.less(position, parent) {
                break;
            }
            self.swap(position, parent);
            position = parent;
        }
    }
    fn down(&mut self, mut position: usize) {
        loop {
            let left = position * 2 + 1;
            if left >= self.heap.len() {
                break;
            }
            let right = left + 1;
            let child = if right < self.heap.len() && self.less(right, left) {
                right
            } else {
                left
            };
            if !self.less(child, position) {
                break;
            }
            self.swap(position, child);
            position = child;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn ros_sleep_heap_matches_sorted_oracle_under_inserts_cancels_and_reused_slots() {
        let mut heap = Deadlines::default();
        let mut oracle = BTreeMap::new();
        let mut keys = Vec::new();
        let mut seed = 91_u64;
        for step in 0..10_000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let deadline = (seed >> 32) as i64;
            let (sender, _receiver) = oneshot::channel();
            let key = heap.insert(deadline, sender).unwrap();
            oracle.insert((deadline, key.slot), key);
            keys.push(key);
            if step % 3 == 0 {
                let key = keys[seed as usize % keys.len()];
                if let Some(removed) = heap.remove(key) {
                    oracle.remove(&(removed.deadline, key.slot));
                }
                assert!(heap.remove(key).is_none());
            }
            if step % 5 == 0 {
                if let Some(expected) = oracle.pop_first() {
                    let removed = heap.pop_due(i64::MAX).unwrap();
                    assert_eq!((removed.deadline, removed.key.slot), expected.0);
                    assert_eq!(removed.key, expected.1);
                } else {
                    assert!(heap.pop_due(i64::MAX).is_none());
                }
            }
            assert_eq!(heap.first(), oracle.first_key_value().map(|(key, _)| key.0));
            for (position, entry) in heap.heap.iter().enumerate() {
                assert_eq!(heap.slots[entry.key.slot].position, position);
            }
        }
    }

    #[test]
    fn ros_sleep_heap_canceled_far_deadlines_leave_no_tombstones_or_slot_growth() {
        let mut heap = Deadlines::default();
        for _ in 0..100_000 {
            let (sender, _receiver) = oneshot::channel();
            let key = heap.insert(i64::MAX, sender).unwrap();
            assert!(heap.remove(key).is_some());
            assert!(heap.is_empty());
            assert_eq!(heap.slots.len(), 1);
        }
    }
}
