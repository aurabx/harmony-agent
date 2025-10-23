//! Control server for Unix sockets (Linux/macOS) and Named Pipes (Windows)
//!
//! This module implements the server that listens for incoming control
//! connections and dispatches commands to the handler.

use crate::control::{ApiError, ApiRequest, ApiResponse, CommandHandler};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tracing::{debug, error, info};

/// Default socket path for Unix systems
#[cfg(unix)]
pub const DEFAULT_SOCKET_PATH: &str = "/var/run/wg-agent.sock";

/// Default pipe name for Windows
#[cfg(windows)]
pub const DEFAULT_PIPE_NAME: &str = r"\\.\pipe\wg-agent";

/// Control server manages the control API socket/pipe
pub struct ControlServer {
    /// Path to Unix socket or Named Pipe
    socket_path: PathBuf,
    /// Command handler
    handler: Arc<CommandHandler>,
}

impl ControlServer {
    /// Create a new control server
    pub fn new(socket_path: PathBuf, handler: Arc<CommandHandler>) -> Self {
        Self {
            socket_path,
            handler,
        }
    }

    /// Start the control server
    #[cfg(unix)]
    pub async fn start(&self) -> Result<(), ApiError> {
        info!("Starting control server at {:?}", self.socket_path);

        // Remove existing socket if present
        if self.socket_path.exists() {
            info!("Removing existing socket at {:?}", self.socket_path);
            std::fs::remove_file(&self.socket_path).map_err(|e| {
                ApiError::InternalError(format!("Failed to remove existing socket: {}", e))
            })?;
        }

        // Create parent directory if needed
        if let Some(parent) = self.socket_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ApiError::InternalError(format!("Failed to create socket directory: {}", e))
            })?;
        }

        // Bind Unix socket
        let listener = UnixListener::bind(&self.socket_path).map_err(|e| {
            ApiError::InternalError(format!("Failed to bind Unix socket: {}", e))
        })?;

        info!("Control server listening at {:?}", self.socket_path);

        // Accept connections
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let handler = self.handler.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, handler).await {
                            error!("Connection handler error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    /// Start the control server (Windows)
    #[cfg(windows)]
    pub async fn start(&self) -> Result<(), ApiError> {
        Err(ApiError::InternalError(
            "Windows server not yet implemented".to_string(),
        ))
    }

    /// Shutdown the server and clean up
    #[cfg(unix)]
    pub async fn shutdown(&self) -> Result<(), ApiError> {
        info!("Shutting down control server");

        // Remove socket file
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path).map_err(|e| {
                ApiError::InternalError(format!("Failed to remove socket: {}", e))
            })?;
        }

        Ok(())
    }

    /// Shutdown (Windows stub)
    #[cfg(windows)]
    pub async fn shutdown(&self) -> Result<(), ApiError> {
        info!("Shutting down control server");
        Ok(())
    }
}

/// Handle a single client connection
#[cfg(unix)]
async fn handle_connection(
    stream: tokio::net::UnixStream,
    handler: Arc<CommandHandler>,
) -> Result<(), ApiError> {
    debug!("New client connection");

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();

        match reader.read_line(&mut line).await {
            Ok(0) => {
                debug!("Client disconnected");
                break;
            }
            Ok(_) => {
                let request_str = line.trim();
                if request_str.is_empty() {
                    continue;
                }

                debug!("Received request: {}", request_str);

                // Parse request
                let response = match ApiRequest::from_json(request_str) {
                    Ok(request) => {
                        // Handle request
                        handler.handle_request(request).await
                    }
                    Err(e) => {
                        error!("Failed to parse request: {}", e);
                        ApiResponse::error(
                            "unknown".to_string(),
                            ApiError::ParseError(format!("Invalid JSON: {}", e)),
                        )
                    }
                };

                // Send response
                let response_str = response.to_json().map_err(|e| {
                    ApiError::InternalError(format!("Failed to serialize response: {}", e))
                })?;

                writer
                    .write_all(response_str.as_bytes())
                    .await
                    .map_err(|e| {
                        ApiError::InternalError(format!("Failed to write response: {}", e))
                    })?;

                writer.write_all(b"\n").await.map_err(|e| {
                    ApiError::InternalError(format!("Failed to write newline: {}", e))
                })?;

                writer.flush().await.map_err(|e| {
                    ApiError::InternalError(format!("Failed to flush response: {}", e))
                })?;
            }
            Err(e) => {
                error!("Failed to read from socket: {}", e);
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_server_creation() {
        let tmp_dir = TempDir::new().unwrap();
        let socket_path = tmp_dir.path().join("test.sock");
        let handler = Arc::new(CommandHandler::new());

        let server = ControlServer::new(socket_path.clone(), handler);
        assert_eq!(server.socket_path, socket_path);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_server_shutdown() {
        let tmp_dir = TempDir::new().unwrap();
        let socket_path = tmp_dir.path().join("test.sock");
        let handler = Arc::new(CommandHandler::new());

        let server = ControlServer::new(socket_path.clone(), handler);

        // Create a dummy socket file
        std::fs::write(&socket_path, "").unwrap();
        assert!(socket_path.exists());

        // Shutdown should remove it
        server.shutdown().await.unwrap();
        assert!(!socket_path.exists());
    }
}
