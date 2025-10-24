#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
WG_SERVER_DIR="$SCRIPT_DIR/wireguard-server"
KEYS_DIR="$SCRIPT_DIR/keys"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "🐳 Setting up Docker WireGuard test environment..."

# Start WireGuard server
cd "$WG_SERVER_DIR"
docker-compose up -d

echo "⏳ Waiting for server to initialize (10 seconds)..."
sleep 10

# Extract server config
echo "📋 Server configuration:"
docker exec wg-test-server cat /config/wg_confs/wg0.conf
echo ""

# Get server public key from private key in config
echo "🔑 Extracting server public key..."
SERVER_PRIVKEY=$(docker exec wg-test-server cat /config/wg_confs/wg0.conf | grep '^PrivateKey' | awk '{print $3}')
SERVER_PUBKEY=$(docker exec wg-test-server sh -c "echo '$SERVER_PRIVKEY' | wg pubkey")
echo "Server Public Key: $SERVER_PUBKEY"

# Generate client keys
echo ""
echo "🔑 Generating client keys..."
mkdir -p "$KEYS_DIR"

# Generate keys using the running server container
docker exec wg-test-server sh -c "wg genkey | tee /tmp/client_private.key | wg pubkey > /tmp/client_public.key"
docker cp wg-test-server:/tmp/client_private.key "$KEYS_DIR/client_private.key"
docker cp wg-test-server:/tmp/client_public.key "$KEYS_DIR/client_public.key"
chmod 600 "$KEYS_DIR/client_private.key"

CLIENT_PUBKEY=$(cat "$KEYS_DIR/client_public.key")
echo "Client Public Key: $CLIENT_PUBKEY"

# Add client as peer to server
echo ""
echo "🔗 Adding client as peer to server..."
docker exec wg-test-server wg set wg0 peer "$CLIENT_PUBKEY" allowed-ips 10.100.0.2/32

# Create harmony-agent config
echo ""
echo "📝 Creating harmony-agent config..."
cat > "$PROJECT_ROOT/config.toml" << EOF
listen_address = "127.0.0.1:8080"

[network.docker_test]
enable_wireguard = true
interface = "wg0"
mtu = 1280
private_key_path = "$KEYS_DIR/client_private.key"
listen_port = 0
dns = ["10.100.0.1"]

[[network.docker_test.peers]]
name = "docker-server"
public_key = "$SERVER_PUBKEY"
endpoint = "127.0.0.1:51820"
allowed_ips = ["10.100.0.0/24"]
persistent_keepalive_secs = 25
EOF

echo ""
echo "✅ Setup complete!"
echo ""
echo "📦 Next steps:"
echo "1. Build: cd $PROJECT_ROOT && cargo build --release"
echo "2. Run:   cd $PROJECT_ROOT && sudo ./target/release/harmony-agent"
echo "3. Test:  ./examples/verify-connection.sh"
echo "4. Ping:  ping 10.100.0.1"
echo ""
echo "🛑 To stop server: cd $WG_SERVER_DIR && docker-compose down"
