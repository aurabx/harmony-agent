# 1. Run with elevated privileges
sudo ./target/release/wg-agent start --config ./examples/privilege-test/config.toml

# 2. Monitor in another terminal
watch -n 1 'curl -s http://localhost:9090/metrics'

# 3. Check health
curl http://localhost:9090/healthz