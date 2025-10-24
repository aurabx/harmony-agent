# Testing Linux Implementation

This document describes how to test the harmony-agent Linux implementation, particularly when developing on macOS or other non-Linux platforms.

## Quick Start

```bash
# Run existing unit tests (works on any platform)
cargo test --lib

# Cross-compile check for Linux
rustup target add x86_64-unknown-linux-gnu
cargo check --target x86_64-unknown-linux-gnu

# Full testing requires Linux - see sections below
```

## Testing Approaches

### 1. Unit Tests (Cross-Platform)

Unit tests can run on any platform as they don't require privileged operations:

```bash
# Run all library tests
cargo test --lib

# Run specific module tests
cargo test --lib wireguard::
cargo test --lib platform::linux
cargo test --lib config::

# Run with verbose output
cargo test --lib -- --nocapture --test-threads=1

# Run specific test
cargo test --lib test_linux_platform_new
```

### 2. Docker-Based Testing (Recommended)

The most practical approach for testing Linux-specific code from macOS.

#### Setup

Create a test Dockerfile:

```bash
cat > dev/linux/Dockerfile.test <<'EOF'
FROM rust:1.81-bookworm

# Install system dependencies
RUN apt-get update && apt-get install -y \
    iproute2 \
    iptables \
    iputils-ping \
    wireguard-tools \
    resolvconf \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy project files
COPY . .

# Build the project
RUN cargo build --release

# Default command runs tests
CMD ["cargo", "test", "--lib"]
EOF
```

#### Build and Test

```bash
# Build test container
docker build -f dev/linux/Dockerfile.test -t harmony-agent-test .

# Run tests (requires privileged mode for network operations)
docker run --rm \
    --privileged \
    --cap-add=NET_ADMIN \
    --cap-add=IPC_LOCK \
    harmony-agent-test

# Run tests with output
docker run --rm \
    --privileged \
    --cap-add=NET_ADMIN \
    --cap-add=IPC_LOCK \
    harmony-agent-test \
    cargo test --lib -- --nocapture

# Interactive shell for debugging
docker run -it --rm \
    --privileged \
    --cap-add=NET_ADMIN \
    --cap-add=IPC_LOCK \
    harmony-agent-test \
    /bin/bash
```

### 3. Linux VM Testing

For more comprehensive testing in a real Linux environment.

#### Using Multipass (Ubuntu VMs)

```bash
# Install Multipass (if not already installed)
# macOS: brew install multipass

# Launch Ubuntu VM
multipass launch --name wg-test \
    --cpus 2 \
    --memory 2G \
    --disk 10G \
    22.04

# Transfer code to VM
multipass mount . wg-test:/home/ubuntu/harmony-agent

# Access VM
multipass shell wg-test

# Inside VM - setup Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Inside VM - install dependencies
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    iproute2 \
    iptables \
    wireguard-tools \
    resolvconf

# Inside VM - run tests
cd /home/ubuntu/harmony-agent
cargo test --lib
cargo build --release

# Cleanup when done
multipass delete wg-test
multipass purge
```

#### Using Vagrant

```bash
# Create Vagrantfile
cat > dev/linux/Vagrantfile <<'EOF'
Vagrant.configure("2") do |config|
  config.vm.box = "ubuntu/jammy64"
  config.vm.network "private_network", type: "dhcp"
  
  config.vm.provider "virtualbox" do |vb|
    vb.memory = "2048"
    vb.cpus = 2
  end
  
  config.vm.provision "shell", inline: <<-SHELL
    apt-get update
    apt-get install -y build-essential iproute2 iptables wireguard-tools resolvconf
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sudo -u vagrant sh
  SHELL
  
  config.vm.synced_folder "../..", "/home/vagrant/harmony-agent"
end
EOF

# Start VM
cd dev/linux
vagrant up

# SSH into VM
vagrant ssh

# Inside VM
cd harmony-agent
cargo test --lib
cargo build --release
```

### 4. Integration Testing with Real WireGuard

Test the agent with actual WireGuard configuration.

#### Setup Test Configuration

```bash
# Create test directory structure
mkdir -p ./tmp/test-config

# Generate WireGuard keys
wg genkey | tee ./tmp/test-config/private.key | wg pubkey > ./tmp/test-config/public.key
wg genkey | tee ./tmp/test-config/peer-private.key | wg pubkey > ./tmp/test-config/peer-public.key

# Get peer public key
PEER_PUBLIC_KEY=$(cat ./tmp/test-config/peer-public.key)

# Create agent configuration
cat > ./tmp/test-config/config.toml <<EOF
[network.test]
enable_wireguard = true
interface = "wg0"
address = "10.100.0.2/24"
mtu = 1420
private_key_path = "./tmp/test-config/private.key"
dns = ["1.1.1.1", "8.8.8.8"]

[[network.peers]]
name = "test-peer"
public_key = "$PEER_PUBLIC_KEY"
endpoint = "127.0.0.1:51820"
allowed_ips = ["10.100.0.0/24"]
persistent_keepalive_secs = 25
EOF

# Create peer configuration (for testing against)
AGENT_PUBLIC_KEY=$(cat ./tmp/test-config/public.key)
cat > ./tmp/test-config/peer.conf <<EOF
[Interface]
PrivateKey = $(cat ./tmp/test-config/peer-private.key)
Address = 10.100.0.1/24
ListenPort = 51820

[Peer]
PublicKey = $AGENT_PUBLIC_KEY
AllowedIPs = 10.100.0.2/32
EOF
```

#### Run Integration Tests (Linux Only)

```bash
# Build release version
cargo build --release

# Terminal 1: Start test peer
sudo wg-quick up ./tmp/test-config/peer.conf

# Terminal 2: Start agent with test config
sudo ./target/release/harmony-agent \
    -c ./tmp/test-config/config.toml \
    --verbose \
    start

# Terminal 3: Verify setup
# Check interface created
ip link show wg0
ip addr show wg0

# Check WireGuard status
sudo wg show wg0

# Check metrics endpoint
curl http://127.0.0.1:9090/healthz
curl http://127.0.0.1:9090/metrics

# Test connectivity (if peer is running)
ping -c 3 10.100.0.1

# Cleanup
sudo ./target/release/harmony-agent stop
sudo wg-quick down ./tmp/test-config/peer.conf
```

### 5. Cross-Compilation Verification

Verify Linux code compiles without running it:

```bash
# Add Linux target
rustup target add x86_64-unknown-linux-gnu

# Check compilation for Linux
cargo check --target x86_64-unknown-linux-gnu

# Build for Linux (binary won't run on macOS)
cargo build --release --target x86_64-unknown-linux-gnu

# The binary will be at:
# target/x86_64-unknown-linux-gnu/release/harmony-agent
```

### 6. Static Analysis and Code Quality

```bash
# Run clippy for Linux target
cargo clippy --target x86_64-unknown-linux-gnu -- -D warnings

# Format check
cargo fmt -- --check

# Generate and check documentation
cargo doc --no-deps --document-private-items --target x86_64-unknown-linux-gnu

# Check for common issues
cargo audit
```

## Test Scenarios

### Scenario 1: Basic Interface Creation

```bash
# Verify TUN device is created correctly
sudo ./target/release/harmony-agent -c config.toml start

# Check interface exists
ip link show wg0

# Expected output:
# X: wg0: <POINTOPOINT,NOARP,UP,LOWER_UP> mtu 1420 ...
```

### Scenario 2: Address Configuration

```bash
# Verify IP address is assigned
ip addr show wg0

# Expected output should include:
# inet 10.100.0.2/24 scope global wg0
```

### Scenario 3: Route Configuration

```bash
# Check routes are added
ip route show | grep wg0

# Expected output for peer allowed_ips:
# 10.100.0.0/24 dev wg0 scope link
```

### Scenario 4: DNS Configuration

```bash
# Check DNS is configured (if resolvconf available)
resolvconf -l wg0

# Expected output:
# nameserver 1.1.1.1
# nameserver 8.8.8.8
```

### Scenario 5: Metrics and Monitoring

```bash
# Health check
curl -s http://127.0.0.1:9090/healthz
# Expected: OK

# Metrics (Prometheus format)
curl -s http://127.0.0.1:9090/metrics | grep wg_

# Expected metrics:
# harmony_agent_info{version="0.1.0"} 1
# wg_network_state{network="test"} 2
# wg_bytes_transmitted{network="test"} ...
# wg_bytes_received{network="test"} ...
```

### Scenario 6: Graceful Shutdown

```bash
# Send SIGTERM
sudo kill -TERM $(pidof harmony-agent)

# Or Ctrl+C

# Verify cleanup:
ip link show wg0  # Should not exist
resolvconf -l wg0 # Should be empty or not exist
```

## Troubleshooting

### Common Issues

#### Permission Denied

```bash
# Error: Failed to create TUN device: Permission denied
# Solution: Run with sudo or CAP_NET_ADMIN
sudo ./target/release/harmony-agent start

# Or use capabilities:
sudo setcap cap_net_admin,cap_ipc_lock=eip ./target/release/harmony-agent
```

#### Interface Already Exists

```bash
# Error: Interface wg0 already exists
# Solution: Remove existing interface
sudo ip link delete wg0
```

#### Missing Dependencies

```bash
# Error: Command not found: ip
# Solution: Install iproute2
sudo apt-get install iproute2

# Error: resolvconf not found
sudo apt-get install resolvconf
```

### Debug Logging

```bash
# Enable verbose logging
sudo ./target/release/harmony-agent -v start

# Or set environment variable
export RUST_LOG=debug
sudo -E ./target/release/harmony-agent start

# Check specific modules
export RUST_LOG=harmony_agent::wireguard=trace,harmony_agent::platform::linux=debug
```

### Verify Platform Detection

```bash
# Check platform is detected correctly
cargo test --lib platform::detection::test_detect_environment -- --nocapture
```

## CI/CD Integration

Example GitHub Actions workflow for Linux testing:

```yaml
name: Linux Tests

on: [push, pull_request]

jobs:
  test-linux:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Install system dependencies
      run: |
        sudo apt-get update
        sudo apt-get install -y iproute2 iptables wireguard-tools resolvconf
    
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        override: true
    
    - name: Run tests
      run: cargo test --lib
    
    - name: Run clippy
      run: cargo clippy -- -D warnings
    
    - name: Build release
      run: cargo build --release
```

## Test Checklist

Before deploying to production:

- [ ] All unit tests pass: `cargo test --lib`
- [ ] No clippy warnings: `cargo clippy -- -D warnings`
- [ ] Code is formatted: `cargo fmt -- --check`
- [ ] Cross-compilation works: `cargo check --target x86_64-unknown-linux-gnu`
- [ ] Docker tests pass with privileged mode
- [ ] Integration test with real WireGuard peer succeeds
- [ ] Interface creation works
- [ ] IP address assignment works
- [ ] Routes are configured correctly
- [ ] DNS configuration works (if resolvconf available)
- [ ] Metrics endpoint responds
- [ ] Graceful shutdown cleans up properly
- [ ] Documentation is up to date

## Additional Resources

- [WireGuard Documentation](https://www.wireguard.com/quickstart/)
- [Linux iproute2 Manual](https://man7.org/linux/man-pages/man8/ip.8.html)
- [boringtun Documentation](https://github.com/cloudflare/boringtun)
- [Rust TUN Crate](https://docs.rs/tun/)
