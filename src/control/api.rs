//! Control API request and response types
//!
//! This module defines the JSON-RPC style API for controlling the WireGuard agent.

use crate::config::ControlAction;
use serde::{Deserialize, Serialize};

/// API request from client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiRequest {
    /// Request ID for tracking
    #[serde(default = "default_request_id")]
    pub id: String,
    
    /// Action to perform
    pub action: ControlAction,
    
    /// Network name to operate on
    #[serde(default = "default_network")]
    pub network: String,
    
    /// Optional configuration data (for connect/reload actions)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}

impl ApiRequest {
    /// Create a new API request
    pub fn new(id: String, action: ControlAction, network: String) -> Self {
        Self {
            id,
            action,
            network,
            config: None,
        }
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, ApiError> {
        serde_json::from_str(json).map_err(|e| ApiError::ParseError(e.to_string()))
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, ApiError> {
        serde_json::to_string(self).map_err(|e| ApiError::SerializationError(e.to_string()))
    }
}

/// API response to client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse {
    /// Request ID this response corresponds to
    pub id: String,
    
    /// Whether the request was successful
    pub success: bool,
    
    /// Optional result data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    
    /// Optional error information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ApiError>,
}

impl ApiResponse {
    /// Create a successful response
    pub fn success(id: String, data: Option<serde_json::Value>) -> Self {
        Self {
            id,
            success: true,
            data,
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: String, error: ApiError) -> Self {
        Self {
            id,
            success: false,
            data: None,
            error: Some(error),
        }
    }

    /// Convert to JSON string
    pub fn to_json(&self) -> Result<String, ApiError> {
        serde_json::to_string(self).map_err(|e| ApiError::SerializationError(e.to_string()))
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, ApiError> {
        serde_json::from_str(json).map_err(|e| ApiError::ParseError(e.to_string()))
    }
}

/// API error types
#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "type", content = "message")]
pub enum ApiError {
    /// Failed to parse request
    #[error("Parse error: {0}")]
    ParseError(String),
    
    /// Failed to serialize response
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    /// Invalid action for current state
    #[error("Invalid state: {0}")]
    InvalidState(String),
    
    /// Network not found
    #[error("Network not found: {0}")]
    NetworkNotFound(String),
    
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    /// Platform error
    #[error("Platform error: {0}")]
    PlatformError(String),
    
    /// Internal server error
    #[error("Internal error: {0}")]
    InternalError(String),
    
    /// Authentication failed
    #[error("Authentication failed")]
    AuthenticationFailed,
    
    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

impl From<crate::error::WgAgentError> for ApiError {
    fn from(err: crate::error::WgAgentError) -> Self {
        use crate::error::WgAgentError;
        match err {
            WgAgentError::Config(msg) => ApiError::ConfigError(msg),
            WgAgentError::Platform(msg) => ApiError::PlatformError(msg),
            WgAgentError::InvalidState(msg) => ApiError::InvalidState(msg),
            WgAgentError::NotFound(msg) => ApiError::NetworkNotFound(msg),
            WgAgentError::Permission(msg) => ApiError::PermissionDenied(msg),
            WgAgentError::Serialization(msg) => ApiError::SerializationError(msg),
            _ => ApiError::InternalError(err.to_string()),
        }
    }
}

fn default_request_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    format!("req-{}", COUNTER.fetch_add(1, Ordering::SeqCst))
}

fn default_network() -> String {
    "default".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_request_new() {
        let req = ApiRequest::new(
            "test-1".to_string(),
            ControlAction::Status,
            "default".to_string(),
        );
        assert_eq!(req.id, "test-1");
        assert_eq!(req.action, ControlAction::Status);
        assert_eq!(req.network, "default");
    }

    #[test]
    fn test_api_request_json() {
        let req = ApiRequest::new(
            "test-1".to_string(),
            ControlAction::Connect,
            "default".to_string(),
        );
        
        let json = req.to_json().unwrap();
        let parsed = ApiRequest::from_json(&json).unwrap();
        
        assert_eq!(req.id, parsed.id);
        assert_eq!(req.action, parsed.action);
    }

    #[test]
    fn test_api_response_success() {
        let resp = ApiResponse::success(
            "test-1".to_string(),
            Some(serde_json::json!({"status": "ok"})),
        );
        
        assert!(resp.success);
        assert!(resp.data.is_some());
        assert!(resp.error.is_none());
    }

    #[test]
    fn test_api_response_error() {
        let resp = ApiResponse::error(
            "test-1".to_string(),
            ApiError::NetworkNotFound("test".to_string()),
        );
        
        assert!(!resp.success);
        assert!(resp.data.is_none());
        assert!(resp.error.is_some());
    }

    #[test]
    fn test_api_response_json() {
        let resp = ApiResponse::success(
            "test-1".to_string(),
            Some(serde_json::json!({"status": "active"})),
        );
        
        let json = resp.to_json().unwrap();
        let parsed = ApiResponse::from_json(&json).unwrap();
        
        assert_eq!(resp.id, parsed.id);
        assert_eq!(resp.success, parsed.success);
    }

    #[test]
    fn test_api_error_conversion() {
        let wg_error = crate::error::WgAgentError::Config("test error".to_string());
        let api_error: ApiError = wg_error.into();
        
        match api_error {
            ApiError::ConfigError(msg) => assert_eq!(msg, "test error"),
            _ => panic!("Wrong error type"),
        }
    }
}
