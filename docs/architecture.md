# Cross-Platform WireGuard Agent Architecture

## Overview

This document describes a portable architecture for enabling WireGuard connectivity across **Linux**, **Windows**, **macOS**, **Docker**, and **Kubernetes**. The goal is to provide a consistent mechanism to enable WireGuard interfaces whenever `[network.default].enable_wireguard = true` is configured, without platform-specific dependencies or manual setup.

## Key Concept

Introduce a **Network Agent** as a separate but portable application responsible for managing the WireGuard interface. This isolates privileged network operations from the main application while maintaining a consistent control surface across all environments.

---

## Architecture

### Components

### 1. Network Agent

A small cross-platform binary that:

* Reads configuration files or receives control messages (JSON, gRPC, REST).
* Creates and manages a WireGuard interface (`wg0`).
* Configures peers, keys, routes, and DNS.
* Monitors connection health and performs rekeying.

It uses **user-space WireGuard** implementations:

* **Linux/macOS:** `wireguard-go` or `boringtun`
* **Windows:** `Wintun` + `wireguard-go`
* **Docker/K8s:** Reuse Linux binary in containerised mode.

### 2. Main Application

The main app (Harmony) does not handle networking directly. Instead, it communicates with the agent via a local socket or API.

When `enable_wireguard = true`:

1. The app signals the agent to bring up the interface.
2. The agent creates the WireGuard tunnel.
3. The app begins routing traffic through that interface.

---

## Configuration Schema from Harmony

```toml
[network.default]
enable_wireguard = true
interface = "wg0"
mtu = 1280
private_key_path = "/etc/aurabox/wireguard/private.key"

[network.default.http]
bind_address = "0.0.0.0"  # Listen on all interfaces
bind_port = 8081          # Use port 8081

[[network.default.peers]]
name = "runbeam-core"
public_key = "base64pubkey"
endpoint = "203.0.113.10:51820"
allowed_ips = ["10.42.0.0/16", "fd42:1::/48"]
persistent_keepalive_secs = 25
```

The application serialises this configuration into a control message:

```json
{
  "action": "connect",
  "interface": "wg0",
  "mtu": 1280,
  "dns": ["10.100.0.2"],
  "privateKeyPath": "/etc/aurabox/wireguard/private.key",
  "peers": [
    {
      "name": "runbeam-core",
      "publicKey": "base64pubkey",
      "endpoint": "203.0.113.10:51820",
      "allowedIps": ["10.42.0.0/16", "fd42:1::/48"],
      "keepaliveSecs": 25
    }
  ]
}
```

---

## Platform Integration

| Platform       | Implementation Notes                                                                                             |
| -------------- | ---------------------------------------------------------------------------------------------------------------- |
| **Linux**      | Use `boringtun` or `wireguard-go`. Optional kernel fallback if `wg` tools available. Run via systemd.            |
| **Windows**    | Use `Wintun` driver. Run as Windows Service. Manage routes via `netsh` or IP Helper API.                         |
| **macOS**      | Use `utun` interface via `wireguard-go`. Run as LaunchDaemon. Configure routes with `route` or `scutil`.         |
| **Docker**     | Ship Linux agent in container. Requires `NET_ADMIN` and `IPC_LOCK`. Shared network namespace or sidecar pattern. |
| **Kubernetes** | Deploy as sidecar or DaemonSet with `NET_ADMIN` and `IPC_LOCK`. Routes configured per pod or node.               |

---

## Deployment and Packaging

### Single Codebase

Build one agent binary per platform from a shared source tree:

```
cmd/harmony-agent/
pkg/tun/
pkg/control/
pkg/wireguard/
internal/platform/
```

Cross-compile with:

```bash
GOOS=linux   GOARCH=amd64 go build -o dist/linux/harmony-agent ./cmd/harmony-agent
GOOS=windows GOARCH=amd64 go build -o dist/windows/harmony-agent.exe ./cmd/harmony-agent
GOOS=darwin  GOARCH=arm64 go build -o dist/macos/harmony-agent ./cmd/harmony-agent
```

### Distribution Models

* **Linux/macOS/Windows Installer:** bundle `harmony-agent` with main app package.
* **Docker/K8s:** include the Linux binary in the image.
* **Advanced:** standalone agent for custom deployments.

Users effectively install *one product*, which internally contains both binaries.

---

## Runtime Modes

| Mode               | Description                                           |
| ------------------ | ----------------------------------------------------- |
| **Service mode**   | Runs as a background daemon or OS service.            |
| **Ephemeral mode** | Launched by main app on-demand, exits after teardown. |
| **Container mode** | Runs as primary or sidecar container.                 |

---

## Packaging Examples

### Linux systemd unit

```
[Unit]
Description=WG Agent
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/usr/local/bin/harmony-agent up --config /etc/aurabox/network.toml
Restart=on-failure
CapabilityBoundingSet=CAP_NET_ADMIN CAP_IPC_LOCK
AmbientCapabilities=CAP_NET_ADMIN CAP_IPC_LOCK
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```

### Windows Service

```
sc.exe create WgAgent binPath= "C:\\Program Files\\Aurabox\\harmony-agent.exe up --config C:\\Aurabox\\network.toml" start= auto
sc.exe description WgAgent "WireGuard Agent for Aurabox"
```

### macOS LaunchDaemon

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>cloud.aurabox.harmony-agent</string>
  <key>ProgramArguments</key>
  <array>
    <string>/usr/local/bin/harmony-agent</string>
    <string>up</string>
    <string>--config</string>
    <string>/etc/aurabox/network.toml</string>
  </array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
</dict></plist>
```

### Docker Compose

```yaml
version: "3.9"
services:
  app:
    image: aurabox/app:latest
    network_mode: "service:wg"
    depends_on: [wg]
  wg:
    image: aurabox/harmony-agent:latest
    cap_add: ["NET_ADMIN", "IPC_LOCK"]
    sysctls:
      net.ipv4.ip_forward: "1"
      net.ipv6.conf.all.forwarding: "1"
    volumes:
      - ./network.toml:/etc/aurabox/network.toml:ro
    command: ["up", "--config", "/etc/aurabox/network.toml"]
```

### Kubernetes Sidecar

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: aurabox-with-wg
spec:
  containers:
    - name: app
      image: aurabox/app:latest
    - name: harmony-agent
      image: aurabox/harmony-agent:latest
      args: ["up", "--config", "/etc/aurabox/network.toml"]
      securityContext:
        capabilities:
          add: ["NET_ADMIN", "IPC_LOCK"]
      volumeMounts:
        - name: wg-config
          mountPath: /etc/aurabox
          readOnly: true
  volumes:
    - name: wg-config
      secret:
        secretName: wg-config
```

---

## Security and Key Management

* Store private keys in `0600`-permission files or inject via environment variables.
* Support zero-downtime key rotation.
* Drop privileges after TUN creation.
* Use `IPC_LOCK` to prevent key data from being swapped to disk.
* Keep routes and DNS isolated to the interface.

---

## Observability

* **Health endpoint:** `/healthz` returns status, handshake age, and traffic stats.
* **Metrics:** Prometheus-compatible metrics for traffic, latency, and reconnections.
* **Logs:** Structured JSON for auditability.

---

## Summary

* A **separate, cross-platform agent** manages WireGuard securely and uniformly across all environments.
* Users install **one package** that includes both the main app and the agent.
* Containers and clusters reuse the same agent image.
* The design isolates privileges, reduces complexity, and supports full automation for secure connectivity.

---

## Optional Next Steps

1. Add CI build scripts to cross-compile the agent for all targets.
2. Package installers and Docker images.
3. Integrate API or gRPC control endpoint for dynamic network changes.
4. Implement observability and health checks for monitoring and automation.
