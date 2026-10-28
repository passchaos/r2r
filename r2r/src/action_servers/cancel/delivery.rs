//! Goal-indexed retry state allocated only for a large cancellation fanout.

use super::*;

pub(crate) struct BulkDeliveries {
    payloads: Vec<Option<PendingCancelDelivery>>,
    by_goal: HashMap<uuid::Uuid, usize>,
    pub(super) remaining: usize,
    ready: Arc<Mutex<ReadyQueue>>,
    restored: Vec<usize>,
}

pub(crate) struct BulkDetached {
    index: usize,
    delivery: DetachedCancelDelivery,
}

impl BulkDeliveries {
    pub(crate) fn new(deliveries: VecDeque<PendingCancelDelivery>, wake: Waker) -> Self {
        let remaining = deliveries.len();
        let by_goal = deliveries
            .iter()
            .enumerate()
            .map(|(i, d)| (d.uuid, i))
            .collect();
        Self {
            payloads: deliveries.into_iter().map(Some).collect(),
            by_goal,
            remaining,
            ready: ReadyQueue::new(remaining, wake),
            restored: Vec::new(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.remaining == 0
    }

    #[cfg(test)]
    pub(crate) fn has_runnable(&self) -> bool {
        self.ready.lock().has_ready()
    }

    pub(crate) fn take_ready(&mut self, senders: &CancelSenders) -> (Vec<BulkDetached>, bool) {
        let indices = self.ready.lock().take();
        let mut detached = Vec::with_capacity(indices.len());
        for index in indices {
            let Some(payload) = self.payloads[index].take() else {
                continue;
            };
            let Some(sender_slot) = senders.get(&payload.uuid).cloned() else {
                self.remaining -= 1;
                continue;
            };
            let Some(sender) = sender_slot.lock().take() else {
                // The completion that owns this sender re-arms restoration.
                // Detached ownership alone must not create a self-wake loop.
                self.payloads[index] = Some(payload);
                continue;
            };
            detached.push(BulkDetached {
                index,
                delivery: DetachedCancelDelivery {
                    sender_slot,
                    sender,
                    delivery: payload,
                },
            });
        }
        let more = self.ready.lock().has_ready();
        (detached, more)
    }

    pub(crate) fn removed_goal(&mut self, uuid: &uuid::Uuid) -> Option<Waker> {
        let index = *self.by_goal.get(uuid)?;
        let mut ready = self.ready.lock();
        ready.push(index).then(|| ready.notify())
    }

    pub(crate) fn retire_missing_restorations(&mut self, senders: &CancelSenders) {
        for index in self.restored.drain(..) {
            if let Some(payload) = &self.payloads[index] {
                if !senders.contains_key(&payload.uuid) {
                    self.ready.lock().push(index);
                }
            }
        }
    }
}

pub(crate) fn deliver(
    detached: Vec<BulkDetached>, mut pending: PendingCancelRequest,
    finish: impl FnOnce(PendingCancelRequest) -> Result<()> + 'static,
) -> ActionServerCompletion {
    Box::new(move || {
        let bulk = pending.bulk_deliveries.as_mut().unwrap();
        let ready = bulk.ready.clone();
        for BulkDetached {
            index,
            delivery:
                DetachedCancelDelivery {
                    sender_slot,
                    mut sender,
                    delivery,
                },
        } in detached
        {
            match sender.try_send(delivery.request) {
                Ok(()) => bulk.remaining -= 1,
                Err(error) if error.is_full() => {
                    bulk.payloads[index] = Some(PendingCancelDelivery {
                        uuid: delivery.uuid,
                        request: error.into_inner(),
                    });
                    bulk.restored.push(index);
                    let wake = ReadyQueue::waker(&ready, index);
                    if sender
                        .poll_ready(&mut Context::from_waker(&wake))
                        .is_ready()
                    {
                        // Capacity may return between try_send and registration.
                        ready.lock().push(index);
                    }
                }
                Err(_) => bulk.remaining -= 1,
            }
            *sender_slot.lock() = Some(sender);
        }
        let pending_delivery = !bulk.is_empty();
        let result = finish(pending);
        let wake = {
            let ready = ready.lock();
            // The last delivery creates runnable decision registration work.
            // Notify that handoff even when no delivery retry remains.
            (!pending_delivery || ready.has_ready()).then(|| ready.notify())
        };
        if let Some(wake) = wake {
            wake.wake();
        }
        result
    })
}
