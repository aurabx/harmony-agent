# Integration Steps for macOS WireGuard Support

## Overview
We need to make `tunnel.rs` use `MacOsWgDevice` on macOS instead of `WgDevice` (boringtun).

## Option 1: Conditional Compilation (Simplest)

Modify `src/wireguard/tunnel.rs` around line 212:

### Before:
```rust
// Create WireGuard device (this creates TUN device and starts packet processing)
let device = match WgDevice::new(device_config, self.platform.as_ref()).await {
    Ok(d) => d,
    Err(e) => {
        error!("Failed to create WireGuard device: {}", e);
        *self.state.write().await = TunnelState::Error;
        return Err(e);
    }
};
```

### After:
```rust
// Create WireGuard device
#[cfg(target_os = "macos")]
let device = {
    use crate::wireguard::MacOsWgDevice;
    
    let mut device = match MacOsWgDevice::new(device_config).await {
        Ok(d) => d,
        Err(e) => {
            error!("Failed to create WireGuard device: {}", e);
            *self.state.write().await = TunnelState::Error;
            return Err(e);
        }
    };
    
    // Start wireguard-go and configure interface
    let address = self.config.address.as_ref().ok_or_else(|| {
        WgAgentError::Config("Address required for macOS WireGuard".to_string())
    })?;
    
    let routes: Vec<String> = self.config.peers
        .iter()
        .flat_map(|p| p.allowed_ips.clone())
        .collect();
    
    if let Err(e) = device.start(address, &routes).await {
        error!("Failed to start WireGuard device: {}", e);
        *self.state.write().await = TunnelState::Error;
        return Err(e);
    }
    
    device
};

#[cfg(not(target_os = "macos"))]
let device = match WgDevice::new(device_config, self.platform.as_ref()).await {
    Ok(d) => d,
    Err(e) => {
        error!("Failed to create WireGuard device: {}", e);
        *self.state.write().await = TunnelState::Error;
        return Err(e);
    }
};
```

## Option 2: Trait-Based (Cleaner, More Work)

Create a common trait that both devices implement:

```rust
// In src/wireguard/mod.rs
#[async_trait]
pub trait WgDeviceTrait {
    fn interface_name(&self) -> &str;
    async fn stats(&self) -> DeviceStats;
    async fn stop(self) -> Result<()>;
}

// Implement for both WgDevice and MacOsWgDevice
```

Then use `Box<dyn WgDeviceTrait>` in the tunnel.

## Recommendation

Use **Option 1** (conditional compilation) because:
- Simpler to implement
- No trait overhead
- Clear separation of platforms
- Easy to test on each platform

## After Integration

Once done, you can run wg-agent normally:
```bash
sudo ./target/release/wg-agent start --config ./config.toml
```

And it will automatically use wireguard-go on macOS, boringtun on Linux/Windows.

## Testing

After making these changes:
1. Build: `cargo build --release`
2. Run: `sudo ./target/release/wg-agent start --config ./config.toml`
3. Test: `ping -c 4 10.100.0.1`
4. Should see: 0% packet loss

## Files to Modify

1. `src/wireguard/tunnel.rs` - lines ~212-220 (device creation)
2. `src/wireguard/tunnel.rs` - lines ~285-290 (skip address/route config on macOS since `start()` handles it)
3. Possibly update `struct Tunnel` field at line ~144 to handle both device types
