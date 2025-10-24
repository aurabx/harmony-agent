#!/bin/bash
# Test script for wireguard-go on macOS

set -e

echo "==> Generating WireGuard config..."
cat > /tmp/wg-test.conf << EOF
[Interface]
PrivateKey = $(cat ./examples/full-setup/keys/client_private.key)
ListenPort = 0

[Peer]
PublicKey = u5GHYt7+NTnhaZP9dVEoJc/Oi1cYgx5V6Dt+Ab1+kkI=
Endpoint = 127.0.0.1:51820
AllowedIPs = 10.100.0.0/24
PersistentKeepalive = 25
EOF

echo "==> Starting wireguard-go..."
sudo wireguard-go -f utun9 &
WG_PID=$!
sleep 1

echo "==> Applying WireGuard config..."
sudo wg setconf utun9 /tmp/wg-test.conf

echo "==> Configuring interface..."
sudo ifconfig utun9 10.100.0.2 10.100.0.1 netmask 255.255.255.0 up
sudo route add -net 10.100.0.0/24 -interface utun9

echo "==> Interface status:"
ifconfig utun9

echo "==> WireGuard status:"
sudo wg show utun9

echo "==> Testing ping..."
ping -c 4 10.100.0.1

echo "==> Cleaning up..."
sudo kill $WG_PID 2>/dev/null || true
sudo ifconfig utun9 down 2>/dev/null || true
rm /tmp/wg-test.conf

echo "==> Done!"
