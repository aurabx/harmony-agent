//! Platform-specific implementations
//!
//! This module provides platform-specific abstractions for TUN/TAP device
//! management, routing, and DNS configuration.

use crate::error::Result;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

mod detection;

pub use detection::{detect_environment, ContainerEnvironment, PlatformInfo};

/// Platform trait for cross-platform abstractions
pub trait Platform: Send + Sync {
    /// Get platform information
    fn info(&self) -> &PlatformInfo;

    /// Create a new TUN/TAP device
    fn create_interface(&self, name: &str) -> Result<()>;

    /// Destroy a TUN/TAP device
    fn destroy_interface(&self, name: &str) -> Result<()>;

    /// Set interface MTU
    fn set_mtu(&self, interface: &str, mtu: u16) -> Result<()>;

    /// Bring interface up
    fn interface_up(&self, interface: &str) -> Result<()>;

    /// Bring interface down
    fn interface_down(&self, interface: &str) -> Result<()>;

    /// Configure routes for the interface
    fn configure_routes(&self, interface: &str, routes: &[String]) -> Result<()>;

    /// Remove routes for the interface
    fn remove_routes(&self, interface: &str, routes: &[String]) -> Result<()>;

    /// Configure DNS for the interface
    fn configure_dns(&self, interface: &str, dns_servers: &[String]) -> Result<()>;

    /// Remove DNS configuration
    fn remove_dns(&self, interface: &str) -> Result<()>;

    /// Check if the platform has required capabilities
    fn check_capabilities(&self) -> Result<Vec<String>>;

    /// Create and configure a TUN device for WireGuard
    /// Returns a configured TUN device that can be used for packet I/O
    fn create_tun_device(&self, name: &str, mtu: u16) -> Result<tun::platform::Device>;
}

/// Get the platform implementation for the current OS
pub fn get_platform() -> Box<dyn Platform> {
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxPlatform::new())
    }

    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOsPlatform::new())
    }

    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsPlatform::new())
    }
}
