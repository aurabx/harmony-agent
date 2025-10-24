# wg-agent API Documentation

This document describes the APIs available for integrating wg-agent into other applications (Aurabox, JMIX, Harmony, etc.).

## Overview

wg-agent provides three ways to interact with it:

1. **Control API** - JSON-based API over Unix sockets (Linux/macOS) or Named Pipes (Windows)
2. **HTTP API** - Metrics and health check endpoints (read-only)
3. **Configuration Files** - TOML-based static configuration

## Control API (Primary Integration Method)

The Control API is the recommended way for applications to control wg-agent. It allows dynamic management of WireGuard tunnels at runtime.

### Connection

**Unix/Linux/macOS:**
```
Unix Socket: /var/run/wg-agent.sock
Protocol: Line-delimited JSON over Unix domain socket
```

**Windows:**
```
Named Pipe: \\.\pipe\wg-agent
Protocol: Line-delimited JSON
```

### Request Format

All requests follow this JSON structure:

```json
{
  "id": "unique-request-id",
  "action": "connect|disconnect|status|reload|rotate_keys",
  "network": "network-name",
  "config": { /* optional configuration object */ }
}
```

#### Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | No | Unique identifier for tracking requests (auto-generated if omitted) |
| `action` | string | Yes | Action to perform (see [Actions](#actions)) |
| `network` | string | No | Network name to operate on (default: "default") |
| `config` | object | No | Configuration data (required for `connect` action) |

### Response Format

All responses follow this JSON structure:

```json
{
  "id": "request-id",
  "success": true,
  "data": { /* optional response data */ },
  "error": { /* optional error information */ }
}
```

#### Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | The request ID this response corresponds to |
| `success` | boolean | Whether the request succeeded |
| `data` | object | Response data (present on success) |
| `error` | object | Error information (present on failure) |

#### Error Object

```json
{
  "type": "error_type",
  "message": "error description"
}
```

**Error Types:**
- `parse_error` - Failed to parse request JSON
- `serialization_error` - Failed to serialize response
- `invalid_state` - Invalid action for current tunnel state
- `network_not_found` - Specified network doesn't exist
- `config_error` - Configuration validation failed
- `platform_error` - Platform-specific error (TUN device, routing, etc.)
- `internal_error` - Internal server error
- `authentication_failed` - Authentication failed (future use)
- `permission_denied` - Insufficient permissions

### Actions

#### 1. Connect

Establish a WireGuard tunnel for a network.

**Request:**
```json
{
  "id": "req-1",
  "action": "connect",
  "network": "default",
  "config": {
    "interface": "wg0",
    "mtu": 1420,
    "address": "10.100.0.2/24",
    "dns": ["1.1.1.1", "8.8.8.8"],
    "privateKeyPath": "/etc/wg-agent/private.key",
    "peers": [
      {
        "name": "runbeam-core",
        "publicKey": "base64-encoded-public-key==",
        "endpoint": "vpn.example.com:51820",
        "allowedIps": ["10.100.0.0/16", "fd42::/48"],
        "keepaliveSecs": 25
      }
    ]
  }
}
```

**Configuration Fields:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `interface` | string | No | "wg0" | WireGuard interface name |
| `mtu` | number | No | 1280 | Maximum Transmission Unit (1280-1500) |
| `address` | string | No | null | Interface IP address in CIDR notation |
| `dns` | array[string] | No | [] | DNS server IP addresses |
| `privateKeyPath` | string | Yes | - | Path to private key file |
| `peers` | array[object] | Yes | - | List of peer configurations |

**Peer Configuration Fields:**

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | Yes | - | Peer name (for identification) |
| `publicKey` | string | Yes | - | Base64-encoded public key |
| `endpoint` | string | Yes | - | Peer endpoint (host:port) |
| `allowedIps` | array[string] | Yes | - | Allowed IP ranges (CIDR notation) |
| `keepaliveSecs` | number | No | 25 | Persistent keepalive interval (seconds) |

**Success Response:**
```json
{
  "id": "req-1",
  "success": true,
  "data": {
    "network": "default",
    "state": "active",
    "interface": "wg0",
    "peers": 1
  }
}
```

**Error Response:**
```json
{
  "id": "req-1",
  "success": false,
  "error": {
    "type": "invalid_state",
    "message": "Network 'default' is already connected (state: active)"
  }
}
```

#### 2. Disconnect

Tear down a WireGuard tunnel.

**Request:**
```json
{
  "id": "req-2",
  "action": "disconnect",
  "network": "default"
}
```

**Success Response:**
```json
{
  "id": "req-2",
  "success": true,
  "data": {
    "network": "default",
    "state": "stopped"
  }
}
```

**Error Response:**
```json
{
  "id": "req-2",
  "success": false,
  "error": {
    "type": "network_not_found",
    "message": "default"
  }
}
```

#### 3. Status

Get current status of a network tunnel.

**Request:**
```json
{
  "id": "req-3",
  "action": "status",
  "network": "default"
}
```

**Success Response:**
```json
{
  "id": "req-3",
  "success": true,
  "data": {
    "network": "default",
    "state": "active",
    "interface": "wg0",
    "peers": {
      "total": 1,
      "active": 1,
      "healthy": 1,
      "names": ["runbeam-core"]
    },
    "traffic": {
      "tx_bytes": 1234567,
      "rx_bytes": 7654321
    }
  }
}
```

**Tunnel States:**
- `uninitialized` - Tunnel not yet created
- `starting` - Tunnel is being established
- `active` - Tunnel is running normally
- `stopping` - Tunnel is being torn down
- `stopped` - Tunnel has been stopped
- `error` - Tunnel encountered an error

#### 4. Reload

Reload tunnel configuration (hot-reload).

**Request:**
```json
{
  "id": "req-4",
  "action": "reload",
  "network": "default"
}
```

**Success Response:**
```json
{
  "id": "req-4",
  "success": true,
  "data": {
    "network": "default",
    "state": "active",
    "reloaded": true
  }
}
```

**Note:** Currently, reload performs a stop + start sequence. Future versions may support hot-reloading of peer configurations.

#### 5. Rotate Keys

Perform WireGuard key rotation.

**Request:**
```json
{
  "id": "req-5",
  "action": "rotate_keys",
  "network": "default"
}
```

**Status:** Not yet implemented. Returns error:
```json
{
  "id": "req-5",
  "success": false,
  "error": {
    "type": "internal_error",
    "message": "Key rotation not yet implemented"
  }
}
```

### Example: Client Implementation (Rust)

```rust
use tokio::net::UnixStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde_json::json;

async fn connect_wireguard() -> anyhow::Result<()> {
    // Connect to Unix socket
    let stream = UnixStream::connect("/var/run/wg-agent.sock").await?;
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    
    // Prepare request
    let request = json!({
        "id": "app-req-1",
        "action": "connect",
        "network": "default",
        "config": {
            "interface": "wg0",
            "mtu": 1420,
            "address": "10.100.0.2/24",
            "dns": ["1.1.1.1"],
            "privateKeyPath": "/etc/app/wireguard-key",
            "peers": [{
                "name": "vpn-server",
                "publicKey": "your-public-key-here==",
                "endpoint": "vpn.example.com:51820",
                "allowedIps": ["10.100.0.0/16"],
                "keepaliveSecs": 25
            }]
        }
    });
    
    // Send request
    let request_str = serde_json::to_string(&request)?;
    writer.write_all(request_str.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    
    // Read response
    let mut response_line = String::new();
    reader.read_line(&mut response_line).await?;
    
    let response: serde_json::Value = serde_json::from_str(&response_line)?;
    
    if response["success"].as_bool().unwrap_or(false) {
        println!("Tunnel established successfully");
        println!("State: {}", response["data"]["state"]);
    } else {
        eprintln!("Failed to establish tunnel: {:?}", response["error"]);
    }
    
    Ok(())
}
```

### Example: Client Implementation (Python)

```python
import json
import socket

def connect_wireguard():
    # Connect to Unix socket
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.connect("/var/run/wg-agent.sock")
    
    # Prepare request
    request = {
        "id": "py-req-1",
        "action": "connect",
        "network": "default",
        "config": {
            "interface": "wg0",
            "mtu": 1420,
            "address": "10.100.0.2/24",
            "dns": ["1.1.1.1"],
            "privateKeyPath": "/etc/app/wireguard-key",
            "peers": [{
                "name": "vpn-server",
                "publicKey": "your-public-key-here==",
                "endpoint": "vpn.example.com:51820",
                "allowedIps": ["10.100.0.0/16"],
                "keepaliveSecs": 25
            }]
        }
    }
    
    # Send request
    request_str = json.dumps(request) + "\n"
    sock.sendall(request_str.encode())
    
    # Receive response
    response_data = sock.recv(4096)
    response = json.loads(response_data.decode())
    
    if response.get("success"):
        print(f"Tunnel established: {response['data']['state']}")
    else:
        print(f"Error: {response['error']}")
    
    sock.close()

if __name__ == "__main__":
    connect_wireguard()
```

### Example: Client Implementation (Go)

```go
package main

import (
    "bufio"
    "encoding/json"
    "fmt"
    "net"
)

type Request struct {
    ID      string                 `json:"id"`
    Action  string                 `json:"action"`
    Network string                 `json:"network"`
    Config  map[string]interface{} `json:"config,omitempty"`
}

type Response struct {
    ID      string                 `json:"id"`
    Success bool                   `json:"success"`
    Data    map[string]interface{} `json:"data,omitempty"`
    Error   map[string]string      `json:"error,omitempty"`
}

func connectWireGuard() error {
    // Connect to Unix socket
    conn, err := net.Dial("unix", "/var/run/wg-agent.sock")
    if err != nil {
        return err
    }
    defer conn.Close()
    
    // Prepare request
    request := Request{
        ID:      "go-req-1",
        Action:  "connect",
        Network: "default",
        Config: map[string]interface{}{
            "interface":      "wg0",
            "mtu":            1420,
            "address":        "10.100.0.2/24",
            "dns":            []string{"1.1.1.1"},
            "privateKeyPath": "/etc/app/wireguard-key",
            "peers": []map[string]interface{}{
                {
                    "name":           "vpn-server",
                    "publicKey":      "your-public-key-here==",
                    "endpoint":       "vpn.example.com:51820",
                    "allowedIps":     []string{"10.100.0.0/16"},
                    "keepaliveSecs":  25,
                },
            },
        },
    }
    
    // Send request
    encoder := json.NewEncoder(conn)
    if err := encoder.Encode(request); err != nil {
        return err
    }
    
    // Read response
    reader := bufio.NewReader(conn)
    var response Response
    decoder := json.NewDecoder(reader)
    if err := decoder.Decode(&response); err != nil {
        return err
    }
    
    if response.Success {
        fmt.Printf("Tunnel established: %v\n", response.Data["state"])
    } else {
        fmt.Printf("Error: %v\n", response.Error)
    }
    
    return nil
}

func main() {
    if err := connectWireGuard(); err != nil {
        panic(err)
    }
}
```

### Example: Client Implementation (Node.js/TypeScript)

```typescript
import * as net from 'net';

interface WgAgentRequest {
  id: string;
  action: 'connect' | 'disconnect' | 'status' | 'reload' | 'rotate_keys';
  network: string;
  config?: {
    interface?: string;
    mtu?: number;
    address?: string;
    dns?: string[];
    privateKeyPath: string;
    peers: Array<{
      name: string;
      publicKey: string;
      endpoint: string;
      allowedIps: string[];
      keepaliveSecs?: number;
    }>;
  };
}

interface WgAgentResponse {
  id: string;
  success: boolean;
  data?: any;
  error?: {
    type: string;
    message: string;
  };
}

async function connectWireGuard(): Promise<void> {
  return new Promise((resolve, reject) => {
    // Connect to Unix socket
    const client = net.connect('/var/run/wg-agent.sock');
    
    client.on('connect', () => {
      // Prepare request
      const request: WgAgentRequest = {
        id: 'node-req-1',
        action: 'connect',
        network: 'default',
        config: {
          interface: 'wg0',
          mtu: 1420,
          address: '10.100.0.2/24',
          dns: ['1.1.1.1'],
          privateKeyPath: '/etc/app/wireguard-key',
          peers: [{
            name: 'vpn-server',
            publicKey: 'your-public-key-here==',
            endpoint: 'vpn.example.com:51820',
            allowedIps: ['10.100.0.0/16'],
            keepaliveSecs: 25
          }]
        }
      };
      
      // Send request
      client.write(JSON.stringify(request) + '\n');
    });
    
    client.on('data', (data) => {
      const response: WgAgentResponse = JSON.parse(data.toString());
      
      if (response.success) {
        console.log(`Tunnel established: ${response.data.state}`);
        resolve();
      } else {
        console.error(`Error: ${response.error}`);
        reject(new Error(response.error?.message));
      }
      
      client.end();
    });
    
    client.on('error', (err) => {
      reject(err);
    });
  });
}

connectWireGuard().catch(console.error);
```

## HTTP API (Read-Only)

The HTTP API provides monitoring and health check endpoints. These are read-only and suitable for prometheus scraping, health checks, and monitoring dashboards.

### Base URL

```
http://127.0.0.1:9090
```

### Endpoints

#### GET /healthz

Health check endpoint.

**Response:**
```
HTTP/1.1 200 OK
Content-Type: text/plain

OK
```

**Use Cases:**
- Kubernetes liveness/readiness probes
- Load balancer health checks
- Monitoring systems

#### GET /metrics

Prometheus-compatible metrics endpoint.

**Response:**
```
HTTP/1.1 200 OK
Content-Type: text/plain; version=0.0.4

# HELP wg_agent_info Agent information
# TYPE wg_agent_info gauge
wg_agent_info{version="0.1.0"} 1

# HELP wg_network_state Network connection state (0=disconnected, 1=connecting, 2=connected, 3=degraded, 4=failed)
# TYPE wg_network_state gauge
wg_network_state{network="default"} 2

# HELP wg_bytes_transmitted Total bytes transmitted
# TYPE wg_bytes_transmitted counter
wg_bytes_transmitted{network="default"} 1234567

# HELP wg_bytes_received Total bytes received
# TYPE wg_bytes_received counter
wg_bytes_received{network="default"} 7654321

# HELP wg_peers_total Total number of peers
# TYPE wg_peers_total gauge
wg_peers_total{network="default"} 1

# HELP wg_peers_active Active peers
# TYPE wg_peers_active gauge
wg_peers_active{network="default"} 1
```

**Metrics:**

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `wg_agent_info` | gauge | version | Agent version information |
| `wg_network_state` | gauge | network | Connection state (0-4) |
| `wg_bytes_transmitted` | counter | network | Total bytes transmitted |
| `wg_bytes_received` | counter | network | Total bytes received |
| `wg_peers_total` | gauge | network | Total number of peers |
| `wg_peers_active` | gauge | network | Number of active peers |

**Network States:**
- `0` = Disconnected
- `1` = Connecting
- `2` = Connected
- `3` = Degraded
- `4` = Failed

**Use Cases:**
- Prometheus monitoring
- Grafana dashboards
- Alerting systems

### Example: Prometheus Configuration

```yaml
scrape_configs:
  - job_name: 'wg-agent'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
```

### Example: Grafana Dashboard Query

```promql
# Network connection state
wg_network_state{network="default"}

# Traffic rate (bytes/sec)
rate(wg_bytes_transmitted{network="default"}[5m])
rate(wg_bytes_received{network="default"}[5m])

# Total traffic (formatted)
wg_bytes_transmitted{network="default"} / 1024 / 1024  # MB transmitted
wg_bytes_received{network="default"} / 1024 / 1024     # MB received

# Peer health
wg_peers_active{network="default"} / wg_peers_total{network="default"}
```

## Configuration File Format (TOML)

For static configuration at startup, wg-agent uses TOML format.

### File Location

Default: `/etc/wg-agent/config.toml`

Override with: `--config` flag

### Format

```toml
# Multi-network configuration
[network.default]
enable_wireguard = true
interface = "wg0"
mtu = 1420
address = "10.100.0.2/24"
private_key_path = "/etc/wg-agent/private.key"
dns = ["1.1.1.1", "8.8.8.8"]

[[network.peers]]
name = "runbeam-core"
public_key = "base64-encoded-public-key=="
endpoint = "vpn.example.com:51820"
allowed_ips = ["10.100.0.0/16", "fd42::/48"]
persistent_keepalive_secs = 25

# Additional network
[network.production]
enable_wireguard = true
interface = "wg1"
mtu = 1280
address = "10.200.0.2/24"
private_key_path = "/etc/wg-agent/prod-key"

[[network.production.peers]]
name = "prod-gateway"
public_key = "another-base64-key=="
endpoint = "prod.example.com:51820"
allowed_ips = ["10.200.0.0/16"]
persistent_keepalive_secs = 30
```

### Configuration Fields

See [Connect Action](#1-connect) for field descriptions. The TOML format uses snake_case instead of camelCase.

## Security Considerations

### Permissions

1. **Socket/Pipe Permissions:**
   - Unix socket should be owned by root with mode `0600` or `0660`
   - Only trusted processes should have access
   - Consider using group permissions for multi-user scenarios

2. **Private Keys:**
   - Store private keys with mode `0600`
   - Never transmit private keys over the network
   - Use secure key management systems in production

3. **Process Privileges:**
   - wg-agent requires `NET_ADMIN` and `IPC_LOCK` capabilities on Linux
   - Run as root or with appropriate capabilities
   - Consider using systemd service hardening

### Authentication

Current version: No authentication required for socket connections.

**Planned for future versions:**
- Unix socket credential checking (SO_PEERCRED)
- Token-based authentication
- mTLS for network-based control plane

### Network Isolation

- Control socket is local-only by default
- HTTP metrics endpoint binds to 127.0.0.1 by default
- No remote management interface (use SSH tunneling if needed)

## Integration Examples

### Aurabox Integration

```rust
// In Aurabox's network setup
use tokio::net::UnixStream;

pub async fn setup_wireguard_tunnel(config: &AuraboxConfig) -> Result<()> {
    let stream = UnixStream::connect("/var/run/wg-agent.sock").await?;
    
    let request = ApiRequest {
        id: format!("aurabox-{}", uuid::Uuid::new_v4()),
        action: ControlAction::Connect,
        network: "aurabox".to_string(),
        config: Some(serde_json::to_value(&config.wireguard)?),
    };
    
    // Send request and handle response...
    
    Ok(())
}
```

### JMIX Integration

```typescript
// In JMIX CLI
import { WgAgentClient } from './wg-agent-client';

async function enableVPN() {
  const client = new WgAgentClient('/var/run/wg-agent.sock');
  
  const response = await client.connect({
    network: 'jmix-vpn',
    config: {
      interface: 'wg0',
      privateKeyPath: process.env.JMIX_WG_KEY,
      peers: [{
        name: 'jmix-gateway',
        publicKey: process.env.JMIX_PEER_KEY,
        endpoint: `${process.env.JMIX_VPN_HOST}:51820`,
        allowedIps: ['10.42.0.0/16']
      }]
    }
  });
  
  console.log(`VPN established: ${response.data.state}`);
}
```

### Harmony Integration

```go
// In Harmony proxy
func (h *Harmony) ConfigureWireGuard(cfg *Config) error {
    conn, err := net.Dial("unix", "/var/run/wg-agent.sock")
    if err != nil {
        return err
    }
    defer conn.Close()
    
    request := WgRequest{
        Action:  "connect",
        Network: cfg.NetworkName,
        Config:  cfg.WireGuardConfig,
    }
    
    // Send and receive...
    
    return nil
}
```

## Troubleshooting

### Common Issues

**Problem:** Connection refused to socket

**Solutions:**
- Check wg-agent is running: `systemctl status wg-agent`
- Verify socket exists: `ls -l /var/run/wg-agent.sock`
- Check permissions on socket file

**Problem:** Permission denied errors

**Solutions:**
- Ensure wg-agent has NET_ADMIN capability
- Run agent as root: `sudo wg-agent start`
- Check private key file permissions (should be 0600)

**Problem:** Network not found error

**Solutions:**
- Verify network configuration is loaded
- Check network name spelling
- Use `status` action to list available networks

**Problem:** Tunnel state stuck in "starting"

**Solutions:**
- Check logs: `journalctl -u wg-agent -f`
- Verify peer endpoint is reachable
- Confirm private key format is correct
- Check firewall rules allow UDP port 51820

## Changelog

### Version 0.1.0 (Current)

- Initial release
- Control API with connect/disconnect/status/reload actions
- HTTP metrics endpoint
- TOML configuration support
- Linux, macOS, Windows support
- boringtun and wireguard-go backends

### Planned Features

- Key rotation (rotate_keys action)
- Hot-reload peer configurations
- Authentication for control socket
- WebSocket control API
- gRPC control API option
- Dynamic peer management
- Multi-interface support

## Support

For issues, questions, or contributions:
- GitHub Issues: [runbeam/wg-agent](https://github.com/runbeam/wg-agent)
- Documentation: See `docs/` directory
- Examples: See `examples/` directory
