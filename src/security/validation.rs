//! Input validation and sanitization
//!
//! This module provides validation for user inputs to prevent
//! injection attacks and ensure data integrity.

use crate::error::WgAgentError;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Validate network name (alphanumeric, dashes, underscores)
pub fn validate_network_name(name: &str) -> Result<(), WgAgentError> {
    if name.is_empty() {
        return Err(WgAgentError::Validation(
            "Network name cannot be empty".to_string(),
        ));
    }

    if name.len() > 64 {
        return Err(WgAgentError::Validation(
            "Network name too long (max 64 characters)".to_string(),
        ));
    }

    // Only allow alphanumeric, dash, underscore
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return Err(WgAgentError::Validation(format!(
            "Invalid network name '{}': only alphanumeric, dash, and underscore allowed",
            name
        )));
    }

    // Must not start with dash or underscore
    if name.starts_with('-') || name.starts_with('_') {
        return Err(WgAgentError::Validation(
            "Network name cannot start with dash or underscore".to_string(),
        ));
    }

    Ok(())
}

/// Validate WireGuard interface name
pub fn validate_interface_name(name: &str) -> Result<(), WgAgentError> {
    if name.is_empty() {
        return Err(WgAgentError::Validation(
            "Interface name cannot be empty".to_string(),
        ));
    }

    // Linux: max 15 characters, must start with letter
    #[cfg(target_os = "linux")]
    {
        if name.len() > 15 {
            return Err(WgAgentError::Validation(
                "Interface name too long (max 15 characters on Linux)".to_string(),
            ));
        }
    }

    // Must start with letter
    if !name.chars().next().unwrap().is_alphabetic() {
        return Err(WgAgentError::Validation(
            "Interface name must start with a letter".to_string(),
        ));
    }

    // Only allow alphanumeric and underscore
    if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(WgAgentError::Validation(format!(
            "Invalid interface name '{}': only alphanumeric and underscore allowed",
            name
        )));
    }

    Ok(())
}

/// Sanitize file path to prevent directory traversal
pub fn sanitize_path(path: &str) -> Result<PathBuf, WgAgentError> {
    let path = Path::new(path);

    // Check for null bytes
    if path.to_str().is_some_and(|s| s.contains('\0')) {
        return Err(WgAgentError::Validation(
            "Path contains null byte".to_string(),
        ));
    }

    // Check for directory traversal attempts
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                warn!("Path contains parent directory reference: {:?}", path);
                return Err(WgAgentError::Validation(
                    "Path contains invalid parent directory reference".to_string(),
                ));
            }
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                // These are fine for absolute paths
            }
            std::path::Component::CurDir | std::path::Component::Normal(_) => {
                // These are acceptable
            }
        }
    }

    Ok(path.to_path_buf())
}

/// Validate IP address string
#[allow(dead_code)]
pub fn validate_ip_address(ip: &str) -> Result<(), WgAgentError> {
    ip.parse::<std::net::IpAddr>()
        .map(|_| ())
        .map_err(|_| WgAgentError::Validation(format!("Invalid IP address: {}", ip)))
}

/// Validate port number
#[allow(dead_code)]
pub fn validate_port(port: u16) -> Result<(), WgAgentError> {
    if port == 0 {
        return Err(WgAgentError::Validation(
            "Port number cannot be 0".to_string(),
        ));
    }
    Ok(())
}

/// Validate CIDR notation
#[allow(dead_code)]
pub fn validate_cidr(cidr: &str) -> Result<(), WgAgentError> {
    // Split into IP and prefix length
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        return Err(WgAgentError::Validation(format!(
            "Invalid CIDR notation: {}",
            cidr
        )));
    }

    // Validate IP part
    validate_ip_address(parts[0])?;

    // Validate prefix length
    let prefix: u8 = parts[1].parse().map_err(|_| {
        WgAgentError::Validation(format!("Invalid prefix length in CIDR: {}", cidr))
    })?;

    // Check prefix length is valid
    let is_ipv4 = parts[0].contains('.');
    let max_prefix = if is_ipv4 { 32 } else { 128 };

    if prefix > max_prefix {
        return Err(WgAgentError::Validation(format!(
            "Invalid prefix length {} for {}",
            prefix,
            if is_ipv4 { "IPv4" } else { "IPv6" }
        )));
    }

    Ok(())
}

/// Validate MTU value
#[allow(dead_code)]
pub fn validate_mtu(mtu: u16) -> Result<(), WgAgentError> {
    const MIN_MTU: u16 = 576;  // IPv4 minimum

    if mtu < MIN_MTU {
        return Err(WgAgentError::Validation(format!(
            "MTU too small: {} (minimum {})",
            mtu, MIN_MTU
        )));
    }

    // Warn if MTU seems unusual
    if mtu < 1280 {
        warn!("MTU {} is below IPv6 minimum (1280)", mtu);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_network_name() {
        assert!(validate_network_name("default").is_ok());
        assert!(validate_network_name("my-network").is_ok());
        assert!(validate_network_name("net_123").is_ok());
        
        assert!(validate_network_name("").is_err());
        assert!(validate_network_name("my network").is_err());
        assert!(validate_network_name("-network").is_err());
        assert!(validate_network_name("network!").is_err());
    }

    #[test]
    fn test_validate_interface_name() {
        assert!(validate_interface_name("wg0").is_ok());
        assert!(validate_interface_name("wg_vpn").is_ok());
        
        assert!(validate_interface_name("").is_err());
        assert!(validate_interface_name("0wg").is_err());
        assert!(validate_interface_name("wg-0").is_err());
    }

    #[test]
    fn test_sanitize_path() {
        assert!(sanitize_path("/etc/config.toml").is_ok());
        assert!(sanitize_path("config/local.toml").is_ok());
        
        assert!(sanitize_path("../../../etc/passwd").is_err());
        assert!(sanitize_path("config/../../../secrets").is_err());
    }

    #[test]
    fn test_validate_ip_address() {
        assert!(validate_ip_address("192.168.1.1").is_ok());
        assert!(validate_ip_address("::1").is_ok());
        assert!(validate_ip_address("2001:db8::1").is_ok());
        
        assert!(validate_ip_address("256.1.1.1").is_err());
        assert!(validate_ip_address("not-an-ip").is_err());
    }

    #[test]
    fn test_validate_cidr() {
        assert!(validate_cidr("192.168.1.0/24").is_ok());
        assert!(validate_cidr("10.0.0.0/8").is_ok());
        assert!(validate_cidr("2001:db8::/32").is_ok());
        
        assert!(validate_cidr("192.168.1.0/33").is_err());
        assert!(validate_cidr("192.168.1.0").is_err());
        assert!(validate_cidr("invalid/24").is_err());
    }

    #[test]
    fn test_validate_port() {
        assert!(validate_port(51820).is_ok());
        assert!(validate_port(1).is_ok());
        assert!(validate_port(65535).is_ok());
        
        assert!(validate_port(0).is_err());
    }

    #[test]
    fn test_validate_mtu() {
        assert!(validate_mtu(1420).is_ok());
        assert!(validate_mtu(1500).is_ok());
        
        assert!(validate_mtu(100).is_err());
        // Note: 70000 exceeds u16::MAX, cannot test
        assert!(validate_mtu(65535).is_ok());
    }
}
