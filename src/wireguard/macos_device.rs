//! macOS-specific WireGuard device using wireguard-go
//!
//! On macOS, we use the native wireguard-go implementation instead of boringtun
//! because the TUN device integration is more mature and stable.

use crate::error::{Result, WgAgentError};
use crate::wireguard::{DeviceConfig, DeviceStats};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use std::fs;

/// macOS WireGuard device using wireguard-go
pub struct MacOsWgDevice {
    _config: DeviceConfig,
    interface_name: String,
    config_file: PathBuf,
    stats: Arc<RwLock<DeviceStats>>,
    wireguard_go_process: Option<Child>,
}

impl MacOsWgDevice {
    /// Create a new macOS WireGuard device
    pub async fn new(config: DeviceConfig) -> Result<Self> {
        info!("Creating macOS WireGuard device using wireguard-go");

        // Check if wireguard-go is available
        if Command::new("which")
            .arg("wireguard-go")
            .output()
            .is_err()
        {
            return Err(WgAgentError::Platform(
                "wireguard-go not found. Install with: brew install wireguard-tools"
                    .to_string(),
            ));
        }

        // Generate WireGuard config file
        let config_dir = PathBuf::from("/tmp/harmony-agent");
        fs::create_dir_all(&config_dir).map_err(|e| {
            WgAgentError::Platform(format!("Failed to create config directory: {}", e))
        })?;

        let config_file = config_dir.join(format!("{}.conf", config.interface));
        let wg_config = Self::generate_wireguard_config(&config)?;

        fs::write(&config_file, wg_config).map_err(|e| {
            WgAgentError::Platform(format!("Failed to write WireGuard config: {}", e))
        })?;

        info!(
            "Generated WireGuard config at {}",
            config_file.display()
        );

        let stats = Arc::new(RwLock::new(DeviceStats::default()));

        Ok(Self {
            interface_name: config.interface.clone(),
            _config: config,
            config_file,
            stats,
            wireguard_go_process: None,
        })
    }

    /// Generate WireGuard configuration file
    fn generate_wireguard_config(config: &DeviceConfig) -> Result<String> {
        let mut wg_config = String::new();

        // [Interface] section
        wg_config.push_str("[Interface]\n");
        wg_config.push_str(&format!(
            "PrivateKey = {}\n",
            config.keypair.private.to_base64()
        ));

        if config.listen_port > 0 {
            wg_config.push_str(&format!("ListenPort = {}\n", config.listen_port));
        }

        wg_config.push_str("\n");

        // [Peer] sections
        for peer in &config.peers {
            wg_config.push_str("[Peer]\n");
            wg_config.push_str(&format!(
                "PublicKey = {}\n",
                peer.public_key.to_base64()
            ));

            if let Some(endpoint) = peer.endpoint {
                wg_config.push_str(&format!("Endpoint = {}\n", endpoint));
            }

            if !peer.allowed_ips.is_empty() {
                wg_config.push_str(&format!(
                    "AllowedIPs = {}\n",
                    peer.allowed_ips.join(", ")
                ));
            }

            if let Some(interval) = peer.keepalive_interval {
                wg_config.push_str(&format!(
                    "PersistentKeepalive = {}\n",
                    interval.as_secs()
                ));
            }

            wg_config.push_str("\n");
        }

        Ok(wg_config)
    }

    /// Start the WireGuard tunnel using wg-quick
    pub async fn start(&mut self, address: &str, routes: &[String]) -> Result<()> {
        info!("Starting wireguard-go tunnel on {}", self.interface_name);

        // First, bring up the interface with wireguard-go
        let utun_fd = self.create_utun_interface()?;

        // Start wireguard-go process
        let child = Command::new("wireguard-go")
            .arg("-f")
            .arg(format!("utun{}", utun_fd))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                WgAgentError::Platform(format!("Failed to start wireguard-go: {}", e))
            })?;

        info!("wireguard-go process started with PID: {:?}", child.id());
        self.wireguard_go_process = Some(child);

        // Give wireguard-go a moment to create the interface
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Apply WireGuard configuration using wg setconf
        let actual_interface = format!("utun{}", utun_fd);
        Command::new("wg")
            .arg("setconf")
            .arg(&actual_interface)
            .arg(&self.config_file)
            .output()
            .map_err(|e| {
                WgAgentError::Platform(format!("Failed to apply WireGuard config: {}", e))
            })?;

        info!("Applied WireGuard configuration to {}", actual_interface);

        // Configure IP address
        Command::new("ifconfig")
            .args(&[
                &actual_interface,
                address.split('/').next().unwrap(),
                address.split('/').next().unwrap(),
                "netmask",
                "255.255.255.0",
            ])
            .output()
            .map_err(|e| {
                WgAgentError::Platform(format!("Failed to set interface address: {}", e))
            })?;

        // Add routes
        for route in routes {
            debug!("Adding route: {} via {}", route, actual_interface);
            let _ = Command::new("route")
                .args(&["add", "-net", route, "-interface", &actual_interface])
                .output();
        }

        self.interface_name = actual_interface;
        info!("WireGuard tunnel started on {}", self.interface_name);

        Ok(())
    }

    /// Create a utun interface and return its number
    fn create_utun_interface(&self) -> Result<u32> {
        // wireguard-go will create the utun device automatically
        // We just need to find the next available utun number by checking which ones exist
        for i in 0..256 {
            let utun_name = format!("utun{}", i);
            let output = Command::new("ifconfig")
                .arg(&utun_name)
                .output();
            
            // If ifconfig fails or returns error, this utun is available
            match output {
                Ok(out) if !out.status.success() => return Ok(i),
                Err(_) => return Ok(i),
                _ => continue, // Interface exists, try next
            }
        }
        Err(WgAgentError::Platform(
            "No available utun interfaces".to_string(),
        ))
    }

    /// Get the actual interface name
    pub fn interface_name(&self) -> &str {
        &self.interface_name
    }

    /// Get device statistics
    pub async fn stats(&self) -> DeviceStats {
        self.stats.read().await.clone()
    }

    /// Stop the device and clean up
    pub async fn stop(mut self) -> Result<()> {
        info!("Stopping macOS WireGuard device");

        // Kill wireguard-go process
        if let Some(mut child) = self.wireguard_go_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        // Bring down the interface
        let _ = Command::new("ifconfig")
            .args(&[&self.interface_name, "down"])
            .output();

        // Remove config file
        let _ = fs::remove_file(&self.config_file);

        info!("macOS WireGuard device stopped");
        Ok(())
    }
}
