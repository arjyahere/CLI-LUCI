# Luci Commands Reference

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

| Category | Feature | Status | Notes |
|----------|---------|--------|-------|
| Network | Create network | ✅ | Generates invite code, UUID network ID |
| Network | Join network | ✅ | Parses invite code, deterministic network ID |
| Network | Leave network | ✅ | Sends LeaveNotification to peers |
| Devices | List devices | ✅ | Shows CPU/RAM/GPU/status in table |
| Devices | Device details | ✅ | Full resource breakdown per node |
| Resources | Cluster resources | ✅ | Aggregated CPU/RAM/GPU with progress bars |
| Jobs | Run script | ✅ | Auto-detects type, parses RAM/VRAM to bytes |
| Jobs | List jobs | ✅ | Shows ID, name, type, status, node |
| Jobs | Job status | ✅ | Detailed job info with exit code |
| Jobs | Cancel job | ✅ | Kills spawned process, updates status |
| Jobs | Job timeout | ✅ | max_duration_secs enforced via tokio |
| Memory | Create pool | ✅ | Name + size in GB, validates 0-1024 |
| Memory | List pools | ✅ | Shows name, size, pages, ID |
| Memory | Pool info | ✅ | Detailed pool metadata |
| Memory | Delete pool | ✅ | Removes from daemon state |
| Daemon | Status | ✅ | PID, uptime, node ID, network, jobs |
| Daemon | Start | ✅ | Pre-flight check, spawns process |
| Daemon | Stop | ✅ | Pre-flight check, kills process |
| Daemon | Restart | ✅ | Stop + start with status checks |
| Logs | View logs | ✅ | Shows log dir, level, filter info |
| Logs | Follow logs | ✅ | Ctrl+C handling, real-time polling |
| Monitor | Real-time | ✅ | Single node or cluster view, configurable interval |
| Sync | Sync files | ✅ | Manifest-based diff, transfer to peers |
| Sync | List synced | ✅ | Shows source, target, file count |
| Proxy | Start | ✅ | Configurable port and target nodes |
| Proxy | Stop | ✅ | Graceful shutdown |
| Proxy | Status | ✅ | Running, port, requests, errors |
| Failover | Status | ✅ | Enabled/disabled, failed/restarted jobs |
| Peers | List peers | ✅ | Node ID, name, address |
| Health | Check health | ✅ | Status, version, uptime |
| Metrics | View metrics | ✅ | Text, JSON, Prometheus formats |
| IPC | All commands | ✅ | 22 command types, length-prefixed JSON |
| Auth | Token validation | ✅ | Auto-generated UUID |
| Discovery | UDP broadcast | ✅ | Configurable port, 10s interval |
| Heartbeat | Health checks | ✅ | 10s interval, real resource data |
| TLS | Encryption | ✅ | Optional TLS for node-to-node |
| Tests | CLI tests | ✅ | 49 tests (parse, display, IPC) |
| Tests | Daemon tests | ✅ | 48 tests (IPC, distributed, integration) |

---

## Requirements

### System Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| OS | Windows 10, Linux, macOS | Windows 11, Ubuntu 22.04 |
| RAM | 2 GB per node | 8 GB per node |
| Disk | 100 MB free | 1 GB free |
| CPU | 2 cores | 4+ cores |
| Network | LAN connectivity | Gigabit LAN |
| GPU | None (optional) | NVIDIA with nvidia-smi |

### Build Requirements

| Tool | Version | Install |
|------|---------|---------|
| Rust | 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Git | Any | `apt install git` / `brew install git` |
| Cargo | Bundled with Rust | Included |

### Runtime Requirements

| Component | Required | Purpose |
|-----------|----------|---------|
| Python | Optional | Running `.py` scripts |
| Node.js | Optional | Running `.js` scripts |
| nvidia-smi | Optional | GPU detection and VRAM info |

### Step-by-Step Setup

#### Step 1: Install Rust

```bash
# Windows (PowerShell)
winget install Rustlang.Rustup

# Linux/macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify
rustc --version  # Should be 1.75+
cargo --version
```

#### Step 2: Clone and Build

```bash
# Clone
git clone <repo-url>
cd luci

# Build (debug)
cargo build

# Build (release, optimized)
cargo build --release

# Verify binaries
ls target/release/luci.exe    # CLI
ls target/release/lucid.exe   # Daemon
```

#### Step 3: Run Tests

```bash
# Run all tests (97 total)
cargo test --all

# Run CLI tests (49 tests)
cargo test -p luci

# Run daemon tests (48 tests)
cargo test -p lucid

# Expected output: 97 passed, 0 failed
```

#### Step 4: Start Daemon

```bash
# Terminal 1: Start daemon
target/release/lucid.exe      # Windows
./target/release/lucid        # Linux/macOS

# Or via CLI
luci start
```

#### Step 5: Verify Daemon

```bash
# Terminal 2: Check status
luci status

# Expected output:
# ┌──────────────────────────────────────────────────────────────┐
# │                      DAEMON STATUS                           │
# ├──────────────────────────────────────────────────────────────┤
# │  Daemon:       ● Running                                    │
# │  PID:          12345                                        │
# │  Uptime:       5m 30s                                       │
# │  Node ID:      90aee663-dd0a-4cf3-8f2c-c69d4aa8037a        │
# └──────────────────────────────────────────────────────────────┘
```

#### Step 6: Create Network (Optional)

```bash
# Create a network
luci create home

# Share invite code with other machines
# Other machines join with:
luci join <invite-code>
```

#### Step 7: Run Jobs

```bash
# Run a script
luci run script.py

# Run with requirements
luci run train.py --ram 8GB --cpu 4 --gpu --vram 8GB

# Monitor
luci jobs
luci job <job-id>
```

---

## Complete Workflow

### Workflow 1: Single Machine Job Execution

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

### Workflow 2: Multi-Machine Distributed Execution

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
      │  4. User runs: luci run train.py    │
      │                                     │
      │  5. Scheduler scores nodes          │
      │  └─> Machine B has more resources   │
      │                                     │
      │  6. JobSubmit via TCP:9528          │
      │  ─────────────────────────────────▶ │
      │  { script, requirements }           │
      │                                     │
      │  7. Machine B executes              │
      │  └─> Spawns python process          │
      │                                     │
      │  8. JobProgress updates             │
      │  ◀───────────────────────────────── │
      │  { progress: 50% }                  │
      │                                     │
      │  9. JobCompleted                    │
      │  ◀───────────────────────────────── │
      │  { exit_code: 0, output: "..." }    │
      │                                     │
      │  10. CLI displays result            │
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

### Workflow 4: Real-Time Monitoring

```
User runs: luci monitor --interval 2

┌─────────────────────────────────────────────────────────────────┐
│ LOOP (every 2 seconds):                                          │
│                                                                  │
│ 1. Clear screen                                                  │
│                                                                  │
│ 2. Send IpcCommand::GetResources                                 │
│                                                                  │
│ 3. Display cluster status:                                       │
│    ┌──────────────────────────────────────────────────────────┐  │
│    │                  CLUSTER MONITOR                         │  │
│    ├──────────────────────────────────────────────────────────┤  │
│    │  Nodes:        3 connected                               │  │
│    │  CPU Cores:    48 cores                                  │  │
│    │                                                           │  │
│    │  Memory:       [████████████░░░░░░░░] 24.5GB / 48GB     │  │
│    │  GPUs:         2 device(s)                               │  │
│    │                                                           │  │
│    │  Last updated: 14:32:15                                  │  │
│    └──────────────────────────────────────────────────────────┘  │
│                                                                  │
│ 4. Sleep for interval                                            │
│                                                                  │
│ 5. Repeat (Ctrl+C to stop)                                       │
└─────────────────────────────────────────────────────────────────┘
```

### Workflow 5: Memory Pool Management

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
│ STEP 3: Response                                                 │
│   ├── Return pool_id to CLI                                      │
│   └── Display:                                                   │
│       ┌──────────────────────────────────────────────────────┐   │
│       │                   MEMORY POOL CREATED                 │   │
│       ├──────────────────────────────────────────────────────┤   │
│       │  Name:      ai-pool                                  │   │
│       │  Pool ID:   ffcd7987-0eed-4466-a119-48b870743c47    │   │
│       │  Size:      8.0 GB                                   │   │
│       └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

### Workflow 6: Auto-Failover

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

## Command Details

### Network Management

#### `luci create <name>`

Create a new network and generate an invite code.

```bash
luci create home
# ┌──────────────────────────────────────────────────────────────┐
# │                    NETWORK INFO                              │
# ├──────────────────────────────────────────────────────────────┤
# │  Network:      home                                         │
# │  Network ID:   b0118a46-2478-49e7-88e0-6597bc623afd        │
# │  Invite Code:  vTyW.zitO.Tmad..Kzi                         │
# └──────────────────────────────────────────────────────────────┘
#
# Share this invite code with others to join your network:
#   luci join vTyW.zitO.Tmad..Kzi
```

**Internal flow:**
1. Daemon generates NetworkId (UUID) and InviteCode
2. Stores in DaemonState, saves to config
3. Adds self as first node
4. Returns network ID and invite code

#### `luci join <invite-code>`

Join an existing network using an invite code.

```bash
luci join vTyW.zitO.Tmad..Kzi
# ✓ Joined network abc123def456!
```

**Internal flow:**
1. Parses invite code (validates format)
2. Generates deterministic network_id from hash
3. Stores network_id, name, code in state
4. Saves config to disk

#### `luci leave [name]`

Leave the current network.

```bash
luci leave
# ✓ Left the network successfully.
```

**Internal flow:**
1. Sends LeaveNotification to all peers via TCP:9528
2. Clears network_id, name, code from state
3. Updates config file

---

### Device Management

#### `luci devices`

List all devices in the network.

```bash
luci devices
# ┌──────────────────┬────────────┬──────────┬────────────┬────────────┬──────────────┐
# │ NAME             │ STATUS     │ CPU      │ RAM        │ FREE       │ GPU          │
# ├──────────────────┼────────────┼──────────┼────────────┼────────────┼──────────────┤
# │ PMSPL-LAP-004    │ ● Online   │ 16c/38%  │ 15.4GB     │ 7.1GB      │ -            │
# └──────────────────┴────────────┴──────────┴────────────┴────────────┴──────────────┘
# Total: 1 device(s) connected
```

#### `luci device <name>`

Show detailed info for a specific device.

```bash
luci device PMSPL-LAP-004
# ──── Resources ──────────────────────────────────────────────
#   CPU:          16 cores (load: 38.0%)
#   RAM:          15.4 GB total, 7.1 GB free
#   GPU:          NVIDIA GeForce RTX 3080
```

---

### Resource Management

#### `luci resources`

Show combined resources across all connected devices.

```bash
luci resources
# ┌──────────────────────────────────────────────────────────────┐
# │                   CLUSTER RESOURCES                          │
# ├──────────────────────────────────────────────────────────────┤
# │  Overview:        3 connected                               │
# │                                                               │
# │  CPU                                                         │
# │    Total Cores:   48 cores                                   │
# │    Capacity:      [████████████░░░░░░░░]                     │
# │                                                               │
# │  Memory                                                      │
# │    Total:         96.0 GB                                    │
# │    Available:     64.0 GB                                    │
# │    Usage:         [████████░░░░░░░░░░░░] 33.3%              │
# │                                                               │
# │  GPU                                                         │
# │    GPU Devices:   2 device(s)                                │
# │    Total VRAM:    24.0 GB                                    │
# │    VRAM Usage:    [██████░░░░░░░░░░░░░░]                     │
# │                                                               │
# │  Status:   ● Healthy                                         │
# │  Available: 67%                                              │
# └──────────────────────────────────────────────────────────────┘
```

---

### Job Management

#### `luci run <script> [args]`

Submit a job for execution.

```bash
# Basic usage
luci run payroll.py

# With requirements
luci run process_data.py --ram 4GB --cpu 2

# With GPU
luci run train.py --gpu --vram 8GB

# With timeout
luci run long_job.py --timeout 3600

# On specific node
luci run payroll.py --node my-laptop
```

**Options:**
- `--ram <size>`: Minimum RAM (4GB, 8GB, 2048MB)
- `--cpu <cores>`: Minimum CPU cores
- `--gpu`: Require GPU
- `--vram <size>`: Minimum VRAM
- `--timeout <secs>`: Max execution time
- `--node <name>`: Target specific node

**Internal flow:**
1. CLI detects job type from extension (.py → python)
2. Parses RAM/VRAM strings to bytes
3. Sends IpcCommand::RunJob with requirements
4. Daemon creates Job with UUID
5. Scheduler scores all eligible nodes
6. Assigns to best node
7. JobManager spawns process
8. Returns job_id to CLI

**Job Types:**
| Type | Detection | Execution |
|------|-----------|-----------|
| python | `.py` extension | `python3 <script>` |
| javascript | `.js`, `.mjs` | `node <script>` |
| shell | `.sh`, `.bash` | `sh -c <script>` |
| rust | `.rs` extension | `rustc <script>` |
| go | `.go` extension | `go run <script>` |
| shell | Default | `cmd /C <script>` (Windows) |

#### `luci jobs`

List all jobs and their status.

```bash
luci jobs
# ┌──────────────┬────────────────────┬──────────┬────────────┬──────────────┐
# │ ID           │ NAME               │ TYPE     │ STATUS     │ NODE         │
# ├──────────────┼────────────────────┼──────────┼────────────┼──────────────┤
# │ abc-123      │ payroll.py         │ python   │ ✓ Complete │ my-laptop    │
# │ def-456      │ train.py           │ python   │ ▶ Running  │ gpu-server   │
# └──────────────┴────────────────────┴──────────┴────────────┴──────────────┘
# Total: 2 job(s)
```

#### `luci job <id>`

Show detailed status for a specific job.

```bash
luci job abc-123
# Job Details
#   ID:          abc-123
#   Name:        payroll.py
#   Type:        python
#   Status:      ✓ Completed
#   Node:        my-laptop
#   Exit Code:   0
```

#### `luci cancel <id>`

Cancel a running or pending job.

```bash
luci cancel abc-123
# ✓ Job abc-123 cancelled.
```

**Internal flow:**
1. Sends IpcCommand::CancelJob
2. Daemon calls JobManager.cancel_job()
3. Kills spawned process via child.kill().await
4. Updates job status to Cancelled

---

### Memory Management

#### `luci memory create <name> <size_gb>`

Create a virtual memory pool.

```bash
luci memory create ai-pool 8
# ✓ Memory pool 'ai-pool' created!
#   Name:      ai-pool
#   Pool ID:   ffcd7987-0eed-4466-a119-48b870743c47
#   Size:      8.0 GB
```

#### `luci memory ls`

List all memory pools.

```bash
luci memory ls
# ┌──────────────────────┬────────────┬──────────────┬───────────────────────┐
# │ NAME                 │ SIZE       │ PAGES        │ ID                    │
# ├──────────────────────┼────────────┼──────────────┼───────────────────────┤
# │ ai-pool              │ 8.0GB      │ 2097152      │ ffcd7987-0eed-4466-a1 │
# │ cache                │ 4.0GB      │ 1048576      │ a1b2c3d4-e5f6-7890-ab │
# └──────────────────────┴────────────┴──────────────┴───────────────────────┘
```

#### `luci memory info <id>`

Show detailed info for a memory pool.

```bash
luci memory info ffcd7987-0eed-4466-a119-48b870743c47
#   Name:       ai-pool
#   Size:       8.0 GB
#   Pages:      2097152
```

#### `luci memory rm <id>`

Delete a memory pool.

```bash
luci memory rm ffcd7987-0eed-4466-a119-48b870743c47
# ✓ Memory pool deleted.
```

---

### Daemon Management

#### `luci status`

Show daemon status with dashboard layout.

```bash
luci status
# ┌──────────────────────────────────────────────────────────────┐
# │                      DAEMON STATUS                           │
# ├──────────────────────────────────────────────────────────────┤
# │  ● Running                                                  │
# │  PID:          12345                                        │
# │  Uptime:       5m 30s                                       │
# │  Node ID:      90aee663-dd0a-4cf3-8f2c-c69d4aa8037a        │
# │                                                               │
# │  ─── Network ──────────────────────────────────────────────  │
# │  Network:      home                                         │
# │  Connected:    3 node(s)                                    │
# │                                                               │
# │  ─── Jobs ─────────────────────────────────────────────────  │
# │  Active:       2 jobs                                       │
# │  Completed:    42 jobs                                      │
# └──────────────────────────────────────────────────────────────┘
```

#### `luci start`

Start the daemon with pre-flight check.

```bash
luci start
# ✓ Daemon started successfully.
```

**Internal flow:**
1. Check if daemon already running via IpcCommand::Ping
2. If running: "Daemon is already running."
3. If not: Spawn lucid process
4. Wait 500ms for startup
5. Verify daemon is responding

#### `luci stop`

Stop the running daemon.

```bash
luci stop
# ✓ Daemon stopped successfully.
```

**Internal flow:**
1. Check if daemon is running
2. If not running: "Daemon is not running."
3. If running: Kill process
4. Wait 500ms for shutdown
5. Verify daemon stopped

#### `luci restart`

Restart the daemon.

```bash
luci restart
# ✓ Daemon restarted successfully.
```

#### `luci logs`

View daemon logs with filter and follow support.

```bash
# Show log info
luci logs
# Log directory:    /var/log/lucid
# Log level:        info
#
# Log streaming requires --follow flag.
# Usage:  luci logs --follow

# Follow logs
luci logs --follow
# Following logs... (Press Ctrl+C to stop)

# Filter by level
luci logs --follow --filter warn
```

---

### Real-Time Monitoring

#### `luci monitor`

Real-time resource monitoring.

```bash
# Cluster view
luci monitor --interval 5
# ┌──────────────────────────────────────────────────────────────┐
# │                  CLUSTER MONITOR                             │
# ├──────────────────────────────────────────────────────────────┤
# │  Nodes:        3 connected                                  │
# │  CPU Cores:    48 cores                                     │
# │                                                               │
# │  Memory:       [████████████░░░░░░░░] 24.5GB / 48GB        │
# │  GPUs:         2 device(s)                                  │
# │                                                               │
# │  Last updated: 14:32:15                                     │
# └──────────────────────────────────────────────────────────────┘

# Single node view
luci monitor --node my-laptop
# ┌──────────────────────────────────────────────────────────────┐
# │                  NODE MONITOR: my-laptop                     │
# ├──────────────────────────────────────────────────────────────┤
# │  Status:       ● Online                                     │
# │  CPU:          16 cores (load: 38.0%)                       │
# │  Memory:       [████████░░░░░░░░░░░░] 6.3GB / 15.4GB       │
# │  GPU:          NVIDIA GeForce RTX 3080                      │
# └──────────────────────────────────────────────────────────────┘
```

---

### File Synchronization

#### `luci sync <source> <target>`

Sync a folder to a peer node.

```bash
luci sync ./project node-b
# ✓ Files synced successfully!
```

#### `luci synced`

List synced folders.

```bash
luci synced
#   source →  target  (files)
#   ./project →  node-a  (15 files)
#   ./data →  node-b  (8 files)
```

---

### Reverse Proxy

#### `luci proxy start`

Start a reverse proxy for long-running servers.

```bash
luci proxy start --port 8080 --nodes node-a node-b
# ✓ Proxy started on port 8080
#   Port:       8080
#   Target nodes:   ["node-a", "node-b"]
```

#### `luci proxy status`

Show proxy status.

```bash
luci proxy status
#   Status:       ● Running
#   Port:         8080
#   Requests:     1234
#   Errors:       0
```

#### `luci proxy stop`

Stop the proxy.

```bash
luci proxy stop
# ✓ Proxy stopped.
```

---

### Health and Metrics

#### `luci health`

Check daemon health.

```bash
luci health
#   Status:       ● Healthy
#   Version:      0.1.0
#   Uptime:       2h 15m 30s
```

#### `luci metrics`

View metrics in different formats.

```bash
# Text format
luci metrics
# ──── Job Metrics ────────────────────────────────────────────
#   Submitted:    42 jobs
#   Completed:    40 jobs
#   Failed:       2 jobs
#
# ──── Network Metrics ───────────────────────────────────────
#   Bytes Sent:   1.5 GB
#   Connections:  3

# JSON format
luci metrics --format json

# Prometheus format
luci metrics --format prometheus
```

---

### Peer Discovery

#### `luci peers`

List discovered peers.

```bash
luci peers
#   ● node-a  (abc-123-def-456)
#     Address: 192.168.1.100:9528
#   ● node-b  (ghi-789-jkl-012)
#     Address: 192.168.1.101:9528
```

#### `luci failover`

Check auto-failover status.

```bash
luci failover
#   Status:       Enabled
#   Failed Jobs:  2
#   Restarted:    2
#   Dead Nodes:   0
```

---

## Global Options

### `--port <port>`

Override the daemon port (default: 9527).

```bash
luci --port 9528 status
```

---

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Daemon not running |
| 3 | Invalid arguments |
| 4 | Network error |

---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `RUST_LOG` | `warn` | Log level (trace/debug/info/warn/error) |
