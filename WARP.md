# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

**wg-agent** is a cross-platform WireGuard network agent written in Rust. It's designed as a standalone daemon that manages WireGuard tunnels on behalf of other applications (Aurabox, JMIX, Runbeam). The agent isolates privileged network operations from the main application while providing a consistent API across Linux, Windows, macOS, Docker, and Kubernetes.

## Build & Development Commands

### Building

```bash
# Build debug version
cargo build

# Build release version
cargo build --release

# Build and run
cargo run
```

### Testing

```bash
# Run all tests
cargo test

# Run a specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture
```

### Code Quality

```bash
# Check code without building
cargo check

# Run linter
cargo clippy

# Format code
cargo fmt

# Check formatting without applying
cargo fmt -- --check
```

### Cross-Compilation

The agent must be built for multiple platforms. Use the following commands for cross-compilation:

```bash
# Linux (amd64)
cargo build --release --target x86_64-unknown-linux-gnu

# Windows (amd64)
cargo build --release --target x86_64-pc-windows-gnu

# macOS (Apple Silicon)
cargo build --release --target aarch64-apple-darwin

# macOS (Intel)
cargo build --release --target x86_64-apple-darwin
```

**Note:** Cross-compilation requires the appropriate target toolchains. Install with:
```bash
rustup target add x86_64-unknown-linux-gnu
rustup target add x86_64-pc-windows-gnu
rustup target add aarch64-apple-darwin
rustup target add x86_64-apple-darwin
```

## Architecture

### Core Concept

The agent acts as a **separate, portable application** responsible for WireGuard interface management. It receives configuration from a main application via local socket/API and handles:

- Creating and managing WireGuard interfaces (e.g., `wg0`)
- Configuring peers, keys, routes, and DNS
- Connection health monitoring and rekeying
- Platform-specific TUN/TAP device management

### Communication Pattern

The main application signals the agent when `enable_wireguard = true`:
1. App sends configuration via control message (JSON/gRPC/REST)
2. Agent creates WireGuard tunnel
3. App routes traffic through the interface

### Configuration Schema

The agent accepts configuration in TOML format and control messages in JSON:

**TOML Configuration:**
```toml
[network.default]
enable_wireguard = true
interface = "wg0"
mtu = 1280
private_key_path = "/etc/aurabox/wireguard/private.key"
dns = ["10.100.0.2"]

[[network.peers]]
name = "runbeam-core"
public_key = "base64pubkey"
endpoint = "203.0.113.10:51820"
allowed_ips = ["10.42.0.0/16", "fd42:1::/48"]
persistent_keepalive_secs = 25
```

**JSON Control Message:**
```json
{
  "action": "connect",
  "interface": "wg0",
  "mtu": 1280,
  "dns": ["10.100.0.2"],
  "privateKeyPath": "/etc/aurabox/wireguard/private.key",
  "peers": [
    {
      "name": "runbeam-core",
      "publicKey": "base64pubkey",
      "endpoint": "203.0.113.10:51820",
      "allowedIps": ["10.42.0.0/16", "fd42:1::/48"],
      "keepaliveSecs": 25
    }
  ]
}
```

### Platform Implementations

| Platform   | Implementation                                    |
|------------|---------------------------------------------------|
| Linux      | `boringtun` or `wireguard-go` (userspace)        |
| Windows    | `Wintun` driver + `wireguard-go`                 |
| macOS      | `utun` interface via `wireguard-go`              |
| Docker     | Linux binary with `NET_ADMIN` + `IPC_LOCK` caps  |
| Kubernetes | Sidecar or DaemonSet with `NET_ADMIN` + `IPC_LOCK` |

### Deployment Modes

| Mode       | Description                                        |
|------------|----------------------------------------------------|
| Service    | Background daemon/OS service (systemd, Windows Service, LaunchDaemon) |
| Ephemeral  | Launched on-demand by main app, exits after teardown |
| Container  | Primary or sidecar container in Docker/K8s         |

### Security Considerations

- Private keys stored with `0600` permissions or injected via environment variables
- Agent drops privileges after TUN device creation
- `IPC_LOCK` capability prevents key material from swapping to disk
- Zero-downtime key rotation support
- Routes and DNS isolated to WireGuard interface

## Project Structure

```
wg-agent/
├── src/
│   └── main.rs          # Entry point (currently minimal)
├── docs/
│   └── architecture.md  # Comprehensive architecture documentation
├── Cargo.toml           # Rust project manifest
└── target/              # Build artifacts (gitignored)
```

**Note:** The project is in early development. The intended structure will include:
- `pkg/tun/` - TUN/TAP device management
- `pkg/control/` - Control API implementation
- `pkg/wireguard/` - WireGuard protocol handling
- `internal/platform/` - Platform-specific implementations

## Key Documentation

- **Architecture Overview:** `docs/architecture.md` - Contains detailed design decisions, deployment patterns, and platform-specific configuration examples
- **Configuration Examples:** See `docs/architecture.md` for systemd units, Windows Services, LaunchDaemons, Docker Compose, and Kubernetes manifests

## Notes

- This is a Rust Edition 2024 project
- Currently has no dependencies (to be added as implementation progresses)
- Requires elevated privileges (`NET_ADMIN`, `IPC_LOCK`) for WireGuard interface creation
- Designed to work with existing Runbeam ecosystem applications (Aurabox, JMIX)
