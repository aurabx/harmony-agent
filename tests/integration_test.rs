//! Integration tests for wg-agent
//!
//! These tests verify the interaction between different modules.

use wg_agent::config::{Config, NetworkConfig, PeerConfig};
use wg_agent::monitoring::{ConnectionState, Monitor};
use wg_agent::security::{validate_interface_name, validate_network_name};

#[test]
fn test_config_integration() {
    // Test that configuration can be created and validated
    let mut config = Config::default();
    
    let network = NetworkConfig {
        enable_wireguard: true,
        interface: "wg0".to_string(),
        mtu: 1420,
        private_key_path: "/tmp/test.key".to_string(),
        dns: vec![],
        peers: vec![],
        http: None,
    };
    
    config.add_network("test-network".to_string(), network);
    
    assert_eq!(config.networks.len(), 1);
    assert!(config.get_network("test-network").is_some());
}

#[test]
fn test_monitoring_integration() {
    // Test monitoring system
    let monitor = Monitor::new();
    
    monitor.register_network("test".to_string());
    monitor.update_state("test", ConnectionState::Connected);
    monitor.update_traffic("test", 1000, 2000);
    monitor.update_peers("test", 3, 2, 2);
    
    let stats = monitor.get_stats("test").unwrap();
    assert_eq!(stats.state, ConnectionState::Connected);
    assert_eq!(stats.tx_bytes, 1000);
    assert_eq!(stats.rx_bytes, 2000);
    assert_eq!(stats.total_peers, 3);
    assert_eq!(stats.active_peers, 2);
    
    let health = monitor.health_check().unwrap();
    assert!(health.is_healthy());
}

#[test]
fn test_security_validation_integration() {
    // Test security validation integration
    assert!(validate_network_name("valid-network").is_ok());
    assert!(validate_network_name("my_network_123").is_ok());
    assert!(validate_network_name("").is_err());
    assert!(validate_network_name("-invalid").is_err());
    
    assert!(validate_interface_name("wg0").is_ok());
    assert!(validate_interface_name("wg_vpn").is_ok());
    assert!(validate_interface_name("0wg").is_err());
}

#[test]
fn test_peer_configuration() {
    // Test peer configuration
    let peer = PeerConfig {
        name: "test-peer".to_string(),
        public_key: "test-key".to_string(),
        endpoint: "192.168.1.1:51820".to_string(),
        allowed_ips: vec!["10.0.0.0/24".to_string()],
        persistent_keepalive_secs: 25,
    };
    
    assert_eq!(peer.name, "test-peer");
    assert!(!peer.allowed_ips.is_empty());
}

#[test]
fn test_network_configuration_with_peers() {
    // Test network configuration with multiple peers
    let peer1 = PeerConfig {
        name: "peer1".to_string(),
        public_key: "key1".to_string(),
        endpoint: "192.168.1.1:51820".to_string(),
        allowed_ips: vec!["10.0.1.0/24".to_string()],
        persistent_keepalive_secs: 25,
    };
    
    let peer2 = PeerConfig {
        name: "peer2".to_string(),
        public_key: "key2".to_string(),
        endpoint: "192.168.1.2:51820".to_string(),
        allowed_ips: vec!["10.0.2.0/24".to_string()],
        persistent_keepalive_secs: 25,
    };
    
    let network = NetworkConfig {
        enable_wireguard: true,
        interface: "wg1".to_string(),
        mtu: 1420,
        private_key_path: "/tmp/test.key".to_string(),
        dns: vec![],
        peers: vec![peer1, peer2],
        http: None,
    };
    
    assert_eq!(network.peers.len(), 2);
    assert_eq!(network.peers[0].name, "peer1");
    assert_eq!(network.peers[1].name, "peer2");
}

#[test]
fn test_monitoring_handshake_tracking() {
    let monitor = Monitor::new();
    monitor.register_network("test".to_string());
    
    // Record successful handshakes
    for _ in 0..10 {
        monitor.record_handshake("test", true);
    }
    
    // Record some failures
    for _ in 0..2 {
        monitor.record_handshake("test", false);
    }
    
    let stats = monitor.get_stats("test").unwrap();
    assert_eq!(stats.handshake_successes, 10);
    assert_eq!(stats.handshake_failures, 2);
    
    let rate = stats.handshake_success_rate();
    assert!((rate - 83.33).abs() < 0.1); // ~83.33%
}

#[test]
fn test_monitoring_state_transitions() {
    let monitor = Monitor::new();
    monitor.register_network("test".to_string());
    
    // Test state transitions
    monitor.update_state("test", ConnectionState::Connecting);
    assert_eq!(
        monitor.get_stats("test").unwrap().state,
        ConnectionState::Connecting
    );
    
    monitor.update_state("test", ConnectionState::Connected);
    assert_eq!(
        monitor.get_stats("test").unwrap().state,
        ConnectionState::Connected
    );
    
    monitor.update_state("test", ConnectionState::Degraded);
    assert_eq!(
        monitor.get_stats("test").unwrap().state,
        ConnectionState::Degraded
    );
}

#[test]
fn test_metrics_export() {
    let monitor = Monitor::new();
    monitor.register_network("test".to_string());
    
    monitor.update_traffic("test", 5000, 3000);
    monitor.update_peers("test", 5, 4, 3);
    monitor.record_handshake("test", true);
    
    let metrics = monitor.metrics();
    let prometheus = metrics.export_prometheus();
    
    assert!(prometheus.contains("wg_agent_bytes_transmitted_total"));
    assert!(prometheus.contains("wg_agent_bytes_received_total"));
    assert!(prometheus.contains("wg_agent_active_peers"));
    
    let json = metrics.export_json();
    assert!(json.is_object());
}
