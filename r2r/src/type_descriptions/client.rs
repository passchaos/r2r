use super::sources::SourceFiles;
use super::*;
use crate::parking_lot::Mutex;
use crate::{
    msg_types::WrappedNativeMsg,
    type_description_interfaces::{msg, srv::GetTypeDescription},
    Client, Node, QosProfile,
};
use futures::{Future, TryFutureExt};
use std::{sync::Arc, time::Duration};

/// An asynchronous client for a node's standard `~/get_type_description` service.
///
/// The server's schema name and calculated RIHS hash are validated before a successful query is
/// returned. The owning node/executor must be driven while awaiting. Clones share a native client.
#[derive(Clone)]
pub struct TypeDescriptionClient {
    client: Client<GetTypeDescription::Service>,
    sources: Arc<Mutex<SourceFiles>>,
}

impl TypeDescriptionClient {
    pub(crate) fn new(node: &mut Node, remote_node_name: &str, qos: QosProfile) -> Result<Self> {
        if remote_node_name.is_empty() || remote_node_name.as_bytes().contains(&0) {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let name = remote_node_name.trim_end_matches('/');
        if name.is_empty() {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        Ok(Self {
            sources: Arc::new(Mutex::new(SourceFiles::default())),
            client: node.create_client(&format!("{name}/get_type_description"), qos)?,
        })
    }

    pub fn service_name(&self) -> Result<String> {
        self.client.service_name()
    }
    pub fn wait_for_service(&self) -> Result<impl Future<Output = Result<()>>> {
        self.client.wait_for_service()
    }
    pub fn wait_for_service_with_timeout(
        &self, node: &mut Node, timeout: Duration,
    ) -> Result<impl Future<Output = Result<()>>> {
        self.client.wait_for_service_with_timeout(node, timeout)
    }

    /// Query a graph/type-support hash, validating the returned name and complete schema hash.
    /// `include_type_sources` selects the native IDL/msg/service/action source texts.
    pub fn describe(
        &self, type_name: &str, hash: &TopicTypeHash, include_type_sources: bool,
    ) -> Result<impl Future<Output = Result<GetTypeDescription::Response>>> {
        if type_name.is_empty() || type_name.as_bytes().contains(&0) || hash.is_unset() {
            return Err(Error::RCL_RET_INVALID_ARGUMENT);
        }
        let name = type_name.to_owned();
        let hash = *hash;
        let response = self.client.request(&GetTypeDescription::Request {
            type_name: name.clone().into(),
            type_hash: hash.to_string().into(),
            include_type_sources,
        })?;
        let sources = self.sources.clone();
        Ok(response.and_then(move |mut response| async move {
            if !response.successful {
                return Err(Error::TypeDescriptionUnavailable {
                    reason: String::from_utf8_lossy(response.failure_reason.as_bytes())
                        .into_owned(),
                });
            }
            let actual_name = response
                .type_description
                .type_description
                .type_name
                .try_to_str()
                .map_err(|error| Error::InvalidUtf8 {
                    context: "type description name",
                    err: error.to_string(),
                })?;
            if actual_name != name {
                return Err(Error::TypeDescriptionIdentityMismatch {
                    expected: name,
                    actual: actual_name.into(),
                });
            }
            let actual = type_description_hash(&response.type_description)?;
            if actual != hash {
                return Err(Error::TypeDescriptionIdentityMismatch {
                    expected: hash.to_string(),
                    actual: actual.to_string(),
                });
            }
            if !response.type_sources.is_empty() {
                let mut sources = sources.lock();
                for source in &mut response.type_sources {
                    let type_name =
                        source
                            .type_name
                            .try_to_str()
                            .map_err(|error| Error::InvalidUtf8 {
                                context: "source type name",
                                err: error.to_string(),
                            })?;
                    let encoding =
                        source
                            .encoding
                            .try_to_str()
                            .map_err(|error| Error::InvalidUtf8 {
                                context: "source encoding",
                                err: error.to_string(),
                            })?;
                    sources.repair_rust(type_name, encoding, &mut source.raw_file_contents)?;
                }
            }
            Ok(response)
        }))
    }

    pub fn describe_with_timeout(
        &self, node: &mut Node, type_name: &str, hash: &TopicTypeHash, include_type_sources: bool,
        timeout: Duration,
    ) -> Result<impl Future<Output = Result<GetTypeDescription::Response>>> {
        self.client.validate_owner(node)?;
        Node::validate_timeout(timeout)?;
        let response = self.describe(type_name, hash, include_type_sources)?;
        node.with_timeout(response, timeout)
    }
}

/// Calculate the canonical REP-2011 RIHS hash using RCL's complete schema representation.
pub fn type_description_hash(description: &msg::TypeDescription) -> Result<TopicTypeHash> {
    let native = WrappedNativeMsg::<msg::TypeDescription>::try_from(description)?;
    let mut hash = r2r_rcl::rosidl_type_hash_t {
        version: 0,
        value: [0; 32],
    };
    let ret = unsafe { r2r_rcl::rcl_calculate_type_hash(native.msg.cast(), &mut hash) };
    if ret != r2r_rcl::RCL_RET_OK as i32 {
        return Err(Error::from_rcl_error(ret));
    }
    Ok(TopicTypeHash {
        version: hash.version,
        value: hash.value,
    })
}
