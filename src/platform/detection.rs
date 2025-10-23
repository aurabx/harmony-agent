//! Platform detection and environment identification
//!
//! This module detects the operating system, container environment,
//! and available capabilities for WireGuard operations.

use std::fs;
use std::path::Path;

/// Container environment detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerEnvironment {
    /// Not running in a container
    None,
    /// Running in Docker
    Docker,
    /// Running in Kubernetes
    Kubernetes,
    /// Running in Podman
    Podman,
    /// Unknown container environment
    Unknown,
}

/// Platform information
#[derive(Debug, Clone)]
pub struct PlatformInfo {
    /// Operating system name
    pub os: String,
    /// OS version
    pub os_version: String,
    /// Container environment
    pub container: ContainerEnvironment,
    /// Whether running with elevated privileges
    pub is_privileged: bool,
    /// Kernel version (Linux only)
    pub kernel_version: Option<String>,
}

impl PlatformInfo {
    /// Create a new PlatformInfo with defaults
    pub fn new() -> Self {
        Self {
            os: std::env::consts::OS.to_string(),
            os_version: String::new(),
            container: ContainerEnvironment::None,
            is_privileged: false,
            kernel_version: None,
        }
    }

    /// Check if running in any container
    pub fn is_containerized(&self) -> bool {
        !matches!(self.container, ContainerEnvironment::None)
    }

    /// Get a human-readable platform description
    pub fn description(&self) -> String {
        let container_str = match self.container {
            ContainerEnvironment::None => String::new(),
            ContainerEnvironment::Docker => " (Docker)".to_string(),
            ContainerEnvironment::Kubernetes => " (Kubernetes)".to_string(),
            ContainerEnvironment::Podman => " (Podman)".to_string(),
            ContainerEnvironment::Unknown => " (Container)".to_string(),
        };

        format!(
            "{} {}{}",
            self.os,
            if self.os_version.is_empty() {
                "unknown"
            } else {
                &self.os_version
            },
            container_str
        )
    }
}

impl Default for PlatformInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Detect the current platform environment
pub fn detect_environment() -> PlatformInfo {
    let mut info = PlatformInfo::new();

    // Detect container environment
    info.container = detect_container();

    // Detect privilege level
    info.is_privileged = is_privileged();

    // Platform-specific detection
    #[cfg(target_os = "linux")]
    {
        info.os_version = detect_linux_version();
        info.kernel_version = detect_kernel_version();
    }

    #[cfg(target_os = "macos")]
    {
        info.os_version = detect_macos_version();
    }

    #[cfg(target_os = "windows")]
    {
        info.os_version = detect_windows_version();
    }

    info
}

/// Detect container environment
fn detect_container() -> ContainerEnvironment {
    // Check for Docker
    if Path::new("/.dockerenv").exists() {
        return ContainerEnvironment::Docker;
    }

    // Check for Kubernetes
    if std::env::var("KUBERNETES_SERVICE_HOST").is_ok() {
        return ContainerEnvironment::Kubernetes;
    }

    // Check for Podman
    if std::env::var("container").as_deref() == Ok("podman") {
        return ContainerEnvironment::Podman;
    }

    // Check cgroup for container indicators
    if let Ok(cgroup) = fs::read_to_string("/proc/1/cgroup") {
        if cgroup.contains("docker") {
            return ContainerEnvironment::Docker;
        }
        if cgroup.contains("kubepods") {
            return ContainerEnvironment::Kubernetes;
        }
        if cgroup.contains("podman") {
            return ContainerEnvironment::Podman;
        }
        // Generic container detection
        if cgroup.contains("/lxc/") || cgroup.contains("/docker/") {
            return ContainerEnvironment::Unknown;
        }
    }

    ContainerEnvironment::None
}

/// Check if running with elevated privileges
fn is_privileged() -> bool {
    #[cfg(unix)]
    {
        // Check if running as root (UID 0)
        unsafe { libc::geteuid() == 0 }
    }

    #[cfg(windows)]
    {
        // On Windows, we'd need to check for Administrator privileges
        // For now, return false as a safe default
        false
    }
}

#[cfg(target_os = "linux")]
fn detect_linux_version() -> String {
    // Try to read /etc/os-release
    if let Ok(contents) = fs::read_to_string("/etc/os-release") {
        for line in contents.lines() {
            if let Some(version) = line.strip_prefix("PRETTY_NAME=\"") {
                if let Some(version) = version.strip_suffix('\"') {
                    return version.to_string();
                }
            }
        }
    }

    // Fallback to /etc/issue
    if let Ok(contents) = fs::read_to_string("/etc/issue") {
        if let Some(first_line) = contents.lines().next() {
            return first_line.replace("\\n", "").replace("\\l", "").trim().to_string();
        }
    }

    String::from("Linux (unknown distribution)")
}

#[cfg(target_os = "linux")]
fn detect_kernel_version() -> Option<String> {
    fs::read_to_string("/proc/version")
        .ok()
        .and_then(|v| v.split_whitespace().nth(2).map(String::from))
}

#[cfg(target_os = "macos")]
fn detect_macos_version() -> String {
    use std::process::Command;

    Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|| String::from("unknown"))
}

#[cfg(target_os = "windows")]
fn detect_windows_version() -> String {
    // Basic Windows version detection
    // Would use Windows API for more detailed version
    String::from("Windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_info_new() {
        let info = PlatformInfo::new();
        assert!(!info.os.is_empty());
        assert_eq!(info.container, ContainerEnvironment::None);
    }

    #[test]
    fn test_platform_info_description() {
        let mut info = PlatformInfo::new();
        info.os = "Linux".to_string();
        info.os_version = "Ubuntu 22.04".to_string();
        info.container = ContainerEnvironment::Docker;

        let desc = info.description();
        assert!(desc.contains("Linux"));
        assert!(desc.contains("Ubuntu"));
        assert!(desc.contains("Docker"));
    }

    #[test]
    fn test_is_containerized() {
        let mut info = PlatformInfo::new();
        assert!(!info.is_containerized());

        info.container = ContainerEnvironment::Docker;
        assert!(info.is_containerized());

        info.container = ContainerEnvironment::Kubernetes;
        assert!(info.is_containerized());
    }

    #[test]
    fn test_detect_environment() {
        let info = detect_environment();
        assert!(!info.os.is_empty());
        // Environment-specific assertions would go here
    }

    #[test]
    fn test_container_environment_equality() {
        assert_eq!(ContainerEnvironment::None, ContainerEnvironment::None);
        assert_eq!(ContainerEnvironment::Docker, ContainerEnvironment::Docker);
        assert_ne!(ContainerEnvironment::Docker, ContainerEnvironment::Kubernetes);
    }
}
