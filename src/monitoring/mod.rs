//! Monitoring and observability
//!
//! This module provides health checks, metrics, and monitoring capabilities
//! for WireGuard tunnels and the agent service.

use crate::error::Result;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, info};

mod health;
mod metrics;

pub use health::{HealthCheck, HealthStatus, check_health};
pub use metrics::{Metrics, MetricsCollector, MetricType};

/// Connection state for monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected
    Disconnected,
    /// Connecting
    Connecting,
    /// Connected and healthy
    Connected,
    /// Connected but degraded
    Degraded,
    /// Connection failed
    Failed,
}

impl std::fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "disconnected"),
            Self::Connecting => write!(f, "connecting"),
            Self::Connected => write!(f, "connected"),
            Self::Degraded => write!(f, "degraded"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    /// Network name
    pub network: String,
    /// Connection state
    pub state: ConnectionState,
    /// Connection established time
    pub connected_at: Option<Instant>,
    /// Total bytes transmitted
    pub tx_bytes: u64,
    /// Total bytes received
    pub rx_bytes: u64,
    /// Total peers
    pub total_peers: usize,
    /// Active peers
    pub active_peers: usize,
    /// Healthy peers
    pub healthy_peers: usize,
    /// Handshake successes
    pub handshake_successes: u64,
    /// Handshake failures
    pub handshake_failures: u64,
}

impl NetworkStats {
    /// Create new network statistics
    pub fn new(network: String) -> Self {
        Self {
            network,
            state: ConnectionState::Disconnected,
            connected_at: None,
            tx_bytes: 0,
            rx_bytes: 0,
            total_peers: 0,
            active_peers: 0,
            healthy_peers: 0,
            handshake_successes: 0,
            handshake_failures: 0,
        }
    }

    /// Get connection uptime
    pub fn uptime(&self) -> Option<Duration> {
        self.connected_at.map(|t| t.elapsed())
    }

    /// Get handshake success rate
    pub fn handshake_success_rate(&self) -> f64 {
        let total = self.handshake_successes + self.handshake_failures;
        if total == 0 {
            return 0.0;
        }
        (self.handshake_successes as f64) / (total as f64) * 100.0
    }
}

/// Monitor for tracking network statistics
pub struct Monitor {
    /// Network statistics by network name
    stats: Arc<RwLock<HashMap<String, NetworkStats>>>,
    /// Metrics collector
    metrics: Arc<MetricsCollector>,
}

impl Monitor {
    /// Create a new monitor
    pub fn new() -> Self {
        Self {
            stats: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(MetricsCollector::new()),
        }
    }

    /// Register a new network
    pub fn register_network(&self, network: String) {
        let mut stats = self.stats.write().unwrap();
        stats.insert(network.clone(), NetworkStats::new(network));
        info!("Registered network for monitoring");
    }

    /// Update connection state
    pub fn update_state(&self, network: &str, state: ConnectionState) {
        let mut stats = self.stats.write().unwrap();
        if let Some(net_stats) = stats.get_mut(network) {
            net_stats.state = state;
            if state == ConnectionState::Connected {
                net_stats.connected_at = Some(Instant::now());
            }
            debug!("Updated connection state for {}: {}", network, state);
        }
    }

    /// Update traffic statistics
    pub fn update_traffic(&self, network: &str, tx_bytes: u64, rx_bytes: u64) {
        let mut stats = self.stats.write().unwrap();
        if let Some(net_stats) = stats.get_mut(network) {
            net_stats.tx_bytes = tx_bytes;
            net_stats.rx_bytes = rx_bytes;
            
            // Record metrics
            self.metrics.record(MetricType::BytesTransmitted, tx_bytes as f64);
            self.metrics.record(MetricType::BytesReceived, rx_bytes as f64);
        }
    }

    /// Update peer statistics
    pub fn update_peers(&self, network: &str, total: usize, active: usize, healthy: usize) {
        let mut stats = self.stats.write().unwrap();
        if let Some(net_stats) = stats.get_mut(network) {
            net_stats.total_peers = total;
            net_stats.active_peers = active;
            net_stats.healthy_peers = healthy;
            
            self.metrics.record(MetricType::ActivePeers, active as f64);
        }
    }

    /// Record handshake result
    pub fn record_handshake(&self, network: &str, success: bool) {
        let mut stats = self.stats.write().unwrap();
        if let Some(net_stats) = stats.get_mut(network) {
            if success {
                net_stats.handshake_successes += 1;
                self.metrics.record(MetricType::HandshakeSuccess, 1.0);
            } else {
                net_stats.handshake_failures += 1;
                self.metrics.record(MetricType::HandshakeFailure, 1.0);
            }
        }
    }

    /// Get statistics for a network
    pub fn get_stats(&self, network: &str) -> Option<NetworkStats> {
        let stats = self.stats.read().unwrap();
        stats.get(network).cloned()
    }

    /// Get all network statistics
    pub fn get_all_stats(&self) -> HashMap<String, NetworkStats> {
        let stats = self.stats.read().unwrap();
        stats.clone()
    }

    /// Get metrics collector
    pub fn metrics(&self) -> Arc<MetricsCollector> {
        self.metrics.clone()
    }

    /// Perform health check
    pub fn health_check(&self) -> Result<HealthCheck> {
        let stats = self.stats.read().unwrap();
        check_health(&stats)
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_state_display() {
        assert_eq!(ConnectionState::Connected.to_string(), "connected");
        assert_eq!(ConnectionState::Disconnected.to_string(), "disconnected");
    }

    #[test]
    fn test_network_stats() {
        let stats = NetworkStats::new("test".to_string());
        assert_eq!(stats.network, "test");
        assert_eq!(stats.state, ConnectionState::Disconnected);
        assert_eq!(stats.uptime(), None);
    }

    #[test]
    fn test_monitor() {
        let monitor = Monitor::new();
        monitor.register_network("test".to_string());
        monitor.update_state("test", ConnectionState::Connected);
        
        let stats = monitor.get_stats("test").unwrap();
        assert_eq!(stats.state, ConnectionState::Connected);
    }
}
