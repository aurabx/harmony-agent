# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-01-25

### Added
- Initial release of harmony-agent (formerly wg-agent)
- Cross-platform WireGuard tunnel management for Linux, macOS, and Windows
- Control server with Unix socket API for configuration management
- TOML and JSON configuration support
- Platform-specific implementations for macOS (utun), Linux (boringtun/wireguard-go), and Windows (Wintun)
- Health monitoring and connection keepalive  
- Prometheus metrics endpoint for observability
- DNS configuration management
- Privileged operation isolation with capability dropping
- Comprehensive security hardening (key zeroing, memory locking, input validation)
- Service management support (systemd, launchd, Windows Service)
- Docker and Kubernetes deployment configurations

### Known Limitations
- Windows implementation requires further testing
- Integration tests for control server need refinement
- Key rotation mechanism not yet implemented
- Full Docker and Kubernetes patterns under active development

### Security
- Private keys never logged (Debug trait redaction)
- Memory zeroing on key drop using `zeroize` crate
- Secure file permissions (0600) for key files
- Input validation and sanitization throughout
- No plaintext secrets in configuration

### Usage
See README.md for installation and configuration instructions.
