//! Health check implementation
//!
//! This module provides health check functionality for the agent and networks.

use super::{ConnectionState, NetworkStats};
use crate::error::Result;
use std::collections::HashMap;
use tracing::debug;

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Service is healthy
    Healthy,
    /// Service is degraded
    Degraded,
    /// Service is unhealthy
    Unhealthy,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Healthy => write!(f, "healthy"),
            Self::Degraded => write!(f, "degraded"),
            Self::Unhealthy => write!(f, "unhealthy"),
        }
    }
}

/// Health check result
#[derive(Debug, Clone)]
pub struct HealthCheck {
    /// Overall health status
    pub status: HealthStatus,
    /// Health check timestamp
    pub timestamp: std::time::SystemTime,
    /// Network statuses
    pub networks: HashMap<String, NetworkHealth>,
    /// Additional details
    pub details: String,
}

/// Network-specific health information
#[derive(Debug, Clone)]
pub struct NetworkHealth {
    /// Network name
    pub network: String,
    /// Health status
    pub status: HealthStatus,
    /// Connection state
    pub state: ConnectionState,
    /// Peer health
    pub peer_health: f64,  // Percentage of healthy peers
    /// Handshake success rate
    pub handshake_rate: f64,
    /// Details
    pub details: String,
}

impl HealthCheck {
    /// Create a new health check
    pub fn new(status: HealthStatus) -> Self {
        Self {
            status,
            timestamp: std::time::SystemTime::now(),
            networks: HashMap::new(),
            details: String::new(),
        }
    }

    /// Check if healthy
    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }

    /// Check if degraded
    pub fn is_degraded(&self) -> bool {
        self.status == HealthStatus::Degraded
    }

    /// Check if unhealthy
    pub fn is_unhealthy(&self) -> bool {
        self.status == HealthStatus::Unhealthy
    }
}

/// Perform health check on networks
pub fn check_health(stats: &HashMap<String, NetworkStats>) -> Result<HealthCheck> {
    debug!("Performing health check");

    let mut overall_status = HealthStatus::Healthy;
    let mut network_healths = HashMap::new();
    let mut details = Vec::new();

    if stats.is_empty() {
        details.push("No networks registered".to_string());
    }

    for (name, net_stats) in stats.iter() {
        let network_health = check_network_health(net_stats);
        
        // Update overall status based on worst network
        match network_health.status {
            HealthStatus::Unhealthy => overall_status = HealthStatus::Unhealthy,
            HealthStatus::Degraded if overall_status == HealthStatus::Healthy => {
                overall_status = HealthStatus::Degraded;
            }
            _ => {}
        }

        if network_health.status != HealthStatus::Healthy {
            details.push(format!("{}: {}", name, network_health.details));
        }

        network_healths.insert(name.clone(), network_health);
    }

    let mut health = HealthCheck::new(overall_status);
    health.networks = network_healths;
    health.details = if details.is_empty() {
        "All systems operational".to_string()
    } else {
        details.join("; ")
    };

    Ok(health)
}

/// Check health of a single network
fn check_network_health(stats: &NetworkStats) -> NetworkHealth {
    let mut status = HealthStatus::Healthy;
    let mut details = Vec::new();

    // Check connection state
    match stats.state {
        ConnectionState::Disconnected => {
            status = HealthStatus::Unhealthy;
            details.push("disconnected".to_string());
        }
        ConnectionState::Failed => {
            status = HealthStatus::Unhealthy;
            details.push("connection failed".to_string());
        }
        ConnectionState::Degraded => {
            status = HealthStatus::Degraded;
            details.push("connection degraded".to_string());
        }
        ConnectionState::Connecting => {
            if status == HealthStatus::Healthy {
                status = HealthStatus::Degraded;
            }
            details.push("connecting".to_string());
        }
        ConnectionState::Connected => {
            // Continue checking other metrics
        }
    }

    // Calculate peer health percentage
    let peer_health = if stats.total_peers > 0 {
        (stats.healthy_peers as f64 / stats.total_peers as f64) * 100.0
    } else {
        0.0
    };

    // Check peer health
    if stats.total_peers > 0 {
        if stats.healthy_peers == 0 {
            status = HealthStatus::Unhealthy;
            details.push("no healthy peers".to_string());
        } else if peer_health < 50.0 && status == HealthStatus::Healthy {
            status = HealthStatus::Degraded;
            details.push(format!("low peer health: {:.1}%", peer_health));
        }
    }

    // Check handshake success rate
    let handshake_rate = stats.handshake_success_rate();
    let total_handshakes = stats.handshake_successes + stats.handshake_failures;
    
    if total_handshakes > 10 {  // Only check if we have enough samples
        if handshake_rate < 50.0 {
            status = HealthStatus::Unhealthy;
            details.push(format!("low handshake rate: {:.1}%", handshake_rate));
        } else if handshake_rate < 80.0 && status == HealthStatus::Healthy {
            status = HealthStatus::Degraded;
            details.push(format!("handshake rate: {:.1}%", handshake_rate));
        }
    }

    NetworkHealth {
        network: stats.network.clone(),
        status,
        state: stats.state,
        peer_health,
        handshake_rate,
        details: if details.is_empty() {
            "healthy".to_string()
        } else {
            details.join(", ")
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "healthy");
        assert_eq!(HealthStatus::Degraded.to_string(), "degraded");
        assert_eq!(HealthStatus::Unhealthy.to_string(), "unhealthy");
    }

    #[test]
    fn test_health_check_empty() {
        let stats = HashMap::new();
        let health = check_health(&stats).unwrap();
        assert_eq!(health.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_network_health_disconnected() {
        let mut stats = NetworkStats::new("test".to_string());
        stats.state = ConnectionState::Disconnected;
        
        let health = check_network_health(&stats);
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_network_health_connected() {
        let mut stats = NetworkStats::new("test".to_string());
        stats.state = ConnectionState::Connected;
        stats.total_peers = 2;
        stats.healthy_peers = 2;
        
        let health = check_network_health(&stats);
        assert_eq!(health.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_peer_health_calculation() {
        let mut stats = NetworkStats::new("test".to_string());
        stats.total_peers = 4;
        stats.healthy_peers = 2;
        
        let health = check_network_health(&stats);
        assert_eq!(health.peer_health, 50.0);
    }
}
