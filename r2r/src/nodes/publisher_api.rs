//! Publisher factories, registry bookkeeping and ownership checks.

use super::*;

impl Node {
    pub(crate) fn publisher_identity(publisher: &Arc<Publisher_>) -> uuid::Uuid {
        publisher.id
    }

    pub(in crate::nodes) fn add_publisher(&mut self, publisher: Arc<Publisher_>) {
        let index = self.pubs.len();
        let identity = Self::publisher_identity(&publisher);
        let previous = self.publisher_indexes.insert(identity, index);
        debug_assert!(previous.is_none());
        self.pubs.push(publisher);
    }

    pub(in crate::nodes) fn remove_dropped_publisher(&mut self, identity: uuid::Uuid) -> bool {
        let Some(index) = self.publisher_indexes.get(&identity).copied() else {
            return true;
        };
        if self.pubs[index].has_pending_waits() {
            return false;
        }
        let removed = self.pubs.swap_remove(index);
        debug_assert_eq!(Self::publisher_identity(&removed), identity);
        self.publisher_indexes.remove(&identity);
        if let Some(moved) = self.pubs.get(index) {
            self.publisher_indexes
                .insert(Self::publisher_identity(moved), index);
        }
        removed.destroy(self.node_handle.as_mut());
        true
    }

    pub(in crate::nodes) fn destroy_publisher_by_id(&mut self, identity: uuid::Uuid) -> bool {
        let Some(index) = self.publisher_indexes.remove(&identity) else {
            return false;
        };
        let removed = self.pubs.swap_remove(index);
        debug_assert_eq!(Self::publisher_identity(&removed), identity);
        if let Some(moved) = self.pubs.get(index) {
            self.publisher_indexes
                .insert(Self::publisher_identity(moved), index);
        }
        self.pending_publisher_removals.remove(&identity);
        self.publishers_with_active_waits.remove(&identity);
        self.dropped_publishers
            .lock()
            .retain(|queued| *queued != identity);
        self.active_wait_publishers
            .lock()
            .retain(|queued| *queued != identity);
        removed.destroy(self.node_handle.as_mut());
        true
    }

    pub(crate) fn owns_publisher(&self, publisher: &Arc<Publisher_>) -> bool {
        self.publisher_indexes
            .contains_key(&Self::publisher_identity(publisher))
    }

    /// Create a ROS publisher.
    pub fn create_publisher<T>(
        &mut self, topic: &str, qos_profile: QosProfile,
    ) -> Result<Publisher<T>>
    where
        T: WrappedTypesupport,
    {
        self.create_publisher_with_options(topic, qos_profile, PublisherOptions::default())
    }

    /// Create a ROS publisher with endpoint creation options.
    pub fn create_publisher_with_options<T>(
        &mut self, topic: &str, qos_profile: QosProfile, options: PublisherOptions,
    ) -> Result<Publisher<T>>
    where
        T: WrappedTypesupport,
    {
        let id = uuid::Uuid::new_v4();
        let publisher_handle = create_publisher_helper(
            self.node_handle.as_mut(),
            topic,
            T::get_ts(),
            qos_profile,
            options,
            id,
            Arc::clone(&self.active_wait_publishers),
        )?;
        let arc = Arc::new(publisher_handle);
        let p = make_publisher(Arc::downgrade(&arc), id, Arc::clone(&self.dropped_publishers));
        self.add_publisher(arc);
        Ok(p)
    }

    /// Create a ROS publisher with a type given at runtime, where the data may either be
    /// supplied as JSON (using the `publish` method) or a pre-serialized ROS message
    /// (i.e. &[u8], using the `publish_raw` method).
    pub fn create_publisher_untyped(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
    ) -> Result<PublisherUntyped> {
        self.create_publisher_untyped_with_options(
            topic,
            topic_type,
            qos_profile,
            PublisherOptions::default(),
        )
    }

    /// Create an untyped ROS publisher with endpoint creation options.
    pub fn create_publisher_untyped_with_options(
        &mut self, topic: &str, topic_type: &str, qos_profile: QosProfile,
        options: PublisherOptions,
    ) -> Result<PublisherUntyped> {
        let dummy = WrappedNativeMsgUntyped::new_from(topic_type)?;
        let id = uuid::Uuid::new_v4();
        let publisher_handle = create_publisher_helper(
            self.node_handle.as_mut(),
            topic,
            dummy.ts,
            qos_profile,
            options,
            id,
            Arc::clone(&self.active_wait_publishers),
        )?;
        let arc = Arc::new(publisher_handle);
        let p = make_publisher_untyped(
            Arc::downgrade(&arc),
            id,
            Arc::clone(&self.dropped_publishers),
            topic_type.to_owned(),
        );
        self.add_publisher(arc);
        Ok(p)
    }

    /// Destroy a ROS publisher.
    pub fn destroy_publisher<T: WrappedTypesupport>(&mut self, p: Publisher<T>) {
        let identity = p.identity();
        if self.destroy_publisher_by_id(identity) {
            p.mark_explicitly_destroyed();
        }
    }

    /// Destroy a ROS publisher.
    pub fn destroy_publisher_untyped(&mut self, p: PublisherUntyped) {
        let identity = p.identity();
        if self.destroy_publisher_by_id(identity) {
            p.mark_explicitly_destroyed();
        }
    }
}
