//! macOS platform implementation
//!
//! This module provides macOS-specific implementations for TUN/TAP device
//! management, routing, and DNS configuration using utun interfaces.

use crate::error::{Result, WgAgentError};
use crate::platform::{detection, Platform, PlatformInfo};
use std::io::Write;
use std::process::{Command, Stdio};
use tun::Device;
use tracing::{debug, info, warn};

/// macOS platform implementation
pub struct MacOsPlatform {
    info: PlatformInfo,
}

impl MacOsPlatform {
    /// Create a new macOS platform instance
    pub fn new() -> Self {
        Self {
            info: detection::detect_environment(),
        }
    }

    /// Execute a system command
    fn run_command(&self, program: &str, args: &[&str]) -> Result<String> {
        debug!("Executing command: {} {:?}", program, args);

        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|e| {
                WgAgentError::Platform(format!(
                    "Failed to execute {} {}: {}",
                    program,
                    args.join(" "),
                    e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WgAgentError::Platform(format!(
                "Command failed: {} {}: {}",
                program,
                args.join(" "),
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl Default for MacOsPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl Platform for MacOsPlatform {
    fn info(&self) -> &PlatformInfo {
        &self.info
    }

    fn create_interface(&self, name: &str) -> Result<()> {
        info!("Creating macOS interface: {}", name);

        // Note: Actual utun creation will be handled by WireGuard in Phase 4
        // macOS uses utun interfaces which are created automatically by the kernel
        
        debug!("Interface {} will be created by WireGuard as utun device", name);
        Ok(())
    }

    fn destroy_interface(&self, name: &str) -> Result<()> {
        info!("Destroying macOS interface: {}", name);

        // Check if interface exists
        if self.run_command("ifconfig", &[name]).is_err() {
            debug!("Interface {} does not exist", name);
            return Ok(());
        }

        // On macOS, utun interfaces are typically destroyed when the process exits
        // or when the file descriptor is closed
        warn!("macOS utun interfaces are managed by the kernel");
        Ok(())
    }

    fn set_mtu(&self, interface: &str, mtu: u16) -> Result<()> {
        info!("Setting MTU for interface {}: {}", interface, mtu);
        self.run_command("ifconfig", &[interface, "mtu", &mtu.to_string()])?;
        Ok(())
    }

    fn interface_up(&self, interface: &str) -> Result<()> {
        info!("Bringing interface {} up", interface);
        self.run_command("ifconfig", &[interface, "up"])?;
        Ok(())
    }

    fn interface_down(&self, interface: &str) -> Result<()> {
        info!("Bringing interface {} down", interface);
        self.run_command("ifconfig", &[interface, "down"])?;
        Ok(())
    }

    fn set_address(&self, interface: &str, address: &str) -> Result<()> {
        info!("Setting address {} on interface {}", address, interface);
        // Parse CIDR notation (e.g., "10.100.0.2/24")
        let parts: Vec<&str> = address.split('/').collect();
        if parts.len() != 2 {
            return Err(WgAgentError::Config(format!(
                "Invalid address format: {} (expected CIDR notation like 10.100.0.2/24)",
                address
            )));
        }
        let ip = parts[0];
        let prefix_len: u8 = parts[1].parse().map_err(|_| {
            WgAgentError::Config(format!("Invalid prefix length: {}", parts[1]))
        })?;
        
        // macOS utun interfaces are point-to-point, so we need to specify both local and remote addresses
        // For WireGuard tunnels, we typically use the first IP in the subnet as the gateway/destination
        let ip_parts: Vec<&str> = ip.split('.').collect();
        if ip_parts.len() != 4 {
            return Err(WgAgentError::Config(format!("Invalid IP address: {}", ip)));
        }
        
        // Use .1 as the gateway (e.g., 10.100.0.1 for 10.100.0.2/24)
        let dest = format!("{}.{}.{}.1", ip_parts[0], ip_parts[1], ip_parts[2]);
        
        // Convert prefix length to netmask
        let netmask = match prefix_len {
            24 => "255.255.255.0",
            16 => "255.255.0.0",
            8 => "255.0.0.0",
            _ => return Err(WgAgentError::Config(format!(
                "Unsupported prefix length: {}", prefix_len
            ))),
        };
        
        // For macOS point-to-point: ifconfig utun8 10.100.0.2 10.100.0.1 netmask 255.255.255.0
        self.run_command("ifconfig", &[interface, ip, &dest, "netmask", netmask])?;
        Ok(())
    }

    fn configure_routes(&self, interface: &str, routes: &[String]) -> Result<()> {
        info!("Configuring {} routes for interface {}", routes.len(), interface);

        for route in routes {
            debug!("Adding route: {} via {}", route, interface);
            // macOS uses 'route add' command
            self.run_command("route", &["add", "-net", route, "-interface", interface])?;
        }

        Ok(())
    }

    fn remove_routes(&self, interface: &str, routes: &[String]) -> Result<()> {
        info!("Removing {} routes from interface {}", routes.len(), interface);

        for route in routes {
            debug!("Removing route: {} via {}", route, interface);
            // Ignore errors when removing routes (they might not exist)
            let _ = self.run_command("route", &["delete", "-net", route]);
        }

        Ok(())
    }

    fn configure_dns(&self, interface: &str, dns_servers: &[String]) -> Result<()> {
        info!("Configuring {} DNS servers for interface {}", dns_servers.len(), interface);

        if dns_servers.is_empty() {
            return Ok(());
        }

        // Use scutil to configure DNS for the interface
        // We create a State:/Network/Service/<interface>/DNS entry
        debug!("Configuring DNS via scutil for interface {}", interface);
        
        // Build scutil configuration
        let mut config = String::new();
        config.push_str("d.init\n");
        config.push_str(&format!("d.add ServerAddresses * {}", dns_servers.join(" ")));
        config.push('\n');
        config.push_str(&format!("set State:/Network/Service/{}/DNS\n", interface));
        config.push_str("quit\n");
        
        // Execute scutil with the configuration
        let mut child = Command::new("scutil")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                WgAgentError::Platform(format!("Failed to spawn scutil: {}", e))
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(config.as_bytes()).map_err(|e| {
                WgAgentError::Platform(format!("Failed to write to scutil stdin: {}", e))
            })?;
        }

        let output = child.wait_with_output().map_err(|e| {
            WgAgentError::Platform(format!("Failed to wait for scutil: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("scutil returned non-zero status: {}", stderr);
            // Don't fail on DNS configuration errors, just warn
        } else {
            info!("DNS configured successfully for interface {}", interface);
        }
        
        Ok(())
    }

    fn remove_dns(&self, interface: &str) -> Result<()> {
        info!("Removing DNS configuration for interface {}", interface);
        
        // Remove DNS configuration using scutil
        debug!("Removing DNS configuration via scutil for interface {}", interface);
        
        let config = format!(
            "remove State:/Network/Service/{}/DNS\nquit\n",
            interface
        );
        
        let mut child = Command::new("scutil")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                WgAgentError::Platform(format!("Failed to spawn scutil: {}", e))
            })?;

        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(config.as_bytes());
        }

        let output = child.wait_with_output().map_err(|e| {
            WgAgentError::Platform(format!("Failed to wait for scutil: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            debug!("scutil remove returned non-zero status: {}", stderr);
            // Ignore errors when removing (entry might not exist)
        } else {
            info!("DNS configuration removed for interface {}", interface);
        }
        
        Ok(())
    }

    fn check_capabilities(&self) -> Result<Vec<String>> {
        let mut missing = Vec::new();

        // Check if running as root
        if !self.info.is_privileged {
            missing.push("Root privileges required for TUN/TAP management".to_string());
        }

        // Check for required commands
        for cmd in &["ifconfig", "route"] {
            if Command::new("which").arg(cmd).status().is_err() {
                missing.push(format!("Required command not found: {}", cmd));
            }
        }

        Ok(missing)
    }

    fn create_tun_device(&self, name: &str, mtu: u16) -> Result<tun::platform::Device> {
        info!("Creating TUN device '{}' with MTU {}", name, mtu);

        // On macOS, we need to let the system assign the utun number
        // The tun crate requires a name, so we'll use "utun" and let the OS pick the number
        let mut config = tun::Configuration::default();
        
        // Don't set a specific name - let macOS auto-assign
        config
            .mtu(mtu as i32)
            .up();

        let device = tun::create(&config).map_err(|e| {
            WgAgentError::TunDevice(format!("Failed to create TUN device: {}", e))
        })?;

        // Get the actual name assigned by macOS
        let actual_name = device.name().map_err(|e| {
            WgAgentError::TunDevice(format!("Failed to get TUN device name: {}", e))
        })?;
        
        info!("TUN device '{}' created successfully (requested: '{}')", actual_name, name);
        Ok(device)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macos_platform_new() {
        let platform = MacOsPlatform::new();
        assert_eq!(platform.info().os, "macos");
    }

    #[test]
    fn test_platform_info() {
        let platform = MacOsPlatform::new();
        let info = platform.info();
        assert!(!info.os.is_empty());
    }
}
