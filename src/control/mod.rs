//! Control API for external applications
//!
//! This module provides the control interface for receiving commands from
//! main applications via Unix sockets (Linux/macOS) or Named Pipes (Windows).

mod api;
mod handler;
mod server;

pub use api::{ApiRequest, ApiResponse, ApiError};
pub use handler::CommandHandler;
pub use server::{ControlServer, DEFAULT_SOCKET_PATH};

#[cfg(windows)]
pub use server::DEFAULT_PIPE_NAME;
