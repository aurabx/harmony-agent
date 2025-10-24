//! TOML configuration file parser
//!
//! This module handles parsing of TOML configuration files for standalone
//! agent operation. It supports the Harmony configuration schema with multiple
//! named networks.

use crate::config::{Config, HttpConfig, NetworkConfig, PeerConfig};
use crate::error::{Result, WgAgentError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// TOML configuration file structure
/// Matches the Harmony configuration schema
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomlConfig {
    /// Network configurations
    #[serde(default)]
    pub network: HashMap<String, TomlNetworkConfig>,
}

/// TOML network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomlNetworkConfig {
    /// Enable WireGuard for this network
    #[serde(default)]
    pub enable_wireguard: bool,

    /// WireGuard interface name
    #[serde(default = "default_interface")]
    pub interface: String,

    /// Maximum Transmission Unit
    #[serde(default = "default_mtu")]
    pub mtu: u16,

    /// Path to private key file
    pub private_key_path: String,

    /// Interface IP address (CIDR notation)
    pub address: Option<String>,

    /// DNS servers
    #[serde(default)]
    pub dns: Vec<String>,

    /// HTTP configuration (optional, from Harmony)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<TomlHttpConfig>,

    /// WireGuard peers
    #[serde(default)]
    pub peers: Vec<TomlPeerConfig>,
}

/// TOML HTTP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomlHttpConfig {
    /// Bind address
    pub bind_address: String,

    /// Bind port
    pub bind_port: u16,
}

/// TOML peer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TomlPeerConfig {
    /// Peer name
    pub name: String,

    /// Base64-encoded public key
    pub public_key: String,

    /// Peer endpoint
    pub endpoint: String,

    /// Allowed IP addresses/ranges
    pub allowed_ips: Vec<String>,

    /// Persistent keepalive interval in seconds
    #[serde(default = "default_keepalive")]
    pub persistent_keepalive_secs: u16,
}

impl TomlConfig {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|e| {
            WgAgentError::Config(format!("Failed to read config file {:?}: {}", path, e))
        })?;

        let config: TomlConfig = toml::from_str(&contents).map_err(|e| {
            WgAgentError::Config(format!("Failed to parse TOML config: {}", e))
        })?;

        Ok(config)
    }

    /// Parse configuration from a TOML string
    pub fn parse(toml: &str) -> Result<Self> {
        toml::from_str(toml).map_err(|e| {
            WgAgentError::Config(format!("Failed to parse TOML: {}", e))
        })
    }
}

// Convert TOML config to internal Config
impl From<TomlConfig> for Config {
    fn from(toml: TomlConfig) -> Self {
        let mut config = Config::new();

        for (name, network) in toml.network {
            config.add_network(name, network.into());
        }

        config
    }
}

impl From<TomlNetworkConfig> for NetworkConfig {
    fn from(toml: TomlNetworkConfig) -> Self {
        NetworkConfig {
            enable_wireguard: toml.enable_wireguard,
            interface: toml.interface,
            mtu: toml.mtu,
            private_key_path: toml.private_key_path,
            dns: toml.dns,
            address: toml.address,
            peers: toml.peers.into_iter().map(|p| p.into()).collect(),
            http: toml.http.map(|h| h.into()),
        }
    }
}

impl From<TomlHttpConfig> for HttpConfig {
    fn from(toml: TomlHttpConfig) -> Self {
        HttpConfig {
            bind_address: toml.bind_address,
            bind_port: toml.bind_port,
        }
    }
}

impl From<TomlPeerConfig> for PeerConfig {
    fn from(toml: TomlPeerConfig) -> Self {
        PeerConfig {
            name: toml.name,
            public_key: toml.public_key,
            endpoint: toml.endpoint,
            allowed_ips: toml.allowed_ips,
            persistent_keepalive_secs: toml.persistent_keepalive_secs,
        }
    }
}

// Default value functions
fn default_interface() -> String {
    "wg0".to_string()
}

fn default_mtu() -> u16 {
    1280
}

fn default_keepalive() -> u16 {
    25
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic_toml() {
        let toml = r#"
            [network.default]
            enable_wireguard = true
            interface = "wg0"
            mtu = 1420
            private_key_path = "/etc/wg-agent/private.key"
            dns = ["10.100.0.2"]

            [[network.default.peers]]
            name = "test-peer"
            public_key = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMN=="
            endpoint = "example.com:51820"
            allowed_ips = ["10.0.0.0/8"]
            persistent_keepalive_secs = 25
        "#;

        let config = TomlConfig::parse(toml).expect("Failed to parse TOML");
        assert!(config.network.contains_key("default"));

        let network = &config.network["default"];
        assert!(network.enable_wireguard);
        assert_eq!(network.interface, "wg0");
        assert_eq!(network.mtu, 1420);
        assert_eq!(network.dns.len(), 1);
        assert_eq!(network.peers.len(), 1);
    }

    #[test]
    fn test_parse_toml_with_http() {
        let toml = r#"
            [network.default]
            enable_wireguard = true
            interface = "wg0"
            mtu = 1280
            private_key_path = "/etc/wg-agent/private.key"

            [network.default.http]
            bind_address = "0.0.0.0"
            bind_port = 8081
        "#;

        let config = TomlConfig::parse(toml).expect("Failed to parse TOML");
        let network = &config.network["default"];
        assert!(network.http.is_some());

        let http = network.http.as_ref().unwrap();
        assert_eq!(http.bind_address, "0.0.0.0");
        assert_eq!(http.bind_port, 8081);
    }

    #[test]
    fn test_parse_multiple_networks() {
        let toml = r#"
            [network.default]
            enable_wireguard = true
            interface = "wg0"
            private_key_path = "/etc/wg-agent/default.key"

            [network.production]
            enable_wireguard = true
            interface = "wg1"
            private_key_path = "/etc/wg-agent/prod.key"
        "#;

        let config = TomlConfig::parse(toml).expect("Failed to parse TOML");
        assert_eq!(config.network.len(), 2);
        assert!(config.network.contains_key("default"));
        assert!(config.network.contains_key("production"));
    }

    #[test]
    fn test_parse_with_defaults() {
        let toml = r#"
            [network.minimal]
            private_key_path = "/etc/wg-agent/private.key"
        "#;

        let config = TomlConfig::parse(toml).expect("Failed to parse TOML");
        let network = &config.network["minimal"];
        
        // Check defaults
        assert_eq!(network.interface, "wg0");
        assert_eq!(network.mtu, 1280);
        assert!(!network.enable_wireguard);
        assert!(network.dns.is_empty());
        assert!(network.peers.is_empty());
    }

    #[test]
    fn test_convert_to_config() {
        let toml = r#"
            [network.default]
            enable_wireguard = true
            interface = "wg0"
            mtu = 1420
            private_key_path = "/etc/wg-agent/private.key"
        "#;

        let toml_config = TomlConfig::parse(toml).expect("Failed to parse TOML");
        let config: Config = toml_config.into();

        assert_eq!(config.networks.len(), 1);
        assert!(config.networks.contains_key("default"));

        let network = config.get_network("default").unwrap();
        assert!(network.enable_wireguard);
        assert_eq!(network.interface, "wg0");
    }
}
