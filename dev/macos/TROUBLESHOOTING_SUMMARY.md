# WireGuard Connectivity Troubleshooting Summary

## Problem
WireGuard tunnels using `boringtun` (Rust userspace implementation) on macOS were experiencing 100% packet loss despite successful handshakes.

## Root Cause
The `tun` crate v0.6 doesn't fully support macOS `utun` devices. Packets were being written to the TUN device, but the macOS kernel wasn't processing them correctly. This is specific to macOS - Linux and Windows work fine with boringtun.

## Investigation Findings

### What Was Working
- ✅ WireGuard handshakes established successfully
- ✅ Boringtun encrypted/decrypted packets correctly
- ✅ UDP communication between client and server
- ✅ Server configuration was correct
- ✅ Routing tables configured properly

### What Wasn't Working
- ❌ macOS network stack didn't see packets written to utun device
- ❌ ICMP echo replies weren't being generated
- ❌ Even self-ping on the interface failed

### Key Evidence
- Server showed 32% RX errors when receiving packets from boringtun client
- wg-agent logs showed "Wrote 84 bytes to TUN device" but Mac didn't respond
- Changing interface configuration (point-to-point destination) had no effect
- Firewall wasn't the issue

## Solution
Use `wireguard-go` on macOS instead of boringtun:

```bash
brew install wireguard-tools
```

### Test Results
Using `wireguard-go` with the same configuration:
- ✅ 4 packets transmitted, 4 packets received, 0.0% packet loss
- ✅ Round-trip time: 1.281/4.095/7.501 ms (min/avg/max)
- ✅ Handshake established immediately
- ✅ Bidirectional traffic working perfectly

## Implementation

### For macOS (Development)
- wg-agent uses `wireguard-go` subprocess
- Requires `brew install wireguard-tools`
- Config file generated and applied via `wg setconf`

### For Linux/Windows (Production)
- Continue using `boringtun` (no changes needed)
- No external dependencies required
- Full Rust implementation

## Files Created
- `src/wireguard/macos_device.rs` - macOS-specific WireGuard device implementation
- `test-wireguard-go.sh` - Test script demonstrating working setup
- `MACOS.md` - macOS installation documentation
- `TROUBLESHOOTING_SUMMARY.md` - This file

## Next Steps
1. Integrate `MacOsWgDevice` into the tunnel management system
2. Add conditional compilation to use `MacOsWgDevice` on macOS, `WgDevice` elsewhere
3. Test full wg-agent functionality with wireguard-go backend
4. Update CI/CD to handle platform-specific builds

## Lessons Learned
- Userspace TUN implementations are platform-specific
- macOS `utun` devices have unique requirements
- Testing on target platforms is critical for network tools
- wireguard-go is the reference implementation for a reason
- Hybrid approaches (boringtun + wireguard-go) are valid for cross-platform tools
