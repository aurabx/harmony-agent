//! Error types for wg-agent
//!
//! This module defines the error types used throughout the application.
//! We use `thiserror` for ergonomic error definitions and `anyhow` for
//! error propagation in application code.

use thiserror::Error;

/// Main error type for wg-agent operations
#[derive(Error, Debug)]
pub enum WgAgentError {
    /// Configuration-related errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// Platform-specific errors (TUN/TAP device, networking)
    #[error("Platform error: {0}")]
    Platform(String),

    /// WireGuard protocol errors
    #[error("WireGuard error: {0}")]
    WireGuard(String),

    /// TUN device errors
    #[error("TUN device error: {0}")]
    TunDevice(String),

    /// Packet processing errors
    #[error("Packet processing error: {0}")]
    PacketProcessing(String),

    /// Handshake errors
    #[error("Handshake error: {0}")]
    Handshake(String),

    /// Control API errors
    #[error("Control API error: {0}")]
    ControlApi(String),

    /// Service/daemon errors
    #[error("Service error: {0}")]
    Service(String),

    /// I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization/deserialization errors
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Permission errors
    #[error("Permission denied: {0}")]
    Permission(String),

    /// Not found errors
    #[error("Not found: {0}")]
    NotFound(String),

    /// Invalid state errors
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Timeout errors
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Security-related errors
    #[error("Security error: {0}")]
    Security(String),

    /// Input validation errors
    #[error("Validation error: {0}")]
    Validation(String),
}

/// Result type alias using WgAgentError
pub type Result<T> = std::result::Result<T, WgAgentError>;

impl From<serde_json::Error> for WgAgentError {
    fn from(err: serde_json::Error) -> Self {
        WgAgentError::Serialization(err.to_string())
    }
}

impl From<toml::de::Error> for WgAgentError {
    fn from(err: toml::de::Error) -> Self {
        WgAgentError::Config(err.to_string())
    }
}
