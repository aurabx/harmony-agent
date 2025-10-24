//! WireGuard device implementation
//!
//! This module manages the core WireGuard device, integrating boringtun for
//! the WireGuard protocol, TUN device for packet I/O, and UDP socket for
//! network communication.
//!
//! Architecture: Each WireGuard peer requires its own boringtun Tunn instance,
//! as Tunn represents a single pairwise tunnel. We manage multiple peers by
//! maintaining a collection of Tunn instances keyed by peer endpoint.

use crate::error::{Result, WgAgentError};
use crate::platform::Platform;
use crate::wireguard::{KeyPair, PeerConfig};
use boringtun::noise::{Tunn, TunnResult};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, UdpSocket};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket as TokioUdpSocket;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time;
use tracing::{debug, error, info, warn};
use tun::Device as TunDevice;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

/// Maximum packet size for WireGuard
const MAX_PACKET_SIZE: usize = 65535;

/// Buffer size for TUN device reads
const TUN_BUFFER_SIZE: usize = 2048;

/// Timer tick interval for WireGuard operations
const TIMER_TICK_INTERVAL: Duration = Duration::from_millis(250);

/// WireGuard device statistics
#[derive(Debug, Clone, Default)]
pub struct DeviceStats {
    /// Total bytes transmitted
    pub tx_bytes: u64,
    /// Total bytes received
    pub rx_bytes: u64,
    /// Total packets transmitted
    pub tx_packets: u64,
    /// Total packets received
    pub rx_packets: u64,
    /// Total errors encountered
    pub errors: u64,
    /// Last handshake time per peer
    pub peer_handshakes: HashMap<String, Instant>,
}

/// WireGuard device configuration
#[derive(Debug, Clone)]
pub struct DeviceConfig {
    /// Interface name
    pub interface: String,
    /// MTU value
    pub mtu: u16,
    /// Local keypair
    pub keypair: KeyPair,
    /// Listen port for UDP (0 = random)
    pub listen_port: u16,
    /// Peers configuration
    pub peers: Vec<PeerConfig>,
}

/// Commands for controlling the device
#[derive(Debug)]
enum DeviceCommand {
    /// Stop the device
    Stop,
    /// Add a new peer
    AddPeer(PeerConfig),
    /// Remove a peer by public key
    RemovePeer(X25519PublicKey),
}

/// Per-peer tunnel state
struct PeerTunnel {
    /// Peer name
    name: String,
    /// Peer's public key
    public_key: X25519PublicKey,
    /// Boringtun tunnel instance for this peer
    tunn: Tunn,
    /// Peer endpoint
    endpoint: Option<SocketAddr>,
    /// Last activity timestamp
    last_activity: Instant,
}

impl PeerTunnel {
    /// Create a new peer tunnel
    fn new(
        name: String,
        local_private: StaticSecret,
        peer_config: &PeerConfig,
        index: u32,
    ) -> Result<Self> {
        let peer_public = X25519PublicKey::from(*peer_config.public_key.as_bytes());
        
        let tunn = Tunn::new(
            local_private,
            peer_public,
            peer_config.preshared_key,
            peer_config.keepalive_interval.map(|d| d.as_secs() as u16),
            index,
            None, // No rate limiter for now
        )
        .map_err(|e| WgAgentError::WireGuard(format!("Failed to create Tunn for peer '{}': {}", name, e)))?;

        Ok(Self {
            name,
            public_key: peer_public,
            tunn,
            endpoint: peer_config.endpoint,
            last_activity: Instant::now(),
        })
    }
}

/// WireGuard device managing the tunnel
pub struct WgDevice {
    /// Device configuration
    config: DeviceConfig,
    /// Actual interface name (may differ from config.interface on macOS)
    actual_interface: String,
    /// TUN device for packet I/O (wrapped in Mutex for mut access)
    tun_device: Arc<Mutex<tun::platform::Device>>,
    /// UDP socket for network communication
    udp_socket: Arc<TokioUdpSocket>,
    /// Per-peer tunnel instances, keyed by peer public key
    peer_tunnels: Arc<RwLock<HashMap<X25519PublicKey, PeerTunnel>>>,
    /// Endpoint to public key mapping for fast lookup
    endpoint_map: Arc<RwLock<HashMap<SocketAddr, X25519PublicKey>>>,
    /// Device statistics
    stats: Arc<RwLock<DeviceStats>>,
    /// Command channel sender
    cmd_tx: mpsc::UnboundedSender<DeviceCommand>,
    /// Task handles for cleanup
    task_handles: Vec<JoinHandle<()>>,
}

impl WgDevice {
    /// Create a new WireGuard device
    pub async fn new(
        config: DeviceConfig,
        platform: &dyn Platform,
    ) -> Result<Self> {
        info!("Creating WireGuard device for interface: {}", config.interface);

        // Validate that we have at least one peer
        if config.peers.is_empty() {
            return Err(WgAgentError::Config(
                "At least one peer must be configured".to_string()
            ));
        }

        // Create TUN device using platform-specific implementation
        let tun_device = platform.create_tun_device(&config.interface, config.mtu)?;
        
        // Get the actual interface name (may differ on macOS)
        let actual_interface = tun_device.name().map_err(|e| {
            WgAgentError::TunDevice(format!("Failed to get TUN device name: {}", e))
        })?;
        
        // Make TUN device non-blocking for async I/O
        tun_device.set_nonblock().map_err(|e| {
            WgAgentError::TunDevice(format!("Failed to set TUN device to non-blocking: {}", e))
        })?;

        // Wrap in Arc<Mutex> for concurrent access
        let tun_device = Arc::new(Mutex::new(tun_device));

        // Create UDP socket for WireGuard communication
        let listen_addr: SocketAddr = format!("0.0.0.0:{}", config.listen_port)
            .parse()
            .map_err(|e| WgAgentError::Config(format!("Invalid listen port: {}", e)))?;

        let std_socket = UdpSocket::bind(listen_addr).map_err(|e| {
            WgAgentError::Platform(format!("Failed to bind UDP socket to {}: {}", listen_addr, e))
        })?;

        std_socket.set_nonblocking(true).map_err(|e| {
            WgAgentError::Platform(format!("Failed to set UDP socket to non-blocking: {}", e))
        })?;

        let udp_socket = Arc::new(TokioUdpSocket::from_std(std_socket).map_err(|e| {
            WgAgentError::Platform(format!("Failed to create tokio UdpSocket: {}", e))
        })?);

        let actual_port = udp_socket.local_addr().map_err(|e| {
            WgAgentError::Platform(format!("Failed to get UDP socket local address: {}", e))
        })?.port();

        info!(
            "UDP socket listening on port {} (requested: {})",
            actual_port, config.listen_port
        );

        // Convert our private key to x25519 StaticSecret
        let local_private = StaticSecret::from(*config.keypair.private.as_bytes());

        // Create peer tunnels
        let mut peer_tunnels = HashMap::new();
        let mut endpoint_map = HashMap::new();

        for (index, peer_config) in config.peers.iter().enumerate() {
            let peer_tunnel = PeerTunnel::new(
                peer_config.name.clone(),
                local_private.clone(),
                peer_config,
                index as u32,
            )?;

            if let Some(endpoint) = peer_config.endpoint {
                endpoint_map.insert(endpoint, peer_tunnel.public_key);
            }

            info!("Created tunnel for peer: {}", peer_config.name);
            peer_tunnels.insert(peer_tunnel.public_key, peer_tunnel);
        }

        let peer_tunnels = Arc::new(RwLock::new(peer_tunnels));
        let endpoint_map = Arc::new(RwLock::new(endpoint_map));

        // Create command channel
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

        let stats = Arc::new(RwLock::new(DeviceStats::default()));

        info!("WireGuard device created successfully with {} peers", config.peers.len());

        let mut device = Self {
            config,
            actual_interface,
            tun_device,
            udp_socket,
            peer_tunnels,
            endpoint_map,
            stats,
            cmd_tx,
            task_handles: Vec::new(),
        };

        // Start packet processing tasks
        device.start_tasks(cmd_rx).await;

        Ok(device)
    }

    /// Start all packet processing tasks
    async fn start_tasks(&mut self, cmd_rx: mpsc::UnboundedReceiver<DeviceCommand>) {
        // Spawn outbound task (TUN -> encrypt -> UDP)
        let outbound_handle = {
            let tun_device = Arc::clone(&self.tun_device);
            let udp_socket = Arc::clone(&self.udp_socket);
            let peer_tunnels = Arc::clone(&self.peer_tunnels);
            let stats = Arc::clone(&self.stats);

            tokio::spawn(async move {
                Self::outbound_task(tun_device, udp_socket, peer_tunnels, stats).await;
            })
        };

        // Spawn inbound task (UDP -> decrypt -> TUN)
        let inbound_handle = {
            let tun_device = Arc::clone(&self.tun_device);
            let udp_socket = Arc::clone(&self.udp_socket);
            let peer_tunnels = Arc::clone(&self.peer_tunnels);
            let endpoint_map = Arc::clone(&self.endpoint_map);
            let stats = Arc::clone(&self.stats);

            tokio::spawn(async move {
                Self::inbound_task(tun_device, udp_socket, peer_tunnels, endpoint_map, stats).await;
            })
        };

        // Spawn timer task for keepalive and rekey
        let timer_handle = {
            let udp_socket = Arc::clone(&self.udp_socket);
            let peer_tunnels = Arc::clone(&self.peer_tunnels);
            let stats = Arc::clone(&self.stats);

            tokio::spawn(async move {
                Self::timer_task(udp_socket, peer_tunnels, stats).await;
            })
        };

        // Spawn command handler task
        let command_handle = {
            let peer_tunnels = Arc::clone(&self.peer_tunnels);
            let endpoint_map = Arc::clone(&self.endpoint_map);
            let local_private = StaticSecret::from(*self.config.keypair.private.as_bytes());

            tokio::spawn(async move {
                Self::command_task(cmd_rx, peer_tunnels, endpoint_map, local_private).await;
            })
        };

        self.task_handles.push(outbound_handle);
        self.task_handles.push(inbound_handle);
        self.task_handles.push(timer_handle);
        self.task_handles.push(command_handle);

        info!("All packet processing tasks started");
    }

    /// Outbound packet processing: TUN -> encrypt -> UDP
    async fn outbound_task(
        tun_device: Arc<Mutex<tun::platform::Device>>,
        udp_socket: Arc<TokioUdpSocket>,
        peer_tunnels: Arc<RwLock<HashMap<X25519PublicKey, PeerTunnel>>>,
        stats: Arc<RwLock<DeviceStats>>,
    ) {
        info!("Outbound task started");
        let mut tun_buffer = vec![0u8; TUN_BUFFER_SIZE];
        let mut wg_buffer = vec![0u8; MAX_PACKET_SIZE];

        loop {
            // Read from TUN device
            let n = {
                let mut device_guard = tun_device.lock().await;
                match device_guard.read(&mut tun_buffer) {
                    Ok(n) => n,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        drop(device_guard);
                        time::sleep(Duration::from_millis(10)).await;
                        continue;
                    }
                    Err(e) => {
                        error!("TUN read error: {}", e);
                        stats.write().await.errors += 1;
                        continue;
                    }
                }
            };

            if n == 0 {
                continue;
            }

            debug!("Read {} bytes from TUN device", n);

            // Try to encapsulate with each peer until one succeeds
            let mut peer_tunnels_guard = peer_tunnels.write().await;
            let mut sent = false;

            for peer_tunnel in peer_tunnels_guard.values_mut() {
                match peer_tunnel.tunn.encapsulate(&tun_buffer[..n], &mut wg_buffer) {
                    TunnResult::Done => {
                        debug!("Packet encapsulated (no output) for peer {}", peer_tunnel.name);
                    }
                    TunnResult::Err(e) => {
                        debug!("Encapsulation error for peer {}: {:?}", peer_tunnel.name, e);
                    }
                    TunnResult::WriteToNetwork(data) => {
                        // Send encrypted packet over UDP
                        if let Some(endpoint) = peer_tunnel.endpoint {
                            match udp_socket.send_to(data, endpoint).await {
                                Ok(sent_bytes) => {
                                    debug!("Sent {} bytes to {} (peer: {})", sent_bytes, endpoint, peer_tunnel.name);
                                    let mut stats_guard = stats.write().await;
                                    stats_guard.tx_bytes += sent_bytes as u64;
                                    stats_guard.tx_packets += 1;
                                    peer_tunnel.last_activity = Instant::now();
                                    sent = true;
                                    break;
                                }
                                Err(e) => {
                                    warn!("UDP send error to {}: {}", endpoint, e);
                                    stats.write().await.errors += 1;
                                }
                            }
                        }
                    }
                    TunnResult::WriteToTunnelV4(_, _) | TunnResult::WriteToTunnelV6(_, _) => {
                        debug!("Unexpected WriteToTunnel result in outbound path");
                    }
                }
            }

            if !sent {
                debug!("Packet not sent - no peer could encapsulate");
            }
        }
    }

    /// Inbound packet processing: UDP -> decrypt -> TUN
    async fn inbound_task(
        tun_device: Arc<Mutex<tun::platform::Device>>,
        udp_socket: Arc<TokioUdpSocket>,
        peer_tunnels: Arc<RwLock<HashMap<X25519PublicKey, PeerTunnel>>>,
        endpoint_map: Arc<RwLock<HashMap<SocketAddr, X25519PublicKey>>>,
        stats: Arc<RwLock<DeviceStats>>,
    ) {
        info!("Inbound task started");
        let mut udp_buffer = vec![0u8; MAX_PACKET_SIZE];
        let mut tun_buffer = vec![0u8; MAX_PACKET_SIZE];

        loop {
            // Read from UDP socket
            let (n, src) = match udp_socket.recv_from(&mut udp_buffer).await {
                Ok((n, src)) => (n, src),
                Err(e) => {
                    error!("UDP recv error: {}", e);
                    stats.write().await.errors += 1;
                    time::sleep(Duration::from_millis(100)).await;
                    continue;
                }
            };

            debug!("Received {} bytes from {}", n, src);

            // Look up which peer this is from
            let endpoint_map_guard = endpoint_map.read().await;
            let peer_key = endpoint_map_guard.get(&src).copied();
            drop(endpoint_map_guard);

            let peer_key = match peer_key {
                Some(key) => key,
                None => {
                    debug!("Received packet from unknown endpoint: {}", src);
                    continue;
                }
            };

            // Decrypt packet with the peer's tunnel
            let mut peer_tunnels_guard = peer_tunnels.write().await;
            let peer_tunnel = match peer_tunnels_guard.get_mut(&peer_key) {
                Some(pt) => pt,
                None => {
                    debug!("Peer tunnel not found for endpoint: {}", src);
                    continue;
                }
            };

            match peer_tunnel.tunn.decapsulate(Some(src.ip()), &udp_buffer[..n], &mut tun_buffer) {
                TunnResult::Done => {
                    debug!("Packet decapsulated (no output)");
                }
                TunnResult::Err(e) => {
                    warn!("Decapsulation error from {}: {:?}", src, e);
                    stats.write().await.errors += 1;
                }
                TunnResult::WriteToNetwork(data) => {
                    // Response packet to send back
                    match udp_socket.send_to(data, src).await {
                        Ok(sent) => {
                            debug!("Sent response {} bytes to {}", sent, src);
                            let mut stats_guard = stats.write().await;
                            stats_guard.tx_bytes += sent as u64;
                            stats_guard.tx_packets += 1;
                            peer_tunnel.last_activity = Instant::now();
                        }
                        Err(e) => {
                            warn!("UDP send error to {}: {}", src, e);
                            stats.write().await.errors += 1;
                        }
                    }
                }
                TunnResult::WriteToTunnelV4(data, _) | TunnResult::WriteToTunnelV6(data, _) => {
                    // Write decrypted packet to TUN device
                    drop(peer_tunnels_guard); // Release lock before waiting for TUN

                    let mut device_guard = tun_device.lock().await;
                    match device_guard.write(data) {
                        Ok(written) => {
                            debug!("Wrote {} bytes to TUN device", written);
                            let mut stats_guard = stats.write().await;
                            stats_guard.rx_bytes += written as u64;
                            stats_guard.rx_packets += 1;
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            debug!("TUN write would block");
                        }
                        Err(e) => {
                            error!("TUN write error: {}", e);
                            stats.write().await.errors += 1;
                        }
                    }
                    drop(device_guard);

                    // Continue to next iteration
                    continue;
                }
            }
        }
    }

    /// Timer task for keepalive and rekey operations
    async fn timer_task(
        udp_socket: Arc<TokioUdpSocket>,
        peer_tunnels: Arc<RwLock<HashMap<X25519PublicKey, PeerTunnel>>>,
        stats: Arc<RwLock<DeviceStats>>,
    ) {
        info!("Timer task started");
        let mut interval = time::interval(TIMER_TICK_INTERVAL);
        let mut wg_buffer = vec![0u8; MAX_PACKET_SIZE];

        loop {
            interval.tick().await;

            let mut peer_tunnels_guard = peer_tunnels.write().await;
            
            for peer_tunnel in peer_tunnels_guard.values_mut() {
                match peer_tunnel.tunn.update_timers(&mut wg_buffer) {
                    TunnResult::Done => {
                        // No action needed
                    }
                    TunnResult::Err(e) => {
                        debug!("Timer update error for peer {}: {:?}", peer_tunnel.name, e);
                    }
                    TunnResult::WriteToNetwork(data) => {
                        // Send keepalive or rekey packet
                        if let Some(endpoint) = peer_tunnel.endpoint {
                            match udp_socket.send_to(data, endpoint).await {
                                Ok(sent) => {
                                    debug!("Sent timer packet {} bytes to {} (peer: {})", sent, endpoint, peer_tunnel.name);
                                    let mut stats_guard = stats.write().await;
                                    stats_guard.tx_bytes += sent as u64;
                                    drop(stats_guard);
                                    peer_tunnel.last_activity = Instant::now();
                                }
                                Err(e) => {
                                    warn!("UDP send error in timer task: {}", e);
                                    stats.write().await.errors += 1;
                                }
                            }
                        }
                    }
                    TunnResult::WriteToTunnelV4(_, _) | TunnResult::WriteToTunnelV6(_, _) => {
                        debug!("Unexpected WriteToTunnel result in timer task");
                    }
                }
            }
        }
    }

    /// Command processing task
    async fn command_task(
        mut cmd_rx: mpsc::UnboundedReceiver<DeviceCommand>,
        peer_tunnels: Arc<RwLock<HashMap<X25519PublicKey, PeerTunnel>>>,
        endpoint_map: Arc<RwLock<HashMap<SocketAddr, X25519PublicKey>>>,
        local_private: StaticSecret,
    ) {
        info!("Command task started");
        let mut next_index = 1000u32; // Start peer indices at 1000

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                DeviceCommand::Stop => {
                    info!("Received stop command");
                    break;
                }
                DeviceCommand::AddPeer(peer_config) => {
                    info!("Adding peer: {}", peer_config.name);
                    
                    match PeerTunnel::new(
                        peer_config.name.clone(),
                        local_private.clone(),
                        &peer_config,
                        next_index,
                    ) {
                        Ok(peer_tunnel) => {
                            let public_key = peer_tunnel.public_key;
                            
                            if let Some(endpoint) = peer_config.endpoint {
                                endpoint_map.write().await.insert(endpoint, public_key);
                            }

                            peer_tunnels.write().await.insert(public_key, peer_tunnel);
                            next_index += 1;
                            info!("Peer '{}' added successfully", peer_config.name);
                        }
                        Err(e) => {
                            error!("Failed to add peer '{}': {}", peer_config.name, e);
                        }
                    }
                }
                DeviceCommand::RemovePeer(public_key) => {
                    info!("Removing peer with public key");
                    
                    if let Some(removed) = peer_tunnels.write().await.remove(&public_key) {
                        if let Some(endpoint) = removed.endpoint {
                            endpoint_map.write().await.remove(&endpoint);
                        }
                        info!("Peer '{}' removed successfully", removed.name);
                    } else {
                        warn!("Peer not found for removal");
                    }
                }
            }
        }

        info!("Command task stopped");
    }

    /// Get the actual interface name
    pub fn interface_name(&self) -> &str {
        &self.actual_interface
    }

    /// Get device statistics
    pub async fn stats(&self) -> DeviceStats {
        self.stats.read().await.clone()
    }

    /// Add a peer dynamically
    pub async fn add_peer(&self, peer: PeerConfig) -> Result<()> {
        self.cmd_tx
            .send(DeviceCommand::AddPeer(peer))
            .map_err(|e| WgAgentError::WireGuard(format!("Failed to send AddPeer command: {}", e)))
    }

    /// Remove a peer dynamically
    pub async fn remove_peer(&self, public_key: &crate::wireguard::PublicKey) -> Result<()> {
        let x25519_key = X25519PublicKey::from(*public_key.as_bytes());
        self.cmd_tx
            .send(DeviceCommand::RemovePeer(x25519_key))
            .map_err(|e| {
                WgAgentError::WireGuard(format!("Failed to send RemovePeer command: {}", e))
            })
    }

    /// Stop the device and clean up
    pub async fn stop(mut self) -> Result<()> {
        info!("Stopping WireGuard device");

        // Send stop command
        let _ = self.cmd_tx.send(DeviceCommand::Stop);

        // Wait for all tasks to complete (with timeout)
        let timeout = Duration::from_secs(5);
        let results = time::timeout(timeout, async {
            for handle in self.task_handles.drain(..) {
                let _ = handle.await;
            }
        })
        .await;

        if results.is_err() {
            warn!("Timeout waiting for tasks to stop, aborting remaining tasks");
        }

        info!("WireGuard device stopped");
        Ok(())
    }
}
