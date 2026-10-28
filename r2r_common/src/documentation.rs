//! Build-time configuration for the checked-in documentation snapshot.

use std::{
    env,
    path::{Path, PathBuf},
};

/// ROS distribution represented by the checked-in doc-only bindings.
///
/// Documentation uses this snapshot even when the host has a different ROS
/// distribution (or no ROS installation). Native builds use `ROS_DISTRO`.
pub fn documentation_ros_distro() -> &'static str {
    include_str!("../documentation_distro.txt").trim()
}

/// Directory containing a crate's documentation bindings.
///
/// `R2R_BINDINGS_DIR` selects a separate snapshot root for generation and
/// validation. Its layout is `<root>/<package>/bindings`; without an override,
/// bindings remain next to that crate's Cargo manifest.
pub fn bindings_directory(manifest_dir: &Path, package_name: &str) -> PathBuf {
    match env::var_os("R2R_BINDINGS_DIR") {
        Some(root) => PathBuf::from(root).join(package_name).join("bindings"),
        None => manifest_dir.join("bindings"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documentation_snapshot_has_supported_service_event_semantics() {
        let distro = documentation_ros_distro();
        assert!(crate::SUPPORTED_ROS_DISTROS.contains(&distro));
        assert_eq!(crate::service_message_suffixes(distro), ["Request", "Response", "Event"]);
    }
}
