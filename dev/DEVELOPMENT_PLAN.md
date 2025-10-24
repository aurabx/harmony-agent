# WG-Agent Development Plan

**Version:** 1.0  
**Date:** 2025-10-23  
**Status:** Initial Planning  

## Executive Summary

This document outlines the development roadmap for **wg-agent**, a cross-platform WireGuard network agent written in Rust. The project will transform the current minimal implementation into a production-ready daemon capable of managing WireGuard tunnels across Linux, Windows, macOS, Docker, and Kubernetes environments.

## Current State

- **Project Phase:** Initial Setup
- **Implementation Status:** Minimal "Hello, world!" stub
- **Architecture:** Comprehensive documentation in `docs/architecture.md`
- **Tech Stack:** Rust Edition 2024
- **Dependencies:** None (baseline project)

## Goals & Objectives

### Primary Goals
1. **Cross-Platform Support:** Single codebase supporting Linux, Windows, macOS, Docker, and Kubernetes
2. **Security First:** Privilege isolation, secure key management, and minimal attack surface
3. **Production Ready:** Service/daemon integration, observability, and robust error handling
4. **Integration Ready:** Seamless integration with Runbeam ecosystem (Aurabox, JMIX)

### Success Criteria
- Successfully create and manage WireGuard tunnels on all target platforms
- Pass security audit for privilege management and key handling
- Achieve < 100ms control API response time
- Maintain < 50MB memory footprint in steady state
- Pass integration tests with Aurabox/JMIX applications

## Development Phases

### Phase 1: Project Foundation & Structure
**Priority:** Critical  
**Dependencies:** None

Create the foundational project structure and establish development standards.

**Deliverables:**
- Module structure: `config/`, `tun/`, `control/`, `platform/`, `wireguard/`, `service/`
- Core dependencies configured in `Cargo.toml`
- Error handling framework with `thiserror` and `anyhow`
- Structured logging with `tracing`
- Code quality standards (rustfmt, clippy)
- `lib.rs` and modularized `main.rs`

**Key Dependencies:**
```toml
tokio = { version = "1.41", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"
clap = { version = "4.5", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1.0"
thiserror = "1.0"
```

### Phase 2: Configuration Management
**Priority:** Critical  
**Dependencies:** Phase 1

Implement configuration parsing for both static TOML files and dynamic JSON control messages.

**Deliverables:**
- TOML configuration parser (`src/config/toml.rs`)
- JSON control message parser (`src/config/json.rs`)
- Configuration validation and schema enforcement
- Default configuration templates
- Configuration merging logic
- Comprehensive unit tests

**Configuration Schema:**
- Static: TOML with network settings, peers, and keys
- Dynamic: JSON control messages for runtime changes

### Phase 3: Platform Abstraction Layer
**Priority:** Critical  
**Dependencies:** Phase 1-2

Create platform-specific abstractions for TUN/TAP device management.

**Deliverables:**
- Platform trait definition (`src/platform/mod.rs`)
- Linux implementation with netlink and resolvconf
- Windows stub with Wintun driver preparation
- macOS stub with utun device management
- Platform detection and conditional compilation
- Route and DNS management per platform

**Platform Priority:**
1. Linux (primary development target)
2. macOS (development platform)
3. Windows (production target)

### Phase 4: WireGuard Integration
**Priority:** Critical  
**Dependencies:** Phase 1-3

Integrate WireGuard protocol implementation using boringtun.

**Deliverables:**
- WireGuard tunnel abstraction
- Key management with secure storage
- Peer management and lifecycle
- Handshake and keepalive logic
- Interface lifecycle (up/down/reload)
- Secure key file handling (0600 permissions)

**Dependencies:**
```toml
boringtun = "0.6"
base64 = "0.22"
x25519-dalek = "2.0"
```

### Phase 5: Control API Implementation
**Priority:** High  
**Dependencies:** Phase 1-4

Build the control interface for receiving commands from main applications.

**Deliverables:**
- Unix socket server (Linux/macOS)
- Named Pipe server (Windows)
- JSON-RPC API handler
- Command dispatcher and state machine
- Authentication/authorization
- Async command handling with tokio
- Client SDK examples

**API Commands:**
- `connect` - Establish WireGuard tunnel
- `disconnect` - Tear down tunnel
- `status` - Get connection status
- `reload` - Reload configuration
- `rotate_keys` - Perform key rotation

### Phase 6: Service/Daemon Implementation
**Priority:** High  
**Dependencies:** Phase 1-5

Implement proper daemon functionality for each platform.

**Deliverables:**
- Service abstraction layer
- Linux systemd integration with SD_NOTIFY
- Windows Service Control Manager integration
- macOS LaunchDaemon integration
- Ephemeral mode for containers
- Graceful shutdown handling
- System integration templates

**Service Files:**
- `wg-agent.service` (systemd)
- `cloud.runbeam.wg-agent.plist` (LaunchDaemon)
- Windows Service installer scripts

### Phase 7: Security Hardening
**Priority:** Critical  
**Dependencies:** Phase 1-6

Implement comprehensive security features.

**Deliverables:**
- Privilege dropping after TUN creation
- Secure key handling with memory zeroing
- File permission validation
- IPC_LOCK capability implementation
- Security audit logging
- Key rotation without disruption
- Input validation and sanitization
- Control API rate limiting

**Security Checklist:**
- [ ] Keys never logged
- [ ] Keys zeroed on drop
- [ ] Minimal privilege principle enforced
- [ ] All inputs validated
- [ ] Secure defaults
- [ ] No hardcoded credentials

### Phase 8: Monitoring & Observability
**Priority:** Medium  
**Dependencies:** Phase 1-7

Add monitoring, metrics, and health checking.

**Deliverables:**
- Health check endpoint (`/healthz`)
- Prometheus metrics exporter
- Structured logging with verbosity levels
- Connection state tracking
- Peer connectivity monitoring
- Diagnostic commands
- Optional OpenTelemetry tracing

**Metrics:**
- Connection uptime
- Handshake success/failure rate
- Bandwidth (tx/rx bytes)
- Peer latency
- Packet loss rate

### Phase 9: Testing Strategy
**Priority:** High  
**Dependencies:** Phase 1-8

Implement comprehensive testing.

**Deliverables:**
- Unit tests for all modules
- Integration tests suite
- Mock implementations
- Property-based testing with `proptest`
- Platform-specific test harness
- Docker-based test environment
- CI/CD pipeline (GitHub Actions)
- Performance benchmarks with `criterion`

**Test Coverage Target:** > 80%

### Phase 10: Cross-Compilation & Packaging
**Priority:** Medium  
**Dependencies:** Phase 1-9

Set up build infrastructure for all target platforms.

**Deliverables:**
- Cross-compilation toolchains configured
- Build scripts per platform
- GitHub Actions CI/CD workflows
- Distribution packages:
  - DEB/RPM (Linux)
  - MSI (Windows)
  - DMG/pkg (macOS)
  - Docker images (multi-arch)
- Version embedding

**Target Platforms:**
- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-pc-windows-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`

### Phase 11: Documentation & Examples
**Priority:** Medium  
**Dependencies:** Phase 1-10

Create comprehensive documentation.

**Deliverables:**
- User guide
- API reference documentation
- Inline code documentation (rustdoc)
- Deployment examples:
  - Docker Compose
  - Kubernetes (DaemonSet, Sidecar)
  - Systemd units
  - Windows Service installation
- Troubleshooting guide
- Quickstart tutorials
- Configuration examples
- Security best practices guide

### Phase 12: Integration & Release
**Priority:** High  
**Dependencies:** Phase 1-11

Final integration and release preparation.

**Deliverables:**
- Integration tests with Aurabox/JMIX
- Performance optimization and profiling
- Security audit and vulnerability scan
- Release checklist
- Semantic versioning setup
- CHANGELOG.md
- Automatic release pipeline
- Container registry publication
- Migration guide

## Technical Architecture

### Module Structure
```
wg-agent/
├── src/
│   ├── main.rs              # Entry point, CLI, daemon setup
│   ├── lib.rs               # Library root
│   ├── config/              # Configuration management
│   │   ├── mod.rs
│   │   ├── toml.rs          # TOML parser
│   │   └── json.rs          # JSON control message parser
│   ├── platform/            # Platform-specific implementations
│   │   ├── mod.rs           # Platform trait
│   │   ├── linux.rs
│   │   ├── windows.rs
│   │   └── macos.rs
│   ├── wireguard/           # WireGuard protocol
│   │   ├── mod.rs
│   │   ├── keys.rs
│   │   └── peers.rs
│   ├── control/             # Control API
│   │   ├── mod.rs
│   │   ├── unix.rs          # Unix socket server
│   │   ├── windows.rs       # Named pipe server
│   │   └── api.rs           # API handlers
│   ├── service/             # Service/daemon integration
│   │   ├── mod.rs
│   │   ├── linux.rs         # systemd
│   │   ├── windows.rs       # Windows Service
│   │   └── macos.rs         # LaunchDaemon
│   └── monitoring/          # Observability
│       ├── mod.rs
│       ├── health.rs
│       └── metrics.rs
├── tests/                   # Integration tests
├── benches/                 # Benchmarks
├── build/                   # Build scripts
├── docs/                    # Documentation
└── examples/                # Usage examples
```

### Key Design Patterns
- **Trait-based abstractions** for platform independence
- **Async I/O** with tokio runtime
- **Zero-copy** where possible for performance
- **Type-safe configuration** with serde
- **Error propagation** with anyhow/thiserror
- **Graceful degradation** for optional features

## Risk Management

### Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| WireGuard library compatibility issues | Medium | High | Use boringtun (pure Rust), fallback to wireguard-go |
| Platform-specific networking complexities | High | High | Incremental platform implementation, extensive testing |
| Privilege management complications | Medium | Critical | Early security audit, follow principle of least privilege |
| Performance bottlenecks | Low | Medium | Benchmarking in Phase 9, optimization in Phase 12 |
| Windows/macOS development environment gaps | Medium | Medium | Cross-compilation, CI/CD testing on target platforms |


## Dependencies & Prerequisites

### Development Environment
- Rust 1.85+ (Edition 2024)
- Cross-compilation toolchains
- Platform-specific SDKs:
  - Linux: libc, netlink libraries
  - Windows: Windows SDK, Wintun driver
  - macOS: Xcode Command Line Tools

### External Dependencies
- **boringtun**: WireGuard implementation
- **tokio**: Async runtime
- **serde**: Serialization
- **tracing**: Logging
- **clap**: CLI parsing

### Infrastructure
- GitHub repository with CI/CD
- Container registry for Docker images
- Package repositories (optional):
  - Debian/Ubuntu PPA
  - Homebrew tap
  - Chocolatey package

## Success Metrics

### Technical Metrics
- **Test Coverage:** > 80%
- **API Response Time:** < 100ms (p95)
- **Memory Footprint:** < 50MB (steady state)
- **Binary Size:** < 10MB (release build)
- **Startup Time:** < 500ms


### Quality Metrics
- Zero critical security vulnerabilities
- All platforms passing integration tests
- Documentation completeness > 95%

## Implementation Order

Phases should be implemented sequentially based on dependencies:

1. **Phase 1** → Foundation (no dependencies)
2. **Phase 2** → Configuration (requires Phase 1)
3. **Phase 3** → Platform Layer (requires Phase 1-2)
4. **Phase 4** → WireGuard (requires Phase 1-3)
5. **Phase 5** → Control API (requires Phase 1-4)
6. **Phase 6** → Service/Daemon (requires Phase 1-5)
7. **Phase 7** → Security (requires Phase 1-6)
8. **Phase 8** → Observability (requires Phase 1-7)
9. **Phase 9** → Testing (requires Phase 1-8)
10. **Phase 10** → Packaging (requires Phase 1-9)
11. **Phase 11** → Documentation (requires Phase 1-10)
12. **Phase 12** → Integration (requires Phase 1-11)

## Appendix

### References
- [WireGuard Protocol Specification](https://www.wireguard.com/protocol/)
- [boringtun Documentation](https://github.com/cloudflare/boringtun)
- [tokio Documentation](https://tokio.rs/)
- Internal: `docs/architecture.md`

### Change Log
- **2025-10-23:** Initial development plan created

