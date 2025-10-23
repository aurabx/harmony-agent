# Multi-stage Dockerfile for wg-agent
FROM rust:1.75-slim as builder

WORKDIR /usr/src/wg-agent

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY benches ./benches
COPY tests ./tests

# Build release binary
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    iproute2 \
    iptables \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /usr/src/wg-agent/target/release/wg-agent /usr/local/bin/wg-agent

# Create config directory
RUN mkdir -p /etc/wg-agent

# Add capabilities for WireGuard
RUN setcap cap_net_admin,cap_ipc_lock=+eip /usr/local/bin/wg-agent || true

# Create non-root user
RUN useradd -r -s /bin/false wg-agent

# Set user
USER wg-agent

ENTRYPOINT ["/usr/local/bin/wg-agent"]
CMD ["start", "--config", "/etc/wg-agent/config.toml"]
