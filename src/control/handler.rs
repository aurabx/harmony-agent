//! Command handler for control API
//!
//! This module handles execution of API commands by dispatching them
//! to the appropriate tunnel operations.

use crate::config::{Config, ControlAction, NetworkConfig};
use crate::control::{ApiError, ApiRequest, ApiResponse};
use crate::wireguard::{Tunnel, TunnelConfig};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Command handler manages tunnels and executes API commands
pub struct CommandHandler {
    /// Active tunnels by network name
    tunnels: Arc<RwLock<HashMap<String, Arc<Tunnel>>>>,
    /// Agent configuration
    config: Arc<RwLock<Option<Config>>>,
}

impl CommandHandler {
    /// Create a new command handler
    pub fn new() -> Self {
        Self {
            tunnels: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(None)),
        }
    }

    /// Load configuration
    pub async fn load_config(&self, config: Config) {
        let mut cfg = self.config.write().await;
        *cfg = Some(config);
        info!("Configuration loaded");
    }

    /// Handle an API request
    pub async fn handle_request(&self, request: ApiRequest) -> ApiResponse {
        debug!(
            "Handling request {}: {:?} for network '{}'",
            request.id, request.action, request.network
        );

        let result = match request.action {
            ControlAction::Connect => self.handle_connect(&request).await,
            ControlAction::Disconnect => self.handle_disconnect(&request).await,
            ControlAction::Status => self.handle_status(&request).await,
            ControlAction::Reload => self.handle_reload(&request).await,
            ControlAction::RotateKeys => self.handle_rotate_keys(&request).await,
        };

        match result {
            Ok(data) => {
                info!(
                    "Request {} completed successfully: {:?}",
                    request.id, request.action
                );
                ApiResponse::success(request.id, data)
            }
            Err(e) => {
                error!("Request {} failed: {}", request.id, e);
                ApiResponse::error(request.id, e)
            }
        }
    }

    /// Handle connect action
    async fn handle_connect(
        &self,
        request: &ApiRequest,
    ) -> Result<Option<serde_json::Value>, ApiError> {
        info!("Connecting network: {}", request.network);

        // Check if already connected
        {
            let tunnels = self.tunnels.read().await;
            if let Some(tunnel) = tunnels.get(&request.network) {
                let state = tunnel.state().await;
                if state.is_running() {
                    return Err(ApiError::InvalidState(format!(
                        "Network '{}' is already connected (state: {})",
                        request.network, state
                    )));
                }
            }
        }

        // Get network configuration
        let network_config = self.get_network_config(&request.network).await?;

        // Create tunnel
        let tunnel = Tunnel::from_network_config(&network_config).map_err(ApiError::from)?;
        let tunnel = Arc::new(tunnel);

        // Start tunnel
        tunnel.start().await.map_err(ApiError::from)?;

        // Store tunnel
        let mut tunnels = self.tunnels.write().await;
        tunnels.insert(request.network.clone(), tunnel.clone());

        // Get stats
        let stats = tunnel.stats().await;

        Ok(Some(serde_json::json!({
            "network": request.network,
            "state": stats.state.to_string(),
            "interface": stats.interface,
            "peers": stats.total_peers,
        })))
    }

    /// Handle disconnect action
    async fn handle_disconnect(
        &self,
        request: &ApiRequest,
    ) -> Result<Option<serde_json::Value>, ApiError> {
        info!("Disconnecting network: {}", request.network);

        // Get tunnel
        let tunnel = {
            let tunnels = self.tunnels.read().await;
            tunnels
                .get(&request.network)
                .cloned()
                .ok_or_else(|| ApiError::NetworkNotFound(request.network.clone()))?
        };

        // Stop tunnel
        tunnel.stop().await.map_err(ApiError::from)?;

        // Remove from active tunnels
        let mut tunnels = self.tunnels.write().await;
        tunnels.remove(&request.network);

        Ok(Some(serde_json::json!({
            "network": request.network,
            "state": "stopped",
        })))
    }

    /// Handle status action
    async fn handle_status(
        &self,
        request: &ApiRequest,
    ) -> Result<Option<serde_json::Value>, ApiError> {
        debug!("Getting status for network: {}", request.network);

        // Get tunnel
        let tunnel = {
            let tunnels = self.tunnels.read().await;
            tunnels
                .get(&request.network)
                .cloned()
                .ok_or_else(|| ApiError::NetworkNotFound(request.network.clone()))?
        };

        // Get stats
        let stats = tunnel.stats().await;
        let peer_names = tunnel.peer_names().await;

        Ok(Some(serde_json::json!({
            "network": request.network,
            "state": stats.state.to_string(),
            "interface": stats.interface,
            "peers": {
                "total": stats.total_peers,
                "active": stats.active_peers,
                "healthy": stats.healthy_peers,
                "names": peer_names,
            },
            "traffic": {
                "tx_bytes": stats.total_tx_bytes,
                "rx_bytes": stats.total_rx_bytes,
            },
        })))
    }

    /// Handle reload action
    async fn handle_reload(
        &self,
        request: &ApiRequest,
    ) -> Result<Option<serde_json::Value>, ApiError> {
        info!("Reloading network: {}", request.network);

        // Get tunnel
        let tunnel = {
            let tunnels = self.tunnels.read().await;
            tunnels
                .get(&request.network)
                .cloned()
                .ok_or_else(|| ApiError::NetworkNotFound(request.network.clone()))?
        };

        // Get new configuration
        let network_config = self.get_network_config(&request.network).await?;
        let new_config = TunnelConfig::from_network_config(&network_config).map_err(ApiError::from)?;

        // Reload tunnel
        tunnel.reload(new_config).await.map_err(ApiError::from)?;

        let stats = tunnel.stats().await;

        Ok(Some(serde_json::json!({
            "network": request.network,
            "state": stats.state.to_string(),
            "reloaded": true,
        })))
    }

    /// Handle rotate_keys action
    async fn handle_rotate_keys(
        &self,
        _request: &ApiRequest,
    ) -> Result<Option<serde_json::Value>, ApiError> {
        warn!("Key rotation not yet implemented");
        Err(ApiError::InternalError(
            "Key rotation not yet implemented".to_string(),
        ))
    }

    /// Get network configuration
    async fn get_network_config(&self, network: &str) -> Result<NetworkConfig, ApiError> {
        let config = self.config.read().await;
        let config = config
            .as_ref()
            .ok_or_else(|| ApiError::ConfigError("No configuration loaded".to_string()))?;

        config
            .get_network(network)
            .cloned()
            .ok_or_else(|| ApiError::NetworkNotFound(network.to_string()))
    }

    /// List all networks
    pub async fn list_networks(&self) -> Vec<String> {
        let tunnels = self.tunnels.read().await;
        tunnels.keys().cloned().collect()
    }

    /// Get all tunnel states
    pub async fn get_all_states(&self) -> HashMap<String, String> {
        let tunnels = self.tunnels.read().await;
        let mut states = HashMap::new();

        for (name, tunnel) in tunnels.iter() {
            let state = tunnel.state().await;
            states.insert(name.clone(), state.to_string());
        }

        states
    }
}

impl Default for CommandHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ControlAction;

    #[tokio::test]
    async fn test_handler_creation() {
        let handler = CommandHandler::new();
        let networks = handler.list_networks().await;
        assert!(networks.is_empty());
    }

    #[tokio::test]
    async fn test_handler_disconnect_not_found() {
        let handler = CommandHandler::new();
        let request = ApiRequest::new(
            "test-1".to_string(),
            ControlAction::Disconnect,
            "nonexistent".to_string(),
        );

        let response = handler.handle_request(request).await;
        assert!(!response.success);
        assert!(response.error.is_some());
    }

    #[tokio::test]
    async fn test_handler_status_not_found() {
        let handler = CommandHandler::new();
        let request = ApiRequest::new(
            "test-1".to_string(),
            ControlAction::Status,
            "nonexistent".to_string(),
        );

        let response = handler.handle_request(request).await;
        assert!(!response.success);
    }
}
