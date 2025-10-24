//! harmony-agent: Cross-platform WireGuard network agent
//!
//! This library provides a portable WireGuard tunnel management system that works
//! across Linux, Windows, macOS, Docker, and Kubernetes environments.
//!
//! # Architecture
//!
//! The agent is designed as a standalone daemon that manages WireGuard tunnels on
//! behalf of other applications (Aurabox, JMIX, Runbeam). It isolates privileged
//! network operations from the main application while providing a consistent API.
//!
//! # Modules
//!
//! - `config`: Configuration parsing and management
//! - `platform`: Platform-specific implementations (Linux, Windows, macOS)
//! - `wireguard`: WireGuard protocol and tunnel management
//! - `control`: Control API for external applications
//! - `service`: Service/daemon integration
//! - `security`: Security hardening and privilege management
//! - `monitoring`: Health checks and metrics
//! - `error`: Error types and handling

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod config;
pub mod control;
pub mod error;
pub mod monitoring;
pub mod platform;
pub mod security;
pub mod service;
pub mod wireguard;

// Re-export commonly used types
pub use error::{Result, WgAgentError};

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Application name
pub const APP_NAME: &str = env!("CARGO_PKG_NAME");
