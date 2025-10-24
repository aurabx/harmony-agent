//! JSON control message parser
//!
//! This module handles parsing of JSON control messages from Harmony or other
//! applications via the control plane API.

use crate::config::{HttpConfig, NetworkConfig, PeerConfig};
use crate::error::{Result, WgAgentError};
use serde::{Deserialize, Serialize};

/// Control action types
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlAction {
    /// Establish WireGuard tunnel
    Connect,
    /// Tear down tunnel
    Disconnect,
    /// Get connection status
    Status,
    /// Reload configuration
    Reload,
    /// Perform key rotation
    RotateKeys,
}

/// Control message received from applications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlMessage {
    /// Action to perform
    pub action: ControlAction,

    /// Network name (e.g., "default", "production")
    #[serde(default = "default_network_name")]
    pub network: String,

    /// Network configuration (for connect/reload actions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<JsonNetworkConfig>,
}

/// JSON network configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonNetworkConfig {
    /// WireGuard interface name
    #[serde(default = "default_interface")]
    pub interface: String,

    /// Maximum Transmission Unit
    #[serde(default = "default_mtu")]
    pub mtu: u16,

    /// DNS servers
    #[serde(default)]
    pub dns: Vec<String>,

    /// Path to private key file
    #[serde(rename = "privateKeyPath")]
    pub private_key_path: String,

    /// Interface IP address (CIDR notation)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,

    /// WireGuard peers
    #[serde(default)]
    pub peers: Vec<JsonPeerConfig>,

    /// Optional HTTP configuration (from Harmony)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http: Option<JsonHttpConfig>,
}

/// JSON peer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonPeerConfig {
    /// Peer name
    pub name: String,

    /// Base64-encoded public key
    #[serde(rename = "publicKey")]
    pub public_key: String,

    /// Peer endpoint
    pub endpoint: String,

    /// Allowed IP addresses/ranges
    #[serde(rename = "allowedIps")]
    pub allowed_ips: Vec<String>,

    /// Persistent keepalive interval in seconds
    #[serde(
        rename = "keepaliveSecs",
        default = "default_keepalive"
    )]
    pub keepalive_secs: u16,
}

/// JSON HTTP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonHttpConfig {
    /// Bind address
    #[serde(rename = "bindAddress")]
    pub bind_address: String,

    /// Bind port
    #[serde(rename = "bindPort")]
    pub bind_port: u16,
}

impl ControlMessage {
    /// Parse control message from JSON string
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| {
            WgAgentError::Serialization(format!("Failed to parse JSON control message: {}", e))
        })
    }

    /// Serialize control message to JSON string
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| {
            WgAgentError::Serialization(format!("Failed to serialize control message: {}", e))
        })
    }

    /// Serialize control message to pretty JSON string
    pub fn to_json_pretty(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| {
            WgAgentError::Serialization(format!("Failed to serialize control message: {}", e))
        })
    }
}

// Convert JSON network config to internal NetworkConfig
impl From<JsonNetworkConfig> for NetworkConfig {
    fn from(json: JsonNetworkConfig) -> Self {
        NetworkConfig {
            enable_wireguard: true, // Always true for control messages
            interface: json.interface,
            mtu: json.mtu,
            private_key_path: json.private_key_path,
            dns: json.dns,
            address: json.address,
            peers: json.peers.into_iter().map(|p| p.into()).collect(),
            http: json.http.map(|h| h.into()),
        }
    }
}

impl From<JsonHttpConfig> for HttpConfig {
    fn from(json: JsonHttpConfig) -> Self {
        HttpConfig {
            bind_address: json.bind_address,
            bind_port: json.bind_port,
        }
    }
}

impl From<JsonPeerConfig> for PeerConfig {
    fn from(json: JsonPeerConfig) -> Self {
        PeerConfig {
            name: json.name,
            public_key: json.public_key,
            endpoint: json.endpoint,
            allowed_ips: json.allowed_ips,
            persistent_keepalive_secs: json.keepalive_secs,
        }
    }
}

// Default value functions
fn default_network_name() -> String {
    "default".to_string()
}

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
    fn test_parse_connect_message() {
        let json = r#"{
            "action": "connect",
            "network": "default",
            "config": {
                "interface": "wg0",
                "mtu": 1420,
                "dns": ["10.100.0.2"],
                "privateKeyPath": "/etc/harmony-agent/private.key",
                "peers": [{
                    "name": "test-peer",
                    "publicKey": "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMN==",
                    "endpoint": "example.com:51820",
                    "allowedIps": ["10.0.0.0/8"],
                    "keepaliveSecs": 25
                }]
            }
        }"#;

        let msg = ControlMessage::from_json(json).expect("Failed to parse JSON");
        assert_eq!(msg.action, ControlAction::Connect);
        assert_eq!(msg.network, "default");
        assert!(msg.config.is_some());

        let config = msg.config.unwrap();
        assert_eq!(config.interface, "wg0");
        assert_eq!(config.mtu, 1420);
        assert_eq!(config.dns.len(), 1);
        assert_eq!(config.peers.len(), 1);
    }

    #[test]
    fn test_parse_disconnect_message() {
        let json = r#"{
            "action": "disconnect",
            "network": "production"
        }"#;

        let msg = ControlMessage::from_json(json).expect("Failed to parse JSON");
        assert_eq!(msg.action, ControlAction::Disconnect);
        assert_eq!(msg.network, "production");
        assert!(msg.config.is_none());
    }

    #[test]
    fn test_parse_status_message() {
        let json = r#"{"action": "status"}"#;

        let msg = ControlMessage::from_json(json).expect("Failed to parse JSON");
        assert_eq!(msg.action, ControlAction::Status);
        assert_eq!(msg.network, "default"); // Should use default
    }

    #[test]
    fn test_parse_with_http_config() {
        let json = r#"{
            "action": "connect",
            "network": "default",
            "config": {
                "interface": "wg0",
                "privateKeyPath": "/etc/harmony-agent/private.key",
                "http": {
                    "bindAddress": "0.0.0.0",
                    "bindPort": 8081
                }
            }
        }"#;

        let msg = ControlMessage::from_json(json).expect("Failed to parse JSON");
        let config = msg.config.unwrap();
        assert!(config.http.is_some());

        let http = config.http.unwrap();
        assert_eq!(http.bind_address, "0.0.0.0");
        assert_eq!(http.bind_port, 8081);
    }

    #[test]
    fn test_serialize_message() {
        let msg = ControlMessage {
            action: ControlAction::Connect,
            network: "default".to_string(),
            config: Some(JsonNetworkConfig {
                interface: "wg0".to_string(),
                mtu: 1420,
                address: Some("10.0.0.2/24".to_string()),
                dns: vec!["10.100.0.2".to_string()],
                private_key_path: "/etc/harmony-agent/private.key".to_string(),
                peers: vec![],
                http: None,
            }),
        };

        let json = msg.to_json().expect("Failed to serialize");
        assert!(json.contains("\"action\":\"connect\""));
        assert!(json.contains("\"network\":\"default\""));
    }

    #[test]
    fn test_convert_to_network_config() {
        let json_config = JsonNetworkConfig {
            interface: "wg0".to_string(),
            mtu: 1420,
            address: Some("10.0.0.1/24".to_string()),
            dns: vec!["10.100.0.2".to_string()],
            private_key_path: "/etc/harmony-agent/private.key".to_string(),
            peers: vec![],
            http: None,
        };

        let network_config: NetworkConfig = json_config.into();
        assert!(network_config.enable_wireguard);
        assert_eq!(network_config.interface, "wg0");
        assert_eq!(network_config.mtu, 1420);
    }

    #[test]
    fn test_action_serialization() {
        assert_eq!(
            serde_json::to_string(&ControlAction::Connect).unwrap(),
            "\"connect\""
        );
        assert_eq!(
            serde_json::to_string(&ControlAction::Disconnect).unwrap(),
            "\"disconnect\""
        );
        assert_eq!(
            serde_json::to_string(&ControlAction::Status).unwrap(),
            "\"status\""
        );
        assert_eq!(
            serde_json::to_string(&ControlAction::Reload).unwrap(),
            "\"reload\""
        );
        assert_eq!(
            serde_json::to_string(&ControlAction::RotateKeys).unwrap(),
            "\"rotate_keys\""
        );
    }
}
