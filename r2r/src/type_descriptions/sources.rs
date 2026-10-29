//! Recover original source whitespace lost by some generated C type-description metadata.

use std::{collections::HashMap, path::Path};

#[derive(Default)]
pub(crate) struct SourceFiles {
    files: HashMap<String, HashMap<String, Option<Vec<u8>>>>,
}

fn original_source<'a>(native: &[u8], original: &'a [u8]) -> Option<&'a [u8]> {
    if native == original {
        return None;
    }
    let prefix = native.strip_suffix(&[0]).unwrap_or(native);
    // Restore only the exact compiled prefix and trailing whitespace. A modified interface file
    // cannot replace unrelated source text merely because its package/type names match.
    let suffix = original.strip_prefix(prefix)?;
    suffix
        .iter()
        .all(u8::is_ascii_whitespace)
        .then_some(original)
}

impl SourceFiles {
    pub(crate) fn repair(
        &mut self, type_name: &str, encoding: &str,
        contents: &mut r2r_rcl::rosidl_runtime_c__String,
    ) -> crate::Result<()> {
        let original = self.lookup(type_name, encoding);
        if let Some(bytes) = original.and_then(|file| original_source(contents.as_bytes(), file)) {
            if !contents.try_assign_bytes(bytes) {
                return Err(crate::Error::RCL_RET_BAD_ALLOC);
            }
        }
        Ok(())
    }
    pub(crate) fn repair_rust(
        &mut self, type_name: &str, encoding: &str, contents: &mut crate::RosString,
    ) -> crate::Result<()> {
        if let Some(original) = self
            .lookup(type_name, encoding)
            .and_then(|file| original_source(contents.as_bytes(), file))
        {
            let prefix = contents
                .as_bytes()
                .strip_suffix(&[0])
                .unwrap_or(contents.as_bytes())
                .len();
            let additional = original.len().saturating_sub(contents.len());
            contents
                .try_reserve_exact(additional)
                .map_err(|_| crate::Error::RCL_RET_BAD_ALLOC)?;
            contents.truncate(prefix);
            contents.extend_from_slice(&original[prefix..]);
        }
        Ok(())
    }

    fn lookup(&mut self, type_name: &str, encoding: &str) -> Option<&[u8]> {
        if !matches!(encoding, "msg" | "srv" | "action" | "idl") {
            return None;
        }
        let mut parts = type_name.split('/');
        let (Some(package), Some(namespace), Some(name), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return None;
        };
        if [package, namespace, name].iter().any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        }) {
            return None;
        }
        // Allocate lookup keys and read files only on cache misses; repeated replies reuse both.
        if !self.files.contains_key(type_name) {
            self.files.insert(type_name.into(), HashMap::new());
        }
        let files = self.files.get_mut(type_name).unwrap();
        if !files.contains_key(encoding) {
            let original = std::env::var_os("AMENT_PREFIX_PATH").and_then(|paths| {
                std::env::split_paths(&paths).find_map(|prefix| {
                    let path = Path::new(&prefix)
                        .join("share")
                        .join(package)
                        .join(namespace)
                        .join(format!("{name}.{encoding}"));
                    std::fs::read(path).ok()
                })
            });
            files.insert(encoding.into(), original);
        }
        files.get(encoding).unwrap().as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_description_source_recovery_restores_only_matching_trailing_whitespace() {
        assert_eq!(
            original_source(b"uint64 data\0", b"uint64 data\n"),
            Some(&b"uint64 data\n"[..])
        );
        assert_eq!(original_source(b"uint64 data", b"uint64 data\n"), Some(&b"uint64 data\n"[..]));
        assert_eq!(original_source(b"uint64 data\0", b"uint64 data"), Some(&b"uint64 data"[..]));
        assert_eq!(
            original_source(b"uint64 data\0", b"uint64 data\r\n\n"),
            Some(&b"uint64 data\r\n\n"[..])
        );
        for original in [
            &b"uint32 data\n"[..],
            &b"uint64 other\n"[..],
            &b"uint64 data\nstring added\n"[..],
        ] {
            assert!(original_source(b"uint64 data\0", original).is_none());
        }
        assert!(original_source(b"uint64 data\n", b"uint64 data\n").is_none());
    }

    #[test]
    fn type_description_source_recovery_reuses_owned_capacity_and_rejects_modified_files() {
        let mut files = SourceFiles::default();
        files.files.insert(
            "demo/msg/Value".into(),
            HashMap::from([("msg".into(), Some(b"uint64 data\n".to_vec()))]),
        );
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(b"uint64 data");
        let mut source = crate::RosString::from(bytes);
        let address = source.as_ptr();
        files
            .repair_rust("demo/msg/Value", "msg", &mut source)
            .unwrap();
        assert_eq!(source.as_bytes(), b"uint64 data\n");
        assert_eq!(source.as_ptr(), address);
        files
            .repair_rust("demo/msg/Value", "msg", &mut source)
            .unwrap();
        assert_eq!(source.as_ptr(), address);
        let mut changed = crate::RosString::from("uint32 data");
        files
            .repair_rust("demo/msg/Value", "msg", &mut changed)
            .unwrap();
        assert_eq!(changed, "uint32 data");
        files
            .repair_rust("../msg/Value", "msg", &mut changed)
            .unwrap();
        assert_eq!(files.files.len(), 1);
    }
}
