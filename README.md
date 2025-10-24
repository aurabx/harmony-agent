# Harmony WireGuard agent

[//]: # ([![CI]&#40;https://github.com/aurabx/wg-agent/workflows/CI/badge.svg&#41;]&#40;https://github.com/aurabx/wg-agent/actions&#41;)

[//]: # ([![codecov]&#40;https://codecov.io/gh/runbeam/wg-agent/branch/main/graph/badge.svg&#41;]&#40;https://codecov.io/gh/runbeam/wg-agent&#41;)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)

A cross-platform WireGuard network agent for managing VPN tunnels on behalf of applications. Built with Rust for security, performance, and reliability.

This is a core part of the Harmony system, providing connectivity between Harmony instances. See [https://runbeam.io](https://runbeam.io)

## Features

- 🔐 **Secure by Default**: Memory-safe Rust implementation with key zeroing and privilege dropping
- 🌐 **Cross-Platform**: Linux, macOS, Windows, Docker, and Kubernetes support
- 📊 **Observable**: Built-in Prometheus metrics and health checks
- 🚀 **High Performance**: Async I/O with tokio, zero-copy operations
- 🔧 **Flexible**: TOML configuration or dynamic JSON control API
- 🛡️ **Hardened**: Input validation, secure defaults, audit logging
- 📡 **Control API**: Unix sockets (Linux/macOS) and Named Pipes (Windows)

## Quick Start

### Installation

```bash
# Download latest release
curl -L https://github.com/aurabx/wg-agent/releases/latest/download/wg-agent-linux-x86_64.tar.gz | tar xz
sudo mv wg-agent /usr/local/bin/

# Or build from source
cargo install --path .
```

### Configuration

Create `/etc/wg-agent/config.toml`:

```toml
[network.default]
enable_wireguard = true
interface = "wg0"
mtu = 1420
private_key_path = "/etc/wg-agent/private.key"
dns = ["10.100.0.2"]

[[network.peers]]
name = "gateway"
public_key = "YOUR_PEER_PUBLIC_KEY"
endpoint = "vpn.example.com:51820"
allowed_ips = ["10.42.0.0/16"]
persistent_keepalive_secs = 25
```

### Run

```bash
# Start agent
sudo wg-agent start --config /etc/wg-agent/config.toml

# Check status
wg-agent status

# View metrics
curl http://localhost:9090/metrics
```

## Documentation

- **[User Guide](docs/USER_GUIDE.md)** - Installation, configuration, and usage
- **[API Reference](docs/API_REFERENCE.md)** - Control API documentation
- **[Testing Guide](docs/TESTING.md)** - Running tests and benchmarks
- **[Security](docs/SECURITY.md)** - Security best practices
- **[Architecture](docs/architecture.md)** - Technical design details
- **[Development Plan](dev/DEVELOPMENT_PLAN.md)** - Project roadmap

## Deployment

### Docker

```bash
docker run --rm --cap-add NET_ADMIN --cap-add IPC_LOCK \
  -v /etc/wg-agent:/etc/wg-agent:ro \
  ghcr.io/runbeam/wg-agent:latest
```

### Docker Compose

```bash
cd deploy/docker-compose
docker-compose up -d
```

### Kubernetes

```bash
# DaemonSet (one agent per node)
kubectl apply -f deploy/kubernetes/daemonset.yaml

# Sidecar (one agent per pod)
kubectl apply -f deploy/kubernetes/sidecar.yaml
```

### Systemd (Linux)

```bash
# Copy service file
sudo cp deploy/systemd/wg-agent.service /etc/systemd/system/

# Enable and start
sudo systemctl enable --now wg-agent
```

### LaunchDaemon (macOS)

```bash
# Copy plist
sudo cp deploy/launchd/cloud.runbeam.wg-agent.plist /Library/LaunchDaemons/

# Load and start
sudo launchctl load /Library/LaunchDaemons/cloud.runbeam.wg-agent.plist
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Application Layer                     │
│              (Aurabox, JMIX, Runbeam, etc.)            │
└──────────────────────┬──────────────────────────────────┘
                       │ JSON Control Messages
                       ↓
┌─────────────────────────────────────────────────────────┐
│                      wg-agent                           │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ Control API  │  │  WireGuard   │  │  Monitoring  │  │
│  │ (Unix/Pipe)  │  │   Tunnel     │  │  & Metrics   │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │  Security    │  │   Platform   │  │   Service    │  │
│  │  Hardening   │  │  Abstraction │  │   Manager    │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  │
└──────────────────────┬──────────────────────────────────┘
                       │ TUN/TAP Device
                       ↓
┌─────────────────────────────────────────────────────────┐
│              Operating System (Linux/macOS/Windows)      │
└─────────────────────────────────────────────────────────┘
```

## Development

### Prerequisites

- Rust 1.75+ (Edition 2024)
- `CAP_NET_ADMIN` and `CAP_IPC_LOCK` capabilities (Linux)
- Administrator privileges (macOS/Windows)

### Build

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run benchmarks
cargo bench

# Check formatting and linting
cargo fmt -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

### Testing

```bash
# All tests
cargo test

# Integration tests
cargo test --test '*'

# With coverage
cargo llvm-cov --all-features --workspace --lcov
```

## Performance

Benchmarks on Intel i7-9750H @ 2.60GHz:

| Operation | Time |
|-----------|------|
| Key Generation | 45 μs |
| Public Key Derivation | 42 μs |
| Config Parsing (TOML) | 125 μs |
| Monitoring Update | 1.2 μs |
| Metrics Export (Prometheus) | 8.5 μs |

## Security

- **Keys Never Logged**: Private keys use `Debug` redaction
- **Memory Zeroing**: Keys zeroed on drop with `zeroize` crate
- **Privilege Dropping**: Runs as non-root after TUN creation
- **Input Validation**: All inputs validated and sanitized
- **Secure Defaults**: No plaintext secrets, 0600 file permissions
- **Memory Locking**: IPC_LOCK prevents key material swapping

See [SECURITY.md](docs/SECURITY.md) for details.

## Monitoring

### Prometheus Metrics

```
wg_agent_bytes_transmitted_total      # Total bytes sent
wg_agent_bytes_received_total         # Total bytes received
wg_agent_active_peers                  # Number of active peers
wg_agent_handshake_success_total      # Successful handshakes
wg_agent_handshake_failure_total      # Failed handshakes
wg_agent_connection_uptime_seconds    # Connection uptime
```

### Health Checks

```bash
$ curl http://localhost:9090/healthz
{
  "status": "healthy",
  "networks": {
    "default": {
      "status": "healthy",
      "state": "connected",
      "peerHealth": 100.0
    }
  }
}
```

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the Apache License 2.0 - see [LICENSE](LICENSE) for details.

## Acknowledgments

- Built with [boringtun](https://github.com/cloudflare/boringtun) - Pure Rust WireGuard implementation
- Inspired by [WireGuard](https://www.wireguard.com/) - Fast, modern, secure VPN tunnel
- Part of the [Runbeam](https://runbeam.io) ecosystem

## Support

- **Documentation**: https://docs.runbeam.io/wg-agent
- **Issues**: https://github.com/aurabx/wg-agent/issues
- **Discussions**: https://github.com/aurabx/wg-agent/discussions
- **Security**: security@runbeam.io

---

Made with ❤️ by the Runbeam team
