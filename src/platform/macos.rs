//! macOS platform implementation
//!
//! This module provides macOS-specific implementations for TUN/TAP device
//! management, routing, and DNS configuration using utun interfaces.

use crate::error::{Result, WgAgentError};
use crate::platform::{detection, Platform, PlatformInfo};
use std::process::Command;
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

        // On macOS, DNS configuration is typically done through:
        // 1. scutil (System Configuration utility)
        // 2. networksetup command

        // Build DNS server list
        let _dns_args: Vec<String> = dns_servers.iter()
            .flat_map(|dns| vec!["-dns".to_string(), dns.clone()])
            .collect();

        // Use scutil to configure DNS
        debug!("Configuring DNS via scutil for interface {}", interface);
        
        // Note: Full scutil integration requires more complex setup
        // This is a placeholder for Phase 4
        warn!("DNS configuration on macOS requires additional implementation");
        
        Ok(())
    }

    fn remove_dns(&self, interface: &str) -> Result<()> {
        info!("Removing DNS configuration for interface {}", interface);
        
        // DNS removal on macOS
        debug!("Removing DNS for interface {}", interface);
        
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
