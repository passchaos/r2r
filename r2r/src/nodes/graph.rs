//! ROS graph introspection: topic/service/node name and type queries,
//! endpoint metadata and graph-change notification.

use super::*;

impl Node {
    /// Returns a map of topic names and type names of the publishers
    /// visible to this node.
    pub fn get_topic_names_and_types(&self) -> Result<HashMap<String, Vec<String>>> {
        self.get_topic_names_and_types_impl(false)
    }

    /// Returns raw middleware topic names and type names without ROS demangling.
    ///
    /// This exposes non-ROS topics and their middleware-specific names for diagnostics and
    /// interoperability. Prefer [`Node::get_topic_names_and_types`] for normal ROS topic
    /// discovery.
    pub fn get_raw_topic_names_and_types(&self) -> Result<HashMap<String, Vec<String>>> {
        self.get_topic_names_and_types_impl(true)
    }

    fn get_topic_names_and_types_impl(
        &self, no_demangle: bool,
    ) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types(|node, allocator, names_and_types| unsafe {
            rcl_get_topic_names_and_types(node, allocator, no_demangle, names_and_types)
        })
    }

    /// Returns a future that completes after the ROS graph next changes.
    ///
    /// The node must be spun while awaiting. All waiters registered before the same graph change
    /// are completed together.
    pub fn wait_for_graph_change(&mut self) -> impl Future<Output = Result<()>> {
        let (sender, receiver) = oneshot::channel();
        self.graph_change_waiters.register(sender);
        receiver.map_err(|_| Error::RCL_RET_NODE_INVALID)
    }

    /// Waits for the next ROS graph change up to `timeout`. The node must be spun.
    pub fn wait_for_graph_change_with_timeout(
        &mut self, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        Self::validate_timeout(timeout)?;
        let changed = self.wait_for_graph_change();
        self.with_timeout(changed, timeout)
    }

    /// Returns a map of service names and type names visible to this node.
    pub fn get_service_names_and_types(&self) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types(|node, allocator, names_and_types| unsafe {
            rcl_get_service_names_and_types(node, allocator, names_and_types)
        })
    }

    /// Returns fully qualified names of nodes visible in the ROS graph.
    pub fn get_node_names(&self) -> Result<Vec<String>> {
        self.get_node_names_and_namespaces().map(|nodes| {
            nodes
                .into_iter()
                .map(|(name, namespace)| qualify_node_name(&name, &namespace))
                .collect()
        })
    }

    /// Returns node names and namespaces visible in the ROS graph.
    pub fn get_node_names_and_namespaces(&self) -> Result<Vec<(String, String)>> {
        let mut names = StringArray::new();
        let mut namespaces = StringArray::new();
        let ret = unsafe {
            rcl_get_node_names(
                self.node_handle.as_ref(),
                rcutils_get_default_allocator(),
                &mut names.0,
                &mut namespaces.0,
            )
        };
        check_rcl_ret(ret)?;
        zip_graph_strings(names.to_vec()?, namespaces.to_vec()?)
    }

    /// Returns node names, namespaces, and security enclaves visible in the ROS graph.
    pub fn get_node_names_with_enclaves(&self) -> Result<Vec<(String, String, String)>> {
        let mut names = StringArray::new();
        let mut namespaces = StringArray::new();
        let mut enclaves = StringArray::new();
        let ret = unsafe {
            rcl_get_node_names_with_enclaves(
                self.node_handle.as_ref(),
                rcutils_get_default_allocator(),
                &mut names.0,
                &mut namespaces.0,
                &mut enclaves.0,
            )
        };
        check_rcl_ret(ret)?;
        let names = names.to_vec()?;
        let namespaces = namespaces.to_vec()?;
        let enclaves = enclaves.to_vec()?;
        if names.len() != namespaces.len() || names.len() != enclaves.len() {
            return Err(Error::RCL_RET_ERROR);
        }
        Ok(names
            .into_iter()
            .zip(namespaces)
            .zip(enclaves)
            .map(|((name, namespace), enclave)| (name, namespace, enclave))
            .collect())
    }

    /// Returns the number of publishers advertised on `topic_name`.
    ///
    /// Relative and private names are expanded in this node's namespace without applying remaps.
    pub fn count_publishers(&self, topic_name: &str) -> Result<usize> {
        self.count_graph_entities(topic_name, false, |node, name, count| unsafe {
            rcl_count_publishers(node, name, count)
        })
    }

    /// Returns the number of subscriptions created for `topic_name`.
    ///
    /// Relative and private names are expanded in this node's namespace without applying remaps.
    pub fn count_subscribers(&self, topic_name: &str) -> Result<usize> {
        self.count_graph_entities(topic_name, false, |node, name, count| unsafe {
            rcl_count_subscribers(node, name, count)
        })
    }

    /// Returns the number of clients created for `service_name`.
    ///
    /// Relative and private names are expanded in this node's namespace without applying remaps.
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    pub fn count_clients(&self, service_name: &str) -> Result<usize> {
        self.count_graph_entities(service_name, true, |node, name, count| unsafe {
            rcl_count_clients(node, name, count)
        })
    }

    /// Returns the number of servers created for `service_name`.
    ///
    /// Relative and private names are expanded in this node's namespace without applying remaps.
    #[cfg(not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)))]
    pub fn count_services(&self, service_name: &str) -> Result<usize> {
        self.count_graph_entities(service_name, true, |node, name, count| unsafe {
            rcl_count_services(node, name, count)
        })
    }

    fn count_graph_entities(
        &self, name: &str, is_service: bool,
        query: impl FnOnce(*const rcl_node_t, *const std::os::raw::c_char, *mut usize) -> rcl_ret_t,
    ) -> Result<usize> {
        let name = self.resolve_name(name, is_service, true)?;
        let name = CString::new(name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let mut count = 0;
        let ret = query(self.node_handle.as_ref(), name.as_ptr(), &mut count);
        check_rcl_ret(ret)?;
        Ok(count)
    }

    /// Returns publisher topic names and types associated with a remote node.
    pub fn get_publisher_names_and_types_by_node(
        &self, node_name: &str, node_namespace: &str, no_demangle: bool,
    ) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types_by_node(
            node_name,
            node_namespace,
            |node, allocator, name, namespace, names_and_types| unsafe {
                rcl_get_publisher_names_and_types_by_node(
                    node,
                    allocator,
                    no_demangle,
                    name,
                    namespace,
                    names_and_types,
                )
            },
        )
    }

    /// Returns subscription topic names and types associated with a remote node.
    pub fn get_subscription_names_and_types_by_node(
        &self, node_name: &str, node_namespace: &str, no_demangle: bool,
    ) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types_by_node(
            node_name,
            node_namespace,
            |node, allocator, name, namespace, names_and_types| unsafe {
                rcl_get_subscriber_names_and_types_by_node(
                    node,
                    allocator,
                    no_demangle,
                    name,
                    namespace,
                    names_and_types,
                )
            },
        )
    }

    /// Returns service names and types associated with a remote node.
    pub fn get_service_names_and_types_by_node(
        &self, node_name: &str, node_namespace: &str,
    ) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types_by_node(
            node_name,
            node_namespace,
            |node, allocator, name, namespace, names_and_types| unsafe {
                rcl_get_service_names_and_types_by_node(
                    node,
                    allocator,
                    name,
                    namespace,
                    names_and_types,
                )
            },
        )
    }

    /// Returns service client names and types associated with a remote node.
    pub fn get_client_names_and_types_by_node(
        &self, node_name: &str, node_namespace: &str,
    ) -> Result<HashMap<String, Vec<String>>> {
        self.get_names_and_types_by_node(
            node_name,
            node_namespace,
            |node, allocator, name, namespace, names_and_types| unsafe {
                rcl_get_client_names_and_types_by_node(
                    node,
                    allocator,
                    name,
                    namespace,
                    names_and_types,
                )
            },
        )
    }

    fn get_names_and_types_by_node(
        &self, node_name: &str, node_namespace: &str,
        query: impl FnOnce(
            *const rcl_node_t,
            *mut rcutils_allocator_t,
            *const std::os::raw::c_char,
            *const std::os::raw::c_char,
            *mut rcl_names_and_types_t,
        ) -> rcl_ret_t,
    ) -> Result<HashMap<String, Vec<String>>> {
        let node_name = CString::new(node_name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let node_namespace =
            CString::new(node_namespace).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        self.get_names_and_types(|node, allocator, names_and_types| {
            query(node, allocator, node_name.as_ptr(), node_namespace.as_ptr(), names_and_types)
        })
    }

    fn get_names_and_types(
        &self,
        query: impl FnOnce(
            *const rcl_node_t,
            *mut rcutils_allocator_t,
            *mut rmw_names_and_types_t,
        ) -> rcl_ret_t,
    ) -> Result<HashMap<String, Vec<String>>> {
        let mut tnat = unsafe { rmw_get_zero_initialized_names_and_types() };
        let mut allocator = unsafe { rcutils_get_default_allocator() };
        let ret = query(self.node_handle.as_ref(), &mut allocator, &mut tnat);
        if ret != RCL_RET_OK as i32 {
            log::error!("could not get ROS graph names and types {}", ret);
            return Err(Error::from_rcl_error(ret));
        }
        let tnat = NamesAndTypes(tnat);

        let mut res = HashMap::new();
        if tnat.0.names.size > 0 {
            if tnat.0.names.data.is_null() || tnat.0.types.is_null() {
                return Err(Error::RCL_RET_ERROR);
            }
            let names = unsafe { std::slice::from_raw_parts(tnat.0.names.data, tnat.0.names.size) };
            let types = unsafe { std::slice::from_raw_parts(tnat.0.types, tnat.0.names.size) };

            for (n, t) in names.iter().zip(types) {
                let entity_name = c_string_from_ptr(*n)?;
                if t.size > 0 && t.data.is_null() {
                    return Err(Error::RCL_RET_ERROR);
                }
                let entity_types = if t.size == 0 {
                    Vec::new()
                } else {
                    unsafe { std::slice::from_raw_parts(t.data, t.size) }
                        .iter()
                        .map(|type_name| c_string_from_ptr(*type_name))
                        .collect::<Result<Vec<_>>>()?
                };
                res.insert(entity_name, entity_types);
            }
        }
        Ok(res)
    }

    /// Returns publisher endpoint information for a topic.
    ///
    /// When `no_mangle` is false, relative and private names are expanded in this node's
    /// namespace without applying remaps. When it is true, `topic_name` is passed through as a
    /// middleware topic name.
    pub fn get_publishers_info_by_topic(
        &self, topic_name: &str, no_mangle: bool,
    ) -> Result<Vec<TopicEndpointInfo>> {
        self.get_endpoint_info_by_topic(
            topic_name,
            no_mangle,
            |node, allocator, name, raw| unsafe {
                rcl_get_publishers_info_by_topic(node, allocator, name, no_mangle, raw)
            },
        )
    }

    /// Returns information about subscriptions visible on `topic_name`.
    ///
    /// When `no_mangle` is false, relative and private names are expanded in this node's
    /// namespace without applying remaps. When it is true, `topic_name` is passed through as a
    /// middleware topic name.
    pub fn get_subscriptions_info_by_topic(
        &self, topic_name: &str, no_mangle: bool,
    ) -> Result<Vec<TopicEndpointInfo>> {
        self.get_endpoint_info_by_topic(
            topic_name,
            no_mangle,
            |node, allocator, name, raw| unsafe {
                rcl_get_subscriptions_info_by_topic(node, allocator, name, no_mangle, raw)
            },
        )
    }

    fn get_endpoint_info_by_topic(
        &self, topic_name: &str, no_mangle: bool,
        query: impl FnOnce(
            *const rcl_node_t,
            *mut rcutils_allocator_t,
            *const std::os::raw::c_char,
            *mut rcl_topic_endpoint_info_array_t,
        ) -> rcl_ret_t,
    ) -> Result<Vec<TopicEndpointInfo>> {
        let topic_name = if no_mangle {
            topic_name.to_owned()
        } else {
            self.resolve_topic_name_without_remapping(topic_name)?
        };
        let topic_name = CString::new(topic_name).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let mut info_array = TopicEndpointInfoArray::new();
        let ret = query(
            self.node_handle.as_ref(),
            &mut info_array.allocator,
            topic_name.as_ptr(),
            &mut info_array.value,
        );
        check_rcl_ret(ret)?;
        convert_info_array_to_vec(&info_array.value)
    }
}

struct NamesAndTypes(rmw_names_and_types_t);

impl Drop for NamesAndTypes {
    fn drop(&mut self) {
        let ret = unsafe { rmw_names_and_types_fini(&mut self.0) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize topic names and types: {ret}");
        }
    }
}

struct TopicEndpointInfoArray {
    value: rcl_topic_endpoint_info_array_t,
    allocator: rcutils_allocator_t,
}

struct StringArray(rcutils_string_array_t);

impl StringArray {
    fn new() -> Self {
        Self(unsafe { rcutils_get_zero_initialized_string_array() })
    }

    fn to_vec(&self) -> Result<Vec<String>> {
        if self.0.size == 0 {
            return Ok(Vec::new());
        }
        if self.0.data.is_null() {
            return Err(Error::RCL_RET_ERROR);
        }
        unsafe { std::slice::from_raw_parts(self.0.data, self.0.size) }
            .iter()
            .map(|value| c_string_from_ptr(*value))
            .collect()
    }
}

impl Drop for StringArray {
    fn drop(&mut self) {
        let ret = unsafe { rcutils_string_array_fini(&mut self.0) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize ROS graph string array: {ret}");
        }
    }
}

impl TopicEndpointInfoArray {
    fn new() -> Self {
        Self {
            value: unsafe { rmw_get_zero_initialized_topic_endpoint_info_array() },
            allocator: unsafe { rcutils_get_default_allocator() },
        }
    }
}

impl Drop for TopicEndpointInfoArray {
    fn drop(&mut self) {
        let ret =
            unsafe { rmw_topic_endpoint_info_array_fini(&mut self.value, &mut self.allocator) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize topic endpoint information: {ret}");
        }
    }
}

pub(crate) fn c_string_from_ptr(ptr: *const std::os::raw::c_char) -> Result<String> {
    if ptr.is_null() {
        return Err(Error::RCL_RET_ERROR);
    }
    c_str_to_owned(unsafe { CStr::from_ptr(ptr) }, "ROS graph string")
}

fn zip_graph_strings(names: Vec<String>, namespaces: Vec<String>) -> Result<Vec<(String, String)>> {
    if names.len() != namespaces.len() {
        return Err(Error::RCL_RET_ERROR);
    }
    Ok(names.into_iter().zip(namespaces).collect())
}

fn qualify_node_name(name: &str, namespace: &str) -> String {
    match namespace {
        "" | "/" => format!("/{name}"),
        namespace if namespace.ends_with('/') => format!("{namespace}{name}"),
        namespace => format!("{namespace}/{name}"),
    }
}

/// Whether graph endpoint information describes a publisher or a subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TopicEndpointType {
    Invalid,
    Publisher,
    Subscription,
}

/// A ROS interface type hash as defined by REP-2011.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TopicTypeHash {
    /// Hash format version. Zero means that no hash was supplied by the middleware.
    pub version: u8,
    /// SHA-256 bytes associated with `version`.
    pub value: [u8; 32],
}

impl TopicTypeHash {
    pub fn is_unset(&self) -> bool {
        self.version == 0
    }
}

impl std::fmt::Display for TopicTypeHash {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "RIHS{:02x}_", self.version)?;
        for byte in self.value {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(not(r2r__ros__distro__humble))]
fn topic_type_hash_from_native(value: &rosidl_type_hash_t) -> TopicTypeHash {
    TopicTypeHash {
        version: value.version,
        value: value.value,
    }
}

impl TryFrom<rmw_endpoint_type_t> for TopicEndpointType {
    type Error = Error;

    fn try_from(value: rmw_endpoint_type_t) -> Result<Self> {
        match value {
            rmw_endpoint_type_t::RMW_ENDPOINT_INVALID => Ok(Self::Invalid),
            rmw_endpoint_type_t::RMW_ENDPOINT_PUBLISHER => Ok(Self::Publisher),
            rmw_endpoint_type_t::RMW_ENDPOINT_SUBSCRIPTION => Ok(Self::Subscription),
        }
    }
}

/// Metadata for one publisher or subscription discovered in the ROS graph.
#[derive(Debug, Clone, PartialEq)]
pub struct TopicEndpointInfo {
    pub node_name: String,
    pub node_namespace: String,
    pub topic_type: String,
    /// Topic schema hash when supported by the ROS distribution (Iron and newer).
    pub topic_type_hash: Option<TopicTypeHash>,
    pub endpoint_type: TopicEndpointType,
    pub endpoint_gid: [u8; RMW_GID_STORAGE_SIZE as usize],
    pub qos_profile: QosProfile,
}

impl TopicEndpointInfo {
    fn try_from_native(info: rmw_topic_endpoint_info_t) -> Result<Self> {
        Ok(Self {
            node_name: c_string_from_ptr(info.node_name)?,
            node_namespace: c_string_from_ptr(info.node_namespace)?,
            topic_type: c_string_from_ptr(info.topic_type)?,
            #[cfg(not(r2r__ros__distro__humble))]
            topic_type_hash: Some(topic_type_hash_from_native(&info.topic_type_hash)),
            #[cfg(r2r__ros__distro__humble)]
            topic_type_hash: None,
            endpoint_type: info.endpoint_type.try_into()?,
            endpoint_gid: info.endpoint_gid,
            qos_profile: QosProfile::try_from(&info.qos_profile)?,
        })
    }
}

fn convert_info_array_to_vec(
    info_array: &rcl_topic_endpoint_info_array_t,
) -> Result<Vec<TopicEndpointInfo>> {
    let mut topic_info_list = Vec::with_capacity(info_array.size);

    if info_array.size > 0 {
        if info_array.info_array.is_null() {
            return Err(Error::RCL_RET_ERROR);
        }
        let infos = unsafe { std::slice::from_raw_parts(info_array.info_array, info_array.size) };
        for &info in infos {
            topic_info_list.push(TopicEndpointInfo::try_from_native(info)?);
        }
    }

    Ok(topic_info_list)
}
