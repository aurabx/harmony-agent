# harmony-agent User Guide

## Introduction

**harmony-agent** is a cross-platform WireGuard network agent designed to manage WireGuard tunnels on behalf of Harmony. It runs as a standalone daemon, isolating privileged network operations from the main application while providing a consistent API across Linux, Windows, macOS, Docker, and Kubernetes.

## Features

- 🔐 **Secure WireGuard Management**: Handles tunnel creation, peer management, and key rotation
- 🌐 **Multi-Platform**: Linux, macOS, Windows, Docker, Kubernetes
- 📊 **Built-in Monitoring**: Prometheus metrics and health checks
- 🔧 **Flexible Configuration**: TOML files or JSON control messages
- 🛡️ **Security Hardened**: Privilege dropping, memory locking, input validation
- 🚀 **High Performance**: Rust-based with zero-copy operations
- 📡 **Control API**: Unix socket (Linux/macOS) or Named Pipe (Windows)

## Installation

### Prerequisites

- **Linux**: `CAP_NET_ADMIN` and `CAP_IPC_LOCK` capabilities
- **macOS**: Administrator privileges for TUN device creation
- **Windows**: Administrator privileges for Wintun driver

### From Binary

Download the latest release for your platform:

```bash
# Linux (x86_64)
curl -L https://github.com/aurabx/harmony-agent/releases/latest/download/harmony-agent-linux-x86_64.tar.gz | tar xz
sudo mv harmony-agent /usr/local/bin/
sudo chmod +x /usr/local/bin/harmony-agent

# macOS (Apple Silicon)
curl -L https://github.com/aurabx/harmony-agent/releases/latest/download/harmony-agent-macos-aarch64.tar.gz | tar xz
sudo mv harmony-agent /usr/local/bin/
sudo chmod +x /usr/local/bin/harmony-agent
```

### From Source

```bash
# Clone repository
git clone https://github.com/aurabx/harmony-agent.git
cd harmony-agent

# Build release binary
cargo build --release

# Install
sudo cp target/release/harmony-agent /usr/local/bin/
```

### Using Docker

```bash
docker pull ghcr.io/aurabx/harmony-agent:latest
```

## Quick Start

### 1. Create Configuration

Create `/etc/harmony-agent/config.toml`:

```toml
[network.default]
enable_wireguard = true
interface = "wg0"
mtu = 1420
private_key_path = "/etc/harmony-agent/private.key"
dns = ["10.100.0.2"]

[[network.peers]]
name = "runbeam-core"
public_key = "YOUR_PEER_PUBLIC_KEY"
endpoint = "vpn.example.com:51820"
allowed_ips = ["10.42.0.0/16", "fd42:1::/48"]
persistent_keepalive_secs = 25
```

### 2. Generate Keys

```bash
# Generate private key
wg genkey > /etc/harmony-agent/private.key
chmod 600 /etc/harmony-agent/private.key

# Derive public key
wg pubkey < /etc/harmony-agent/private.key > /etc/harmony-agent/public.key
```

### 3. Start Agent

```bash
# Foreground (for testing)
sudo harmony-agent start --config /etc/harmony-agent/config.toml

# As systemd service (Linux)
sudo systemctl start harmony-agent

# As LaunchDaemon (macOS)
sudo launchctl load /Library/LaunchDaemons/cloud.runbeam.harmony-agent.plist
```

### 4. Verify Connection

```bash
# Check status
harmony-agent status

# View metrics
curl http://localhost:9090/metrics

# Check health
curl http://localhost:9090/healthz
```

## Configuration

### TOML Configuration

The agent uses TOML for static configuration. Multiple networks can be defined:

```toml
[network.production]
enable_wireguard = true
interface = "wg0"
mtu = 1420
private_key_path = "/etc/harmony-agent/prod.key"
dns = ["10.0.0.1", "10.0.0.2"]

[[network.peers]]
name = "gateway-1"
public_key = "base64encodedkey="
endpoint = "gateway1.example.com:51820"
allowed_ips = ["10.42.0.0/16"]
persistent_keepalive_secs = 25

[[network.peers]]
name = "gateway-2"
public_key = "base64encodedkey2="
endpoint = "gateway2.example.com:51820"
allowed_ips = ["10.43.0.0/16"]
persistent_keepalive_secs = 25

[network.staging]
enable_wireguard = true
interface = "wg1"
mtu = 1280
private_key_path = "/etc/harmony-agent/staging.key"
dns = ["10.1.0.1"]

[[network.staging.peers]]
name = "staging-gateway"
public_key = "stagingkeybase64="
endpoint = "staging.example.com:51820"
allowed_ips = ["10.50.0.0/16"]
```

### JSON Control Messages

For dynamic control via Harmony or other applications:

```json
{
  "id": "req-123",
  "action": "connect",
  "network": "default",
  "config": {
    "interface": "wg0",
    "mtu": 1420,
    "privateKeyPath": "/etc/harmony-agent/private.key",
    "dns": ["10.100.0.2"],
    "peers": [
      {
        "name": "peer1",
        "publicKey": "base64key=",
        "endpoint": "vpn.example.com:51820",
        "allowedIps": ["10.42.0.0/16"],
        "keepaliveSecs": 25
      }
    ]
  }
}
```

## Control API

### Unix Socket (Linux/macOS)

Default socket: `/var/run/harmony-agent.sock`

```bash
# Connect network
echo '{"id":"1","action":"connect","network":"default"}' | \
  socat - UNIX-CONNECT:/var/run/harmony-agent.sock

# Get status
echo '{"id":"2","action":"status","network":"default"}' | \
  socat - UNIX-CONNECT:/var/run/harmony-agent.sock

# Disconnect
echo '{"id":"3","action":"disconnect","network":"default"}' | \
  socat - UNIX-CONNECT:/var/run/harmony-agent.sock
```

### Named Pipe (Windows)

Default pipe: `\\.\pipe\harmony-agent`

```powershell
# PowerShell example
$request = @{
    id = "1"
    action = "status"
    network = "default"
} | ConvertTo-Json

$pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", "harmony-agent", [System.IO.Pipes.PipeDirection]::InOut)
$pipe.Connect()
# ... (send request and read response)
```

## Monitoring

### Health Checks

```bash
# Check overall health
curl http://localhost:9090/healthz

# Response format
{
  "status": "healthy",
  "timestamp": "2024-01-15T10:30:00Z",
  "networks": {
    "default": {
      "status": "healthy",
      "state": "connected",
      "peerHealth": 100.0,
      "handshakeRate": 98.5
    }
  }
}
```

### Prometheus Metrics

```bash
# Scrape metrics
curl http://localhost:9090/metrics

# Example metrics
harmony_agent_bytes_transmitted_total 1048576
harmony_agent_bytes_received_total 2097152
harmony_agent_active_peers 3
harmony_agent_handshake_success_total 150
harmony_agent_handshake_failure_total 2
harmony_agent_connection_uptime_seconds 3600
```

### Grafana Dashboard

Import the provided Grafana dashboard from `deploy/grafana/harmony-agent-dashboard.json` for visualization.

## Security

### File Permissions

```bash
# Private keys must be 0600
sudo chmod 600 /etc/harmony-agent/*.key

# Configuration can be 0640
sudo chmod 640 /etc/harmony-agent/config.toml

# Socket should be 0660
sudo chmod 660 /var/run/harmony-agent.sock
```

### Privilege Dropping

The agent starts with elevated privileges to create TUN devices, then drops to a non-privileged user:

```bash
# Run as specific user (Linux)
sudo -u harmony-agent harmony-agent start --config /etc/harmony-agent/config.toml

# Systemd service runs as User=harmony-agent
```

### Memory Locking

The agent locks memory to prevent key material from swapping to disk:

```bash
# Requires IPC_LOCK capability
sudo setcap cap_net_admin,cap_ipc_lock=+eip /usr/local/bin/harmony-agent
```

## Troubleshooting

### Agent Won't Start

**Check logs:**
```bash
# Systemd
sudo journalctl -u harmony-agent -f

# Direct run with verbose logging
sudo harmony-agent start --config /etc/harmony-agent/config.toml --verbose
```

**Common issues:**
- Missing capabilities: `setcap cap_net_admin,cap_ipc_lock=+eip /usr/local/bin/harmony-agent`
- Invalid configuration: `harmony-agent validate --config /etc/harmony-agent/config.toml`
- Port conflicts: Check if another process is using the WireGuard port

### Connection Not Establishing

**Verify configuration:**
```bash
# Check interface exists
ip link show wg0

# Verify routing
ip route show

# Test connectivity to peer
ping -c 3 10.42.0.1
```

**Check firewall:**
```bash
# Linux (iptables)
sudo iptables -L -n

# Linux (firewalld)
sudo firewall-cmd --list-all

# macOS
sudo pfctl -s rules
```

### High Latency

**Check MTU:**
```bash
# Current MTU
ip link show wg0 | grep mtu

# Test optimal MTU
ping -M do -s 1400 -c 3 10.42.0.1
```

**Monitor metrics:**
```bash
# Watch peer latency
watch -n 1 'curl -s http://localhost:9090/metrics | grep peer_latency'
```

### Key Rotation

```bash
# Generate new key
wg genkey > /etc/harmony-agent/new-private.key

# Update configuration
sudo nano /etc/harmony-agent/config.toml

# Reload agent
sudo systemctl reload harmony-agent
```

## Best Practices

1. **Use Dedicated User**: Run agent as `harmony-agent` user, not root
2. **Secure Keys**: Store keys with 0600 permissions, use hardware security modules if available
3. **Monitor Health**: Set up alerts on handshake failures and peer health
4. **Regular Updates**: Keep agent and dependencies updated
5. **Test Failover**: Verify redundant peers work correctly
6. **Backup Config**: Store configuration in version control (without keys)
7. **Log Rotation**: Configure log rotation for long-running deployments
8. **Resource Limits**: Set appropriate limits in systemd unit files

## Advanced Topics

### Custom DNS

```toml
[network.default]
# Use custom DNS servers
dns = ["1.1.1.1", "8.8.8.8"]
```

### Split Tunneling

```toml
[[network.peers]]
# Only route specific subnets through VPN
allowed_ips = ["10.42.0.0/16", "192.168.100.0/24"]
```

### Multiple Networks

Run multiple WireGuard networks simultaneously:

```toml
[network.work]
interface = "wg0"
# ... work VPN config

[network.personal]
interface = "wg1"
# ... personal VPN config
```

## Support

- **Documentation**: https://docs.runbeam.cloud/harmony-agent
- **Issues**: https://github.com/aurabx/harmony-agent/issues
- **Discussions**: https://github.com/aurabx/harmony-agent/discussions
- **Security**: security@aurabox.cloud

## License

This project is licensed under the Apache License 2.0 - see LICENSE file for details.
