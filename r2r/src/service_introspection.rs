use r2r_rcl::*;

use crate::{
    Client, ClientUntyped, Error, Node, QosProfile, Result, Service, ServiceUntyped,
    WrappedServiceTypeSupport,
};

/// Controls native service event publication on Iron and newer ROS distributions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceIntrospectionState {
    /// Do not publish service events.
    #[default]
    Off,
    /// Publish timestamps, client identity and request sequence numbers.
    Metadata,
    /// Publish metadata and the request or response payload.
    Contents,
}

impl ServiceIntrospectionState {
    fn into_native(self) -> rcl_service_introspection_state_t {
        match self {
            Self::Off => rcl_service_introspection_state_t::RCL_SERVICE_INTROSPECTION_OFF,
            Self::Metadata => rcl_service_introspection_state_t::RCL_SERVICE_INTROSPECTION_METADATA,
            Self::Contents => rcl_service_introspection_state_t::RCL_SERVICE_INTROSPECTION_CONTENTS,
        }
    }
}

fn publisher_options(event_qos: QosProfile) -> rcl_publisher_options_t {
    let mut options = unsafe { rcl_publisher_get_default_options() };
    options.qos = event_qos.into();
    options
}

fn check_result(ret: rcl_ret_t) -> Result<()> {
    if ret == RCL_RET_OK as i32 {
        Ok(())
    } else {
        Err(Error::from_rcl_error(ret))
    }
}

fn configure_client(
    client: &mut rcl_client_t, type_support: &'static rosidl_service_type_support_t,
    node: &mut Node, state: ServiceIntrospectionState, event_qos: QosProfile,
) -> Result<()> {
    let clock = node.get_ros_clock();
    let mut clock = clock.lock();
    check_result(unsafe {
        rcl_client_configure_service_introspection(
            client,
            node.node_handle.as_mut(),
            clock.clock_handle.as_mut(),
            type_support,
            publisher_options(event_qos),
            state.into_native(),
        )
    })
}

pub(crate) fn configure_service(
    service: &mut rcl_service_t, type_support: &'static rosidl_service_type_support_t,
    node: &mut Node, state: ServiceIntrospectionState, event_qos: QosProfile,
) -> Result<()> {
    let clock = node.get_ros_clock();
    let mut clock = clock.lock();
    check_result(unsafe {
        rcl_service_configure_service_introspection(
            service,
            node.node_handle.as_mut(),
            clock.clock_handle.as_mut(),
            type_support,
            publisher_options(event_qos),
            state.into_native(),
        )
    })
}

impl<T: 'static + WrappedServiceTypeSupport> Client<T> {
    /// Configures event publication on `<service_name>/_service_event`.
    ///
    /// Events use the owning node's ROS clock, including simulated time, and `event_qos`.
    /// The node keeps the clock alive until its native endpoints are destroyed. Configuration
    /// is serialized with requests and response handling. Passing a foreign node is rejected.
    pub fn configure_introspection(
        &self, node: &mut Node, state: ServiceIntrospectionState, event_qos: QosProfile,
    ) -> Result<()> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        if !node.owns_client(&client) {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let mut client = client.lock();
        configure_client(&mut client.rcl_handle, T::get_ts(), node, state, event_qos)
    }
}

impl ClientUntyped {
    /// Configures native service events using the owning node's ROS clock.
    ///
    /// See [`Client::configure_introspection`] for publication and ownership behavior.
    pub fn configure_introspection(
        &self, node: &mut Node, state: ServiceIntrospectionState, event_qos: QosProfile,
    ) -> Result<()> {
        let client = self.client.upgrade().ok_or(Error::RCL_RET_CLIENT_INVALID)?;
        if !node.owns_client(&client) {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let mut client = client.lock();
        let type_support = client.service_type.ts;
        configure_client(&mut client.rcl_handle, type_support, node, state, event_qos)
    }
}

impl<T: WrappedServiceTypeSupport> Service<T> {
    /// Configures request-received and response-sent events on `<service_name>/_service_event`.
    ///
    /// Events use the owning node's ROS clock, including simulated time, and `event_qos`.
    /// Configuration is serialized with request/response operations. A foreign node is rejected.
    pub fn configure_introspection(
        &self, node: &mut Node, state: ServiceIntrospectionState, event_qos: QosProfile,
    ) -> Result<()> {
        node.configure_service_introspection(self.id, state, event_qos)
    }
}

impl ServiceUntyped {
    /// Configures native service events using the owning node's ROS clock.
    ///
    /// See [`Service::configure_introspection`] for publication and ownership behavior.
    pub fn configure_introspection(
        &self, node: &mut Node, state: ServiceIntrospectionState, event_qos: QosProfile,
    ) -> Result<()> {
        node.configure_service_introspection(self.id, state, event_qos)
    }
}

#[cfg(all(
    test,
    r2r__example_interfaces__srv__AddTwoInts,
    r2r__rosgraph_msgs__msg__Clock
))]
mod tests;
