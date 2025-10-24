#!/bin/bash

echo "🔍 WireGuard Connection Verification"
echo "===================================="
echo ""

# Check if harmony-agent is running
if ! curl -s http://127.0.0.1:8080/metrics > /dev/null 2>&1; then
    echo "❌ harmony-agent is not running!"
    echo "   Start with: sudo ./target/release/harmony-agent"
    exit 1
fi

# Get metrics
echo "📊 harmony-agent Metrics:"
curl -s http://127.0.0.1:8080/metrics | grep -E "(tunnel_state|peer_)"
echo ""

# Check server status
echo "🐳 Docker Server Status:"
docker exec wg-test-server wg show wg0
echo ""

# Test connectivity
echo "🏓 Testing Connectivity:"
echo -n "Ping 10.100.0.1 (server): "
if ping -c 1 -W 2 10.100.0.1 > /dev/null 2>&1; then
    echo "✅ SUCCESS"
else
    echo "❌ FAILED"
fi
echo ""

# Check handshake time
echo "⏱️  Last Handshake:"
HANDSHAKE=$(curl -s http://127.0.0.1:8080/metrics | grep peer_last_handshake | awk '{print $2}')
if [ -n "$HANDSHAKE" ] && [ "$HANDSHAKE" != "0" ]; then
    AGE=$(($(date +%s) - $HANDSHAKE))
    echo "   ${AGE} seconds ago ✅"
else
    echo "   No handshake yet ❌"
fi
echo ""

# Transfer stats
echo "📈 Transfer Stats:"
docker exec wg-test-server wg show wg0 transfer
echo ""

echo "✅ Verification complete!"
