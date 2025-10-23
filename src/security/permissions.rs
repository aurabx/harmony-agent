//! File permission validation and enforcement
//!
//! This module ensures that sensitive files (keys, configs) have
//! appropriate permissions to prevent unauthorized access.

use crate::error::WgAgentError;
use std::path::Path;
use tracing::{debug, warn};

/// Secure file mode requirements
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecureFileMode {
    /// Private key file (0600 or stricter)
    PrivateKey,
    /// Configuration file (0640 or stricter)
    Config,
    /// Socket file (0660 or stricter)
    Socket,
    /// Directory (0755 or stricter)
    Directory,
}

impl SecureFileMode {
    /// Get the maximum allowed permission mode
    #[cfg(unix)]
    pub fn max_mode(&self) -> u32 {
        match self {
            Self::PrivateKey => 0o600,
            Self::Config => 0o640,
            Self::Socket => 0o660,
            Self::Directory => 0o755,
        }
    }

    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            Self::PrivateKey => "0600 (owner read/write only)",
            Self::Config => "0640 (owner read/write, group read)",
            Self::Socket => "0660 (owner/group read/write)",
            Self::Directory => "0755 (owner full, others read/execute)",
        }
    }
}

/// Validate file permissions
#[cfg(unix)]
pub fn validate_file_permissions(path: &Path, mode: SecureFileMode) -> Result<(), WgAgentError> {
    use std::os::unix::fs::PermissionsExt;

    debug!("Validating permissions for {:?}", path);

    let metadata = std::fs::metadata(path).map_err(|e| {
        WgAgentError::Security(format!("Failed to read metadata for {:?}: {}", path, e))
    })?;

    let perms = metadata.permissions();
    let file_mode = perms.mode() & 0o777;
    let max_mode = mode.max_mode();

    if file_mode > max_mode {
        warn!(
            "File {:?} has insecure permissions: {:o} (max: {:o})",
            path, file_mode, max_mode
        );
        return Err(WgAgentError::Security(format!(
            "File {:?} has insecure permissions: {:o}, expected {}",
            path,
            file_mode,
            mode.description()
        )));
    }

    // Check that file is owned by current user or root
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::MetadataExt;
        let file_uid = metadata.uid();
        let current_uid = unsafe { libc::getuid() };
        
        if file_uid != current_uid && file_uid != 0 {
            return Err(WgAgentError::Security(format!(
                "File {:?} is not owned by current user or root",
                path
            )));
        }
    }

    debug!("Permissions valid for {:?}: {:o}", path, file_mode);
    Ok(())
}

/// Validate file permissions (non-Unix stub)
#[cfg(not(unix))]
pub fn validate_file_permissions(path: &Path, _mode: SecureFileMode) -> Result<(), WgAgentError> {
    debug!("Permission validation not implemented for this platform: {:?}", path);
    Ok(())
}

/// Set secure file permissions
#[cfg(unix)]
#[allow(dead_code)]
pub fn set_secure_permissions(path: &Path, mode: SecureFileMode) -> Result<(), WgAgentError> {
    use std::os::unix::fs::PermissionsExt;

    let perms = std::fs::Permissions::from_mode(mode.max_mode());
    std::fs::set_permissions(path, perms).map_err(|e| {
        WgAgentError::Security(format!("Failed to set permissions on {:?}: {}", path, e))
    })?;

    debug!("Set secure permissions on {:?}: {:o}", path, mode.max_mode());
    Ok(())
}

/// Set secure file permissions (non-Unix stub)
#[cfg(not(unix))]
pub fn set_secure_permissions(path: &Path, _mode: SecureFileMode) -> Result<(), WgAgentError> {
    debug!("Permission setting not implemented for this platform: {:?}", path);
    Ok(())
}

/// Validate directory is not world-writable
#[cfg(unix)]
#[allow(dead_code)]
pub fn validate_directory_security(path: &Path) -> Result<(), WgAgentError> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::metadata(path).map_err(|e| {
        WgAgentError::Security(format!("Failed to read directory metadata: {}", e))
    })?;

    if !metadata.is_dir() {
        return Err(WgAgentError::Security(format!(
            "{:?} is not a directory",
            path
        )));
    }

    let perms = metadata.permissions();
    let mode = perms.mode() & 0o777;

    // Check for world-writable
    if mode & 0o002 != 0 {
        return Err(WgAgentError::Security(format!(
            "Directory {:?} is world-writable",
            path
        )));
    }

    debug!("Directory {:?} has secure permissions: {:o}", path, mode);
    Ok(())
}

/// Validate directory security (non-Unix stub)
#[cfg(not(unix))]
pub fn validate_directory_security(path: &Path) -> Result<(), WgAgentError> {
    debug!("Directory validation not implemented for this platform: {:?}", path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secure_file_mode() {
        assert_eq!(SecureFileMode::PrivateKey.description(), "0600 (owner read/write only)");
    }

    #[cfg(unix)]
    #[test]
    fn test_max_mode() {
        assert_eq!(SecureFileMode::PrivateKey.max_mode(), 0o600);
        assert_eq!(SecureFileMode::Config.max_mode(), 0o640);
    }
}
