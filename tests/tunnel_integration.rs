//! Integration tests for WireGuard tunnel operations
//!
//! These tests verify end-to-end tunnel functionality including:
//! - Tunnel creation and initialization
//! - Starting and stopping tunnels
//! - Statistics collection
//! - Configuration validation
//!
//! Note: These tests require root privileges to create TUN devices.
//! Run with: sudo -E cargo test --test tunnel_integration

use wg_agent::config::NetworkConfig;
use wg_agent::wireguard::{KeyPair, Tunnel, TunnelConfig, TunnelState};
use std::time::Duration;

/// Helper function to create a basic tunnel configuration for testing
fn create_test_tunnel_config() -> TunnelConfig {
    let keypair = KeyPair::generate();
    
    TunnelConfig {
        interface: if cfg!(target_os = "macos") {
            "utun".to_string()
        } else {
            "wg-test".to_string()
        },
        mtu: 1420,
        dns_servers: vec![],
        keypair,
        peers: vec![],
    }
}

/// Helper to create a tunnel config with a peer
fn create_tunnel_config_with_peer() -> TunnelConfig {
    let local_keypair = KeyPair::generate();
    let peer_keypair = KeyPair::generate();
    
    let peer_config = wg_agent::wireguard::PeerConfig {
        name: "test-peer".to_string(),
        public_key: peer_keypair.public,
        endpoint: Some("127.0.0.1:51820".parse().unwrap()),
        allowed_ips: vec!["10.0.0.0/24".to_string()],
        keepalive_interval: Some(Duration::from_secs(25)),
        preshared_key: None,
    };
    
    TunnelConfig {
        interface: if cfg!(target_os = "macos") {
            "utun".to_string()
        } else {
            "wg-test".to_string()
        },
        mtu: 1420,
        dns_servers: vec!["10.0.0.2".to_string()],
        keypair: local_keypair,
        peers: vec![peer_config],
    }
}

#[tokio::test]
async fn test_tunnel_creation() {
    let config = create_test_tunnel_config();
    
    let tunnel = Tunnel::new(config);
    assert!(tunnel.is_ok(), "Tunnel creation should succeed");
    
    let tunnel = tunnel.unwrap();
    let state = tunnel.state().await;
    assert_eq!(state, TunnelState::Uninitialized, "New tunnel should be uninitialized");
}

#[tokio::test]
async fn test_tunnel_config_validation() {
    let keypair = KeyPair::generate();
    
    // Test invalid MTU
    let config = TunnelConfig {
        interface: "wg0".to_string(),
        mtu: 2000, // Invalid
        dns_servers: vec![],
        keypair: keypair.clone(),
        peers: vec![],
    };
    
    assert!(config.validate().is_err(), "Invalid MTU should fail validation");
    
    // Test empty interface name
    let config = TunnelConfig {
        interface: "".to_string(),
        mtu: 1420,
        dns_servers: vec![],
        keypair: keypair.clone(),
        peers: vec![],
    };
    
    assert!(config.validate().is_err(), "Empty interface should fail validation");
    
    // Test valid config
    let config = TunnelConfig {
        interface: "wg0".to_string(),
        mtu: 1420,
        dns_servers: vec![],
        keypair,
        peers: vec![],
    };
    
    assert!(config.validate().is_ok(), "Valid config should pass validation");
}

#[tokio::test]
#[ignore] // Requires root privileges
async fn test_tunnel_start_stop() {
    let config = create_tunnel_config_with_peer();
    let tunnel = Tunnel::new(config).expect("Tunnel creation failed");
    
    // Check initial state
    assert_eq!(tunnel.state().await, TunnelState::Uninitialized);
    
    // Start tunnel
    match tunnel.start().await {
        Ok(()) => {
            println!("✓ Tunnel started successfully");
            
            // Verify state
            let state = tunnel.state().await;
            assert_eq!(state, TunnelState::Active, "Tunnel should be active after start");
            
            // Get initial stats
            let stats = tunnel.stats().await;
            println!("Initial stats: TX={}, RX={}", stats.total_tx_bytes, stats.total_rx_bytes);
            assert_eq!(stats.total_peers, 1, "Should have 1 peer");
            
            // Let it run briefly
            tokio::time::sleep(Duration::from_secs(2)).await;
            
            // Check stats again
            let stats = tunnel.stats().await;
            println!("After 2s: TX={}, RX={}", stats.total_tx_bytes, stats.total_rx_bytes);
            
            // Stop tunnel
            tunnel.stop().await.expect("Tunnel stop failed");
            
            // Verify stopped state
            let state = tunnel.state().await;
            assert_eq!(state, TunnelState::Stopped, "Tunnel should be stopped");
            
            println!("✓ Tunnel stopped successfully");
        }
        Err(e) => {
            println!("✗ Tunnel start failed: {}", e);
            println!("  This is expected if not running with root privileges");
            println!("  Run with: sudo -E cargo test --test tunnel_integration -- --ignored");
            panic!("Tunnel start failed: {}", e);
        }
    }
}

#[tokio::test]
#[ignore] // Requires root privileges
async fn test_tunnel_lifecycle_multiple_cycles() {
    let config = create_tunnel_config_with_peer();
    
    // Test multiple start/stop cycles
    for cycle in 1..=3 {
        println!("Cycle {}/3", cycle);
        
        let tunnel = Tunnel::new(config.clone()).expect("Tunnel creation failed");
        
        // Start
        tunnel.start().await.expect("Tunnel start failed");
        assert_eq!(tunnel.state().await, TunnelState::Active);
        
        // Run briefly
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Stop
        tunnel.stop().await.expect("Tunnel stop failed");
        assert_eq!(tunnel.state().await, TunnelState::Stopped);
        
        println!("  ✓ Cycle {} completed", cycle);
        
        // Brief pause between cycles
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    println!("✓ All cycles completed successfully");
}

#[tokio::test]
async fn test_tunnel_from_network_config() {
    // Create a network config
    let keypair = KeyPair::generate();
    let temp_dir = tempfile::tempdir().unwrap();
    let key_path = temp_dir.path().join("private.key");
    
    // Save keypair
    keypair.private.save_to_file(&key_path).expect("Failed to save key");
    
    let peer_keypair = KeyPair::generate();
    let peer_config = wg_agent::config::PeerConfig {
        name: "test-peer".to_string(),
        public_key: peer_keypair.public.to_base64(),
        endpoint: "127.0.0.1:51820".to_string(),
        allowed_ips: vec!["10.0.0.0/24".to_string()],
        persistent_keepalive_secs: 25,
    };
    
    let network_config = NetworkConfig {
        enable_wireguard: true,
        interface: if cfg!(target_os = "macos") {
            "utun".to_string()
        } else {
            "wg-test".to_string()
        },
        mtu: 1420,
        private_key_path: key_path.to_string_lossy().to_string(),
        dns: vec!["10.0.0.2".to_string()],
        peers: vec![peer_config],
        http: None,
    };
    
    // Create tunnel from network config
    let tunnel = Tunnel::from_network_config(&network_config);
    assert!(tunnel.is_ok(), "Should create tunnel from network config");
    
    let tunnel = tunnel.unwrap();
    assert_eq!(tunnel.state().await, TunnelState::Uninitialized);
    
    // Verify peer names
    let peer_names = tunnel.peer_names().await;
    assert_eq!(peer_names.len(), 0, "Peers not initialized until start");
}

#[tokio::test]
async fn test_tunnel_state_transitions() {
    let config = create_test_tunnel_config();
    let tunnel = Tunnel::new(config).unwrap();
    
    // Test invalid state transitions
    let state = tunnel.state().await;
    assert!(state.can_start());
    assert!(!state.can_stop());
    
    // Can't stop an uninitialized tunnel
    let result = tunnel.stop().await;
    assert!(result.is_err(), "Stopping uninitialized tunnel should fail");
}

#[tokio::test]
async fn test_tunnel_stats() {
    let config = create_tunnel_config_with_peer();
    let tunnel = Tunnel::new(config).unwrap();
    
    // Get stats before starting
    let stats = tunnel.stats().await;
    assert_eq!(stats.state, TunnelState::Uninitialized);
    assert_eq!(stats.total_tx_bytes, 0);
    assert_eq!(stats.total_rx_bytes, 0);
    assert_eq!(stats.total_peers, 0);
}

#[tokio::test]
async fn test_peer_config_validation() {
    use wg_agent::wireguard::PeerConfig;
    
    let keypair = KeyPair::generate();
    
    // Valid IPv4 CIDR
    let config = PeerConfig {
        name: "test".to_string(),
        public_key: keypair.public.clone(),
        endpoint: Some("127.0.0.1:51820".parse().unwrap()),
        allowed_ips: vec!["10.0.0.0/24".to_string()],
        keepalive_interval: Some(Duration::from_secs(25)),
        preshared_key: None,
    };
    assert!(config.validate().is_ok());
    
    // Valid IPv6 CIDR
    let config = PeerConfig {
        name: "test".to_string(),
        public_key: keypair.public.clone(),
        endpoint: Some("127.0.0.1:51820".parse().unwrap()),
        allowed_ips: vec!["fd42::/48".to_string()],
        keepalive_interval: None,
        preshared_key: None,
    };
    assert!(config.validate().is_ok());
    
    // Invalid CIDR (no prefix)
    let config = PeerConfig {
        name: "test".to_string(),
        public_key: keypair.public.clone(),
        endpoint: Some("127.0.0.1:51820".parse().unwrap()),
        allowed_ips: vec!["10.0.0.0".to_string()],
        keepalive_interval: None,
        preshared_key: None,
    };
    assert!(config.validate().is_err(), "Invalid CIDR should fail");
    
    // Invalid prefix length
    let config = PeerConfig {
        name: "test".to_string(),
        public_key: keypair.public,
        endpoint: Some("127.0.0.1:51820".parse().unwrap()),
        allowed_ips: vec!["10.0.0.0/33".to_string()],
        keepalive_interval: None,
        preshared_key: None,
    };
    assert!(config.validate().is_err(), "Invalid prefix length should fail");
}

#[tokio::test]
async fn test_concurrent_tunnel_operations() {
    // Test creating multiple tunnel configs concurrently
    let handles: Vec<_> = (0..5)
        .map(|i| {
            tokio::spawn(async move {
                let config = create_test_tunnel_config();
                let tunnel = Tunnel::new(config);
                assert!(tunnel.is_ok(), "Tunnel {} creation should succeed", i);
                let tunnel = tunnel.unwrap();
                assert_eq!(tunnel.state().await, TunnelState::Uninitialized);
            })
        })
        .collect();
    
    for handle in handles {
        handle.await.expect("Task should complete");
    }
}
