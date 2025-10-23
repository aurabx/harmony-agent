//! Configuration validation functions
//!
//! This module provides validation for all configuration fields including
//! interface names, IP addresses, file paths, keys, and network parameters.

use crate::error::{Result, WgAgentError};
use std::net::IpAddr;
use std::path::Path;

/// Validate interface name (alphanumeric, max 15 chars)
pub fn validate_interface_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(WgAgentError::Config(
            "Interface name cannot be empty".to_string(),
        ));
    }

    if name.len() > 15 {
        return Err(WgAgentError::Config(format!(
            "Interface name '{}' exceeds maximum length of 15 characters",
            name
        )));
    }

    if !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err(WgAgentError::Config(format!(
            "Interface name '{}' contains invalid characters (only alphanumeric, '_', and '-' allowed)",
            name
        )));
    }

    Ok(())
}

/// Validate MTU value (1280-1500 range for WireGuard)
pub fn validate_mtu(mtu: u16) -> Result<()> {
    if !(1280..=1500).contains(&mtu) {
        return Err(WgAgentError::Config(format!(
            "MTU value {} is out of valid range (1280-1500)",
            mtu
        )));
    }
    Ok(())
}

/// Validate IP address
pub fn validate_ip_address(ip: &str) -> Result<()> {
    ip.parse::<IpAddr>()
        .map_err(|_| WgAgentError::Config(format!("Invalid IP address: {}", ip)))?;
    Ok(())
}

/// Validate CIDR notation (IP/prefix)
pub fn validate_cidr(cidr: &str) -> Result<()> {
    let parts: Vec<&str> = cidr.split('/').collect();
    
    if parts.len() != 2 {
        return Err(WgAgentError::Config(format!(
            "Invalid CIDR notation: {} (expected format: IP/prefix)",
            cidr
        )));
    }

    // Validate IP part
    validate_ip_address(parts[0])?;

    // Validate prefix length
    let prefix: u8 = parts[1].parse().map_err(|_| {
        WgAgentError::Config(format!("Invalid prefix length in CIDR: {}", cidr))
    })?;

    // Check prefix range based on IP version
    let ip: IpAddr = parts[0].parse().unwrap();
    let max_prefix = match ip {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };

    if prefix > max_prefix {
        return Err(WgAgentError::Config(format!(
            "Prefix length {} exceeds maximum {} for IP address {}",
            prefix, max_prefix, parts[0]
        )));
    }

    Ok(())
}

/// Validate endpoint format (host:port)
pub fn validate_endpoint(endpoint: &str) -> Result<()> {
    let parts: Vec<&str> = endpoint.rsplitn(2, ':').collect();
    
    if parts.len() != 2 {
        return Err(WgAgentError::Config(format!(
            "Invalid endpoint format: {} (expected format: host:port)",
            endpoint
        )));
    }

    // Validate port
    let port: u16 = parts[0].parse().map_err(|_| {
        WgAgentError::Config(format!("Invalid port in endpoint: {}", endpoint))
    })?;

    if port == 0 {
        return Err(WgAgentError::Config(
            "Port number cannot be 0".to_string(),
        ));
    }

    // Host validation is lenient (can be hostname or IP)
    let host = parts[1];
    if host.is_empty() {
        return Err(WgAgentError::Config(
            "Host cannot be empty in endpoint".to_string(),
        ));
    }

    Ok(())
}

/// Validate file path exists and is readable
pub fn validate_file_path(path: &str) -> Result<()> {
    if path.is_empty() {
        return Err(WgAgentError::Config(
            "File path cannot be empty".to_string(),
        ));
    }

    let path_obj = Path::new(path);
    
    // Note: We don't check if the file exists here because it might be created later
    // Just validate the path is syntactically valid
    if path_obj.to_str().is_none() {
        return Err(WgAgentError::Config(format!(
            "Invalid file path: {}",
            path
        )));
    }

    Ok(())
}

/// Validate base64-encoded public key
pub fn validate_public_key(key: &str) -> Result<()> {
    if key.is_empty() {
        return Err(WgAgentError::Config(
            "Public key cannot be empty".to_string(),
        ));
    }

    // WireGuard keys are 32 bytes, base64 encoded = 44 characters (with padding)
    if key.len() != 44 {
        return Err(WgAgentError::Config(format!(
            "Invalid public key length: {} (expected 44 characters)",
            key.len()
        )));
    }

    // Basic base64 character validation
    if !key.chars().all(|c| c.is_alphanumeric() || c == '+' || c == '/' || c == '=') {
        return Err(WgAgentError::Config(
            "Public key contains invalid base64 characters".to_string(),
        ));
    }

    Ok(())
}

/// Validate keepalive timeout
pub fn validate_keepalive(secs: u16) -> Result<()> {
    // Reasonable range: 0 (disabled) or 10-300 seconds
    if secs > 0 && secs < 10 {
        return Err(WgAgentError::Config(format!(
            "Keepalive interval {} is too short (minimum 10 seconds or 0 to disable)",
            secs
        )));
    }

    if secs > 300 {
        return Err(WgAgentError::Config(format!(
            "Keepalive interval {} is too long (maximum 300 seconds)",
            secs
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_interface_name() {
        assert!(validate_interface_name("wg0").is_ok());
        assert!(validate_interface_name("wg-test").is_ok());
        assert!(validate_interface_name("wg_test").is_ok());
        assert!(validate_interface_name("").is_err());
        assert!(validate_interface_name("wg@test").is_err());
        assert!(validate_interface_name("toolonginterfacename").is_err());
    }

    #[test]
    fn test_validate_mtu() {
        assert!(validate_mtu(1280).is_ok());
        assert!(validate_mtu(1420).is_ok());
        assert!(validate_mtu(1500).is_ok());
        assert!(validate_mtu(1279).is_err());
        assert!(validate_mtu(1501).is_err());
    }

    #[test]
    fn test_validate_ip_address() {
        assert!(validate_ip_address("192.168.1.1").is_ok());
        assert!(validate_ip_address("10.0.0.1").is_ok());
        assert!(validate_ip_address("::1").is_ok());
        assert!(validate_ip_address("fe80::1").is_ok());
        assert!(validate_ip_address("invalid").is_err());
        assert!(validate_ip_address("256.1.1.1").is_err());
    }

    #[test]
    fn test_validate_cidr() {
        assert!(validate_cidr("192.168.1.0/24").is_ok());
        assert!(validate_cidr("10.0.0.0/8").is_ok());
        assert!(validate_cidr("fe80::/64").is_ok());
        assert!(validate_cidr("192.168.1.1").is_err());
        assert!(validate_cidr("192.168.1.0/33").is_err());
        assert!(validate_cidr("fe80::/129").is_err());
    }

    #[test]
    fn test_validate_endpoint() {
        assert!(validate_endpoint("example.com:51820").is_ok());
        assert!(validate_endpoint("192.168.1.1:51820").is_ok());
        assert!(validate_endpoint("[::1]:51820").is_ok());
        assert!(validate_endpoint("invalid").is_err());
        assert!(validate_endpoint("example.com:0").is_err());
        assert!(validate_endpoint(":51820").is_err());
    }

    #[test]
    fn test_validate_public_key() {
        let valid_key = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOP==";
        assert_eq!(valid_key.len(), 44);
        assert!(validate_public_key(valid_key).is_ok());
        assert!(validate_public_key("").is_err());
        assert!(validate_public_key("tooshort").is_err());
        assert!(validate_public_key("invalid@characters#here1234567890123456==").is_err());
    }

    #[test]
    fn test_validate_keepalive() {
        assert!(validate_keepalive(0).is_ok());
        assert!(validate_keepalive(25).is_ok());
        assert!(validate_keepalive(300).is_ok());
        assert!(validate_keepalive(5).is_err());
        assert!(validate_keepalive(301).is_err());
    }
}
