# Luci CLI

**One Runtime. Infinite Resources.**

A distributed runtime that virtualizes compute, memory, storage, and AI acceleration across trusted devices through a single daemon and simple CLI.

---

## Table of Contents

1. [Vision](#vision)
2. [Problem Statement](#problem-statement)
3. [Solution Overview](#solution-overview)
4. [Architecture](#architecture)
5. [Components](#components)
6. [Installation](#installation)
7. [Commands Reference](#commands-reference)
8. [How It Works](#how-it-works)
9. [Memory Model](#memory-model)
10. [Scheduler](#scheduler)
11. [Security](#security)
12. [Networking](#networking)
13. [Roadmap](#roadmap)
14. [Development Setup](#development-setup)
15. [Technology Stack](#technology-stack)
16. [File Structure](#file-structure)
17. [Contributing](#contributing)
18. [License](#license)

---

## Vision

Luci transforms any collection of personal computers into a unified compute platform. Install one CLI, and your idle machines become a programmable cluster—accessible as if they were a single computer.

**Target Users:**
- Developers running heavy workloads (payroll, data migration, AI inference)
- Small teams sharing compute resources across office/home PCs
- Individuals wanting to utilize idle machines for useful work

**Key Principle:** No Docker. No Python installation on workers. No complex setup. One CLI. One daemon. Background execution.

---

## Problem Statement

Modern personal computers are underutilized. Most sit idle 80% of the time, yet developers constantly run out of resources for:

- Large AI model inference (70B+ models need 40-80GB RAM)
- Data processing pipelines (GB-scale transformations)
- Build systems (compilation, testing, deployment)
- PDF generation, CSV migration, batch processing

**Existing Solutions Fall Short:**

| Solution | Limitation |
|----------|------------|
| BOINC | Designed for clusters, not personal PCs |
| Celery | Requires task queue infrastructure |
| Ray/Dask | Complex setup, resource-heavy |
| Hyperspace | AI-only, model sharding focus |

**Luci fills the gap:** A lightweight, general-purpose distributed runtime for personal compute meshes.

---

## Solution Overview

```
Your Laptop (CLI)
       │
    lucid (Daemon)
       │
  ═══════════════════════════════
    Encrypted Mesh Network
  ═══════════════════════════════
       │            │            │
    Worker 1     Worker 2     Worker 3
    (8GB RAM)   (8GB RAM)    (8GB RAM)
```

**What Luci Does:**

1. **Discovers** devices on your network (or via Tailscale across internet)
2. **Measures** idle resources (CPU, RAM, GPU, storage)
3. **Routes** work to available machines
4. **Executes** jobs transparently
5. **Returns** results to your terminal
6. **Virtualizes** memory across machines for AI workloads

**What Luci Does NOT Do:**

- ❌ Replace Windows memory manager
- ❌ Create one giant RAM pool
- ❌ Expose raw pointers or memory addresses
- ❌ Require Docker or Python on workers
- ❌ Run arbitrary untrusted code

---

## Architecture

### High-Level Architecture

```
                    ┌─────────────────┐
                    │   luci (CLI)    │
                    │  User Interface │
                    └────────┬────────┘
                             │ IPC / Named Pipe
                    ┌────────▼────────┐
                    │  lucid (Daemon) │
                    │   Core Process  │
                    └────────┬────────┘
                             │
          ┌──────────────────┼──────────────────┐
          │                  │                  │
    ┌─────▼─────┐      ┌─────▼─────┐      ┌─────▼─────┐
    │ Scheduler │      │ Runtime   │      │  Memory   │
    │  Engine   │      │  Engine   │      │  Manager  │
    └─────┬─────┘      └─────┬─────┘      └─────┬─────┘
          │                  │                  │
    ┌─────▼─────┐      ┌─────▼─────┐      ┌─────▼─────┐
    │   Task    │      │  Network  │      │   Page    │
    │   Queue   │      │  Layer    │      │   Cache   │
    └───────────┘      └─────┬─────┘      └───────────┘
                             │
  ═══════════════════════════════════════════════════════════
           Luci Mesh Network (QUIC + TLS 1.3)
  ═══════════════════════════════════════════════════════════
           │                  │                  │
    ┌──────▼──────┐    ┌──────▼──────┐    ┌──────▼──────┐
    │   Worker    │    │   Worker    │    │   Worker    │
    │  Node A     │    │  Node B     │    │  Node C     │
    │  CPU/RAM/GPU│    │  CPU/RAM/GPU│    │  CPU/RAM/GPU│
    └─────────────┘    └─────────────┘    └─────────────┘
```

### Internal Component Architecture

```
                    lucid
                      │
        ┌─────────────┼─────────────┐
        │             │             │
  ┌─────▼─────┐ ┌─────▼─────┐ ┌─────▼─────┐
  │ Scheduler │ │ Runtime   │ │ Memory    │
  └─────┬─────┘ └─────┬─────┘ └─────┬─────┘
        │             │             │
  ┌─────▼─────┐ ┌─────▼─────┐ ┌─────▼─────┐
  │ Task Queue│ │ Protocol  │ │ Page Cache│
  │ Priority  │ │ Buffers   │ │ Prefetch  │
  │ Deadline  │ │ Serialize │ │ Compress  │
  └───────────┘ └───────────┘ └───────────┘
        │             │             │
        └─────────────┼─────────────┘
                      │
              ┌───────▼───────┐
              │  Object Store │
              │  (RocksDB)    │
              └───────────────┘
```

### Data Flow

```
User executes: luci run payroll.py

CLI (luci)
  │
  ├── Parse command
  ├── Analyze job requirements
  └── Send to daemon via IPC
      │
      ▼
Daemon (lucid)
  │
  ├── Receive job
  ├── Check resource requirements
  ├── Query scheduler for best node
  │
  ▼
Scheduler
  │
  ├── Evaluate all nodes
  │   ├── CPU load
  │   ├── Available RAM
  │   ├── GPU availability
  │   ├── Network latency
  │   └── Battery level
  ├── Select optimal node
  └── Return node assignment
      │
      ▼
Runtime
  │
  ├── Package job files
  ├── Serialize dependencies
  ├── Transfer to worker node
  └── Execute
      │
      ▼
Worker Node
  │
  ├── Receive job package
  ├── Unpack dependencies
  ├── Execute in sandbox
  ├── Stream stdout/stderr
  └── Return results
      │
      ▼
CLI (luci)
  │
  ├── Stream logs to terminal
  ├── Return exit code
  └── Save artifacts
```

---

## Components

### 1. luci (CLI)

**Role:** User interface only. Never executes work.

**Responsibilities:**
- Parse user commands
- Send requests to local daemon
- Display results and logs
- Manage configurations

**Binary Size:** ~5-10 MB

### 2. lucid (Daemon)

**Role:** Core process. The operating system of Luci.

**Responsibilities:**
- Device discovery and authentication
- Resource monitoring (CPU, RAM, GPU, disk, battery)
- Memory management and page caching
- Job scheduling and execution
- Network communication
- Encryption and security
- Storage management

**Binary Size:** ~20-50 MB

**Runtime:**
- Windows: Service via NSSM or native Windows Service
- Linux: systemd service
- macOS: LaunchDaemon

### 3. Luci Runtime

**Role:** Abstraction layer between applications and workers.

**Responsibilities:**
- Hide networking, serialization, caching
- Manage distributed objects (buffers, arrays, tensors)
- Handle retries and failures
- Provide Python/Node.js/Rust SDKs

### 4. Luci Scheduler

**Role:** Intelligent node selection.

**Responsibilities:**
- Maintain live cluster map
- Evaluate node capabilities
- Route jobs to optimal nodes
- Handle node failures
- Support priority and deadlines

### 5. Luci Memory

**Role:** Distributed memory virtualization.

**Responsibilities:**
- Split virtual memory into 4KB pages
- Track page ownership across nodes
- Cache frequently accessed pages locally
- Compress and encrypt pages
- Prefetch predicted pages

### 6. Luci Object Store

**Role:** Persistent metadata and state storage.

**Responsibilities:**
- Store node registry
- Maintain page tables
- Track job history
- Cache configuration

**Backend:** RocksDB or SQLite

---

## Installation

### Quick Install

**Windows:**
```powershell
iwr -useb https://luci.dev/install.ps1 | iex
```

**Linux:**
```bash
curl -fsSL https://luci.dev/install.sh | bash
```

**macOS:**
```bash
curl -fsSL https://luci.dev/install.sh | bash
```

### What Happens During Install

```
1. Download luci + lucid binaries
2. Install to /usr/local/bin (Linux/macOS)
   or C:\Program Files\Luci (Windows)
3. Register system service
4. Start lucid daemon
5. Generate node keys
6. Ready
```

### Verify Installation

```bash
luci --version
# luci 0.1.0

lucid status
# Status: Running
# Node ID: 8f3a1c4d-...
```

---

## Commands Reference

### Network Management

```bash
# Create a new network
luci create home

# Output:
# Network ID : home
# Invite Code : AB4K-X91M-QW34-5678

# Join a network
luci join AB4K-X91M-QW34-5678

# List networks
luci networks

# Leave a network
luci leave home
```

### Device Management

```bash
# List all devices
luci devices

# Output:
# NAME        NODE ID     STATUS    CPU    RAM      GPU
# my-laptop   8f3a1c4d    online    45%    6.2GB    RTX3060
# mom-pc      2b7e9f1a    idle      12%    7.1GB    -
# office-pc   5c1d3e8b    busy      89%    2.4GB    GTX1660

# Show device details
luci device info mom-pc
```

### Resource Monitoring

```bash
# Show combined resources
luci resources

# Output:
# Devices : 6
# CPU     : 48 Cores
# RAM     : 42 GB (available: 38 GB)
# GPU     : 4 devices
# VRAM    : 32 GB

# Watch resources in real-time
luci resources --watch
```

### Job Execution

```bash
# Run a script
luci run payroll.py

# Run with specific node
luci run payroll.py --node mom-pc

# Run with resource requirements
luci run payroll.py --ram 8GB --cpu 4

# Run Python directly
luci python app.py

# Run Node.js
luci node server.js

# Run shell command
luci exec "python3 process_data.py --input data.csv"
```

### Memory Management

```bash
# Create virtual memory
luci memory create 32GB

# Output:
# Memory Created
# ID      : mem://f4d81bc2
# Pages   : 8,388,608
# Size    : 32 GB

# List memory pools
luci memory ls

# Show memory details
luci memory info mem://f4d81bc2

# Delete memory pool
luci memory rm mem://f4d81bc2
```

### AI Integration

```bash
# Load and run AI model
luci ai load llama70b

# Chat with model
luci ai chat "Explain quantum computing"

# Generate text
luci ai generate "Write a Python function to sort a list"

# List available models
luci ai models

# Shard model across devices
luci ai shard llama70b --devices all
```

### System Management

```bash
# Check daemon status
luci status

# Start daemon
luci start

# Stop daemon
luci stop

# Restart daemon
luci restart

# Update Luci
luci update

# View logs
luci logs

# View logs with filter
luci logs --filter "error"

# Uninstall
luci uninstall
```

---

## How It Works

### Device Discovery

When a device joins a network:

```
1. Node receives invite code
2. Connects to network bootstrap node
3. Authenticates with public/private key pair
4. Downloads network configuration
5. Begins heartbeat broadcast
6. Advertises available resources
7. Becomes available for work
```

### Heartbeat System

Every 5 seconds, each node broadcasts:

```json
{
  "node_id": "8f3a1c4d-...",
  "timestamp": 1719756800,
  "resources": {
    "cpu_cores": 8,
    "cpu_load": 45.2,
    "ram_total_gb": 16,
    "ram_available_gb": 12.4,
    "gpu_name": "RTX 3060",
    "vram_total_gb": 12,
    "vram_available_gb": 8.2,
    "disk_free_gb": 256,
    "battery_percent": 87,
    "is_charging": true,
    "idle_seconds": 340,
    "temperature_c": 62
  },
  "capabilities": [
    "cpu",
    "ram",
    "gpu",
    "disk",
    "network"
  ],
  "status": "idle"
}
```

### Job Lifecycle

```
1. SUBMITTED
   User runs: luci run payroll.py
   Job enters queue

2. SCHEDULING
   Scheduler evaluates nodes
   Selects optimal candidate

3. TRANSFERRING
   Package files and dependencies
   Send to worker node

4. EXECUTING
   Worker unpacks job
   Runs in sandboxed environment
   Streams logs back

5. COMPLETED / FAILED
   Return results to user
   Clean up temporary files
   Update job history
```

### Memory Page Workflow

```
Virtual Memory: 32GB
                │
    Split into 4KB pages
                │
        ┌───────┼───────┐
        │       │       │
   Page Table  Page Table  Page Table
        │       │       │
    Node A   Node B   Node C
```

**Reading a Page:**

```
Application requests Page #438
        │
        ▼
Luci Runtime checks local cache
        │
        ├── Cache HIT → Return immediately (1μs)
        │
        └── Cache MISS → Fetch from owning node
                            │
                            ▼
                     Network request (5-20ms)
                            │
                            ▼
                     Cache locally
                            │
                            ▼
                     Return to application
```

**Writing a Page:**

```
Application writes to Page #245
        │
        ▼
Mark page as DIRTY
        │
        ▼
Write to local cache
        │
        ▼
Asynchronous write-back to owner
        │
        ▼
Replicate to backup node
```

---

## Memory Model

### Virtual Memory Architecture

```
┌─────────────────────────────────────────┐
│        Application View                 │
│        32 GB Virtual Memory             │
└─────────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────┐
│        Luci Memory Manager              │
│        Page Table + Cache               │
└─────────────────────────────────────────┘
                  │
    ┌─────────────┼─────────────┐
    │             │             │
┌───▼───┐   ┌────▼────┐   ┌────▼────┐
│Node A │   │ Node B  │   │ Node C  │
│8 GB   │   │ 8 GB    │   │ 8 GB    │
└───────┘   └─────────┘   └─────────┘
```

### Page Table Structure

| Page ID | Owner Node | Cached Locally | Dirty | Compressed | Encrypted | Access Count |
|---------|------------|----------------|-------|------------|-----------|--------------|
| 0 | Node A | Yes | No | Yes | Yes | 142 |
| 1 | Node A | Yes | No | No | Yes | 89 |
| 2 | Node B | No | No | Yes | Yes | 203 |
| 3 | Node C | No | No | Yes | Yes | 67 |
| ... | ... | ... | ... | ... | ... | ... |

### Locality-Aware Caching

```
Frequently accessed pages
        │
        ▼
Kept in local RAM cache
        │
        ▼
Fastest access (1-5μs)

Infrequently accessed pages
        │
        ▼
Evicted to owner node
        │
        ▼
Standard access (5-20ms)
```

### Memory Limits

```
Per Node:
  - Max contributed RAM: 60% of available
  - Min free RAM: 2GB (reserved for OS)
  - Max pages per node: 1,048,576 (4GB)

Per Network:
  - Max nodes: 256
  - Max total memory: 1 PB (theoretical)
  - Max pages: 268,435,456

Per Memory Pool:
  - Max size: 128 GB
  - Max page size: 4KB (default), 2MB (huge pages)
```

---

## Scheduler

### Node Selection Algorithm

```python
def select_node(job_requirements, available_nodes):
    scores = []
    
    for node in available_nodes:
        score = 0
        
        # CPU Score (0-25 points)
        if node.cpu_available >= job_requirements.cpu:
            score += 25
        
        # RAM Score (0-25 points)
        if node.ram_available >= job_requirements.ram:
            score += 25
        
        # GPU Score (0-25 points)
        if job_requirements.gpu:
            if node.gpu_available >= job_requirements.gpu:
                score += 25
        
        # Latency Score (0-15 points)
        latency_score = max(0, 15 - (node.latency_ms / 10))
        score += latency_score
        
        # Battery Score (0-10 points)
        if node.is_charging:
            score += 10
        else:
            score += node.battery_percent / 10
        
        scores.append((node, score))
    
    # Return highest scoring node
    return max(scores, key=lambda x: x[1])
```

### Scheduling Policies

| Policy | Description | Use Case |
|--------|-------------|----------|
| `first-available` | Use first node with enough resources | Quick jobs |
| `best-fit` | Select node with best resource match | General purpose |
| `load-balanced` | Distribute across all nodes | Batch processing |
| `priority` | High-priority jobs first | Critical tasks |
| `deadline` | Meet time constraints | Time-sensitive jobs |

---

## Security

### Authentication

```
Each node has:
  - Public Key (shared with network)
  - Private Key (stored locally)
  - Node ID (derived from public key)

Joining a network:
  1. Receive invite code
  2. Exchange keys with bootstrap node
  3. Verify identity
  4. Receive network token
  5. Begin participating
```

### Encryption

```
All communication:
  - Transport: TLS 1.3
  - Protocol: QUIC with AES-256-GCM
  - Key Exchange: X25519
  - Signatures: Ed25519

Data at rest:
  - Page storage: AES-256-GCM
  - Metadata: AES-256-GCM
  - Keys: Stored in OS keychain
```

### Job Sandboxing

```
Each job runs in isolated environment:
  - Limited file system access
  - No network access (unless requested)
  - Resource limits (CPU, RAM, disk)
  - Time limits
  - No access to other jobs
  - No access to host system services
```

### Trust Model

```
Trusted:
  - Nodes you explicitly added
  - Nodes with valid credentials
  - Nodes on your local network

Untrusted:
  - Arbitrary code execution: DISABLED
  - Remote shell: DISABLED
  - Direct memory access: DISABLED

Jobs are:
  - Type-restricted (Python, Node.js, etc.)
  - Dependency-verified
  - Sandboxed during execution
  - Monitored for resource abuse
```

---

## Networking

### Protocol Stack

```
Application Layer
  │  Luci Protocol (Protobuf)
  │
Transport Layer
  │  QUIC (UDP)
  │
Security Layer
  │  TLS 1.3
  │
Network Layer
  │  IP (IPv4/IPv6)
  │
Physical Layer
     Wi-Fi / Ethernet / Tailscale
```

### Node Discovery

```
Local Network:
  - mDNS/DNS-SD broadcast
  - UDP multicast
  - No internet required

Remote Network:
  - Tailscale integration
  - NAT traversal
  - Relay fallback
```

### Connection Modes

```
Mode 1: Local Network
  Node A ←──→ Node B
  Direct connection
  Lowest latency (1-5ms)

Mode 2: Tailscale
  Node A ←──→ Tailscale ←──→ Node B
  Encrypted tunnel
  Good latency (10-50ms)

Mode 3: Relay
  Node A ←──→ Relay ←──→ Node B
  Fallback when direct fails
  Higher latency (50-200ms)
```

---

## Roadmap

### Phase 1: Foundation (Days 1-5)

```
Day 1-2:
  [x] Project setup
  [x] Core architecture
  [x] CLI scaffolding
  [x] Daemon scaffolding

Day 3-4:
  [ ] Node discovery
  [ ] Heartbeat system
  [ ] Resource monitoring
  [ ] Basic scheduling

Day 5:
  [ ] IPC between CLI and daemon
  [ ] Basic job execution
  [ ] Error handling
```

### Phase 2: Core Features (Days 6-10)

```
Day 6-7:
  [ ] Job packaging
  [ ] File transfer
  [ ] Sandboxed execution
  [ ] Log streaming

Day 8-9:
  [ ] Memory manager
  [ ] Page table
  [ ] Page caching
  [ ] Virtual memory API

Day 10:
  [ ] Security layer
  [ ] Encryption
  [ ] Authentication
  [ ] Access control
```

### Phase 3: Polish (Days 11-15)

```
Day 11-12:
  [ ] Python SDK
  [ ] Node.js SDK
  [ ] Rust SDK
  [ ] API documentation

Day 13-14:
  [ ] Windows installer
  [ ] Linux installer
  [ ] macOS installer
  [ ] Auto-update

Day 15:
  [ ] Testing
  [ ] Documentation
  [ ] Release preparation
  [ ] GitHub repository
```

### Future Phases

```
Phase 4: AI Integration
  - Model sharding
  - Inference routing
  - GPU memory pooling
  - Ollama integration

Phase 5: Advanced Memory
  - Distributed page cache
  - Prefetching
  - Compression
  - NUMA-aware placement

Phase 6: Container Support
  - Docker workload execution
  - Container migration
  - Snapshot and restore
```

---

## Development Setup

### Prerequisites

```
Required:
  - Rust 1.75+
  - Python 3.10+ (for SDK development)
  - Node.js 18+ (for SDK development)
  - Git

Optional:
  - Protobuf compiler (protoc)
  - Tailscale (for remote access)
  - VS Code with Rust analyzer
```

### Build from Source

```bash
# Clone repository
git clone https://github.com/luci-dev/luci.git
cd luci

# Build all components
cargo build --release

# Run tests
cargo test

# Build CLI
cargo build --release --bin luci

# Build daemon
cargo build --release --bin lucid
```

### Development Commands

```bash
# Start daemon in development mode
cargo run --bin lucid -- --dev

# Run CLI against local daemon
cargo run --bin luci -- resources

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run --bin lucid
```

---

## Technology Stack

### Core

| Component | Technology | Reason |
|-----------|------------|--------|
| CLI | Rust | Fast startup, small binary |
| Daemon | Rust | Memory safety, performance |
| Runtime | Rust | Low-level control |
| Protocol | Protobuf | Efficient serialization |
| Transport | QUIC | Low latency, multiplexing |
| Encryption | TLS 1.3 | Industry standard |
| Storage | RocksDB | Fast key-value store |
| Logging | tracing | Structured logging |

### SDKs

| Language | Binding | Use Case |
|----------|---------|----------|
| Python | PyO3 + maturin | AI/ML workloads |
| Node.js | NAPI-RS | Web backends |
| Rust | Native | Systems programming |

### Infrastructure

| Component | Technology | Purpose |
|-----------|------------|---------|
| Service (Windows) | NSSM | Easy service management |
| Service (Linux) | systemd | Standard Linux services |
| Service (macOS) | LaunchDaemon | Standard macOS services |
| Installer | Shell scripts | Cross-platform install |
| Updates | Self-update | Auto-update mechanism |

---

## File Structure

```
luci/
├── Cargo.toml                    # Workspace root
├── crates/
│   ├── luci-cli/                 # CLI binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── commands/
│   │       │   ├── mod.rs
│   │       │   ├── create.rs
│   │       │   ├── join.rs
│   │       │   ├── run.rs
│   │       │   ├── memory.rs
│   │       │   ├── ai.rs
│   │       │   └── status.rs
│   │       └── output/
│   │           ├── mod.rs
│   │           └── format.rs
│   │
│   ├── lucid/                    # Daemon binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── daemon/
│   │       │   ├── mod.rs
│   │       │   ├── scheduler.rs
│   │       │   ├── runtime.rs
│   │       │   └── heartbeat.rs
│   │       ├── memory/
│   │       │   ├── mod.rs
│   │       │   ├── manager.rs
│   │       │   ├── page_table.rs
│   │       │   ├── cache.rs
│   │       │   └── prefetch.rs
│   │       ├── network/
│   │       │   ├── mod.rs
│   │       │   ├── discovery.rs
│   │       │   ├── transport.rs
│   │       │   └── protocol.rs
│   │       └── security/
│   │           ├── mod.rs
│   │           ├── auth.rs
│   │           ├── crypto.rs
│   │           └── sandbox.rs
│   │
│   ├── luci-runtime/             # Runtime library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── buffer.rs
│   │       ├── array.rs
│   │       ├── tensor.rs
│   │       └── executor.rs
│   │
│   ├── luci-protocol/            # Protocol definitions
│   │   ├── Cargo.toml
│   │   ├── proto/
│   │   │   ├── messages.proto
│   │   │   ├── resources.proto
│   │   │   └── memory.proto
│   │   └── src/
│   │       └── lib.rs
│   │
│   └── luci-common/              # Shared utilities
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── config.rs
│           ├── error.rs
│           └── types.rs
│
├── sdks/
│   ├── python/
│   │   ├── pyproject.toml
│   │   └── src/luci/
│   │       ├── __init__.py
│   │       ├── client.py
│   │       ├── memory.py
│   │       └── tensor.py
│   │
│   ├── nodejs/
│   │   ├── package.json
│   │   └── src/
│   │       ├── index.ts
│   │       ├── client.ts
│   │       └── memory.ts
│   │
│   └── rust/
│       └── luci-sdk/
│           ├── Cargo.toml
│           └── src/
│               └── lib.rs
│
├── docs/
│   ├── architecture.md
│   ├── commands.md
│   ├── memory-model.md
│   ├── security.md
│   ├── networking.md
│   └── contributing.md
│
├── install/
│   ├── install.sh
│   ├── install.ps1
│   └── uninstall.sh
│
├── tests/
│   ├── integration/
│   ├── unit/
│   └── benchmarks/
│
├── .github/
│   ├── workflows/
│   │   ├── ci.yml
│   │   └── release.yml
│   └── ISSUE_TEMPLATE/
│       ├── bug_report.md
│       └── feature_request.md
│
├── Cargo.toml
├── README.md
├── LICENSE
└── CHANGELOG.md
```

---

## Contributing

### Development Workflow

```bash
# Fork and clone
git clone https://github.com/YOUR_USERNAME/luci.git
cd luci

# Create feature branch
git checkout -b feature/amazing-feature

# Make changes
# ...

# Run tests
cargo test

# Run linter
cargo clippy -- -D warnings

# Format code
cargo fmt

# Commit
git commit -m "Add amazing feature"

# Push
git push origin feature/amazing-feature

# Create Pull Request
```

### Code Standards

- Follow Rust idioms and conventions
- Write documentation for public APIs
- Add tests for new functionality
- Keep commits atomic and well-described
- Run `cargo clippy` and `cargo fmt` before committing

---

## License

MIT License

```
MIT License

Copyright (c) 2024 Luci Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

---

## Summary

**Luci** is a distributed runtime that transforms idle personal computers into a unified compute platform.

**Core Features:**
- One CLI, one daemon, zero configuration
- Automatic device discovery and resource monitoring
- Distributed job execution with smart scheduling
- Virtual memory pooling across machines
- AI model sharding and inference
- End-to-end encryption and sandboxing

**Target:** Build MVP in 15 days, starting with core infrastructure and ending with SDKs and installers.

**Next Steps:**
1. Initialize Rust workspace
2. Implement core daemon
3. Build CLI
4. Add node discovery
5. Implement job execution
6. Add memory management
7. Build SDKs
8. Create installers
9. Release v0.1.0

---

*Built with Rust, QUIC, and a vision to make distributed computing accessible to everyone.*
