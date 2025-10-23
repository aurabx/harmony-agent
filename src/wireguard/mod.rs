//! WireGuard protocol and tunnel management
//!
//! This module handles the WireGuard protocol implementation, key management,
//! and peer configuration using boringtun.

mod keys;
mod peer;
mod tunnel;

pub use keys::{KeyPair, PrivateKey, PublicKey};
pub use peer::{Peer, PeerConfig, PeerStats};
pub use tunnel::{Tunnel, TunnelConfig, TunnelState};
