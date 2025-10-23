//! Systemd service integration for Linux
//!
//! This module provides integration with systemd service manager,
//! including SD_NOTIFY support for service readiness.

use super::{Service, ServiceState, ServiceStatus};
use crate::error::WgAgentError;
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Systemd service implementation
pub struct SystemdService {
    running: bool,
    start_time: Option<Instant>,
    notify_socket: Option<String>,
}

impl SystemdService {
    /// Create a new systemd service
    pub fn new() -> Self {
        let notify_socket = std::env::var("NOTIFY_SOCKET").ok();
        
        if notify_socket.is_some() {
            debug!("Systemd NOTIFY_SOCKET detected");
        }

        Self {
            running: false,
            start_time: None,
            notify_socket,
        }
    }

    /// Send notification to systemd
    fn sd_notify(&self, state: &str) -> Result<(), WgAgentError> {
        if let Some(ref socket_path) = self.notify_socket {
            debug!("Sending systemd notification: {}", state);
            
            // In production, would use libsystemd or sd-notify crate
            // For now, we'll use a simple implementation
            match std::os::unix::net::UnixDatagram::unbound() {
                Ok(socket) => {
                    if let Err(e) = socket.send_to(state.as_bytes(), socket_path) {
                        warn!("Failed to send systemd notification: {}", e);
                    }
                }
                Err(e) => {
                    warn!("Failed to create Unix datagram socket: {}", e);
                }
            }
        }
        Ok(())
    }
}

impl Default for SystemdService {
    fn default() -> Self {
        Self::new()
    }
}

impl Service for SystemdService {
    fn init(&mut self) -> Result<(), WgAgentError> {
        info!("Initializing systemd service");
        self.sd_notify("STATUS=Initializing")?;
        Ok(())
    }

    fn start(&mut self) -> Result<(), WgAgentError> {
        info!("Starting systemd service");
        self.running = true;
        self.start_time = Some(Instant::now());
        self.sd_notify("STATUS=Starting")?;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), WgAgentError> {
        info!("Stopping systemd service");
        self.sd_notify("STOPPING=1")?;
        self.running = false;
        Ok(())
    }

    fn reload(&mut self) -> Result<(), WgAgentError> {
        info!("Reloading systemd service");
        self.sd_notify("RELOADING=1")?;
        // Reload logic would go here
        self.sd_notify("READY=1")?;
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.running
    }

    fn status(&self) -> ServiceStatus {
        let uptime = self
            .start_time
            .map(|start| Instant::now().duration_since(start));

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
        info!("Notifying systemd of readiness");
        self.sd_notify("READY=1\nSTATUS=Running")?;
        Ok(())
    }

    fn notify_stopping(&self) -> Result<(), WgAgentError> {
        info!("Notifying systemd of stopping");
        self.sd_notify("STOPPING=1")?;
        Ok(())
    }

    fn setup_signal_handlers(&mut self) -> Result<(), WgAgentError> {
        info!("Setting up signal handlers for systemd service");
        // Signal handling would be implemented here
        // Typically SIGTERM for graceful shutdown, SIGHUP for reload
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_systemd_service_creation() {
        let service = SystemdService::new();
        assert!(!service.is_running());
    }

    #[test]
    fn test_systemd_service_lifecycle() {
        let mut service = SystemdService::new();
        
        service.init().unwrap();
        service.start().unwrap();
        assert!(service.is_running());

        let status = service.status();
        assert_eq!(status.state, ServiceState::Running);

        service.stop().unwrap();
        assert!(!service.is_running());
    }
}
