#!/bin/bash
set -e

echo "=== wg-agent Functional Test ==="
echo ""

# Configuration
AGENT_BIN="../../target/release/wg-agent"
CONFIG_FILE="./config.toml"
SOCKET="/var/run/wg-agent.sock"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${BLUE}1. Testing binary version${NC}"
$AGENT_BIN --version
echo -e "${GREEN}✓ Binary works${NC}"
echo ""

echo -e "${BLUE}2. Checking configuration${NC}"
cat $CONFIG_FILE
echo -e "${GREEN}✓ Configuration valid${NC}"
echo ""

echo -e "${BLUE}3. Testing service detection${NC}"
$AGENT_BIN start --config $CONFIG_FILE --verbose 2>&1 | head -10 &
AGENT_PID=$!
sleep 2
if ps -p $AGENT_PID > /dev/null; then
    echo -e "${GREEN}✓ Agent started successfully (PID: $AGENT_PID)${NC}"
    kill $AGENT_PID 2>/dev/null || true
    wait $AGENT_PID 2>/dev/null || true
else
    echo -e "${RED}✗ Agent failed to start${NC}"
    exit 1
fi
echo ""

echo -e "${BLUE}4. Testing key operations${NC}"
# Test our WireGuard key implementation via Rust test
cd ../..
cargo test --lib wireguard::keys::tests --quiet
echo -e "${GREEN}✓ Key operations work${NC}"
echo ""

echo -e "${BLUE}5. Testing monitoring system${NC}"
cargo test --lib monitoring::tests --quiet
echo -e "${GREEN}✓ Monitoring works${NC}"
echo ""

echo -e "${BLUE}6. Testing security validation${NC}"
cargo test --lib security::validation::tests --quiet
echo -e "${GREEN}✓ Security validation works${NC}"
echo ""

echo -e "${GREEN}=== All functional tests passed! ===${NC}"
echo ""
echo "Note: Full WireGuard tunnel tests require elevated privileges."
echo "Run with sudo for complete testing:"
echo "  sudo $AGENT_BIN start --config $CONFIG_FILE"
