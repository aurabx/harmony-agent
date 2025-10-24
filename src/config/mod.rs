//! Configuration management
//!
//! This module handles parsing and validation of configuration from both
//! static TOML files and dynamic JSON control messages.

mod json;
mod toml_parser;
mod validation;

pub use json::{ControlAction, ControlMessage};
pub use toml_parser::TomlConfig;

use crate::error::{Result, WgAgentError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Main configuration structure supporting multiple named networks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Named network configurations
    #[serde(default)]
    pub networks: HashMap<String, NetworkConfig>,
}

/// Configuration for a single network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Enable WireGuard for this network
    #[serde(default)]
    pub enable_wireguard: bool,

    /// WireGuard interface name (e.g., "wg0")
    #[serde(default = "default_interface")]
    pub interface: String,

    /// Maximum Transmission Unit
    #[serde(default = "default_mtu")]
    pub mtu: u16,

    /// Path to private key file
    pub private_key_path: String,

    /// DNS servers for this network
    #[serde(default)]
    pub dns: Vec<String>,

    /// Interface IP address (CIDR notation, e.g., "10.100.0.2/24")
    pub address: Option<String>,

    /// WireGuard peers
    #[serde(default)]
    pub peers: Vec<PeerConfig>,

    /// Optional HTTP configuration (from Harmony)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpConfig>,
}

/// Peer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    /// Peer name (for identification)
    pub name: String,

    /// Base64-encoded public key
    pub public_key: String,

    /// Peer endpoint (host:port)
    pub endpoint: String,

    /// Allowed IP addresses/ranges (CIDR notation)
    pub allowed_ips: Vec<String>,

    /// Persistent keepalive interval in seconds
    #[serde(default = "default_keepalive")]
    pub persistent_keepalive_secs: u16,
}

/// HTTP configuration (preserved from Harmony, not used by agent)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Bind address for HTTP server
    pub bind_address: String,

    /// Bind port for HTTP server
    pub bind_port: u16,
}

impl Config {
    /// Create a new empty configuration
    pub fn new() -> Self {
        Self {
            networks: HashMap::new(),
        }
    }

    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let toml_config = TomlConfig::from_file(path)?;
        Ok(toml_config.into())
    }

    /// Parse configuration from JSON control message
    pub fn from_json(json: &str) -> Result<ControlMessage> {
        ControlMessage::from_json(json)
    }

    /// Add or update a network configuration
    pub fn add_network(&mut self, name: String, config: NetworkConfig) {
        self.networks.insert(name, config);
    }

    /// Get a network configuration by name
    pub fn get_network(&self, name: &str) -> Option<&NetworkConfig> {
        self.networks.get(name)
    }

    /// Get a mutable network configuration by name
    pub fn get_network_mut(&mut self, name: &str) -> Option<&mut NetworkConfig> {
        self.networks.get_mut(name)
    }

    /// Validate the entire configuration
    pub fn validate(&self) -> Result<()> {
        for (name, network) in &self.networks {
            network.validate()
                .map_err(|e| WgAgentError::Config(format!("Network '{}': {}", name, e)))?;
        }
        Ok(())
    }
}

impl NetworkConfig {
    /// Validate network configuration
    pub fn validate(&self) -> Result<()> {
        validation::validate_interface_name(&self.interface)?;
        validation::validate_mtu(self.mtu)?;
        validation::validate_file_path(&self.private_key_path)?;
        
        for dns in &self.dns {
            validation::validate_ip_address(dns)?;
        }
        
        for peer in &self.peers {
            peer.validate()?;
        }
        
        Ok(())
    }
}

impl PeerConfig {
    /// Validate peer configuration
    pub fn validate(&self) -> Result<()> {
        validation::validate_public_key(&self.public_key)?;
        validation::validate_endpoint(&self.endpoint)?;
        
        for allowed_ip in &self.allowed_ips {
            validation::validate_cidr(allowed_ip)?;
        }
        
        validation::validate_keepalive(self.persistent_keepalive_secs)?;
        
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

// Default value functions for serde
fn default_interface() -> String {
    "wg0".to_string()
}

fn default_mtu() -> u16 {
    1280
}

fn default_keepalive() -> u16 {
    25
}
