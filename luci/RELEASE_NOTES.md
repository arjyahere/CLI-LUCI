# Luci Release Notes

## v0.1.0 (Initial Release)

**Release Date**: August 9, 2026

### Vision

**4 devices connected via Luci should work as ONE machine.**

When you connect 4 laptops/desktops via Luci, they become a single unified computer with:
- 64 CPU cores (16 × 4 devices)
- 64 GB RAM (16GB × 4 devices)
- 30 GB VRAM (2GB × 4 GPUs)
- 2 TB Storage (500GB × 4 devices)

### What's Included

#### Core Features
- **Resource Discovery**: Automatic device discovery via UDP broadcast
- **Resource Aggregation**: Unified view of CPU, RAM, GPU, disk across all nodes
- **Job Distribution**: Intelligent scheduler picks the best node automatically
- **Remote Execution**: Jobs executed on optimal node via TCP
- **Heartbeat System**: Continuous health monitoring (10s interval)
- **File Sync**: Manifest-based automatic file synchronization
- **Auto-Failover**: Failed jobs automatically retried on other nodes

#### Memory Distribution
- **Page-Based Memory**: 4KB pages distributed across nodes
- **Remote Read/Write**: Access memory pages on any node
- **DistributedArray**: Transparent array access across nodes (Rust/Python/Node.js)

#### GPU Pooling
- **Aggregated VRAM**: All GPUs appear as one pool
- **Automatic Selection**: System picks the best GPU for your job

#### Security
- **Node Authentication**: Ed25519 keypairs with JWT tokens
- **Job Sandboxing**: Resource limits enforced (CPU, RAM, time)
- **Rate Limiting**: Token bucket per-connection (100 req/min)
- **Audit Logging**: Structured log for all security events

#### Observability
- **Prometheus Metrics**: Full metrics in Prometheus text format
- **Distributed Tracing**: Cross-node trace correlation
- **Web Dashboard**: Real-time monitoring UI at port 9531

#### SDKs
- **Rust SDK**: Full client with DistributedArray
- **Python SDK**: Full client with DistributedArray
- **Node.js SDK**: Full client with async DistributedArray

### Installation

#### Linux/macOS
```bash
# Download and extract
tar xzf luci-linux-amd64-v0.1.0.tar.gz

# Run daemon
./lucid

# In another terminal
./luci status
```

#### Windows
```powershell
# Extract zip
Expand-Archive -Path luci-windows-amd64-v0.1.0.zip -DestinationPath C:\luci

# Run daemon
C:\luci\lucid.exe

# In another terminal
C:\luci\luci.exe status
```

### Quick Start

1. **Start the daemon** on each machine:
   ```bash
   lucid
   ```

2. **Create a network** on the first machine:
   ```bash
   luci create home
   ```

3. **Join from other machines** using the invite code:
   ```bash
   luci join <invite-code>
   ```

4. **Run a job** - system picks the best node:
   ```bash
   luci run python -- -c "print('Hello from the cluster!')"
   ```

5. **Monitor the cluster**:
   ```bash
   luci status
   luci devices
   luci monitor
   ```

### New Features in This Release

- Complete memory distribution with page-based storage
- GPU pooling with aggregated VRAM
- Node authentication with Ed25519
- Job sandboxing with resource limits
- Prometheus metrics endpoint
- Distributed tracing
- Web dashboard for monitoring
- Python and Node.js SDKs

### Known Limitations

- Single-machine testing only (multi-node testing in progress)
- NVIDIA GPUs only (AMD/Intel support planned)
- No web UI yet (CLI only, dashboard available at port 9531)

### Future Plans

- Web-based dashboard UI
- Container support (Docker/Podman)
- AMD/Intel GPU support
- WAN discovery (cross-network)
- Resource quotas and billing

### Support

- **Issues**: https://github.com/luci-runtime/luci/issues
- **Discussions**: https://github.com/luci-runtime/luci/discussions
- **Documentation**: https://docs.luci.dev

### Contributors

Thank you to all contributors who helped make this release possible!

---

**Full Changelog**: https://github.com/luci-runtime/luci/commits/v0.1.0
