# Testing Tools for wg-agent Control Server

## Overview

This document describes the testing tools available to verify the control server is running and accepting connections.

## Quick Reference

| Tool | Type | Use Case | Requirements | Status |
|------|------|----------|--------------|--------|
| `test-control-server.sh` | Shell script | Quick health check | Bash, Python 3 or socat | ✅ **Recommended** |
| `control_server_integration.rs` | Rust tests | Self-contained unit tests | None (starts own server) | ✅ Works |
| `control_server_test.rs` | Rust tests | Tests with running agent | Running agent | ⚠️ Requires `--ignored` flag |
| Example client | Rust binary | Manual testing | Running agent | ✅ Works |

## 1. Shell Script Test (Recommended)

**Location:** `dev/test-control-server.sh`

**Usage:**
```bash
# Test default socket
./dev/test-control-server.sh

# Test custom socket
./dev/test-control-server.sh /path/to/socket
```

**What it checks:**
1. Socket file exists
2. Socket has correct permissions
3. Server accepts connections
4. Server responds to requests
5. Response is valid JSON

**Exit codes:**
- `0` - All tests passed
- `1` - One or more tests failed

**Example output (passing):**
```
Testing wg-agent Control Server
================================

1. Checking if socket exists: /var/run/wg-agent.sock
   ✅ PASS: Socket exists

2. Checking socket permissions
   ✅ PASS: Socket is accessible

3. Testing connection with Python
   ✅ PASS: Server responded

   ✅ PASS: Response is valid JSON

   Response:
   {
     "id": "test-1",
     "success": false,
     "error": {
       "type": "network_not_found",
       "message": "default"
     }
   }

================================
✅ ALL TESTS PASSED

Control server is running and responding correctly!
Socket: /var/run/wg-agent.sock
```

**Example output (failing):**
```
Testing wg-agent Control Server
================================

1. Checking if socket exists: /var/run/wg-agent.sock
   ❌ FAIL: Socket does not exist at /var/run/wg-agent.sock

   Is wg-agent running?
   Try: sudo ./target/release/wg-agent start
```

## 2. Rust Integration Tests

**Location:** `tests/control_server_test.rs`

**Usage:**
```bash
# Run all control server tests
cargo test --test control_server_test -- --ignored --nocapture

# Run specific test
cargo test --test control_server_test test_control_server_connection -- --ignored --nocapture
```

**Available tests:**

| Test | Description |
|------|-------------|
| `test_control_server_socket_exists` | Verifies socket file exists and is a socket |
| `test_control_server_connection` | Tests basic connection to server |
| `test_control_server_status_request` | Sends status request and validates response |
| `test_control_server_network_not_found` | Tests error handling for non-existent networks |

**Note:** Tests are marked with `#[ignore]` because they require a running server. Use `-- --ignored` to run them.

## 3. Example Client

**Location:** `examples/control_client.rs`

**Usage:**
```bash
cargo run --example control_client
```

**What it does:**
- Connects to control socket
- Sends status request
- Receives and displays response
- Demonstrates proper client implementation

## 4. Manual Testing Tools

### Using socat

```bash
echo '{"id":"test-1","action":"status","network":"default"}' | \
  socat - UNIX-CONNECT:/var/run/wg-agent.sock
```

### Using nc (netcat)

```bash
echo '{"id":"test-1","action":"status","network":"default"}' | \
  nc -U /var/run/wg-agent.sock
```

### Using Python

```python
import socket
import json

sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect("/var/run/wg-agent.sock")

request = {"id": "test-1", "action": "status", "network": "default"}
sock.sendall((json.dumps(request) + "\n").encode())

response = json.loads(sock.recv(4096).decode())
print(json.dumps(response, indent=2))
sock.close()
```

### Using curl (if HTTP bridge exists)

Not currently supported. Control API is Unix socket only.

## Testing Workflow

### 1. Pre-deployment Testing

Before deploying to production:

```bash
# Build release binary
cargo build --release

# Create minimal config
mkdir -p tmp/test-config
echo "" > tmp/test-config/config.toml

# Start agent in background
sudo ./target/release/wg-agent -c tmp/test-config/config.toml start &

# Wait for startup
sleep 2

# Run shell script test
./dev/test-control-server.sh

# Run Rust integration tests
cargo test --test control_server_test -- --ignored

# Stop agent
sudo pkill wg-agent
```

### 2. CI/CD Pipeline

```yaml
# GitHub Actions example
jobs:
  test-control-server:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Build agent
        run: cargo build --release
      
      - name: Start agent
        run: |
          mkdir -p tmp/test-config
          echo "" > tmp/test-config/config.toml
          sudo ./target/release/wg-agent -c tmp/test-config/config.toml start &
          sleep 2
      
      - name: Test control server
        run: ./dev/test-control-server.sh
      
      - name: Run integration tests
        run: cargo test --test control_server_test -- --ignored
      
      - name: Cleanup
        if: always()
        run: sudo pkill wg-agent || true
```

### 3. Production Monitoring

For production systems, integrate with monitoring:

```bash
# Prometheus healthcheck
while true; do
  if ./dev/test-control-server.sh; then
    echo "control_server_health 1"
  else
    echo "control_server_health 0"
  fi | curl --data-binary @- http://pushgateway:9091/metrics/job/wg-agent
  sleep 60
done
```

## Troubleshooting

### Test fails immediately

**Symptom:**
```
❌ FAIL: Socket does not exist at /var/run/wg-agent.sock
```

**Solutions:**
1. Check if agent is running: `ps aux | grep wg-agent`
2. Start the agent: `sudo ./target/release/wg-agent start`
3. Check for startup errors in logs

### Permission denied

**Symptom:**
```
Socket error: [Errno 13] Permission denied
```

**Solutions:**
1. Run test with sudo: `sudo ./dev/test-control-server.sh`
2. Adjust socket permissions: `sudo chmod 666 /var/run/wg-agent.sock`
3. Add user to socket group

### Connection timeout

**Symptom:**
```
❌ FAIL: Connection timeout after 5s
```

**Solutions:**
1. Agent may be hung - check logs
2. High system load - check resources
3. Socket may be stale - restart agent

### Invalid JSON response

**Symptom:**
```
❌ FAIL: Response is not valid JSON
```

**Solutions:**
1. Server implementation bug - check logs
2. Incomplete response - may need longer timeout
3. Report issue with raw response data

## Best Practices

### Development

- Run `test-control-server.sh` after every build
- Use integration tests before commits
- Test with empty config AND real config

### CI/CD

- Always test in CI before merge
- Use timeout for hanging tests
- Cleanup processes in `finally` blocks

### Production

- Monitor socket health continuously
- Alert on test failures
- Log test results for debugging
- Have rollback plan if tests fail

## Related Documentation

- `CONTROL_API_TESTING.md` - Comprehensive testing guide
- `IMPLEMENTATION_SUMMARY.md` - Architecture details
- `../docs/API.md` - API reference
- `README.md` - General dev documentation
