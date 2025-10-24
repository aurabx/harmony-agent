## macOS Development Setup

### Prerequisites

harmony-agent requires `wireguard-tools` on macOS for proper TUN device support:

```bash
brew install wireguard-tools
```

This installs both `wireguard-go` (userspace WireGuard implementation) and `wg`/`wg-quick` management tools.

### Why wireguard-tools?

macOS's `utun` devices require special handling that the Rust `tun` crate doesn't fully support. The `wireguard-go` implementation provides:
- Proper macOS TUN device integration
- Stable packet processing
- Full WireGuard protocol compatibility

**Note:** Linux and Windows builds use the native `boringtun` Rust implementation and don't require external dependencies.
