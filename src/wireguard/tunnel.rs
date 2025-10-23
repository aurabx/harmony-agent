//! WireGuard tunnel management
//!
//! This module handles the complete lifecycle of a WireGuard tunnel,
//! including creation, configuration, and teardown.

use crate::config::NetworkConfig;
use crate::error::{Result, WgAgentError};
use crate::platform::{get_platform, Platform};
use crate::wireguard::{DeviceConfig, KeyPair, Peer, PeerConfig, WgDevice};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Tunnel state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelState {
    /// Tunnel is not initialized
    Uninitialized,
    /// Tunnel is being set up
    Starting,
    /// Tunnel is active and running
    Active,
    /// Tunnel is being torn down
    Stopping,
    /// Tunnel is stopped
    Stopped,
    /// Tunnel encountered an error
    Error,
}

impl TunnelState {
    /// Check if the tunnel is in a running state
    pub fn is_running(&self) -> bool {
        matches!(self, TunnelState::Active)
    }

    /// Check if the tunnel can be started
    pub fn can_start(&self) -> bool {
        matches!(
            self,
            TunnelState::Uninitialized | TunnelState::Stopped | TunnelState::Error
        )
    }

    /// Check if the tunnel can be stopped
    pub fn can_stop(&self) -> bool {
        matches!(self, TunnelState::Active | TunnelState::Starting)
    }
}

impl std::fmt::Display for TunnelState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TunnelState::Uninitialized => write!(f, "uninitialized"),
            TunnelState::Starting => write!(f, "starting"),
            TunnelState::Active => write!(f, "active"),
            TunnelState::Stopping => write!(f, "stopping"),
            TunnelState::Stopped => write!(f, "stopped"),
            TunnelState::Error => write!(f, "error"),
        }
    }
}

/// Tunnel configuration
#[derive(Debug, Clone)]
pub struct TunnelConfig {
    /// Interface name
    pub interface: String,
    /// MTU value
    pub mtu: u16,
    /// DNS servers
    pub dns_servers: Vec<String>,
    /// Our key pair
    pub keypair: KeyPair,
    /// Peer configurations
    pub peers: Vec<PeerConfig>,
}

impl TunnelConfig {
    /// Create tunnel configuration from network configuration
    pub fn from_network_config(config: &NetworkConfig) -> Result<Self> {
        // Load the private key
        let keypair = KeyPair::from_file(&config.private_key_path)?;

        // Convert peer configurations
        let peers: Vec<PeerConfig> = config
            .peers
            .iter()
            .map(|p| PeerConfig::from(p.clone()))
            .collect();

        Ok(Self {
            interface: config.interface.clone(),
            mtu: config.mtu,
            dns_servers: config.dns.clone(),
            keypair,
            peers,
        })
    }

    /// Validate the tunnel configuration
    pub fn validate(&self) -> Result<()> {
        // Validate interface name
        if self.interface.is_empty() {
            return Err(WgAgentError::Config("Interface name cannot be empty".to_string()));
        }

        // Validate MTU
        if !(1280..=1500).contains(&self.mtu) {
            return Err(WgAgentError::Config(format!(
                "MTU {} is out of valid range (1280-1500)",
                self.mtu
            )));
        }

        // Validate each peer
        for peer in &self.peers {
            peer.validate()?;
        }

        if self.peers.is_empty() {
            warn!("Tunnel has no peers configured");
        }

        Ok(())
    }
}

/// WireGuard tunnel
pub struct Tunnel {
    /// Tunnel configuration
    config: TunnelConfig,
    /// Current tunnel state
    state: Arc<RwLock<TunnelState>>,
    /// Active peers
    peers: Arc<RwLock<HashMap<String, Peer>>>,
    /// Platform implementation
    platform: Box<dyn Platform>,
    /// WireGuard device (None when stopped)
    device: Arc<RwLock<Option<WgDevice>>>,
}

impl Tunnel {
    /// Create a new tunnel from configuration
    pub fn new(config: TunnelConfig) -> Result<Self> {
        config.validate()?;

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(TunnelState::Uninitialized)),
            peers: Arc::new(RwLock::new(HashMap::new())),
            platform: get_platform(),
            device: Arc::new(RwLock::new(None)),
        })
    }

    /// Create a tunnel from network configuration
    pub fn from_network_config(config: &NetworkConfig) -> Result<Self> {
        let tunnel_config = TunnelConfig::from_network_config(config)?;
        Self::new(tunnel_config)
    }

    /// Get the current tunnel state
    pub async fn state(&self) -> TunnelState {
        *self.state.read().await
    }

    /// Start the tunnel
    pub async fn start(&self) -> Result<()> {
        let mut state = self.state.write().await;

        if !state.can_start() {
            return Err(WgAgentError::InvalidState(format!(
                "Cannot start tunnel in state: {}",
                state
            )));
        }

        info!(
            "Starting WireGuard tunnel on interface: {}",
            self.config.interface
        );
        *state = TunnelState::Starting;
        drop(state);

        // Check platform capabilities
        if let Ok(missing) = self.platform.check_capabilities() {
            if !missing.is_empty() {
                error!("Missing capabilities: {:?}", missing);
                *self.state.write().await = TunnelState::Error;
                return Err(WgAgentError::Platform(format!(
                    "Missing required capabilities: {}",
                    missing.join(", ")
                )));
            }
        }

        // Create WireGuard device configuration
        let device_config = DeviceConfig {
            interface: self.config.interface.clone(),
            mtu: self.config.mtu,
            keypair: self.config.keypair.clone(),
            listen_port: 0, // Use random port
            peers: self.config.peers.clone(),
        };

        // Create WireGuard device (this creates TUN device and starts packet processing)
        let device = match WgDevice::new(device_config, self.platform.as_ref()).await {
            Ok(d) => d,
            Err(e) => {
                error!("Failed to create WireGuard device: {}", e);
                *self.state.write().await = TunnelState::Error;
                return Err(e);
            }
        };

        // Get the actual interface name (may differ on macOS)
        let interface_name = &self.config.interface;

        // Configure routes for all peers
        for peer_config in &self.config.peers {
            if !peer_config.allowed_ips.is_empty() {
                debug!(
                    "Configuring routes for peer: {} ({} routes)",
                    peer_config.name,
                    peer_config.allowed_ips.len()
                );
                
                if let Err(e) = self.platform.configure_routes(
                    interface_name,
                    &peer_config.allowed_ips,
                ) {
                    warn!("Failed to configure routes for peer {}: {}", peer_config.name, e);
                }
            }
        }

        // Configure DNS
        if !self.config.dns_servers.is_empty() {
            debug!(
                "Configuring DNS servers: {:?}",
                self.config.dns_servers
            );
            
            if let Err(e) = self.platform.configure_dns(
                interface_name,
                &self.config.dns_servers,
            ) {
                warn!("Failed to configure DNS: {}", e);
            }
        }

        // Initialize peer tracking (for stats/monitoring)
        let mut peers = self.peers.write().await;
        for peer_config in &self.config.peers {
            match Peer::new(peer_config.clone()) {
                Ok(mut peer) => {
                    peer.activate();
                    peers.insert(peer_config.name.clone(), peer);
                    info!("Peer '{}' initialized and activated", peer_config.name);
                }
                Err(e) => {
                    warn!("Failed to initialize peer '{}': {}", peer_config.name, e);
                }
            }
        }
        drop(peers);

        // WireGuard device has already brought the interface up, skip manual interface_up
        // Store the device
        *self.device.write().await = Some(device);

        *self.state.write().await = TunnelState::Active;
        info!(
            "WireGuard tunnel started successfully on interface: {}",
            self.config.interface
        );

        Ok(())
    }

    /// Stop the tunnel
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.state.write().await;

        if !state.can_stop() {
            return Err(WgAgentError::InvalidState(format!(
                "Cannot stop tunnel in state: {}",
                state
            )));
        }

        info!(
            "Stopping WireGuard tunnel on interface: {}",
            self.config.interface
        );
        *state = TunnelState::Stopping;
        drop(state);

        // Stop WireGuard device first (this stops packet processing and TUN device)
        let device = self.device.write().await.take();
        if let Some(device) = device {
            info!("Stopping WireGuard device");
            if let Err(e) = device.stop().await {
                warn!("Failed to stop WireGuard device: {}", e);
            }
        }

        // Deactivate all peers
        let mut peers = self.peers.write().await;
        for peer in peers.values_mut() {
            peer.deactivate();
        }
        peers.clear();
        drop(peers);

        // Remove DNS configuration
        if let Err(e) = self.platform.remove_dns(&self.config.interface) {
            warn!("Failed to remove DNS configuration: {}", e);
        }

        // Remove routes for all peers
        for peer_config in &self.config.peers {
            if !peer_config.allowed_ips.is_empty() {
                if let Err(e) = self.platform.remove_routes(
                    &self.config.interface,
                    &peer_config.allowed_ips,
                ) {
                    warn!(
                        "Failed to remove routes for peer {}: {}",
                        peer_config.name, e
                    );
                }
            }
        }

        // Destroy the interface
        if let Err(e) = self.platform.destroy_interface(&self.config.interface) {
            warn!("Failed to destroy interface: {}", e);
        }

        *self.state.write().await = TunnelState::Stopped;
        info!(
            "WireGuard tunnel stopped on interface: {}",
            self.config.interface
        );

        Ok(())
    }

    /// Reload the tunnel configuration
    pub async fn reload(&self, new_config: TunnelConfig) -> Result<()> {
        info!("Reloading tunnel configuration");

        // For now, simple approach: stop and restart
        // Future: implement hot-reload for peer changes
        self.stop().await?;
        
        // Create new tunnel with new config
        let new_tunnel = Self::new(new_config)?;
        new_tunnel.start().await?;

        Ok(())
    }

    /// Get peer status
    pub async fn peer_status(&self, peer_name: &str) -> Option<String> {
        let peers = self.peers.read().await;
        peers.get(peer_name).map(|p| p.status())
    }

    /// Get all peer names
    pub async fn peer_names(&self) -> Vec<String> {
        let peers = self.peers.read().await;
        peers.keys().cloned().collect()
    }

    /// Get tunnel statistics
    pub async fn stats(&self) -> TunnelStats {
        let peers = self.peers.read().await;
        let state = self.state.read().await;

        // Get real stats from WgDevice if available
        let (total_tx, total_rx) = if let Some(device) = self.device.read().await.as_ref() {
            let device_stats = device.stats().await;
            (device_stats.tx_bytes, device_stats.rx_bytes)
        } else {
            // Fallback to peer stats if device not available
            let mut total_tx = 0;
            let mut total_rx = 0;
            for peer in peers.values() {
                total_tx += peer.stats.tx_bytes;
                total_rx += peer.stats.rx_bytes;
            }
            (total_tx, total_rx)
        };

        let active_peers = peers.values().filter(|p| p.active).count();
        let healthy_peers = peers.values().filter(|p| p.is_healthy()).count();

        TunnelStats {
            state: *state,
            interface: self.config.interface.clone(),
            total_peers: peers.len(),
            active_peers,
            healthy_peers,
            total_tx_bytes: total_tx,
            total_rx_bytes: total_rx,
        }
    }
}

/// Tunnel statistics
#[derive(Debug, Clone)]
pub struct TunnelStats {
    /// Current tunnel state
    pub state: TunnelState,
    /// Interface name
    pub interface: String,
    /// Total number of configured peers
    pub total_peers: usize,
    /// Number of active peers
    pub active_peers: usize,
    /// Number of healthy peers (with recent handshake)
    pub healthy_peers: usize,
    /// Total bytes transmitted
    pub total_tx_bytes: u64,
    /// Total bytes received
    pub total_rx_bytes: u64,
}

impl std::fmt::Display for TunnelStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Tunnel {} [{}]: {} peers ({} active, {} healthy), TX: {} bytes, RX: {} bytes",
            self.interface,
            self.state,
            self.total_peers,
            self.active_peers,
            self.healthy_peers,
            self.total_tx_bytes,
            self.total_rx_bytes
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wireguard::PrivateKey;

    #[test]
    fn test_tunnel_state_transitions() {
        let state = TunnelState::Uninitialized;
        assert!(state.can_start());
        assert!(!state.can_stop());
        assert!(!state.is_running());

        let state = TunnelState::Active;
        assert!(!state.can_start());
        assert!(state.can_stop());
        assert!(state.is_running());
    }

    #[test]
    fn test_tunnel_config_validation() {
        let keypair = KeyPair::generate();
        
        let config = TunnelConfig {
            interface: "wg0".to_string(),
            mtu: 1420,
            dns_servers: vec![],
            keypair,
            peers: vec![],
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_tunnel_config_invalid_mtu() {
        let keypair = KeyPair::generate();
        
        let config = TunnelConfig {
            interface: "wg0".to_string(),
            mtu: 2000, // Invalid MTU
            dns_servers: vec![],
            keypair,
            peers: vec![],
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn test_tunnel_config_empty_interface() {
        let keypair = KeyPair::generate();
        
        let config = TunnelConfig {
            interface: "".to_string(), // Empty interface
            mtu: 1420,
            dns_servers: vec![],
            keypair,
            peers: vec![],
        };

        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_tunnel_creation() {
        let keypair = KeyPair::generate();
        
        let config = TunnelConfig {
            interface: "wg0".to_string(),
            mtu: 1420,
            dns_servers: vec![],
            keypair,
            peers: vec![],
        };

        let tunnel = Tunnel::new(config).unwrap();
        assert_eq!(tunnel.state().await, TunnelState::Uninitialized);
    }

    #[tokio::test]
    async fn test_tunnel_stats() {
        let keypair = KeyPair::generate();
        
        let config = TunnelConfig {
            interface: "wg0".to_string(),
            mtu: 1420,
            dns_servers: vec![],
            keypair,
            peers: vec![],
        };

        let tunnel = Tunnel::new(config).unwrap();
        let stats = tunnel.stats().await;
        
        assert_eq!(stats.interface, "wg0");
        assert_eq!(stats.total_peers, 0);
        assert_eq!(stats.state, TunnelState::Uninitialized);
    }
}
