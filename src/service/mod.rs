//! Service and daemon management
//!
//! This module provides abstractions for running wg-agent as a system service
//! or daemon across different platforms (systemd, Windows Service, LaunchDaemon).

use crate::error::WgAgentError;
use std::time::Duration;
use tracing::{debug, info};

#[cfg(target_os = "linux")]
mod systemd;

#[cfg(target_os = "macos")]
mod launchd;

#[cfg(target_os = "windows")]
mod windows_service;

#[cfg(target_os = "linux")]
pub use systemd::SystemdService;

#[cfg(target_os = "macos")]
pub use launchd::LaunchdService;

#[cfg(target_os = "windows")]
pub use windows_service::WindowsService;

/// Service lifecycle trait
pub trait Service {
    /// Initialize the service
    fn init(&mut self) -> Result<(), WgAgentError>;

    /// Start the service
    fn start(&mut self) -> Result<(), WgAgentError>;

    /// Stop the service gracefully
    fn stop(&mut self) -> Result<(), WgAgentError>;

    /// Reload configuration
    fn reload(&mut self) -> Result<(), WgAgentError>;

    /// Check if service is running
    fn is_running(&self) -> bool;

    /// Get service status
    fn status(&self) -> ServiceStatus;

    /// Notify service manager of readiness (e.g., SD_NOTIFY)
    fn notify_ready(&self) -> Result<(), WgAgentError>;

    /// Notify service manager of stopping
    fn notify_stopping(&self) -> Result<(), WgAgentError>;

    /// Setup signal handlers for graceful shutdown
    fn setup_signal_handlers(&mut self) -> Result<(), WgAgentError>;
}

/// Service status information
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceStatus {
    /// Service state
    pub state: ServiceState,
    /// Uptime in seconds
    pub uptime: Option<Duration>,
    /// Process ID
    pub pid: Option<u32>,
    /// Last error if any
    pub last_error: Option<String>,
}

/// Service state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    /// Service is initializing
    Initializing,
    /// Service is running
    Running,
    /// Service is stopping
    Stopping,
    /// Service is stopped
    Stopped,
    /// Service encountered an error
    Failed,
}

impl std::fmt::Display for ServiceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initializing => write!(f, "initializing"),
            Self::Running => write!(f, "running"),
            Self::Stopping => write!(f, "stopping"),
            Self::Stopped => write!(f, "stopped"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Service mode (how the service should run)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceMode {
    /// Run as a system daemon/service
    Daemon,
    /// Run ephemerally (foreground, exits when done)
    Ephemeral,
    /// Run in container mode
    Container,
}

impl ServiceMode {
    /// Detect service mode from environment
    pub fn detect() -> Self {
        // Check if running in container
        if is_container() {
            debug!("Detected container environment");
            return Self::Container;
        }

        // Check if running as systemd service
        #[cfg(target_os = "linux")]
        if std::env::var("NOTIFY_SOCKET").is_ok() || std::env::var("INVOCATION_ID").is_ok() {
            debug!("Detected systemd service");
            return Self::Daemon;
        }

        // Check if running as LaunchDaemon
        #[cfg(target_os = "macos")]
        if std::env::var("LAUNCH_DAEMON").is_ok() {
            debug!("Detected LaunchDaemon");
            return Self::Daemon;
        }

        // Default to ephemeral
        debug!("Defaulting to ephemeral mode");
        Self::Ephemeral
    }
}

/// Check if running in a container environment
fn is_container() -> bool {
    // Check for Docker
    if std::path::Path::new("/.dockerenv").exists() {
        return true;
    }

    // Check for Kubernetes
    if std::env::var("KUBERNETES_SERVICE_HOST").is_ok() {
        return true;
    }

    // Check cgroup for container indicators
    #[cfg(target_os = "linux")]
    if let Ok(contents) = std::fs::read_to_string("/proc/1/cgroup") {
        if contents.contains("docker")
            || contents.contains("kubepods")
            || contents.contains("containerd")
        {
            return true;
        }
    }

    false
}

/// Platform-specific service factory
pub fn create_service(mode: ServiceMode) -> Box<dyn Service> {
    match mode {
        #[cfg(target_os = "linux")]
        ServiceMode::Daemon | ServiceMode::Container => Box::new(SystemdService::new()),

        #[cfg(target_os = "macos")]
        ServiceMode::Daemon => Box::new(LaunchdService::new()),

        #[cfg(target_os = "windows")]
        ServiceMode::Daemon => Box::new(WindowsService::new()),

        #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
        _ => {
            Box::new(DummyService::new())
        }

        #[allow(unreachable_patterns)]
        _ => {
            info!("Using ephemeral service mode");
            Box::new(EphemeralService::new())
        }
    }
}

/// Ephemeral service (foreground process)
pub struct EphemeralService {
    running: bool,
    start_time: Option<std::time::Instant>,
}

impl EphemeralService {
    /// Create a new ephemeral service
    pub fn new() -> Self {
        Self {
            running: false,
            start_time: None,
        }
    }
}

impl Default for EphemeralService {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for EphemeralService {
    fn init(&mut self) -> Result<(), WgAgentError> {
        info!("Initializing ephemeral service");
        Ok(())
    }

    fn start(&mut self) -> Result<(), WgAgentError> {
        info!("Starting ephemeral service");
        self.running = true;
        self.start_time = Some(std::time::Instant::now());
        Ok(())
    }

    fn stop(&mut self) -> Result<(), WgAgentError> {
        info!("Stopping ephemeral service");
        self.running = false;
        Ok(())
    }

    fn reload(&mut self) -> Result<(), WgAgentError> {
        info!("Reloading ephemeral service");
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running
    }
    
    fn status(&self) -> ServiceStatus {
        let uptime = self
            .start_time
            .map(|start| std::time::Instant::now().duration_since(start));

        ServiceStatus {
            state: if self.running {
                ServiceState::Running
            } else {
                ServiceState::Stopped
            },
            uptime,
            pid: Some(std::process::id()),
            last_error: None,
        }
    }

    fn notify_ready(&self) -> Result<(), WgAgentError> {
        debug!("Ephemeral service ready (no notification needed)");
        Ok(())
    }

    fn notify_stopping(&self) -> Result<(), WgAgentError> {
        debug!("Ephemeral service stopping (no notification needed)");
        Ok(())
    }

    fn setup_signal_handlers(&mut self) -> Result<(), WgAgentError> {
        debug!("Setting up signal handlers for ephemeral service");
        Ok(())
    }
}

/// Dummy service for unsupported platforms
pub struct DummyService;

impl DummyService {
    /// Create a new dummy service
    pub fn new() -> Self {
        Self
    }
}

impl Default for DummyService {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for DummyService {
    fn init(&mut self) -> Result<(), WgAgentError> {
        Err(WgAgentError::Platform(
            "Service not supported on this platform".to_string(),
        ))
    }

    fn start(&mut self) -> Result<(), WgAgentError> {
        Err(WgAgentError::Platform(
            "Service not supported on this platform".to_string(),
        ))
    }

    fn stop(&mut self) -> Result<(), WgAgentError> {
        Ok(())
    }

    fn reload(&mut self) -> Result<(), WgAgentError> {
        Ok(())
    }

    fn is_running(&self) -> bool {
        false
    }

    fn status(&self) -> ServiceStatus {
        ServiceStatus {
            state: ServiceState::Failed,
            uptime: None,
            pid: None,
            last_error: Some("Unsupported platform".to_string()),
        }
    }

    fn notify_ready(&self) -> Result<(), WgAgentError> {
        Ok(())
    }

    fn notify_stopping(&self) -> Result<(), WgAgentError> {
        Ok(())
    }

    fn setup_signal_handlers(&mut self) -> Result<(), WgAgentError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_mode_detect() {
        let mode = ServiceMode::detect();
        // Should return a valid mode
        assert!(matches!(
            mode,
            ServiceMode::Daemon | ServiceMode::Ephemeral | ServiceMode::Container
        ));
    }

    #[test]
    fn test_ephemeral_service() {
        let mut service = EphemeralService::new();
        assert!(!service.is_running());

        service.init().unwrap();
        service.start().unwrap();
        assert!(service.is_running());

        let status = service.status();
        assert_eq!(status.state, ServiceState::Running);
        assert!(status.pid.is_some());

        service.stop().unwrap();
        assert!(!service.is_running());
    }

    #[test]
    fn test_service_state_display() {
        assert_eq!(ServiceState::Running.to_string(), "running");
        assert_eq!(ServiceState::Stopped.to_string(), "stopped");
        assert_eq!(ServiceState::Failed.to_string(), "failed");
    }
}
