use super::*;

pub(in crate::nodes) struct Sources {
    pub(in crate::nodes) node_id: uuid::Uuid,
    pub(super) rows: Vec<Box<dyn Source>>,
    pub(super) indexes: HashMap<uuid::Uuid, usize>,
    active: Vec<(usize, usize)>,
}
impl Default for Sources {
    fn default() -> Self {
        Self {
            node_id: uuid::Uuid::new_v4(),
            rows: Vec::new(),
            indexes: HashMap::new(),
            active: Vec::new(),
        }
    }
}
impl Sources {
    pub(in crate::nodes) fn remove(
        &mut self, id: uuid::Uuid, node: &mut rcl_node_t,
        events: &mut Option<Box<qos_events::QosEvents>>,
    ) -> bool {
        let Some(index) = self.indexes.get(&id).copied() else {
            return false;
        };
        self.active.clear();
        self.remove_index(index, node, events);
        true
    }
    pub(in crate::nodes) fn groups(&self) -> impl Iterator<Item = CallbackGroupId> + '_ {
        self.rows
            .iter()
            .filter(|row| !row.is_closed())
            .map(|row| row.group_id())
    }
    pub(in crate::nodes) fn subscriber(&self, id: uuid::Uuid) -> Result<&dyn Subscriber_> {
        self.indexes
            .get(&id)
            .and_then(|index| self.rows.get(*index))
            .and_then(|row| row.subscriber())
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)
    }
    pub(in crate::nodes) fn subscriber_mut(
        &mut self, id: uuid::Uuid,
    ) -> Result<&mut (dyn Subscriber_ + '_)> {
        let index = *self
            .indexes
            .get(&id)
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)?;
        self.rows[index]
            .subscriber_mut()
            .ok_or(Error::RCL_RET_SUBSCRIPTION_INVALID)
    }
    pub(in crate::nodes) fn set_wake(&mut self, wake: Option<Waker>) {
        for row in &mut self.rows {
            row.set_wake(wake.clone());
        }
    }
    fn remove_index(
        &mut self, index: usize, node: &mut rcl_node_t,
        events: &mut Option<Box<qos_events::QosEvents>>,
    ) {
        let id = self.rows[index].id();
        if self.rows[index].subscriber().is_some() {
            if let Some(events) = events {
                events.remove_endpoint(qos_events::EventEndpoint::Subscription(id));
            }
        }
        self.indexes.remove(&id);
        let mut row = self.rows.swap_remove(index);
        row.destroy(node);
        if let Some(row) = self.rows.get(index) {
            self.indexes.insert(row.id(), index);
        }
    }
    pub(in crate::nodes) fn prepare(
        &mut self, node: &mut rcl_node_t, events: &mut Option<Box<qos_events::QosEvents>>,
    ) -> Result<WaitSetCapacities> {
        self.active.clear();
        let mut index = 0;
        let mut capacities = WaitSetCapacities::default();
        while index < self.rows.len() {
            if self.rows[index].is_closed() {
                self.remove_index(index, node, events);
            } else {
                self.rows[index].prepare()?;
                if self.rows[index].is_waitable() {
                    self.active.push((index, 0));
                    capacities.add(self.rows[index].capacities());
                }
                index += 1;
            }
        }
        Ok(capacities)
    }
    pub(in crate::nodes) fn add_to_wait_set(&mut self, ws: &mut rcl_wait_set_t) -> Result<()> {
        for (source, index) in &mut self.active {
            self.rows[*source].register(ws, index)?;
        }
        Ok(())
    }
    pub(in crate::nodes) fn dispatch(&mut self, ws: &rcl_wait_set_t) -> Result<()> {
        let mut error = None;
        for &(source, index) in &self.active {
            let row = &mut self.rows[source];
            if !row.ready(ws, index)? {
                continue;
            }
            for _ in 0..row.maximum() {
                match row.take() {
                    Ok(true) => {}
                    Ok(false) => break,
                    Err(failure) => {
                        error.get_or_insert(failure);
                        break;
                    }
                }
            }
        }
        error.map_or(Ok(()), Err)
    }
    pub(in crate::nodes) fn destroy_all(&mut self, node: &mut rcl_node_t) {
        for row in &mut self.rows {
            row.destroy(node);
        }
    }
}
