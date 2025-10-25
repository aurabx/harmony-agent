//! Self-contained integration tests for the control server
//!
//! These tests start their own instance of the control server and test it,
//! so they don't require a separately running server.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

/// Test that the control server can be started and accepts connections
#[tokio::test]
async fn test_control_server_lifecycle() {
    use harmony_agent::control::{CommandHandler, ControlServer};
    use std::sync::Arc;

    // Create temporary directory for socket
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let socket_path = temp_dir.path().join("test.sock");

    // Create command handler
    let handler = Arc::new(CommandHandler::new());

    // Create control server
    let server = Arc::new(ControlServer::new(socket_path.clone(), handler));

    // Start server in background task
    let server_clone = server.clone();
    let server_task = tokio::spawn(async move {
        server_clone.start().await
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify socket was created
    assert!(
        socket_path.exists(),
        "Control server did not create socket at {:?}",
        socket_path
    );

    // Test connection
    let stream = UnixStream::connect(&socket_path)
        .expect("Failed to connect to control server");

    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("Failed to set timeout");

    drop(stream);

    // Shutdown server
    server
        .shutdown()
        .await
        .expect("Failed to shutdown server");

    // Abort the server task
    server_task.abort();

    // Verify socket was cleaned up
    assert!(
        !socket_path.exists(),
        "Control server did not cleanup socket"
    );
}

/// Test that the control server responds to status requests
#[tokio::test]
async fn test_control_server_status_response() {
    use harmony_agent::config::Config;
    use harmony_agent::control::{CommandHandler, ControlServer};
    use std::sync::Arc;

    // Create temporary directory for socket
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let socket_path = temp_dir.path().join("test.sock");

    // Create command handler with empty config
    let handler = Arc::new(CommandHandler::new());
    let config = Config::new();
    handler.load_config(config).await;

    // Create and start control server
    let server = Arc::new(ControlServer::new(socket_path.clone(), handler));
    let server_clone = server.clone();
    let server_task = tokio::spawn(async move {
        let _ = server_clone.start().await;
    });

    // Give server time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Connect and send request
    let result = timeout(Duration::from_secs(2), async {
        let mut stream = UnixStream::connect(&socket_path)?;

        // Send status request
        let request = r#"{"id":"test-1","action":"status","network":"default"}"#;
        stream.write_all(request.as_bytes())?;
        stream.write_all(b"\n")?;
        stream.flush()?;

        // Read response
        let mut reader = BufReader::new(&stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line)?;

        // Parse response
        let response: serde_json::Value = serde_json::from_str(&response_line)?;

        Ok::<serde_json::Value, std::io::Error>(response)
    })
    .await;

    // Cleanup
    server.shutdown().await.ok();
    server_task.abort();

    // Verify response
    let response = result
        .expect("Request timeout")
        .expect("Failed to get response");

    assert!(response.get("id").is_some(), "Response missing id");
    assert!(response.get("success").is_some(), "Response missing success");

    // Should fail with network_not_found since we have no networks configured
    assert_eq!(response["success"], false);
    assert!(response.get("error").is_some(), "Response missing error");
}

/// Test that multiple sequential connections work
#[tokio::test]
async fn test_control_server_multiple_connections() {
    use harmony_agent::config::Config;
    use harmony_agent::control::{CommandHandler, ControlServer};
    use std::sync::Arc;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let socket_path = temp_dir.path().join("test.sock");

    let handler = Arc::new(CommandHandler::new());
    handler.load_config(Config::new()).await;

    let server = Arc::new(ControlServer::new(socket_path.clone(), handler));
    let server_clone = server.clone();
    let server_task = tokio::spawn(async move {
        let _ = server_clone.start().await;
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test multiple separate connections
    for i in 1..=3 {
        let mut stream = UnixStream::connect(&socket_path)
            .expect("Failed to connect");
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .expect("Failed to set timeout");

        let request = format!(
            r#"{{"id":"test-{}","action":"status","network":"default"}}"#,
            i
        );
        stream.write_all(request.as_bytes()).expect("Write failed");
        stream.write_all(b"\n").expect("Write newline failed");
        stream.flush().expect("Flush failed");

        let mut reader = BufReader::new(&stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).expect("Read failed");

        let response: serde_json::Value =
            serde_json::from_str(&response_line).expect("Parse failed");

        assert_eq!(
            response["id"],
            format!("test-{}", i),
            "Response id mismatch for connection {}",
            i
        );
    }

    server.shutdown().await.ok();
    server_task.abort();
}

/// Test that invalid JSON is handled gracefully
#[tokio::test]
async fn test_control_server_invalid_json() {
    use harmony_agent::config::Config;
    use harmony_agent::control::{CommandHandler, ControlServer};
    use std::sync::Arc;

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let socket_path = temp_dir.path().join("test.sock");

    let handler = Arc::new(CommandHandler::new());
    handler.load_config(Config::new()).await;

    let server = Arc::new(ControlServer::new(socket_path.clone(), handler));
    let server_clone = server.clone();
    let server_task = tokio::spawn(async move {
        let _ = server_clone.start().await;
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut stream = UnixStream::connect(&socket_path).expect("Failed to connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("Failed to set timeout");

    // Send invalid JSON
    stream
        .write_all(b"this is not json\n")
        .expect("Write failed");
    stream.flush().expect("Flush failed");

    // Read error response
    let mut reader = BufReader::new(&stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line).expect("Read failed");

    let response: serde_json::Value =
        serde_json::from_str(&response_line).expect("Parse failed");

    // Should get parse error
    assert_eq!(response["success"], false);
    assert!(response.get("error").is_some());

    server.shutdown().await.ok();
    server_task.abort();
}
