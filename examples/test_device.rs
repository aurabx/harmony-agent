//! Basic WireGuard device test
//!
//! This example creates a WireGuard device and verifies it can be initialized.
//! Run with: cargo run --example test_device

use wg_agent::platform::get_platform;
use wg_agent::wireguard::{DeviceConfig, KeyPair, PeerConfig, WgDevice};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    println!("🔧 WireGuard Device Test");
    println!("========================\n");

    // Generate keypairs
    println!("Generating local keypair...");
    let local_keypair = KeyPair::generate();
    println!("  Local public key: {}", local_keypair.public);

    println!("\nGenerating peer keypair...");
    let peer_keypair = KeyPair::generate();
    println!("  Peer public key: {}", peer_keypair.public);

    // Create peer configuration
    println!("\nConfiguring peer...");
    let peer_config = PeerConfig {
        name: "test-peer".to_string(),
        public_key: peer_keypair.public.clone(),
        endpoint: Some("127.0.0.1:51820".parse()?),
        allowed_ips: vec!["10.0.0.0/24".to_string()],
        keepalive_interval: Some(Duration::from_secs(25)),
        preshared_key: None,
    };
    println!("  Peer: {} -> {}", peer_config.name, peer_config.endpoint.unwrap());

    // Create device configuration
    println!("\nCreating device configuration...");
    // Note: On macOS, use "utun" to let the system assign a number
    // On Linux, use "wg0" or similar
    let interface_name = if cfg!(target_os = "macos") {
        "utun".to_string()
    } else {
        "wg0".to_string()
    };
    
    let device_config = DeviceConfig {
        interface: interface_name.clone(),
        mtu: 1420,
        keypair: local_keypair,
        listen_port: 0, // Random port
        peers: vec![peer_config],
    };
    println!("  Interface: {}", device_config.interface);
    println!("  MTU: {}", device_config.mtu);
    println!("  Peers: {}", device_config.peers.len());

    // Get platform
    println!("\nDetecting platform...");
    let platform = get_platform();
    println!("  Platform: {:?}", platform.info().os);
    println!("  Privileged: {}", platform.info().is_privileged);

    // Check capabilities
    println!("\nChecking platform capabilities...");
    match platform.check_capabilities() {
        Ok(missing) => {
            if missing.is_empty() {
                println!("  ✓ All capabilities available");
            } else {
                println!("  ⚠ Missing capabilities:");
                for cap in &missing {
                    println!("    - {}", cap);
                }
                println!("\n  Note: This test may not work without proper privileges.");
                println!("  Try running with: sudo -E cargo run --example test_device");
                return Ok(());
            }
        }
        Err(e) => {
            println!("  ✗ Failed to check capabilities: {}", e);
            return Err(e.into());
        }
    }

    // Create WireGuard device
    println!("\n🚀 Creating WireGuard device...");
    match WgDevice::new(device_config, platform.as_ref()).await {
        Ok(device) => {
            println!("  ✓ Device created successfully!");
            
            // Get initial stats
            println!("\n📊 Initial statistics:");
            let stats = device.stats().await;
            println!("  TX bytes: {}", stats.tx_bytes);
            println!("  RX bytes: {}", stats.rx_bytes);
            println!("  TX packets: {}", stats.tx_packets);
            println!("  RX packets: {}", stats.rx_packets);
            println!("  Errors: {}", stats.errors);

            // Let it run for a few seconds
            println!("\n⏱  Running for 5 seconds to verify packet processing tasks...");
            tokio::time::sleep(Duration::from_secs(5)).await;

            // Check stats again
            let stats = device.stats().await;
            println!("\n📊 Statistics after 5 seconds:");
            println!("  TX bytes: {}", stats.tx_bytes);
            println!("  RX bytes: {}", stats.rx_bytes);
            println!("  TX packets: {}", stats.tx_packets);
            println!("  RX packets: {}", stats.rx_packets);
            println!("  Errors: {}", stats.errors);

            // Stop device
            println!("\n🛑 Stopping device...");
            device.stop().await?;
            println!("  ✓ Device stopped successfully!");

            println!("\n✅ Test completed successfully!");
        }
        Err(e) => {
            println!("  ✗ Failed to create device: {}", e);
            println!("\n  This is expected if:");
            println!("    1. You're not running as root/sudo");
            println!("    2. TUN device creation requires privileges");
            println!("\n  Try: sudo -E cargo run --example test_device");
            return Err(e.into());
        }
    }

    Ok(())
}
