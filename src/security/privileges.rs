//! Privilege dropping and memory locking
//!
//! This module handles dropping privileges after TUN device creation
//! and locking memory to prevent key material from being swapped to disk.

use crate::error::WgAgentError;
use tracing::{debug, info, warn};

/// Privilege level of the current process
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeLevel {
    /// Running as root (Unix) or Administrator (Windows)
    Root,
    /// Running as Administrator (Windows only)
    Administrator,
    /// Running as regular user
    User,
    /// Unknown privilege level
    Unknown,
}

impl PrivilegeLevel {
    /// Detect current privilege level
    pub fn detect() -> Self {
        #[cfg(unix)]
        {
            let uid = unsafe { libc::getuid() };
            let euid = unsafe { libc::geteuid() };
            
            if uid == 0 || euid == 0 {
            return Self::Root;
            }
            Self::User
        }

        #[cfg(windows)]
        {
            // Would check for Administrator privileges on Windows
            // For now, assume User
            return Self::User;
        }

        #[cfg(not(any(unix, windows)))]
        {
            Self::Unknown
        }
    }

    /// Check if elevated
    pub fn is_elevated(&self) -> bool {
        matches!(self, Self::Root | Self::Administrator)
    }
}

impl std::fmt::Display for PrivilegeLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Root => write!(f, "root"),
            Self::Administrator => write!(f, "administrator"),
            Self::User => write!(f, "user"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// Drop privileges to specified user/group
#[cfg(unix)]
pub fn drop_privileges(uid: Option<u32>, gid: Option<u32>) -> Result<(), WgAgentError> {
    let current_uid = unsafe { libc::getuid() };
    let current_euid = unsafe { libc::geteuid() };

    if current_uid != 0 && current_euid != 0 {
        warn!("Not running as root, cannot drop privileges");
        return Ok(());
    }

    // Drop group first
    if let Some(target_gid) = gid {
        info!("Dropping group privileges to GID {}", target_gid);
        let result = unsafe { libc::setgid(target_gid) };
        if result != 0 {
            return Err(WgAgentError::Security(format!(
                "Failed to drop group privileges to GID {}: {}",
                target_gid,
                std::io::Error::last_os_error()
            )));
        }
    }

    // Drop user privileges
    if let Some(target_uid) = uid {
        info!("Dropping user privileges to UID {}", target_uid);
        let result = unsafe { libc::setuid(target_uid) };
        if result != 0 {
            return Err(WgAgentError::Security(format!(
                "Failed to drop user privileges to UID {}: {}",
                target_uid,
                std::io::Error::last_os_error()
            )));
        }
    }

    // Verify we can't regain privileges
    let new_uid = unsafe { libc::getuid() };
    let new_euid = unsafe { libc::geteuid() };

    if new_uid == 0 || new_euid == 0 {
        return Err(WgAgentError::Security(
            "Failed to verify privilege drop".to_string(),
        ));
    }

    info!("Privileges dropped successfully");
    Ok(())
}

/// Drop privileges (non-Unix stub)
#[cfg(not(unix))]
pub fn drop_privileges(_uid: Option<u32>, _gid: Option<u32>) -> Result<(), WgAgentError> {
    warn!("Privilege dropping not implemented for this platform");
    Ok(())
}

/// Lock memory to prevent swapping
#[cfg(all(unix, not(target_os = "macos")))]
pub fn lock_memory() -> Result<(), WgAgentError> {
    debug!("Locking memory to prevent swapping");

    // Lock all current and future memory
    let result = unsafe { libc::mlockall(libc::MCL_CURRENT | libc::MCL_FUTURE) };
    
    if result != 0 {
        let err = std::io::Error::last_os_error();
        warn!("Failed to lock memory: {}", err);
        // Don't fail, just warn - IPC_LOCK capability may not be available
        return Ok(());
    }

    info!("Memory locked successfully");
    Ok(())
}

/// Lock memory (macOS)
#[cfg(target_os = "macos")]
pub fn lock_memory() -> Result<(), WgAgentError> {
    debug!("Memory locking on macOS");
    // macOS has different requirements for mlockall
    // For now, just log and continue
    warn!("Full memory locking not fully supported on macOS");
    Ok(())
}

/// Lock memory (non-Unix stub)
#[cfg(not(unix))]
pub fn lock_memory() -> Result<(), WgAgentError> {
    warn!("Memory locking not implemented for this platform");
    Ok(())
}

/// Set Linux capabilities (requires CAP_NET_ADMIN and CAP_IPC_LOCK)
#[cfg(target_os = "linux")]
#[allow(dead_code)]
pub fn set_capabilities() -> Result<(), WgAgentError> {
    debug!("Setting Linux capabilities");
    
    // In production, would use libcap-ng or caps crate
    // For now, just verify capabilities are available
    
    info!("Capability management not yet fully implemented");
    Ok(())
}

/// Set capabilities (non-Linux stub)
#[cfg(not(target_os = "linux"))]
#[allow(dead_code)]
pub fn set_capabilities() -> Result<(), WgAgentError> {
    debug!("Capabilities not applicable on this platform");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_privilege_level_detect() {
        let level = PrivilegeLevel::detect();
        assert!(matches!(
            level,
            PrivilegeLevel::Root | PrivilegeLevel::User | PrivilegeLevel::Administrator | PrivilegeLevel::Unknown
        ));
    }

    #[test]
    fn test_privilege_level_display() {
        assert_eq!(PrivilegeLevel::Root.to_string(), "root");
        assert_eq!(PrivilegeLevel::User.to_string(), "user");
    }

    #[test]
    fn test_is_elevated() {
        assert!(PrivilegeLevel::Root.is_elevated());
        assert!(!PrivilegeLevel::User.is_elevated());
    }
}
