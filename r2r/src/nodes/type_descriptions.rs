//! Standard native type-description endpoint startup and teardown.

use super::*;

#[cfg(all(
    not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
    r2r__type_description_interfaces__srv__GetTypeDescription
))]
mod native;

impl Node {
    /// Start `~/get_type_description`, answering directly from RCL's live type cache during spin.
    /// Returns `false` when this node already owns the service. Humble/Iron or a filtered-out
    /// `type_description_interfaces` package return `RCL_RET_UNSUPPORTED`.
    pub fn start_type_description_service(&mut self) -> Result<bool> {
        self.start_type_description_service_native(None)
    }

    /// Start the standard endpoint with explicit request/response DDS history and QoS.
    /// Existing registrations are left unchanged; stop the service before changing QoS.
    pub fn start_type_description_service_with_qos(&mut self, qos: QosProfile) -> Result<bool> {
        self.start_type_description_service_native(Some(qos))
    }

    fn start_type_description_service_native(&mut self, _qos: Option<QosProfile>) -> Result<bool> {
        #[cfg(all(
            not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
            r2r__type_description_interfaces__srv__GetTypeDescription
        ))]
        {
            if self
                .type_description_service
                .is_some_and(|id| self.service_indexes.contains_key(&id))
            {
                return Ok(false);
            }
            let id = uuid::Uuid::new_v4();
            let service =
                native::TypeDescriptionService::create(self.node_handle.as_mut(), id, _qos)?;
            self.add_service(id, service);
            self.type_description_service = Some(id);
            Ok(true)
        }
        #[cfg(not(all(
            not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
            r2r__type_description_interfaces__srv__GetTypeDescription
        )))]
        {
            Err(Error::RCL_RET_UNSUPPORTED)
        }
    }

    /// Stop this node's standard type-description endpoint, leaving other service IDs intact.
    pub fn stop_type_description_service(&mut self) -> bool {
        let Some(id) = self.type_description_service.take() else {
            return false;
        };
        Self::remove_service(
            &mut self.services,
            &mut self.service_indexes,
            id,
            self.node_handle.as_mut(),
        )
    }

    /// Return the resolved native endpoint name after namespace expansion and ROS remapping.
    pub fn type_description_service_name(&self) -> Result<Option<String>> {
        self.type_description_service
            .filter(|id| self.service_indexes.contains_key(id))
            .map(|id| self.get_service_name(id))
            .transpose()
    }

    #[cfg(all(
        not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
        r2r__type_description_interfaces__srv__GetTypeDescription
    ))]
    pub fn create_type_description_client(
        &mut self, remote_node_name: &str,
    ) -> Result<crate::TypeDescriptionClient> {
        self.create_type_description_client_with_qos(
            remote_node_name,
            QosProfile::services_default(),
        )
    }

    #[cfg(all(
        not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
        r2r__type_description_interfaces__srv__GetTypeDescription
    ))]
    pub fn create_type_description_client_with_qos(
        &mut self, remote_node_name: &str, qos: QosProfile,
    ) -> Result<crate::TypeDescriptionClient> {
        crate::TypeDescriptionClient::new(self, remote_node_name, qos)
    }
}

#[cfg(test)]
mod tests;
