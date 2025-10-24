# wg-agent Examples

This directory contains examples and testing utilities for wg-agent.

## Docker WireGuard Testing

Test wg-agent against a real WireGuard peer using Docker.

### Prerequisites

- Docker and Docker Compose installed
- Rust toolchain (cargo)
- macOS with sudo access (for TUN device creation)

### Quick Start

```bash
# 1. Setup Docker WireGuard server
./examples/test-with-docker.sh

# 2. Build and run wg-agent (in project root)
cargo build --release
sudo ./target/release/wg-agent

# 3. Verify connection (in another terminal)
./examples/verify-connection.sh
```

### What Gets Created

- **wireguard-server/** - Docker Compose setup for WireGuard server
- **keys/** - Generated WireGuard keys (client private/public)
- **config.toml** - wg-agent configuration (in project root)

### Files

- **test-with-docker.sh** - Automated setup script
  - Starts Docker WireGuard server
  - Generates cryptographic keys
  - Configures mutual peer authentication
  - Creates wg-agent config.toml

- **verify-connection.sh** - Connection verification script
  - Checks wg-agent metrics
  - Tests tunnel connectivity
  - Shows handshake status
  - Displays transfer statistics

- **wireguard-server/docker-compose.yml** - Docker WireGuard server
  - linuxserver/wireguard image
  - 10.100.0.0/24 internal network
  - Port 51820/udp exposed

### Testing Workflow

1. **Start server**: `./examples/test-with-docker.sh`
2. **Build agent**: `cargo build --release`
3. **Run agent**: `sudo ./target/release/wg-agent`
4. **Verify**: `./examples/verify-connection.sh`
5. **Test traffic**: `ping 10.100.0.1`

### Expected Results

```
📊 wg-agent Metrics:
tunnel_state{name="docker_test"} 1
peer_last_handshake{tunnel="docker_test",peer="docker-server"} 1729725431

🏓 Testing Connectivity:
Ping 10.100.0.1 (server): ✅ SUCCESS

⏱️  Last Handshake:
   5 seconds ago ✅
```

### Troubleshooting

**No handshake?**
```bash
# Check Docker logs
docker logs wg-test-server

# Enable debug logging
RUST_LOG=debug sudo ./target/release/wg-agent
```

**Can't ping?**
```bash
# Check routing
netstat -rn | grep 10.100

# Check TUN device
ifconfig | grep utun
```

**Permission denied?**
```bash
# wg-agent requires sudo for TUN device creation
sudo ./target/release/wg-agent
```

### Cleanup

```bash
# Stop wg-agent (Ctrl+C)

# Stop Docker server
cd ./examples/wireguard-server
docker-compose down

# Remove generated files (optional)
rm -rf ./examples/keys ./examples/wireguard-server/config
rm config.toml
```

## Other Examples

- **test_device.rs** - Manual WireGuard device testing example
  - Requires `--features testing`
  - Run with: `cargo run --example test_device --features testing`
