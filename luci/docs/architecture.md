# Luci Architecture

---

## Core Vision

**4 devices connected via Luci should work as ONE machine.**

When you connect 4 laptops/desktops via Luci, they become a single unified computer with:
- 64 CPU cores (16 × 4 devices)
- 64 GB RAM (16GB × 4 devices)
- 30 GB VRAM (2GB × 4 GPUs)
- 2 TB Storage (500GB × 4 devices)

User runs: `luci run train.py --ram 32GB --gpu`
System automatically picks the best node and runs it.
User doesn't know or care which device executed the job.

---

## Working Features Summary

| Layer | Component | Status | Tests |
|-------|-----------|--------|-------|
| CLI | Command parsing | ✅ | 49 tests |
| CLI | Display formatting | ✅ | 22 tests |
| CLI | IPC client | ✅ | 7 tests |
| CLI | 22 commands | ✅ | All working |
| Daemon | IPC server | ✅ | 36 tests |
| Daemon | Resource collection | ✅ | Verified |
| Daemon | Job scheduling | ✅ | Verified |
| Daemon | Job execution | ✅ | Verified |
| Daemon | Network (TCP:9528) | ✅ | Verified |
| Daemon | Discovery (UDP) | ✅ | Verified |
| Daemon | Heartbeat | ✅ | Verified |
| Protocol | Length-prefixed codec | ✅ | Verified |
| Protocol | 24 message types | ✅ | Verified |
| Runtime | Distributed executor | ✅ | Verified |
| Runtime | Distributed buffer | ✅ | Built |

---

## Requirements

### System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | 2 cores | 4+ cores |
| RAM | 2 GB | 8+ GB |
| Disk | 100 MB | 1+ GB |
| Network | LAN | Gigabit LAN |
| GPU | None | NVIDIA (optional) |

### Software Requirements

| Software | Version | Purpose |
|----------|---------|---------|
| Rust | 1.75+ | Build system |
| Cargo | Bundled | Package manager |
| Python | 3.8+ | Running .py jobs |
| Node.js | 18+ | Running .js jobs |
| nvidia-smi | Latest | GPU detection |

### Port Requirements

| Port | Protocol | Direction | Purpose |
|------|----------|-----------|---------|
| 9527 | TCP | Inbound | CLI ↔ Daemon IPC |
| 9528 | TCP | Bidirectional | Node ↔ Node communication |
| 9529 | UDP | Broadcast | Node discovery |

---

## System Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                        Machine A                            │
│                                                             │
│   luci (CLI)                                                │
│       │                                                     │
│       │  TCP:9527  Length-prefixed JSON + Auth Token        │
│       ▼                                                     │
│   luci (daemon mode)                                            │
│       │                                                     │
│       ├── IpcServer ............. handles CLI commands      │
│       ├── ResourceCollector ..... sysinfo + nvidia-smi      │
│       ├── Scheduler ............. score-based node picker   │
│       ├── JobManager ............ spawns python/node/cmd    │
│       ├── NetworkManager ........ luci-protocol on :9528   │
│       ├── DiscoveryService ...... UDP broadcast on :9529   │
│       ├── HeartbeatSender ....... periodic health checks   │
│       ├── FileTransferManager ... manifest-based sync      │
│       └── MemoryDistributor ..... remote page read/write   │
│                                                             │
└──────────────────────────────┬──────────────────────────────┘
                               │
                    UDP:9529 (discovery)
                    TCP:9528 (communication)
                               │
┌──────────────────────────────▼──────────────────────────────┐
│                        Machine B                            │
│                                                             │
│   luci (daemon mode)                                            │
│       │                                                     │
│       ├── IpcServer                                     │
│       ├── ResourceCollector                             │
│       ├── Scheduler                                     │
│       ├── JobManager                                    │
│       ├── NetworkManager                                │
│       ├── DiscoveryService                              │
│       ├── HeartbeatSender                               │
│       ├── FileTransferManager                           │
│       └── MemoryDistributor                             │
│                                                             │
│   luci (CLI)                                                │
│       │  TCP:9527                                          │
│       ▼                                                     │
│   (same structure)                                          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Crate Dependency Graph

```
luci-common          (shared types, IPC, config)
    │
    ├──▶ luci-protocol    (wire protocol, codec)
    │       │
    │       └──▶ luci-runtime    (buffer, executor)
    │               │
    │               └──▶ luci --internal-daemon (daemon mode)
    │
    └──▶ luci-cli        (CLI binary)
```

---

## Complete Workflows

### Workflow 1: Job Execution

```
User runs: luci run payroll.py --ram 4GB --cpu 2

┌─────────────────────────────────────────────────────────────────┐
│ STEP 1: CLI Parsing                                              │
│   ├── Parse command line arguments                               │
│   ├── Detect job type from extension (.py → python)              │
│   ├── Parse RAM string "4GB" → bytes (4294967296)               │
│   └── Build RunConfig struct                                     │
├─────────────────────────────────────────────────────────────────┤
│ STEP 2: IPC Request                                              │
│   ├── Serialize IpcCommand::RunJob to JSON                       │
│   ├── Send over TCP:9527 with length prefix                      │
│   └── Wait for response                                          │
├─────────────────────────────────────────────────────────────────┤
│ STEP 3: Daemon Processing                                        │
│   ├── Receive and deserialize IPC request                        │
│   ├── Validate auth token                                        │
│   ├── Create Job object with UUID                                │
│   ├── Validate job name (not empty)                              │
│   └── Return job_id to CLI                                       │
├─────────────────────────────────────────────────────────────────┤
│ STEP 4: Scheduling                                               │
│   ├── Query all available nodes                                  │
│   ├── Filter nodes by requirements (RAM, CPU)                    │
│   ├── Score each eligible node                                   │
│   └── Select node with highest score                             │
├─────────────────────────────────────────────────────────────────┤
│ STEP 5: Execution                                                │
│   ├── JobManager spawns python process                           │
│   ├── Captures stdout/stderr                                     │
│   ├── Tracks exit code                                           │
│   └── Updates job status: Running → Completed/Failed             │
├─────────────────────────────────────────────────────────────────┤
│ STEP 6: CLI Output                                               │
│   ├── Display job ID                                             │
│   ├── Show track command                                         │
│   └── User can check with: luci job <id>                         │
└─────────────────────────────────────────────────────────────────┘
```

### Workflow 2: Multi-Machine Discovery

```
Machine A (Master)                    Machine B (Worker)
      │                                     │
      │  1. UDP Discovery Broadcast         │
      │  ─────────────────────────────────▶ │
      │  { node_id, node_name, tcp_port }   │
      │                                     │
      │  2. TCP Connection                  │
      │  ─────────────────────────────────▶ │
      │  { Announce message }               │
      │                                     │
      │  3. Heartbeat (every 10s)           │
      │  ─────────────────────────────────▶ │
      │  { resources, status }              │
      │                                     │
      │  4. Resource Update                 │
      │  ◀───────────────────────────────── │
      │  { cpu, ram, gpu }                  │
      │                                     │
      │  5. Peer tracked                    │
      │  └─> Node B added to peer list      │
```

### Workflow 3: File Synchronization

```
User runs: luci sync ./project node-b

┌─────────────────────────────────────────────────────────────────┐
│ STEP 1: Build Manifest                                           │
│   ├── Scan source directory                                      │
│   ├── Calculate file hashes (SHA-256)                            │
│   └── Create file manifest: [{path, hash, size}]                 │
├─────────────────────────────────────────────────────────────────┤
│ STEP 2: Send to Target                                           │
│   ├── Send IpcCommand::SyncFiles { source, target }              │
│   └── Daemon forwards to target node via TCP:9528                │
├─────────────────────────────────────────────────────────────────┤
│ STEP 3: Diff Manifests                                           │
│   ├── Target receives manifest                                   │
│   ├── Compares with existing files                               │
│   └── Identifies new/changed/deleted files                       │
├─────────────────────────────────────────────────────────────────┤
│ STEP 4: Transfer Files                                           │
│   ├── Send only changed files (delta sync)                       │
│   ├── Target writes to local filesystem                          │
│   └── Update sync state                                          │
├─────────────────────────────────────────────────────────────────┤
│ STEP 5: Confirmation                                              │
│   ├── Return sync result to CLI                                  │
│   └── Display: "Files synced successfully!"                      │
└─────────────────────────────────────────────────────────────────┘
```

### Workflow 4: Memory Distribution

```
User runs: luci memory create ai-pool 8

┌─────────────────────────────────────────────────────────────────┐
│ STEP 1: Validation                                               │
│   ├── Validate name (not empty)                                  │
│   ├── Validate size_gb (0 < size <= 1024)                        │
│   └── Calculate pages: 8GB / 4KB = 2,097,152 pages              │
├─────────────────────────────────────────────────────────────────┤
│ STEP 2: Create Pool                                              │
│   ├── Generate UUID for pool ID                                  │
│   ├── Create MemoryPool metadata                                 │
│   └── Store in DaemonState.memory_pools                          │
├─────────────────────────────────────────────────────────────────┤
│ STEP 3: Remote Read/Write (when distributed)                     │
│   ├── Write: Send MemoryPageWrite to target node                 │
│   ├── Read: Send MemoryPageRequest to node                       │
│   └── Receive: MemoryPageResponse with data                      │
├─────────────────────────────────────────────────────────────────┤
│ STEP 4: Response                                                 │
│   ├── Return pool_id to CLI                                      │
│   └── Display formatted output                                   │
└─────────────────────────────────────────────────────────────────┘
```

### Workflow 5: Auto-Failover

```
Node failure detected:

┌─────────────────────────────────────────────────────────────────┐
│ STEP 1: Detection                                                │
│   ├── HeartbeatSender sends heartbeat to node                    │
│   ├── No response within timeout                                 │
│   └── Mark node as Offline                                       │
├─────────────────────────────────────────────────────────────────┤
│ STEP 2: Assessment                                                │
│   ├── Check for running jobs on failed node                      │
│   ├── Identify jobs that need retry                              │
│   └── Log failover event                                         │
├─────────────────────────────────────────────────────────────────┤
│ STEP 3: Retry                                                     │
│   ├── Reschedule failed jobs                                     │
│   ├── Find new best node                                         │
│   └── Submit job to new node                                     │
├─────────────────────────────────────────────────────────────────┤
│ STEP 4: Notification                                              │
│   ├── Update job status                                          │
│   └── Log: "Job <id> failed over to <node>"                      │
└─────────────────────────────────────────────────────────────────┘
```

---

## Data Flow: Node Discovery

```
Every 10 seconds:

1. DiscoveryService broadcasts UDP packet to 255.255.255.255:{discovery_port}
   └─> { "node_id": "uuid", "node_name": "my-laptop", "tcp_port": 9528 }

2. Other daemons listen on UDP:{discovery_port}
   └─> Collect peer addresses, connect on TCP:9528

3. On TCP connection:
   └─> Exchange Announce messages, update peer resources
```

---

## Data Flow: Heartbeat

```
Every 10 seconds:

1. HeartbeatSender iterates connected peers
   └─> Sends Heartbeat message via TCP:9528 with real ResourceSnapshot

2. Peer responds with HeartbeatAck
   └─> Updates peer resources and last_heartbeat timestamp
   └─> Reports actual cluster_size (peers.len() + 1)

3. Stale peer detection:
   └─> If last_heartbeat > 60s ago, mark peer as Offline
```

---

## Data Flow: Resource Collection

```
Every 5 seconds:

1. ResourceCollector.collect()
   │
   ├── System::new_all() + refresh_all()
   │   └──> CPU cores, load%, frequency
   │   └──> RAM total, available, used
   │
   ├── nvidia-smi --query-gpu=...  (if available, all platforms)
   │   └──> GPU name, VRAM, utilization, temperature
   │
   ├── Disks::new_with_refreshed_list()
   │   └──> Disk total/free space
   │
   ├── Networks::new_with_refreshed_list()
   │   └──> Bytes sent/received
   │
   └── Idle tracking
       └──> If CPU < 5%, increment idle_seconds by 5

2. Stores in DaemonState.nodes[node_id].resources

3. CLI queries via IpcCommand::GetResources / ListDevices
   └──> Aggregates across all nodes
```

---

## IPC Protocol

**Transport**: TCP on localhost (port 9527)

**Framing**: 4-byte little-endian length prefix + JSON payload

```
Client sends:
┌──────────────┬──────────────────────────────┐
│ len (4 bytes)│ JSON IpcRequest              │
└──────────────┴──────────────────────────────┘

Server responds:
┌──────────────┬──────────────────────────────┐
│ len (4 bytes)│ JSON IpcResponse             │
└──────────────┴──────────────────────────────┘
```

**IpcRequest**:
```json
{
  "command": {
    "type": "<variant>",
    // variant-specific fields
  }
}
```

**IpcResponse**:
```json
{
  "type": "Success",
  "payload": { ... }
}
// or
{
  "type": "Error",
  "message": "error description"
}
```

---

## Network Protocol

**Transport**: TCP on all interfaces (port 9528)

**Codec**: `LengthPrefixedCodec<ProtocolMessage>` from luci-protocol

**Message structure**:
```json
{
  "id": "uuid",
  "msg_type": "Heartbeat",
  "sender_id": "node-uuid",
  "timestamp": "2026-06-30T18:00:00Z",
  "payload": {
    "type": "Heartbeat",
    "resources": { ... },
    "status": "online"
  }
}
```

**24 message types**:

| Category | Messages |
|----------|----------|
| Discovery | Ping, Pong, Announce, Goodbye |
| Heartbeat | Heartbeat, HeartbeatAck |
| Resource | ResourceUpdate, ResourceQuery, ResourceResponse |
| Job | JobSubmit, JobAssigned, JobStarted, JobProgress, JobCompleted, JobFailed, JobCancel |
| Memory | MemoryPageRequest, MemoryPageResponse, MemoryPageWrite, MemoryPageAck |
| Network | JoinRequest, JoinResponse, LeaveNotification |

---

## Scheduling Algorithm

```rust
fn score_node(node, job) -> f64 {
    let mut score = 0.0;

    // RAM: more available = better (weight: 10)
    score += available_ram_gb * 10.0;

    // CPU: less load = better (weight: 0.5)
    score += (100.0 - cpu_load_percent) * 0.5;

    // Idle bonus: +20 if idle > 5 minutes
    if idle_seconds > 300 { score += 20.0; }

    // Charging bonus: +5
    if is_charging { score += 5.0; }

    // GPU bonus: +10
    if has_gpu { score += 10.0; }

    // VRAM bonus: +5 if meets requirement
    if let Some(min_vram) = requirements.min_vram_bytes {
        if vram_available >= min_vram { score += 5.0; }
    }

    // Preferred node bonus: +25
    if requirements.preferred_nodes.contains(&node_id) { score += 25.0; }

    // Status bonus
    match status {
        Idle => score += 15.0,
        Online => score += 10.0,
        _ => {}
    }

    score
}
```

**Filtering**: Nodes must meet minimum requirements before scoring:
- `available_ram >= job.requirements.min_ram_bytes`
- `cpu_cores >= job.requirements.min_cpu_cores`
- If GPU required: `vram_available >= job.requirements.min_vram_bytes`

---

## Memory Model

Memory pools are virtual collections of 4KB pages:

```
Memory Pool "ai-pool" (4 GB = 1,048,576 pages)
    │
    ├── Page 0 ──▶ [4KB data] ──▶ Node A
    ├── Page 1 ──▶ [4KB data] ──▶ Node B
    ├── Page 2 ──▶ [4KB data] ──▶ Node A
    └── ...
```

**Current state**: Pools are metadata-only (tracked in DaemonState). The `DistributedBuffer` in luci-runtime implements page-based read/write but is not yet wired into the daemon.

---

## Configuration

**Location**: `~/.config/luci/config.json`

```json
{
  "node_id": "90aee663-dd0a-4cf3-8f2c-c69d4aa8037a",
  "node_name": "my-laptop",
  "daemon_port": 9527,
  "node_port": 9528,
  "discovery_port": 9529,
  "network_id": null,
  "network_name": null,
  "auth_token": "auto-generated-uuid",
  "max_jobs": 4
}
```

- `node_id`: Unique identifier (UUID v4), auto-generated on first run
- `node_name`: Human-readable name (defaults to hostname)
- `daemon_port`: TCP port for CLI ↔ Daemon IPC
- `node_port`: TCP port for inter-node communication
- `discovery_port`: UDP port for node discovery broadcast
- `network_id`: Set when `luci create` or `luci join` is used
- `network_name`: Human-readable network name
- `auth_token`: Auto-generated UUID for IPC authentication
- `max_jobs`: Maximum concurrent jobs (default: 4)

---

## Error Handling

All errors use `anyhow::Result` for the daemon and CLI. The IPC protocol wraps errors in `IpcResponse::Error { message }`. The `luci-common::Error` enum provides typed errors:

```rust
enum Error {
    DaemonNotRunning,
    Config(String),
    Network(String),
    Job(String),
    Memory(String),
    Io(String),
}
```

---

## Threading Model

The daemon runs on a single tokio runtime with 5 spawned tasks:

```
tokio::spawn ──▶ IpcServer        (accepts CLI connections)
tokio::spawn ──▶ ResourcePoller    (every 5s: collect resources)
tokio::spawn ──▶ HeartbeatSender   (every 10s: send heartbeats to peers)
tokio::spawn ──▶ NetworkListener   (accepts node connections on TCP:9528)
tokio::spawn ──▶ DiscoveryService  (UDP broadcast/listen on port 9529)
```

All tasks share state via `Arc<RwLock<DaemonState>>`. The IPC server handles each connection in a separate task. The `JobManager` is embedded inside `DaemonState` so all tasks can access it.

---

## Shutdown Flow

```
1. SIGINT received (ctrl_c)
2. Send Goodbye messages to all connected peers
3. Cancel all running jobs (kill spawned processes)
4. Clean up PID file
5. Abort all spawned tasks
6. Exit
```
