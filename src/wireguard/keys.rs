//! WireGuard key management
//!
//! This module handles secure generation, storage, and usage of WireGuard
//! cryptographic keys using x25519.

use crate::error::{Result, WgAgentError};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::fmt;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};
use zeroize::Zeroizing;

/// WireGuard private key (32 bytes, x25519)
#[derive(Clone)]
pub struct PrivateKey {
    secret: Zeroizing<[u8; 32]>,
}

impl PrivateKey {
    /// Generate a new random private key
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(rand::rngs::OsRng);
        Self {
            secret: Zeroizing::new(secret.to_bytes()),
        }
    }

    /// Create a private key from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            secret: Zeroizing::new(bytes),
        }
    }

    /// Parse a private key from base64-encoded string
    pub fn from_base64(s: &str) -> Result<Self> {
        let decoded = BASE64
            .decode(s.trim())
            .map_err(|e| WgAgentError::Config(format!("Invalid base64 private key: {}", e)))?;

        if decoded.len() != 32 {
            return Err(WgAgentError::Config(format!(
                "Invalid private key length: expected 32 bytes, got {}",
                decoded.len()
            )));
        }

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&decoded);
        Ok(Self::from_bytes(bytes))
    }

    /// Load a private key from a file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        
        // Check file permissions (should be 0600 or stricter)
        #[cfg(unix)]
        {
            let metadata = fs::metadata(path).map_err(|e| {
                WgAgentError::Config(format!("Failed to read key file {:?}: {}", path, e))
            })?;
            let permissions = metadata.permissions();
            let mode = permissions.mode();
            
            // Check if file is readable by others
            if mode & 0o077 != 0 {
                return Err(WgAgentError::Permission(format!(
                    "Private key file {:?} has insecure permissions: {:o} (should be 0600)",
                    path, mode & 0o777
                )));
            }
        }

        let content = fs::read_to_string(path).map_err(|e| {
            WgAgentError::Config(format!("Failed to read private key file {:?}: {}", path, e))
        })?;

        Self::from_base64(content.trim())
    }

    /// Save the private key to a file with secure permissions (0600)
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let encoded = self.to_base64();

        // Create file with restricted permissions from the start
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|e| {
                WgAgentError::Config(format!("Failed to create key file {:?}: {}", path, e))
            })?;

        file.write_all(encoded.as_bytes()).map_err(|e| {
            WgAgentError::Config(format!("Failed to write key file {:?}: {}", path, e))
        })?;

        file.write_all(b"\n").map_err(|e| {
            WgAgentError::Config(format!("Failed to write newline to key file {:?}: {}", path, e))
        })?;

        Ok(())
    }

    /// Convert to base64-encoded string
    pub fn to_base64(&self) -> String {
        BASE64.encode(*self.secret)
    }

    /// Get the corresponding public key
    pub fn public_key(&self) -> PublicKey {
        let secret = StaticSecret::from(*self.secret);
        let public = X25519PublicKey::from(&secret);
        PublicKey {
            key: public.to_bytes(),
        }
    }

    /// Get raw bytes (for boringtun)
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.secret
    }
}

impl fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PrivateKey([REDACTED])")
    }
}

// Ensure private keys are never accidentally logged
impl fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// WireGuard public key (32 bytes, x25519)
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PublicKey {
    key: [u8; 32],
}

impl PublicKey {
    /// Create a public key from raw bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { key: bytes }
    }

    /// Parse a public key from base64-encoded string
    pub fn from_base64(s: &str) -> Result<Self> {
        let decoded = BASE64
            .decode(s.trim())
            .map_err(|e| WgAgentError::Config(format!("Invalid base64 public key: {}", e)))?;

        if decoded.len() != 32 {
            return Err(WgAgentError::Config(format!(
                "Invalid public key length: expected 32 bytes, got {}",
                decoded.len()
            )));
        }

        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&decoded);
        Ok(Self::from_bytes(bytes))
    }

    /// Convert to base64-encoded string
    pub fn to_base64(&self) -> String {
        BASE64.encode(self.key)
    }

    /// Get raw bytes (for boringtun)
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", self.to_base64())
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base64())
    }
}

/// WireGuard key pair (private + public)
#[derive(Clone)]
pub struct KeyPair {
    /// Private key
    pub private: PrivateKey,
    /// Public key (derived from private)
    pub public: PublicKey,
}

impl KeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Self {
        let private = PrivateKey::generate();
        let public = private.public_key();
        Self { private, public }
    }

    /// Create a key pair from a private key
    pub fn from_private(private: PrivateKey) -> Self {
        let public = private.public_key();
        Self { private, public }
    }

    /// Load a key pair from a private key file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let private = PrivateKey::from_file(path)?;
        Ok(Self::from_private(private))
    }
}

impl fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeyPair")
            .field("private", &"[REDACTED]")
            .field("public", &self.public)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_generate_keypair() {
        let keypair = KeyPair::generate();
        assert_eq!(keypair.private.as_bytes().len(), 32);
        assert_eq!(keypair.public.as_bytes().len(), 32);
    }

    #[test]
    fn test_private_key_to_base64() {
        let private = PrivateKey::generate();
        let base64_str = private.to_base64();
        assert_eq!(base64_str.len(), 44); // Base64 of 32 bytes
    }

    #[test]
    fn test_private_key_from_base64() {
        let private = PrivateKey::generate();
        let base64_str = private.to_base64();
        let restored = PrivateKey::from_base64(&base64_str).unwrap();
        assert_eq!(private.as_bytes(), restored.as_bytes());
    }

    #[test]
    fn test_public_key_derivation() {
        let private = PrivateKey::generate();
        let public1 = private.public_key();
        let public2 = private.public_key();
        assert_eq!(public1, public2);
    }

    #[test]
    fn test_public_key_base64() {
        let public = PrivateKey::generate().public_key();
        let base64_str = public.to_base64();
        let restored = PublicKey::from_base64(&base64_str).unwrap();
        assert_eq!(public, restored);
    }

    #[test]
    fn test_private_key_not_logged() {
        let private = PrivateKey::generate();
        let debug_str = format!("{:?}", private);
        assert!(debug_str.contains("REDACTED"));
        assert!(!debug_str.contains(&private.to_base64()));
    }

    #[test]
    fn test_save_and_load_private_key() {
        let private = PrivateKey::generate();
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Save the key
        private.save_to_file(path).unwrap();

        // Verify file permissions
        let metadata = fs::metadata(path).unwrap();
        let permissions = metadata.permissions();
        assert_eq!(permissions.mode() & 0o777, 0o600);

        // Load the key back
        let loaded = PrivateKey::from_file(path).unwrap();
        assert_eq!(private.as_bytes(), loaded.as_bytes());
    }

    #[test]
    fn test_invalid_base64() {
        assert!(PrivateKey::from_base64("invalid!@#$").is_err());
    }

    #[test]
    fn test_invalid_length() {
        let short_key = BASE64.encode([0u8; 16]);
        assert!(PrivateKey::from_base64(&short_key).is_err());
    }
}
