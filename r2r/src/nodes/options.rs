//! Node creation options and their native rcl conversion.

use super::*;

pub(crate) struct NodeOptionsNative(pub(crate) rcl_node_options_t);

impl NodeOptionsNative {
    pub(crate) fn new(config: &NodeOptions) -> Result<Self> {
        let mut options = unsafe { rcl_node_get_default_options() };
        options.use_global_arguments = config.use_global_arguments;
        options.enable_rosout = config.enable_rosout;
        if let Some(qos) = &config.rosout_qos {
            options.rosout_qos = qos.clone().into();
        }
        let arguments = config
            .arguments
            .iter()
            .map(|argument| {
                CString::new(argument.as_str()).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)
            })
            .collect::<Result<Vec<_>>>()?;
        let argument_ptrs = arguments
            .iter()
            .map(|argument| argument.as_ptr())
            .collect::<Vec<_>>();
        let argc =
            i32::try_from(argument_ptrs.len()).map_err(|_| Error::RCL_RET_INVALID_ARGUMENT)?;
        let argv = if argument_ptrs.is_empty() {
            std::ptr::null()
        } else {
            argument_ptrs.as_ptr()
        };
        let ret =
            unsafe { rcl_parse_arguments(argc, argv, options.allocator, &mut options.arguments) };
        if ret != RCL_RET_OK as i32 {
            unsafe { rcl_node_options_fini(&mut options) };
            return Err(Error::from_rcl_error(ret));
        }
        if unsafe { rcl_arguments_get_count_unparsed_ros(&options.arguments) } != 0 {
            unsafe { rcl_node_options_fini(&mut options) };
            return Err(Error::RCL_RET_INVALID_ROS_ARGS);
        }
        Ok(Self(options))
    }
}

impl Drop for NodeOptionsNative {
    fn drop(&mut self) {
        let ret = unsafe { rcl_node_options_fini(&mut self.0) };
        if ret != RCL_RET_OK as i32 {
            log::error!("could not finalize node options: {ret}");
        }
    }
}

/// Options applied while creating a [`Node`].
#[derive(Debug, Clone)]
pub struct NodeOptions {
    /// ROS-specific arguments applied only to this node.
    pub arguments: Vec<String>,
    /// Whether the node consumes global ROS arguments from its [`Context`].
    pub use_global_arguments: bool,
    /// Programmatic parameter overrides. These take precedence over global CLI/YAML overrides.
    pub parameter_overrides: IndexMap<String, ParameterValue>,
    /// Whether parameter overrides are immediately exposed as declared parameters.
    pub automatically_declare_parameters_from_overrides: bool,
    /// Whether setting an undeclared parameter implicitly declares it.
    pub allow_undeclared_parameters: bool,
    /// Whether the node publishes logs on `/rosout`.
    pub enable_rosout: bool,
    /// Optional override for the `/rosout` publisher QoS.
    pub rosout_qos: Option<QosProfile>,
}

impl Default for NodeOptions {
    fn default() -> Self {
        Self {
            arguments: Vec::new(),
            use_global_arguments: true,
            parameter_overrides: IndexMap::new(),
            automatically_declare_parameters_from_overrides: true,
            allow_undeclared_parameters: true,
            enable_rosout: true,
            rosout_qos: None,
        }
    }
}

impl NodeOptions {
    /// Replaces the ROS arguments applied only to this node.
    ///
    /// Include an executable name as the first item when matching normal process argv semantics.
    pub fn arguments<I, S>(mut self, arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.arguments = arguments.into_iter().map(Into::into).collect();
        self
    }

    pub fn use_global_arguments(mut self, enabled: bool) -> Self {
        self.use_global_arguments = enabled;
        self
    }

    pub fn parameter_override(
        mut self, name: impl Into<String>, value: impl Into<ParameterValue>,
    ) -> Self {
        self.parameter_overrides.insert(name.into(), value.into());
        self
    }

    pub fn parameter_overrides<I, N, V>(mut self, overrides: I) -> Self
    where
        I: IntoIterator<Item = (N, V)>,
        N: Into<String>,
        V: Into<ParameterValue>,
    {
        self.parameter_overrides.extend(
            overrides
                .into_iter()
                .map(|(name, value)| (name.into(), value.into())),
        );
        self
    }

    /// Controls whether overrides are exposed as parameters during node creation.
    ///
    /// Disabling this keeps overrides available to later explicit declarations without making
    /// them visible through parameter lookup or listing beforehand. The default is `true` for
    /// compatibility with r2r's historical behavior.
    pub fn automatically_declare_parameters_from_overrides(mut self, enabled: bool) -> Self {
        self.automatically_declare_parameters_from_overrides = enabled;
        self
    }

    /// Controls whether setting an undeclared parameter implicitly declares it.
    ///
    /// Disable this to catch misspelled or missing declarations before callbacks, providers, or
    /// parameter state are changed. The default is `true` for compatibility with r2r's historical
    /// behavior.
    pub fn allow_undeclared_parameters(mut self, enabled: bool) -> Self {
        self.allow_undeclared_parameters = enabled;
        self
    }

    pub fn enable_rosout(mut self, enabled: bool) -> Self {
        self.enable_rosout = enabled;
        self
    }

    pub fn rosout_qos(mut self, qos: QosProfile) -> Self {
        self.rosout_qos = Some(qos);
        self
    }
}

/// Options for the standard parameter services and event publisher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterHandlerOptions {
    /// Whether to create the six standard parameter service endpoints.
    pub start_parameter_services: bool,
    /// Whether to publish parameter changes on `/parameter_events`.
    pub start_parameter_event_publisher: bool,
    /// QoS used by each standard parameter service.
    pub service_qos: QosProfile,
    /// QoS used by the `/parameter_events` publisher.
    pub parameter_event_qos: QosProfile,
}

impl Default for ParameterHandlerOptions {
    fn default() -> Self {
        Self {
            start_parameter_services: true,
            start_parameter_event_publisher: true,
            service_qos: QosProfile::default(),
            parameter_event_qos: QosProfile::parameter_events(),
        }
    }
}

impl ParameterHandlerOptions {
    pub const fn start_parameter_services(mut self, enabled: bool) -> Self {
        self.start_parameter_services = enabled;
        self
    }

    pub const fn start_parameter_event_publisher(mut self, enabled: bool) -> Self {
        self.start_parameter_event_publisher = enabled;
        self
    }

    pub fn service_qos(mut self, qos: QosProfile) -> Self {
        self.service_qos = qos;
        self
    }

    pub fn parameter_event_qos(mut self, qos: QosProfile) -> Self {
        self.parameter_event_qos = qos;
        self
    }
}
