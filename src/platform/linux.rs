//! Linux platform implementation
//!
//! This module provides Linux-specific implementations for TUN/TAP device
//! management, routing, and DNS configuration.

use crate::error::{Result, WgAgentError};
use crate::platform::{detection, Platform, PlatformInfo};
use std::process::Command;
use tracing::{debug, info, warn};

/// Linux platform implementation
pub struct LinuxPlatform {
    info: PlatformInfo,
}

impl LinuxPlatform {
    /// Create a new Linux platform instance
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

impl Default for LinuxPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl Platform for LinuxPlatform {
    fn info(&self) -> &PlatformInfo {
        &self.info
    }

    fn create_interface(&self, name: &str) -> Result<()> {
        info!("Creating Linux interface: {}", name);

        // Note: Actual TUN/TAP creation will be handled by WireGuard in Phase 4
        // For now, we just validate the interface name and check if it exists
        
        // Check if interface already exists
        let output = self.run_command("ip", &["link", "show", name]);
        if output.is_ok() {
            warn!("Interface {} already exists", name);
            return Ok(());
        }

        debug!("Interface {} does not exist yet (will be created by WireGuard)", name);
        Ok(())
    }

    fn destroy_interface(&self, name: &str) -> Result<()> {
        info!("Destroying Linux interface: {}", name);

        // Check if interface exists
        if self.run_command("ip", &["link", "show", name]).is_err() {
            debug!("Interface {} does not exist", name);
            return Ok(());
        }

        // Bring interface down first
        self.interface_down(name)?;

        // Delete the interface
        self.run_command("ip", &["link", "delete", name])?;

        info!("Interface {} destroyed", name);
        Ok(())
    }

    fn set_mtu(&self, interface: &str, mtu: u16) -> Result<()> {
        info!("Setting MTU for interface {}: {}", interface, mtu);
        self.run_command("ip", &["link", "set", interface, "mtu", &mtu.to_string()])?;
        Ok(())
    }

    fn interface_up(&self, interface: &str) -> Result<()> {
        info!("Bringing interface {} up", interface);
        self.run_command("ip", &["link", "set", interface, "up"])?;
        Ok(())
    }

    fn interface_down(&self, interface: &str) -> Result<()> {
        info!("Bringing interface {} down", interface);
        self.run_command("ip", &["link", "set", interface, "down"])?;
        Ok(())
    }

    fn configure_routes(&self, interface: &str, routes: &[String]) -> Result<()> {
        info!("Configuring {} routes for interface {}", routes.len(), interface);

        for route in routes {
            debug!("Adding route: {} via {}", route, interface);
            self.run_command("ip", &["route", "add", route, "dev", interface])?;
        }

        Ok(())
    }

    fn remove_routes(&self, interface: &str, routes: &[String]) -> Result<()> {
        info!("Removing {} routes from interface {}", routes.len(), interface);

        for route in routes {
            debug!("Removing route: {} via {}", route, interface);
            // Ignore errors when removing routes (they might not exist)
            let _ = self.run_command("ip", &["route", "del", route, "dev", interface]);
        }

        Ok(())
    }

    fn configure_dns(&self, interface: &str, dns_servers: &[String]) -> Result<()> {
        info!("Configuring {} DNS servers for interface {}", dns_servers.len(), interface);

        // On Linux, DNS configuration can be done through:
        // 1. resolvconf (if available)
        // 2. systemd-resolved
        // 3. Direct /etc/resolv.conf manipulation (not recommended)

        // Try resolvconf first
        if Command::new("which").arg("resolvconf").status().is_ok() {
            let dns_config = dns_servers.iter()
                .map(|dns| format!("nameserver {}", dns))
                .collect::<Vec<_>>()
                .join("\n");

            let mut cmd = Command::new("resolvconf");
            cmd.arg("-a").arg(interface);
            
            use std::io::Write;
            let mut child = cmd.stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| WgAgentError::Platform(format!("Failed to spawn resolvconf: {}", e)))?;

            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(dns_config.as_bytes())
                    .map_err(|e| WgAgentError::Platform(format!("Failed to write DNS config: {}", e)))?;
            }

            child.wait()
                .map_err(|e| WgAgentError::Platform(format!("resolvconf failed: {}", e)))?;

            debug!("DNS configured via resolvconf");
            return Ok(());
        }

        warn!("No DNS configuration method available (resolvconf not found)");
        Ok(())
    }

    fn remove_dns(&self, interface: &str) -> Result<()> {
        info!("Removing DNS configuration for interface {}", interface);

        // Try resolvconf
        if Command::new("which").arg("resolvconf").status().is_ok() {
            let _ = self.run_command("resolvconf", &["-d", interface]);
        }

        Ok(())
    }

    fn check_capabilities(&self) -> Result<Vec<String>> {
        let mut missing = Vec::new();

        // Check if running as root or with NET_ADMIN capability
        if !self.info.is_privileged {
            missing.push("NET_ADMIN capability required (run as root or with CAP_NET_ADMIN)".to_string());
        }

        // Check for required commands
        for cmd in &["ip", "iptables"] {
            if Command::new("which").arg(cmd).status().is_err() {
                missing.push(format!("Required command not found: {}", cmd));
            }
        }

        Ok(missing)
    }

    fn create_tun_device(&self, name: &str, mtu: u16) -> Result<tun::platform::Device> {
        info!("Creating TUN device '{}' with MTU {}", name, mtu);

        let mut config = tun::Configuration::default();
        
        config
            .name(name)
            .mtu(mtu as i32)
            .up();

        #[cfg(target_os = "linux")]
        config.platform(|config| {
            // Linux-specific: use TUN (not TAP) mode
            config.packet_information(false);
        });

        let device = tun::create(&config).map_err(|e| {
            WgAgentError::TunDevice(format!("Failed to create TUN device '{}': {}", name, e))
        })?;

        info!("TUN device '{}' created successfully", name);
        Ok(device)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linux_platform_new() {
        let platform = LinuxPlatform::new();
        assert_eq!(platform.info().os, "linux");
    }

    #[test]
    fn test_platform_info() {
        let platform = LinuxPlatform::new();
        let info = platform.info();
        assert!(!info.os.is_empty());
    }
}
