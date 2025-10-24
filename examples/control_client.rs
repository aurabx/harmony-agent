//! Example control API client
//!
//! This demonstrates how to interact with the harmony-agent control API
//! from an external application (like Harmony).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to control socket
    let socket_path = PathBuf::from("/var/run/harmony-agent.sock");
    println!("Connecting to {:?}", socket_path);
    
    let mut stream = UnixStream::connect(&socket_path)?;
    println!("Connected!");

    // Example 1: Get status
    println!("\n--- Getting status ---");
    let status_request = r#"{"id":"req-1","action":"status","network":"default"}"#;
    writeln!(stream, "{}", status_request)?;
    
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut response = String::new();
    reader.read_line(&mut response)?;
    println!("Response: {}", response);

    // Example 2: Connect network
    println!("\n--- Connecting network ---");
    let connect_request = r#"{"id":"req-2","action":"connect","network":"default"}"#;
    writeln!(stream, "{}", connect_request)?;
    
    response.clear();
    reader.read_line(&mut response)?;
    println!("Response: {}", response);

    // Example 3: Get status again
    println!("\n--- Getting status after connect ---");
    let status_request = r#"{"id":"req-3","action":"status","network":"default"}"#;
    writeln!(stream, "{}", status_request)?;
    
    response.clear();
    reader.read_line(&mut response)?;
    println!("Response: {}", response);

    // Example 4: Disconnect network
    println!("\n--- Disconnecting network ---");
    let disconnect_request = r#"{"id":"req-4","action":"disconnect","network":"default"}"#;
    writeln!(stream, "{}", disconnect_request)?;
    
    response.clear();
    reader.read_line(&mut response)?;
    println!("Response: {}", response);

    Ok(())
}
