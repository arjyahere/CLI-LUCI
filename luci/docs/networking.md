# Luci Networking

## Overview

Luci uses three network ports for communication:

| Port | Protocol | Purpose | Status |
|------|----------|---------|--------|
| 9527 | TCP | CLI ↔ Daemon IPC | ✅ Working |
| 9528 | TCP | Node ↔ Node communication | ✅ Working |
| 9529 | UDP | Node discovery (LAN broadcast) | ✅ Working |

---

## Port 9527: CLI ↔ Daemon IPC

**Direction**: CLI → Daemon (localhost only)
**Transport**: TCP
**Framing**: 4-byte little-endian length prefix + JSON payload

### Wire Format

```
Request:
┌──────────────────┬──────────────────────────────┐
│ Length (4 bytes)  │ JSON IpcRequest              │
│ u32 LE            │ UTF-8 encoded                │
└──────────────────┴──────────────────────────────┘

Response:
┌──────────────────┬──────────────────────────────┐
│ Length (4 bytes)  │ JSON IpcResponse             │
│ u32 LE            │ UTF-8 encoded                │
└──────────────────┴──────────────────────────────┘
```

### Example Exchange

```
Client → Server:
00 00 00 00  ← length = 36 bytes
{"command":{"type":"Ping"}}

Server → Client:
00 00 00 00  ← length = 52 bytes
{"type":"Success","payload":{"status":"ok","pid":12345}}
```

### Implementation

- **Client**: `crates/luci-cli/src/ipc_client.rs` — synchronous `TcpStream`
- **Server**: `crates/lucid/src/ipc_server.rs` — async `TcpListener` with tokio

### Max Message Size

10 MB. Messages larger than this are rejected.

### Auth Token

Requests without an auth token are allowed from localhost (defense-in-depth). Requests with an invalid auth token are rejected.

---

## Port 9528: Node ↔ Node Communication

**Direction**: Daemon ↔ Daemon (all interfaces)
**Transport**: TCP
**Framing**: `LengthPrefixedCodec<ProtocolMessage>` (tokio-util codec)

### Wire Format

```
Message:
┌──────────────────┬──────────────────────────────┐
│ Length (4 bytes)  │ JSON ProtocolMessage         │
│ u32 LE            │ UTF-8 encoded                │
└──────────────────┴──────────────────────────────┘
```

### ProtocolMessage Structure

```json
{
  "id": "uuid-v4",
  "msg_type": "Heartbeat",
  "sender_id": "node-uuid",
  "timestamp": "2026-06-30T18:00:00.000Z",
  "payload": {
    "type": "Heartbeat",
    "resources": { ... },
    "status": "online"
  }
}
```

### Message Types (24 total)

#### Discovery
| Type | Payload | Direction | Purpose |
|------|---------|-----------|---------|
| `Ping` | `Ping` | Any → Any | Check if node is alive |
| `Pong` | `Pong { node_name }` | Response | Reply to Ping |
| `Announce` | `Announce { node_id, node_name, hostname, os, arch, capabilities }` | New → All | Announce presence |
| `Goodbye` | `Goodbye { reason }` | Any → All | Announce departure |

#### Heartbeat
| Type | Payload | Direction | Purpose |
|------|---------|-----------|---------|
| `Heartbeat` | `Heartbeat { resources, status }` | Periodic | Share resource state |
| `HeartbeatAck` | `HeartbeatAck { cluster_size }` | Response | Acknowledge heartbeat |

#### Resource
| Type | Payload | Direction | Purpose |
|------|---------|-----------|---------|
| `ResourceUpdate` | `ResourceUpdate { resources }` | Periodic | Push resource update |
| `ResourceQuery` | `ResourceQuery` | Any → Any | Request resources |
| `ResourceResponse` | `ResourceResponse { nodes }` | Response | Return all resources |

#### Job
| Type | Payload | Direction | Purpose |
|------|---------|-----------|---------|
| `JobSubmit` | `JobSubmit { job }` | Local → Remote | Send job for execution |
| `JobAssigned` | `JobAssigned { job_id, node_id }` | Remote → Local | Confirm assignment |
| `JobStarted` | `JobStarted { job_id }` | Remote → Local | Job started |
| `JobProgress` | `JobProgress { job_id, progress_percent, message }` | Remote → Local | Progress update |
| `JobCompleted` | `JobCompleted { job_id, result }` | Remote → Local | Job finished |
| `JobFailed` | `JobFailed { job_id, error }` | Remote → Local | Job failed |
| `JobCancel` | `JobCancel { job_id }` | Local → Remote | Cancel job |

#### Memory
| Type | Payload | Direction | Purpose |
|------|---------|-----------|---------|
| `MemoryPageRequest` | `MemoryPageRequest { page_id, pool_id }` | Any → Owner | Fetch page |
| `MemoryPageResponse` | `MemoryPageResponse { page_id, pool_id, data, compressed, encrypted }` | Owner → Any | Return page |
| `MemoryPageWrite` | `MemoryPageWrite { page_id, pool_id, data, compressed, encrypted }` | Any → Owner | Write page |
| `MemoryPageAck` | `MemoryPageAck { page_id, pool_id, success }` | Owner → Any | Ack write |

#### Network
| Type | Payload | Direction | Purpose |
|------|---------|-----------|---------|
| `JoinRequest` | `JoinRequest { invite_code, node_name, hostname, os, arch }` | New → Bootstrap | Request to join |
| `JoinResponse` | `JoinResponse { success, node_id, network_id, error }` | Bootstrap → New | Accept/reject |
| `LeaveNotification` | `LeaveNotification { node_id, reason }` | Leaving → All | Announce departure |

### Implementation

- **Codec**: `crates/luci-protocol/src/codec.rs` — `LengthPrefixedCodec<T>` using tokio-util
- **Messages**: `crates/luci-protocol/src/message.rs` — all 24 types with helper constructors
- **Handler**: `crates/lucid/src/daemon/network.rs` — processes incoming messages via `Framed`

### What's Connected

The network listener is fully functional:
1. Daemon binds TCP listener on port 9528
2. Accepts incoming connections
3. Wraps each connection in `Framed<TcpStream, LengthPrefixedCodec<ProtocolMessage>>`
4. Reads messages in a loop
5. Processes Ping, Heartbeat, Announce, JoinRequest, JobSubmit
6. Sends appropriate responses (Pong, HeartbeatAck, JoinResponse)
7. Stores peer connections with resources and last heartbeat time
8. Validates invite codes on JoinRequest
9. Returns real network_id in JoinResponse (not hardcoded)

### What's Missing

- **No reconnection**: If a connection drops, it's not re-established
- **No message routing**: Messages are only processed, not forwarded

---

## Port 9529: Node Discovery (UDP)

**Direction**: Daemon ↔ Daemon (broadcast on LAN)
**Transport**: UDP
**Status**: ✅ Working

### Behavior

1. Every 10 seconds, each daemon broadcasts a UDP packet to `255.255.255.255:{discovery_port}`
2. Packet contains: `node_id`, `node_name`, `tcp_port`, `hostname`, `os`, `arch`, `ip_addresses`
3. Other daemons listen on the discovery port and collect peer addresses
4. After discovery, daemons connect to each other on TCP:9528

### Wire Format

```json
{
  "node_id": "uuid",
  "node_name": "my-laptop",
  "hostname": "PMSPL-LAP-004",
  "os": "windows",
  "arch": "x86_64",
  "tcp_port": 9528,
  "ip_addresses": ["192.168.1.86"],
  "last_seen": "2026-07-01T12:00:00Z"
}
```

### Implementation

- **Service**: `crates/lucid/src/daemon/discovery.rs` — `DiscoveryService`
- **Broadcast**: Sends UDP packet every 10 seconds
- **Listen**: Receives packets from other daemons
- **Connect**: Automatically connects to newly discovered peers on TCP:9528
- **Configurable port**: Uses `config.discovery_port` (default: 9529)

### Flow

```
Machine A                          Machine B
    │                                  │
    │  UDP broadcast (port 9529)       │
    │─────────────────────────────────▶│
    │                                  │  Receive discovery packet
    │                                  │  Connect to A:9528
    │◀─────────────────────────────────│  TCP connection established
    │                                  │
    │  Exchange Announce messages      │
    │─────────────────────────────────▶│
    │◀─────────────────────────────────│
    │                                  │
    │  Heartbeat every 10s             │
    │─────────────────────────────────▶│
    │◀─────────────────────────────────│
```

---

## Network Configuration

### Config File

```json
{
  "daemon_port": 9527,
  "node_port": 9528,
  "discovery_port": 9529
}
```

### Firewall Rules

For multi-node operation, open these ports:

| Port | Direction | Purpose |
|------|-----------|---------|
| 9527 | Inbound (localhost only) | CLI access |
| 9528 | Inbound + Outbound | Node-to-node |
| 9529/UDP | Broadcast + Inbound | Discovery |

The daemon auto-configures Windows firewall rules on startup.

### Common Issues

**Connection refused on port 9527:**
- Daemon not running → `lucid` or `luci start`

**Cannot connect to remote node:**
- Firewall blocking port 9528
- Wrong IP address
- Remote daemon not running

**Discovery not working:**
- Port 9529 not open
- UDP broadcast blocked by router
- Not on same subnet

---

## Full Distributed Flow

```
Machine A (Creator)              Machine B (Joiner)
       │                              │
       │  luci create home            │
       │  ──▶ Generate invite code    │
       │                              │
       │  UDP broadcast (port 9529)   │
       │─────────────────────────────▶│  Receive discovery
       │◀─────────────────────────────│  Connect on TCP:9528
       │                              │
       │  Exchange Announce messages  │
       │─────────────────────────────▶│
       │◀─────────────────────────────│
       │                              │
       │  Heartbeat every 10s         │
       │◀─────────────────────────────▶│
       │                              │
       │  luci run job.py             │
       │  ──▶ Scheduler picks B       │
       │  ──▶ Send JobSubmit to B     │
       │─────────────────────────────▶│
       │                              │  Execute job
       │◀─────────────────────────────│  Send JobCompleted
       │  ──▶ Display result          │
```
