# Control API Testing Guide

This guide explains how to test the Control API implementation.

## Prerequisites

- Rust toolchain installed
- Root/sudo access (required for creating TUN devices)
- WireGuard tools (for generating keys)

## Quick Test (Without Real WireGuard)

### 1. Build the Agent

```bash
cargo build --release
```

### 2. Create Test Configuration

```bash
# Create config directory
mkdir -p ./tmp/test-config

# Generate WireGuard keys
wg genkey | tee ./tmp/test-config/private.key | wg pubkey > ./tmp/test-config/public.key

# Create minimal config (won't auto-start tunnels)
cat > ./tmp/test-config/config.toml <<EOF
# Empty config - no auto-start tunnels
EOF
```

### 3. Start the Agent

```bash
# Run agent (requires sudo for socket creation in /var/run)
sudo ./target/release/wg-agent -c ./tmp/test-config/config.toml start
```

You should see:
```
INFO Starting wg-agent v0.1.0
INFO Starting agent with config: ./tmp/test-config/config.toml
INFO Service mode: ...
INFO Started 0 WireGuard tunnel(s)
INFO Starting control server at "/var/run/wg-agent.sock"
INFO Control server listening at "/var/run/wg-agent.sock"
INFO Starting HTTP server on 127.0.0.1:9090
```

### 4. Test with Example Client

In another terminal:

```bash
# Build and run the example client
cargo run --example control_client
```

### 5. Test with Manual Commands

Using `nc` or `socat`:

```bash
# Using socat
echo '{"id":"test-1","action":"status","network":"default"}' | socat - UNIX-CONNECT:/var/run/wg-agent.sock

# Or using Python
python3 << 'EOF'
import socket
import json

sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect("/var/run/wg-agent.sock")

request = {"id": "test-1", "action": "status", "network": "default"}
sock.sendall((json.dumps(request) + "\n").encode())

response = sock.recv(4096)
print(json.loads(response.decode()))
sock.close()
EOF
```

## Full Test (With Real WireGuard Tunnel)

### 1. Create Full Test Configuration

```bash
# Create config directory
mkdir -p ./tmp/test-config

# Generate keys
wg genkey | tee ./tmp/test-config/private.key | wg pubkey > ./tmp/test-config/public.key
wg genkey | tee ./tmp/test-config/peer-private.key | wg pubkey > ./tmp/test-config/peer-public.key

PEER_PUBLIC_KEY=$(cat ./tmp/test-config/peer-public.key)

# Create agent configuration with auto-start network
cat > ./tmp/test-config/config.toml <<EOF
[network.testnet]
enable_wireguard = true
interface = "wg0"
address = "10.100.0.2/24"
mtu = 1420
private_key_path = "./tmp/test-config/private.key"
dns = ["1.1.1.1"]

[[network.testnet.peers]]
name = "test-peer"
public_key = "$PEER_PUBLIC_KEY"
endpoint = "127.0.0.1:51820"
allowed_ips = ["10.100.0.0/24"]
persistent_keepalive_secs = 25
EOF

# Create peer config
AGENT_PUBLIC_KEY=$(cat ./tmp/test-config/public.key)
cat > ./tmp/test-config/peer.conf <<EOF
[Interface]
PrivateKey = $(cat ./tmp/test-config/peer-private.key)
Address = 10.100.0.1/24
ListenPort = 51820

[Peer]
PublicKey = $AGENT_PUBLIC_KEY
AllowedIPs = 10.100.0.2/32
EOF
```

### 2. Start Test Peer (Terminal 1)

```bash
sudo wg-quick up ./tmp/test-config/peer.conf
```

### 3. Start Agent (Terminal 2)

```bash
sudo ./target/release/wg-agent -c ./tmp/test-config/config.toml start
```

### 4. Test Control API (Terminal 3)

```bash
# Check status of auto-started tunnel
echo '{"id":"req-1","action":"status","network":"testnet"}' | socat - UNIX-CONNECT:/var/run/wg-agent.sock

# Expected response should show "state": "active"

# Test connectivity
ping -c 3 10.100.0.1
```

### 5. Test Dynamic Control

```bash
# Disconnect the tunnel
echo '{"id":"req-2","action":"disconnect","network":"testnet"}' | socat - UNIX-CONNECT:/var/run/wg-agent.sock

# Verify interface is gone
ip link show wg0  # Should error: "does not exist"

# Reconnect (requires full config in request)
cat > /tmp/reconnect.json <<'EOF'
{
  "id": "req-3",
  "action": "connect",
  "network": "testnet",
  "config": {
    "interface": "wg0",
    "mtu": 1420,
    "address": "10.100.0.2/24",
    "dns": ["1.1.1.1"],
    "privateKeyPath": "./tmp/test-config/private.key",
    "peers": [{
      "name": "test-peer",
      "publicKey": "PEER_PUBLIC_KEY_HERE",
      "endpoint": "127.0.0.1:51820",
      "allowedIps": ["10.100.0.0/24"],
      "keepaliveSecs": 25
    }]
  }
}
EOF

# Replace PEER_PUBLIC_KEY_HERE with actual key, then:
cat /tmp/reconnect.json | socat - UNIX-CONNECT:/var/run/wg-agent.sock
```

## Test HTTP Endpoints

While agent is running:

```bash
# Health check
curl http://127.0.0.1:9090/healthz
# Expected: OK

# Metrics
curl http://127.0.0.1:9090/metrics
# Expected: Prometheus metrics
```

## Expected Control API Responses

### Status (Network Exists)
```json
{
  "id": "req-1",
  "success": true,
  "data": {
    "network": "testnet",
    "state": "active",
    "interface": "wg0",
    "peers": {
      "total": 1,
      "active": 1,
      "healthy": 1,
      "names": ["test-peer"]
    },
    "traffic": {
      "tx_bytes": 1234,
      "rx_bytes": 5678
    }
  }
}
```

### Status (Network Not Found)
```json
{
  "id": "req-1",
  "success": false,
  "error": {
    "type": "network_not_found",
    "message": "nonexistent"
  }
}
```

### Connect Success
```json
{
  "id": "req-2",
  "success": true,
  "data": {
    "network": "testnet",
    "state": "active",
    "interface": "wg0",
    "peers": 1
  }
}
```

### Disconnect Success
```json
{
  "id": "req-3",
  "success": true,
  "data": {
    "network": "testnet",
    "state": "stopped"
  }
}
```

## Cleanup

```bash
# Stop agent (Ctrl+C in terminal 2)

# Remove peer interface
sudo wg-quick down ./tmp/test-config/peer.conf

# Check socket is removed
ls -l /var/run/wg-agent.sock  # Should not exist
```

## Troubleshooting

### Socket Permission Denied
```bash
# Check socket exists
ls -l /var/run/wg-agent.sock

# If needed, adjust permissions (not recommended in production)
sudo chmod 666 /var/run/wg-agent.sock
```

### Socket Already Exists
```bash
# Remove stale socket
sudo rm /var/run/wg-agent.sock

# Restart agent
sudo ./target/release/wg-agent -c config.toml start
```

### Connection Refused
```bash
# Check if agent is running
ps aux | grep wg-agent

# Check logs
sudo journalctl -u wg-agent -f  # If running as systemd service

# Or check stderr if running directly
```

### Parse Error
```bash
# Validate JSON
echo '{"id":"test","action":"status"}' | python3 -m json.tool

# Check for proper newline
echo '{"id":"test","action":"status","network":"default"}' | cat -A
# Should show: {"id":"test","action":"status","network":"default"}$
```

## Integration Testing

See `tests/tunnel_integration.rs` for automated integration tests that cover:
- Starting agent with config
- Connecting via Control API
- Status queries
- Disconnecting tunnels
- Graceful shutdown

Run with:
```bash
cargo test --test tunnel_integration -- --nocapture
```

## Next Steps

Once the Control API is working:
1. Integrate with Harmony proxy
2. Add authentication (Unix socket credentials)
3. Implement key rotation
4. Add hot-reload for peer changes
5. Create client libraries for Go, Python, Node.js
