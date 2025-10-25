//! Integration tests for the control server
//!
//! These tests verify that the control server starts, accepts connections,
//! and responds to API requests correctly.

use std::os::unix::net::UnixStream;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::time::Duration;
use serde_json::json;

/// Test that we can connect to the control server socket
#[test]
#[ignore] // Requires running server
fn test_control_server_socket_exists() {
    let socket_path = PathBuf::from("/var/run/wg-agent.sock");
    
    // Check if socket file exists
    assert!(
        socket_path.exists(),
        "Control socket does not exist at {:?}. Is wg-agent running?",
        socket_path
    );
    
    // Verify it's a socket
    assert!(
        socket_path.is_socket(),
        "File at {:?} is not a socket",
        socket_path
    );
}

/// Test that we can connect to the control server
#[test]
#[ignore] // Requires running server
fn test_control_server_connection() {
    let socket_path = "/var/run/wg-agent.sock";
    
    let result = UnixStream::connect(socket_path);
    assert!(
        result.is_ok(),
        "Failed to connect to control server: {:?}",
        result.err()
    );
}

/// Test that the control server responds to status requests
#[test]
#[ignore] // Requires running server
fn test_control_server_status_request() {
    let socket_path = "/var/run/wg-agent.sock";
    
    // Connect to server
    let mut stream = UnixStream::connect(socket_path)
        .expect("Failed to connect to control server");
    
    // Set timeout
    stream.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("Failed to set read timeout");
    
    // Send status request
    let request = json!({
        "id": "test-1",
        "action": "status",
        "network": "default"
    });
    
    let request_str = serde_json::to_string(&request)
        .expect("Failed to serialize request");
    
    stream.write_all(request_str.as_bytes())
        .expect("Failed to write request");
    stream.write_all(b"\n")
        .expect("Failed to write newline");
    stream.flush()
        .expect("Failed to flush stream");
    
    // Read response
    let mut reader = BufReader::new(&stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)
        .expect("Failed to read response");
    
    // Parse response
    let response: serde_json::Value = serde_json::from_str(&response_line)
        .expect("Response is not valid JSON");
    
    // Verify response structure
    assert!(response.get("id").is_some(), "Response missing 'id' field");
    assert!(response.get("success").is_some(), "Response missing 'success' field");
    
    println!("Response: {}", serde_json::to_string_pretty(&response).unwrap());
}

/// Test that the control server returns proper error for network not found
#[test]
#[ignore] // Requires running server
fn test_control_server_network_not_found() {
    let socket_path = "/var/run/wg-agent.sock";
    
    // Connect to server
    let mut stream = UnixStream::connect(socket_path)
        .expect("Failed to connect to control server");
    
    stream.set_read_timeout(Some(Duration::from_secs(5)))
        .expect("Failed to set read timeout");
    
    // Send status request for non-existent network
    let request = json!({
        "id": "test-2",
        "action": "status",
        "network": "nonexistent-network-xyz"
    });
    
    let request_str = serde_json::to_string(&request)
        .expect("Failed to serialize request");
    
    stream.write_all(request_str.as_bytes())
        .expect("Failed to write request");
    stream.write_all(b"\n")
        .expect("Failed to write newline");
    stream.flush()
        .expect("Failed to flush stream");
    
    // Read response
    let mut reader = BufReader::new(&stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)
        .expect("Failed to read response");
    
    // Parse response
    let response: serde_json::Value = serde_json::from_str(&response_line)
        .expect("Response is not valid JSON");
    
    // Verify error response
    assert_eq!(response["success"], false, "Expected success=false for non-existent network");
    assert!(response.get("error").is_some(), "Response missing 'error' field");
    
    let error = &response["error"];
    assert!(error.get("type").is_some(), "Error missing 'type' field");
    assert!(error.get("message").is_some(), "Error missing 'message' field");
    
    println!("Error response: {}", serde_json::to_string_pretty(&response).unwrap());
}

/// Helper trait to check if a path is a socket
trait PathExt {
    fn is_socket(&self) -> bool;
}

impl PathExt for PathBuf {
    fn is_socket(&self) -> bool {
        use std::os::unix::fs::FileTypeExt;
        self.metadata()
            .map(|m| m.file_type().is_socket())
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod helper_tests {
    use super::*;
    
    /// Test helper to verify socket detection works
    #[test]
    fn test_socket_detection_helper() {
        // This should be a regular file, not a socket
        let regular_file = PathBuf::from("/etc/hosts");
        if regular_file.exists() {
            assert!(!regular_file.is_socket(), "/etc/hosts should not be detected as socket");
        }
    }
}
