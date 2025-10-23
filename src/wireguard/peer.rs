//! WireGuard peer management
//!
//! This module handles peer configuration, lifecycle, and statistics.

use crate::config::PeerConfig as ConfigPeer;
use crate::error::{Result, WgAgentError};
use crate::wireguard::PublicKey;
use std::net::{IpAddr, SocketAddr};
use std::time::{Duration, SystemTime};
use tracing::{debug, warn};

/// Peer statistics
#[derive(Debug, Clone, Default)]
pub struct PeerStats {
    /// Total bytes transmitted to this peer
    pub tx_bytes: u64,
    /// Total bytes received from this peer
    pub rx_bytes: u64,
    /// Last handshake time
    pub last_handshake: Option<SystemTime>,
    /// Number of handshake attempts
    pub handshake_attempts: u64,
    /// Number of successful handshakes
    pub successful_handshakes: u64,
}

impl PeerStats {
    /// Check if the peer had a recent handshake (within last 3 minutes)
    pub fn has_recent_handshake(&self) -> bool {
        if let Some(last) = self.last_handshake {
            if let Ok(elapsed) = last.elapsed() {
                return elapsed < Duration::from_secs(180);
            }
        }
        false
    }

    /// Get handshake success rate (0.0 to 1.0)
    pub fn handshake_success_rate(&self) -> f64 {
        if self.handshake_attempts == 0 {
            return 0.0;
        }
        self.successful_handshakes as f64 / self.handshake_attempts as f64
    }
}

/// WireGuard peer configuration
#[derive(Debug, Clone)]
pub struct PeerConfig {
    /// Peer name (for identification)
    pub name: String,
    /// Peer's public key
    pub public_key: PublicKey,
    /// Peer endpoint address
    pub endpoint: Option<SocketAddr>,
    /// Allowed IP addresses/ranges
    pub allowed_ips: Vec<String>,
    /// Persistent keepalive interval
    pub keepalive_interval: Option<Duration>,
    /// Preshared key (optional, for additional security)
    pub preshared_key: Option<[u8; 32]>,
}

impl PeerConfig {
    /// Create a new peer configuration
    pub fn new(name: String, public_key: PublicKey) -> Self {
        Self {
            name,
            public_key,
            endpoint: None,
            allowed_ips: Vec::new(),
            keepalive_interval: None,
            preshared_key: None,
        }
    }

    /// Parse endpoint from string (host:port)
    pub fn set_endpoint(&mut self, endpoint: &str) -> Result<()> {
        let addr: SocketAddr = endpoint.parse().map_err(|e| {
            WgAgentError::Config(format!("Invalid endpoint '{}': {}", endpoint, e))
        })?;
        self.endpoint = Some(addr);
        Ok(())
    }

    /// Set keepalive interval in seconds
    pub fn set_keepalive_secs(&mut self, secs: u16) {
        if secs > 0 {
            self.keepalive_interval = Some(Duration::from_secs(secs as u64));
        } else {
            self.keepalive_interval = None;
        }
    }

    /// Validate the peer configuration
    pub fn validate(&self) -> Result<()> {
        // Validate allowed IPs
        for allowed_ip in &self.allowed_ips {
            Self::validate_allowed_ip(allowed_ip)?;
        }

        // Warn if no endpoint and no allowed IPs
        if self.endpoint.is_none() && self.allowed_ips.is_empty() {
            warn!(
                "Peer '{}' has no endpoint and no allowed IPs - this peer may not be reachable",
                self.name
            );
        }

        Ok(())
    }

    /// Validate an allowed IP (CIDR notation)
    fn validate_allowed_ip(ip: &str) -> Result<()> {
        let parts: Vec<&str> = ip.split('/').collect();
        
        if parts.len() != 2 {
            return Err(WgAgentError::Config(format!(
                "Invalid allowed IP format: {} (expected CIDR notation like 10.0.0.0/24)",
                ip
            )));
        }

        // Validate IP part
        let _addr: IpAddr = parts[0].parse().map_err(|e| {
            WgAgentError::Config(format!("Invalid IP address in '{}': {}", ip, e))
        })?;

        // Validate prefix length
        let prefix: u8 = parts[1].parse().map_err(|e| {
            WgAgentError::Config(format!("Invalid prefix length in '{}': {}", ip, e))
        })?;

        // Check prefix range based on IP version
        let addr: IpAddr = parts[0].parse().unwrap();
        let max_prefix = match addr {
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
}

impl From<ConfigPeer> for PeerConfig {
    fn from(config: ConfigPeer) -> Self {
        let mut peer = Self::new(
            config.name,
            PublicKey::from_base64(&config.public_key)
                .expect("Public key should be validated in config phase"),
        );

        // Set endpoint
        if let Err(e) = peer.set_endpoint(&config.endpoint) {
            warn!("Failed to parse peer endpoint: {}", e);
        }

        // Set allowed IPs
        peer.allowed_ips = config.allowed_ips;

        // Set keepalive
        peer.set_keepalive_secs(config.persistent_keepalive_secs);

        peer
    }
}

/// Active WireGuard peer
pub struct Peer {
    /// Peer configuration
    pub config: PeerConfig,
    /// Peer statistics
    pub stats: PeerStats,
    /// Whether the peer is currently active
    pub active: bool,
}

impl Peer {
    /// Create a new peer from configuration
    pub fn new(config: PeerConfig) -> Result<Self> {
        config.validate()?;
        
        Ok(Self {
            config,
            stats: PeerStats::default(),
            active: false,
        })
    }

    /// Activate the peer
    pub fn activate(&mut self) {
        debug!("Activating peer: {}", self.config.name);
        self.active = true;
    }

    /// Deactivate the peer
    pub fn deactivate(&mut self) {
        debug!("Deactivating peer: {}", self.config.name);
        self.active = false;
    }

    /// Update peer statistics
    pub fn update_stats(&mut self, tx_bytes: u64, rx_bytes: u64) {
        self.stats.tx_bytes = tx_bytes;
        self.stats.rx_bytes = rx_bytes;
    }

    /// Record a handshake attempt
    pub fn record_handshake_attempt(&mut self) {
        self.stats.handshake_attempts += 1;
    }

    /// Record a successful handshake
    pub fn record_successful_handshake(&mut self) {
        self.stats.successful_handshakes += 1;
        self.stats.last_handshake = Some(SystemTime::now());
    }

    /// Check if the peer is healthy (has recent handshake)
    pub fn is_healthy(&self) -> bool {
        self.active && self.stats.has_recent_handshake()
    }

    /// Get a human-readable status
    pub fn status(&self) -> String {
        if !self.active {
            return "inactive".to_string();
        }

        if self.stats.has_recent_handshake() {
            format!(
                "active (tx: {}, rx: {}, handshakes: {}/{})",
                format_bytes(self.stats.tx_bytes),
                format_bytes(self.stats.rx_bytes),
                self.stats.successful_handshakes,
                self.stats.handshake_attempts
            )
        } else {
            "active (no recent handshake)".to_string()
        }
    }
}

impl fmt::Debug for Peer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Peer")
            .field("name", &self.config.name)
            .field("public_key", &self.config.public_key)
            .field("endpoint", &self.config.endpoint)
            .field("active", &self.active)
            .field("stats", &self.stats)
            .finish()
    }
}

use std::fmt;

/// Format bytes in human-readable form
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wireguard::PrivateKey;

    #[test]
    fn test_peer_config_new() {
        let public_key = PrivateKey::generate().public_key();
        let config = PeerConfig::new("test-peer".to_string(), public_key.clone());
        
        assert_eq!(config.name, "test-peer");
        assert_eq!(config.public_key, public_key);
        assert!(config.endpoint.is_none());
        assert!(config.allowed_ips.is_empty());
    }

    #[test]
    fn test_peer_config_set_endpoint() {
        let public_key = PrivateKey::generate().public_key();
        let mut config = PeerConfig::new("test-peer".to_string(), public_key);
        
        config.set_endpoint("192.168.1.1:51820").unwrap();
        assert!(config.endpoint.is_some());
        assert_eq!(config.endpoint.unwrap().port(), 51820);
    }

    #[test]
    fn test_peer_config_set_keepalive() {
        let public_key = PrivateKey::generate().public_key();
        let mut config = PeerConfig::new("test-peer".to_string(), public_key);
        
        config.set_keepalive_secs(25);
        assert_eq!(config.keepalive_interval, Some(Duration::from_secs(25)));
        
        config.set_keepalive_secs(0);
        assert_eq!(config.keepalive_interval, None);
    }

    #[test]
    fn test_peer_config_validate_allowed_ip() {
        assert!(PeerConfig::validate_allowed_ip("10.0.0.0/24").is_ok());
        assert!(PeerConfig::validate_allowed_ip("192.168.1.0/24").is_ok());
        assert!(PeerConfig::validate_allowed_ip("fe80::/64").is_ok());
        assert!(PeerConfig::validate_allowed_ip("0.0.0.0/0").is_ok());
        
        assert!(PeerConfig::validate_allowed_ip("10.0.0.0").is_err());
        assert!(PeerConfig::validate_allowed_ip("10.0.0.0/33").is_err());
        assert!(PeerConfig::validate_allowed_ip("invalid/24").is_err());
    }

    #[test]
    fn test_peer_new() {
        let public_key = PrivateKey::generate().public_key();
        let mut config = PeerConfig::new("test-peer".to_string(), public_key);
        config.allowed_ips = vec!["10.0.0.0/24".to_string()];
        
        let peer = Peer::new(config).unwrap();
        assert!(!peer.active);
        assert_eq!(peer.stats.tx_bytes, 0);
        assert_eq!(peer.stats.rx_bytes, 0);
    }

    #[test]
    fn test_peer_activation() {
        let public_key = PrivateKey::generate().public_key();
        let config = PeerConfig::new("test-peer".to_string(), public_key);
        let mut peer = Peer::new(config).unwrap();
        
        assert!(!peer.active);
        peer.activate();
        assert!(peer.active);
        peer.deactivate();
        assert!(!peer.active);
    }

    #[test]
    fn test_peer_stats_update() {
        let public_key = PrivateKey::generate().public_key();
        let config = PeerConfig::new("test-peer".to_string(), public_key);
        let mut peer = Peer::new(config).unwrap();
        
        peer.update_stats(1024, 2048);
        assert_eq!(peer.stats.tx_bytes, 1024);
        assert_eq!(peer.stats.rx_bytes, 2048);
    }

    #[test]
    fn test_peer_handshake_tracking() {
        let public_key = PrivateKey::generate().public_key();
        let config = PeerConfig::new("test-peer".to_string(), public_key);
        let mut peer = Peer::new(config).unwrap();
        
        peer.record_handshake_attempt();
        peer.record_handshake_attempt();
        peer.record_successful_handshake();
        
        assert_eq!(peer.stats.handshake_attempts, 2);
        assert_eq!(peer.stats.successful_handshakes, 1);
        assert!(peer.stats.last_handshake.is_some());
        assert!(peer.stats.has_recent_handshake());
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }
}
