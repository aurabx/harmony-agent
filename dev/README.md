# harmony-agent Examples

This directory contains examples and testing utilities for harmony-agent.

## Docker WireGuard Testing

Test harmony-agent against a real WireGuard peer using Docker.

### Prerequisites

- Docker and Docker Compose installed
- Rust toolchain (cargo)
- macOS with sudo access (for TUN device creation)

### Quick Start

```bash
# 1. Setup Docker WireGuard server
./examples/test-with-docker.sh

# 2. Build and run harmony-agent (in project root)
cargo build --release
sudo ./target/release/harmony-agent

# 3. Verify connection (in another terminal)
./examples/verify-connection.sh
```

### What Gets Created

- **wireguard-server/** - Docker Compose setup for WireGuard server
- **keys/** - Generated WireGuard keys (client private/public)
- **config.toml** - harmony-agent configuration (in project root)

### Files

- **test-with-docker.sh** - Automated setup script
  - Starts Docker WireGuard server
  - Generates cryptographic keys
  - Configures mutual peer authentication
  - Creates harmony-agent config.toml

- **verify-connection.sh** - Connection verification script
  - Checks harmony-agent metrics
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
3. **Run agent**: `sudo ./target/release/harmony-agent`
4. **Verify**: `./examples/verify-connection.sh`
5. **Test traffic**: `ping 10.100.0.1`

### Expected Results

```
📊 harmony-agent Metrics:
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
RUST_LOG=debug sudo ./target/release/harmony-agent
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
# harmony-agent requires sudo for TUN device creation
sudo ./target/release/harmony-agent
```

### Cleanup

```bash
# Stop harmony-agent (Ctrl+C)

# Stop Docker server
cd ./examples/wireguard-server
docker-compose down

# Remove generated files (optional)
rm -rf ./examples/keys ./examples/wireguard-server/config
rm config.toml
```

## Control Server Testing

Test the control server API quickly:

```bash
# Quick test - verifies server is running and responding
./dev/test-control-server.sh

# Test with custom socket path
./dev/test-control-server.sh /tmp/custom.sock
```

**What it tests:**
- Socket file exists
- Socket is accessible
- Server responds to requests
- Response is valid JSON

**Expected output:**
```
Testing wg-agent Control Server
================================

1. Checking if socket exists: /var/run/wg-agent.sock
   ✅ PASS: Socket exists

2. Checking socket permissions
   ✅ PASS: Socket is accessible

3. Testing connection with Python
   ✅ PASS: Server responded
   ✅ PASS: Response is valid JSON

================================
✅ ALL TESTS PASSED
```

### Rust Integration Tests

```bash
# Run control server integration tests
cargo test --test control_server_test -- --ignored --nocapture
```

### Manual Testing

```bash
# Using socat
echo '{"id":"test-1","action":"status","network":"default"}' | \
  socat - UNIX-CONNECT:/var/run/wg-agent.sock

# Using Python
python3 << 'EOF'
import socket, json
sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect("/var/run/wg-agent.sock")
sock.sendall(json.dumps({"action":"status","network":"default"}).encode() + b"\n")
print(json.loads(sock.recv(4096)))
EOF
```

### Documentation

- `CONTROL_API_TESTING.md` - Comprehensive testing guide
- `IMPLEMENTATION_SUMMARY.md` - Architecture details
- `../docs/API.md` - Complete API reference

## Other Examples

- **test_device.rs** - Manual WireGuard device testing example
  - Requires `--features testing`
  - Run with: `cargo run --example test_device --features testing`
