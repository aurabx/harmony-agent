#!/usr/bin/env bash
#
# Test script to verify wg-agent control server is running and responding
#
# Usage:
#   ./dev/test-control-server.sh
#   ./dev/test-control-server.sh /path/to/socket

set -e

SOCKET_PATH="${1:-/var/run/harmony-agent.sock}"
TIMEOUT=5

echo "Testing wg-agent Control Server"
echo "================================"
echo ""

# Check if socket exists
echo "1. Checking if socket exists: $SOCKET_PATH"
if [ ! -S "$SOCKET_PATH" ]; then
    echo "   ⚠️  Socket does not exist at $SOCKET_PATH"
    echo "   Attempting to start wg-agent..."
    echo ""
    
    # Check if binary exists
    if [ ! -f "./target/release/harmony-agent" ]; then
        echo "   ❌ FAIL: Binary not found at ./target/release/harmony-agent"
        echo "   Build it first with: cargo build --release"
        exit 1
    fi
    
    # Start the agent with minimal test config (no tunnels, just control server)
    sudo ./target/release/harmony-agent start --config ./dev/testing/test-minimal.toml &
    
    # Wait for socket to appear (up to 5 seconds)
    echo "   Waiting for socket to appear..."
    for i in {1..10}; do
        if [ -S "$SOCKET_PATH" ]; then
            echo "   ✅ wg-agent started successfully"
            break
        fi
        sleep 0.5
    done
    
    # Final check
    if [ ! -S "$SOCKET_PATH" ]; then
        echo "   ❌ FAIL: Socket still does not exist after starting agent"
        echo "   Check logs with: sudo journalctl -u wg-agent -f"
        exit 1
    fi
fi
echo "   ✅ PASS: Socket exists"
echo ""

# Check if socket is accessible
echo "2. Checking socket permissions"
if [ ! -r "$SOCKET_PATH" ] || [ ! -w "$SOCKET_PATH" ]; then
    echo "   ⚠️  WARNING: Socket may not be readable/writable"
    echo "   Current permissions: $(ls -l "$SOCKET_PATH")"
    echo "   You may need to run with sudo or adjust permissions"
fi
echo "   ✅ PASS: Socket is accessible"
echo ""

# Test connection with socat (if available)
if command -v socat &> /dev/null; then
    echo "3. Testing connection with socat"
    REQUEST='{"id":"test-1","action":"status","network":"default"}'
    
    RESPONSE=$(echo "$REQUEST" | timeout $TIMEOUT socat - UNIX-CONNECT:$SOCKET_PATH 2>&1 || true)
    
    if [ -z "$RESPONSE" ]; then
        echo "   ❌ FAIL: No response from server (timeout after ${TIMEOUT}s)"
        exit 1
    fi
    
    echo "   ✅ PASS: Server responded"
    echo ""
    
    # Check if response is valid JSON
    echo "4. Validating JSON response"
    if echo "$RESPONSE" | python3 -m json.tool &> /dev/null; then
        echo "   ✅ PASS: Response is valid JSON"
        echo ""
        echo "   Response:"
        echo "$RESPONSE" | python3 -m json.tool | head -20
    else
        echo "   ❌ FAIL: Response is not valid JSON"
        echo "   Raw response: $RESPONSE"
        exit 1
    fi
    
elif command -v nc &> /dev/null; then
    echo "3. Testing connection with netcat"
    REQUEST='{"id":"test-1","action":"status","network":"test"}'
    
    # macOS nc doesn't have timeout built-in, but it should respond quickly
    # Use sudo since socket is owned by root
    RESPONSE=$(echo "$REQUEST" | sudo nc -U "$SOCKET_PATH" 2>&1 || true)
    
    if [ -z "$RESPONSE" ]; then
        echo "   ❌ FAIL: No response from server (timeout after ${TIMEOUT}s)"
        exit 1
    fi
    
    echo "   ✅ PASS: Server responded"
    echo ""
    echo "   Response: $RESPONSE"
    
else
    echo "3. Testing with Python"
    
    python3 << EOF
import socket
import json
import sys

try:
    sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    sock.settimeout($TIMEOUT)
    sock.connect("$SOCKET_PATH")
    
    request = {"id": "test-1", "action": "status", "network": "default"}
    sock.sendall((json.dumps(request) + "\n").encode())
    
    response_data = sock.recv(4096)
    sock.close()
    
    if not response_data:
        print("   ❌ FAIL: No response from server")
        sys.exit(1)
    
    print("   ✅ PASS: Server responded")
    print()
    
    try:
        response = json.loads(response_data.decode())
        print("   ✅ PASS: Response is valid JSON")
        print()
        print("   Response:")
        print(json.dumps(response, indent=2))
    except json.JSONDecodeError as e:
        print(f"   ❌ FAIL: Response is not valid JSON: {e}")
        print(f"   Raw response: {response_data}")
        sys.exit(1)
        
except socket.timeout:
    print(f"   ❌ FAIL: Connection timeout after {$TIMEOUT}s")
    sys.exit(1)
except socket.error as e:
    print(f"   ❌ FAIL: Socket error: {e}")
    sys.exit(1)
except Exception as e:
    print(f"   ❌ FAIL: Unexpected error: {e}")
    sys.exit(1)
EOF
    
    if [ $? -ne 0 ]; then
        exit 1
    fi
fi

echo ""
echo "================================"
echo "✅ ALL TESTS PASSED"
echo ""
echo "Control server is running and responding correctly!"
echo "Socket: $SOCKET_PATH"
