//! WireGuard protocol and tunnel management
//!
//! This module handles the WireGuard protocol implementation, key management,
//! and peer configuration using boringtun on Linux/Windows, and wireguard-go on macOS.

mod device;
mod keys;
mod peer;
mod tunnel;

#[cfg(target_os = "macos")]
mod macos_device;

pub use device::{DeviceConfig, DeviceStats, WgDevice};
pub use keys::{KeyPair, PrivateKey, PublicKey};
pub use peer::{Peer, PeerConfig, PeerStats};
pub use tunnel::{Tunnel, TunnelConfig, TunnelState};

#[cfg(target_os = "macos")]
pub use macos_device::MacOsWgDevice;
