//! Security hardening and privilege management
//!
//! This module provides security features including:
//! - Privilege dropping after TUN device creation
//! - File permission validation
//! - Capability management (Linux)
//! - Input validation and sanitization
//! - Security audit logging

use crate::error::WgAgentError;
use tracing::{debug, info, warn};

mod permissions;
mod privileges;
mod validation;

pub use permissions::{validate_file_permissions, SecureFileMode};
pub use privileges::{drop_privileges, lock_memory, PrivilegeLevel};
pub use validation::{sanitize_path, validate_interface_name, validate_network_name};

/// Security context for the agent
#[derive(Debug, Clone)]
pub struct SecurityContext {
    /// Current privilege level
    pub privilege_level: PrivilegeLevel,
    /// Whether memory is locked
    pub memory_locked: bool,
    /// User ID to drop to (if applicable)
    pub target_uid: Option<u32>,
    /// Group ID to drop to (if applicable)
    pub target_gid: Option<u32>,
}

impl SecurityContext {
    /// Create a new security context
    pub fn new() -> Self {
        Self {
            privilege_level: PrivilegeLevel::detect(),
            memory_locked: false,
            target_uid: None,
            target_gid: None,
        }
    }

    /// Check if running with elevated privileges
    pub fn is_elevated(&self) -> bool {
        matches!(self.privilege_level, PrivilegeLevel::Root | PrivilegeLevel::Administrator)
    }

    /// Lock memory to prevent swapping sensitive data
    pub fn lock_memory(&mut self) -> Result<(), WgAgentError> {
        if self.memory_locked {
            debug!("Memory already locked");
            return Ok(());
        }

        lock_memory()?;
        self.memory_locked = true;
        info!("Memory locked successfully");
        Ok(())
    }

    /// Drop privileges to specified user/group
    pub fn drop_privileges(&mut self, uid: Option<u32>, gid: Option<u32>) -> Result<(), WgAgentError> {
        if !self.is_elevated() {
            warn!("Not running with elevated privileges, cannot drop");
            return Ok(());
        }

        drop_privileges(uid, gid)?;
        self.privilege_level = PrivilegeLevel::User;
        self.target_uid = uid;
        self.target_gid = gid;
        info!("Privileges dropped successfully");
        Ok(())
    }
}

impl Default for SecurityContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Security audit event types
#[derive(Debug, Clone)]
pub enum SecurityEvent {
    /// Privilege level changed
    PrivilegeChange {
        /// Previous privilege level
        from: String,
        /// New privilege level
        to: String
    },
    /// File permission validation failed
    InvalidPermissions {
        /// File path
        path: String,
        /// Expected permissions
        expected: String,
        /// Actual permissions
        actual: String
    },
    /// Suspicious input detected
    SuspiciousInput {
        /// The suspicious input
        input: String,
        /// Reason for flagging
        reason: String
    },
    /// Key rotation performed
    KeyRotation {
        /// Network name
        network: String
    },
    /// Authentication attempt
    AuthenticationAttempt {
        /// Whether authentication succeeded
        success: bool,
        /// Optional reason for failure
        reason: Option<String>
    },
}

impl SecurityEvent {
    /// Log a security event
    pub fn log(&self) {
        match self {
            Self::PrivilegeChange { from, to } => {
                info!("Security: Privilege changed from {} to {}", from, to);
            }
            Self::InvalidPermissions { path, expected, actual } => {
                warn!(
                    "Security: Invalid permissions on {}: expected {}, got {}",
                    path, expected, actual
                );
            }
            Self::SuspiciousInput { input, reason } => {
                warn!("Security: Suspicious input '{}': {}", input, reason);
            }
            Self::KeyRotation { network } => {
                info!("Security: Key rotation performed for network '{}'", network);
            }
            Self::AuthenticationAttempt { success, reason } => {
                if *success {
                    info!("Security: Authentication successful");
                } else {
                    warn!(
                        "Security: Authentication failed: {}",
                        reason.as_deref().unwrap_or("unknown reason")
                    );
                }
            }
        }
    }
}

/// Check if running in secure mode
pub fn is_secure_mode() -> bool {
    // Check environment variable
    if let Ok(val) = std::env::var("WG_AGENT_INSECURE") {
        if val == "1" || val.to_lowercase() == "true" {
            warn!("Running in INSECURE mode - security features disabled");
            return false;
        }
    }

    true
}

/// Validate secure defaults are in place
pub fn validate_secure_defaults() -> Result<(), WgAgentError> {
    debug!("Validating secure defaults");

    // Check that we're running in secure mode
    if !is_secure_mode() {
        return Err(WgAgentError::Security(
            "Insecure mode is not recommended for production".to_string(),
        ));
    }

    // Additional security checks would go here
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_context_creation() {
        let ctx = SecurityContext::new();
        assert!(!ctx.memory_locked);
    }

    #[test]
    fn test_is_secure_mode() {
        // Should default to secure
        assert!(is_secure_mode());
    }

    #[test]
    fn test_security_event_logging() {
        let event = SecurityEvent::PrivilegeChange {
            from: "root".to_string(),
            to: "user".to_string(),
        };
        event.log(); // Should not panic
    }
}
