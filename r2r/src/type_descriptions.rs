//! RIHS type identity and remote native type descriptions.

use crate::{
    Error, Result, TopicTypeHash, WrappedActionTypeSupport, WrappedServiceTypeSupport,
    WrappedTypesupport,
};

impl std::str::FromStr for TopicTypeHash {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self> {
        let bytes = value.as_bytes();
        let invalid = || Error::InvalidTypeHash {
            value: value.into(),
        };
        fn nibble(byte: u8) -> Option<u8> {
            match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                b'A'..=b'F' => Some(byte - b'A' + 10),
                _ => None,
            }
        }
        let hex = |a, b| -> Result<u8> {
            Ok((nibble(a).ok_or_else(invalid)? << 4) | nibble(b).ok_or_else(invalid)?)
        };
        if bytes.len() != 71 || &bytes[..4] != b"RIHS" || bytes[6] != b'_' {
            return Err(invalid());
        }
        let version = hex(bytes[4], bytes[5])?;
        let mut hash = [0; 32];
        for (output, pair) in hash.iter_mut().zip(bytes[7..].chunks_exact(2)) {
            *output = hex(pair[0], pair[1])?;
        }
        Ok(Self {
            version,
            value: hash,
        })
    }
}

/// Read the generated message's RIHS identity without constructing a payload.
pub fn message_type_hash<T: WrappedTypesupport>() -> Result<TopicTypeHash> {
    #[cfg(r2r__ros__distro__humble)]
    {
        Err(Error::RCL_RET_UNSUPPORTED)
    }
    #[cfg(not(r2r__ros__distro__humble))]
    {
        let support = T::get_ts();
        let get = support
            .get_type_hash_func
            .ok_or(Error::RCL_RET_UNSUPPORTED)?;
        native_hash(unsafe { get(support) })
    }
}

/// Read the generated service's RIHS identity without constructing request/response payloads.
pub fn service_type_hash<T: WrappedServiceTypeSupport>() -> Result<TopicTypeHash> {
    #[cfg(r2r__ros__distro__humble)]
    {
        Err(Error::RCL_RET_UNSUPPORTED)
    }
    #[cfg(not(r2r__ros__distro__humble))]
    {
        let support = T::get_ts();
        let get = support
            .get_type_hash_func
            .ok_or(Error::RCL_RET_UNSUPPORTED)?;
        native_hash(unsafe { get(support) })
    }
}

/// Read the generated action's RIHS identity without constructing goal/feedback/result payloads.
pub fn action_type_hash<T: WrappedActionTypeSupport>() -> Result<TopicTypeHash> {
    #[cfg(r2r__ros__distro__humble)]
    {
        Err(Error::RCL_RET_UNSUPPORTED)
    }
    #[cfg(not(r2r__ros__distro__humble))]
    {
        let support = T::get_ts();
        let get = support
            .get_type_hash_func
            .ok_or(Error::RCL_RET_UNSUPPORTED)?;
        native_hash(unsafe { get(support) })
    }
}

#[cfg(not(r2r__ros__distro__humble))]
fn native_hash(hash: *const r2r_rcl::rosidl_type_hash_t) -> Result<TopicTypeHash> {
    let hash = unsafe { hash.as_ref() }.ok_or(Error::RCL_RET_ERROR)?;
    Ok(TopicTypeHash {
        version: hash.version,
        value: hash.value,
    })
}

#[cfg(all(
    not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
    r2r__type_description_interfaces__srv__GetTypeDescription
))]
mod client;
#[cfg(all(
    not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
    r2r__type_description_interfaces__srv__GetTypeDescription
))]
pub(crate) mod sources;
#[cfg(all(
    not(any(r2r__ros__distro__humble, r2r__ros__distro__iron)),
    r2r__type_description_interfaces__srv__GetTypeDescription
))]
pub use client::{type_description_hash, TypeDescriptionClient};

#[cfg(test)]
mod tests;
