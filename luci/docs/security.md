# Luci Security

## Current Security Posture

> **v0.1.0 — Auth token for IPC, localhost-only access. No encryption.**

This document describes the current security state and the planned security model.

---

## What Exists Now

### Local-only IPC Access

- The IPC server (port 9527) only listens on `127.0.0.1`
- Only processes on the same machine can send commands
- No remote access to the IPC port

### Auth Token

- Auto-generated UUID stored in config file
- Validated on every IPC request
- Requests without token allowed from localhost (defense-in-depth)
- Requests with invalid token rejected

### Process Isolation

- Jobs run as regular user processes (no containerization)
- No sandboxing — jobs have the same permissions as the daemon
- Jobs can access the filesystem, network, and system resources

### Shell Injection Prevention

- Custom jobs use direct exec (`Command::new(name).args()`) without shell interpretation
- Job names with shell metacharacters (`|`, `;`, `&`, `$`, etc.) are rejected
- Only the Shell job type uses shell interpretation (`cmd /C` on Windows, `sh -c` on Unix)

### Input Validation

- Memory pool size validated (0-1024 GB)
- Job name validated (not empty)
- Invite code validated (format check)
- Request size limited (10 MB max)

### Invite Codes

- 15-character base64url-encoded codes with dot separator (e.g., `vTyW.zitO.Tmad..Kzi`)
- Generated from UUID v4 (128 bits of randomness)
- Not cryptographically validated — just string comparison
- No expiration

### Firewall Auto-configuration

- Windows firewall rules auto-configured on daemon start
- Uses configurable ports (not hardcoded)
- Handles errors with proper logging

---

## Planned Security Model

### Phase 1: Node Authentication (Future)

```
Node A                          Node B
  │                               │
  │  JoinRequest + PublicKey_A    │
  │──────────────────────────────▶│
  │                               │  Verify signature
  │  JoinResponse + PublicKey_B   │
  │◀──────────────────────────────│
  │                               │
  │  Both nodes now trust each    │
  │  other's identity             │
```

- Each node generates an Ed25519 key pair on first run
- Public key stored in config, private key in secure storage
- JoinRequest signed with private key
- Bootstrap node verifies signature before accepting

### Phase 2: Transport Encryption (Future)

- TLS 1.3 for all node-to-node communication
- Self-signed certificates or Let's Encrypt for LAN
- Mutual TLS (mTLS) for node authentication

### Phase 3: Job Sandboxing (Future)

- **Option A**: Run jobs in separate OS user accounts
- **Option B**: Use containers (Docker/Podman)
- **Option C**: Use OS sandboxing (Windows Job Objects, Linux namespaces)
- Resource limits (CPU, RAM, disk, network) enforced per job

### Phase 4: Access Control (Future)

- Role-based access: Admin, User, Viewer
- Per-network authentication
- Audit logging

---

## Threat Model

### Current Threats (v0.1.0)

| Threat | Risk | Mitigation |
|--------|------|------------|
| Malicious local process sends IPC commands | Medium | Auth token required (but visible in config) |
| Network eavesdropping on port 9528 | Low | Only on LAN, no sensitive data yet |
| Malicious job executes harmful code | High | None — no sandboxing |
| Invite code brute-force | Low | 128 bits of entropy, impractical to brute-force |
| Denial of service via resource exhaustion | Medium | Jobs limited by scheduler (RAM/CPU requirements) |
| Shell injection via job name | Low | Metacharacters rejected, direct exec used |

### What's Protected

- Invite codes are random (not predictable)
- Config file has standard file permissions
- No remote access by default (IPC is localhost-only)
- Auth token validates IPC requests
- Shell injection prevented for custom jobs

### What's Not Protected

- Job scripts run with full user permissions
- Network traffic is unencrypted
- No rate limiting on IPC or network connections
- Auth token stored in plaintext config file

---

## Recommendations for Production

1. **Run daemon as a dedicated user** with minimal privileges
2. **Firewall port 9528** to only trusted machines
3. **Use VPN** (Tailscale, WireGuard) for WAN connections
4. **Monitor logs** for unexpected connections
5. **Don't run untrusted code** without sandboxing
6. **Keep nvidia-smi and sysinfo updated** for accurate resource detection

---

## Security Checklist

- [x] Auth token for IPC requests
- [x] Shell injection prevention
- [x] Input validation (memory pool size, job name)
- [x] Request size limit (10 MB)
- [x] Firewall auto-configuration
- [ ] Authenticate nodes with public/private keys
- [ ] Encrypt all network traffic (TLS)
- [ ] Implement job sandboxing
- [ ] Add rate limiting to IPC server
- [ ] Add rate limiting to network listener
- [ ] Validate invite codes cryptographically
- [ ] Add invite code expiration
- [ ] Implement audit logging
- [ ] Add role-based access control
- [ ] Scan for dependency vulnerabilities (`cargo audit`)
