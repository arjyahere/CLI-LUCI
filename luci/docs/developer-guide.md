# Developer Setup Guide

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

| Area | What Works | Test Coverage |
|------|------------|---------------|
| CLI Build | Zero errors, zero warnings | Compiles clean on Windows/Linux/macOS |
| CLI Tests | 49 tests passing | parse_bytes, detect_job_type, display, IPC |
| Daemon Build | Zero errors, zero warnings | Compiles clean |
| Daemon Tests | 48 tests passing | IPC, distributed execution, integration |
| IPC Protocol | 22 command types | Length-prefixed JSON over TCP:9527 |
| Display System | ANSI colors, boxes, progress bars | Used by all 22 commands |
| Error Handling | No panicking unwrap() | Graceful IPC errors |
| Clippy | Zero warnings | All suggestions applied |

---

## Prerequisites

### Required

| Tool | Version | Purpose | Install |
|------|---------|---------|---------|
| Rust | 1.75+ | Build system | `rustup.rs` |
| Cargo | Bundled | Package manager | Included with Rust |
| Git | Any | Version control | OS package manager |

### Optional

| Tool | Version | Purpose | Install |
|------|---------|---------|---------|
| Python | 3.8+ | Running `.py` jobs | `python.org` |
| Node.js | 18+ | Running `.js` jobs | `nodejs.org` |
| nvidia-smi | Latest | GPU detection | NVIDIA drivers |
| VS Code | Latest | IDE | `code.visualstudio.com` |
| rust-analyzer | Latest | Rust LSP | VS Code extension |

---

## Step-by-Step Setup

### Step 1: Install Rust

```bash
# Windows (PowerShell)
winget install Rustlang.Rustup

# Linux/macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Verify installation
rustc --version    # Should output: rustc 1.75.0 or later
cargo --version    # Should output: cargo 1.75.0 or later
```

### Step 2: Clone Repository

```bash
git clone <repo-url>
cd luci

# Verify workspace structure
ls Cargo.toml           # Workspace root
ls crates/              # 4 crates
ls sdks/                # SDKs
ls docs/                # Documentation
```

### Step 3: Build Project

```bash
# Debug build (faster compile, larger binaries)
cargo build

# Release build (optimized, smaller binaries)
cargo build --release

# Verify binaries exist
ls target/debug/luci.exe      # CLI (debug)
ls target/debug/luci.exe     # Unified CLI + daemon (debug)
ls target/release/luci.exe    # CLI (release, ~2.24 MB)
ls target/release/luci.exe   # Unified CLI + daemon (release)
```

### Step 4: Run Tests

```bash
# Run all tests (97 total)
cargo test --all

# Run only CLI tests (49 tests)
cargo test -p luci

# Run only daemon tests (48 tests)
cargo test -p lucid

# Run with output
cargo test --all -- --nocapture

# Expected output:
# test result: ok. 49 passed; 0 failed; 0 ignored
# test result: ok. 48 passed; 0 failed; 0 ignored
```

### Step 5: Start Daemon

```bash
# Terminal 1: Start daemon
target/release/luci.exe      # Windows
./target/release/luci        # Linux/macOS

# Or use CLI command
luci start
```

### Step 6: Verify Installation

```bash
# Terminal 2: Check status
luci status

# Should show daemon status with PID, uptime, node ID
# Should show zero build warnings
# Should show zero clippy warnings
```

---

## Project Structure

```
luci/
├── Cargo.toml                    # Workspace definition
├── crates/
│   ├── luci-common/              # Shared types and protocols
│   │   └── src/
│   │       ├── types.rs          # NodeId, Job, MemoryPool, ResourceSnapshot
│   │       ├── config.rs         # Config load/save
│   │       ├── ipc.rs            # IpcCommand (22 variants), IpcResponse
│   │       └── error.rs          # Error enum
│   │
│   ├── luci-cli/                 # CLI binary
│   │   └── src/
│   │       ├── main.rs           # Clap CLI, command dispatch
│   │       ├── display.rs        # ANSI colors, banner, formatting
│   │       ├── ipc_client.rs     # TCP client to daemon
│   │       └── commands/         # 22 command modules
│   │           ├── mod.rs
│   │           ├── create.rs
│   │           ├── join.rs
│   │           ├── leave.rs
│   │           ├── devices.rs
│   │           ├── device.rs
│   │           ├── resources.rs
│   │           ├── run.rs        # parse_bytes, detect_job_type, RunConfig
│   │           ├── jobs.rs
│   │           ├── job.rs
│   │           ├── cancel.rs
│   │           ├── memory.rs     # create, list, info, remove
│   │           ├── status.rs
│   │           ├── daemon.rs     # start, stop, restart
│   │           ├── logs.rs
│   │           ├── monitor.rs
│   │           ├── sync_files.rs
│   │           ├── proxy.rs      # start, stop, status
│   │           ├── failover.rs
│   │           ├── peers.rs
│   │           ├── health.rs
│   │           └── metrics.rs
│   │
│   ├── lucid/                    # Daemon library/crate
│   │   └── src/
│   │       ├── main.rs           # Entry point, logging, firewall
│   │       ├── ipc_server.rs     # IPC handler
│   │       ├── lib.rs            # Library exports
│   │       └── daemon/
│   │           ├── mod.rs        # DaemonState, run()
│   │           ├── resource.rs   # sysinfo + nvidia-smi
│   │           ├── scheduler.rs  # Score-based node selection
│   │           ├── network.rs    # TCP:9528, TLS support
│   │           ├── discovery.rs  # UDP broadcast
│   │           └── job.rs        # JobManager, distributed exec
│   │
│   ├── luci-protocol/            # Wire protocol
│   │   └── src/
│   │       ├── codec.rs          # LengthPrefixedCodec<T>
│   │       └── message.rs        # ProtocolMessage, 24 variants
│   │
│   └── luci-runtime/             # Distributed runtime
│       └── src/
│           ├── buffer.rs         # DistributedBuffer
│           └── executor.rs       # DistributedExecutor
│
├── sdks/
│   └── rust/                     # Rust SDK
│
├── tests/
│   └── integration/daemon.rs     # Integration tests
│
├── docs/                         # Documentation
│   ├── commands.md               # Commands reference
│   ├── architecture.md           # Architecture overview
│   ├── developer-guide.md        # This file
│   ├── status.md                 # Project status
│   ├── security.md               # Security model
│   ├── networking.md             # Network protocol
│   ├── memory-model.md           # Memory model
│   └── contributing.md           # Contributing guide
│
└── install/                      # Install scripts
```

---

## Key Files Reference

### luci-common

| File | Purpose | Lines |
|------|---------|-------|
| `src/types.rs` | NodeId, Job, MemoryPool, ResourceSnapshot | ~400 |
| `src/config.rs` | Config load/save, defaults | ~150 |
| `src/ipc.rs` | IpcCommand (22 variants), IpcResponse | ~150 |
| `src/error.rs` | Error enum with thiserror | ~50 |

### luci-cli

| File | Purpose | Lines |
|------|---------|-------|
| `src/main.rs` | Clap CLI, command dispatch | ~300 |
| `src/display.rs` | ANSI colors, banner, formatting | ~250 |
| `src/ipc_client.rs` | TCP client, IPC protocol | ~60 |
| `src/commands/run.rs` | parse_bytes, detect_job_type, RunConfig | ~230 |
| `src/commands/*.rs` | 22 command modules | ~1500 total |

### lucid

| File | Purpose | Lines |
|------|---------|-------|
| `src/main.rs` | Daemon entry, logging | ~200 |
| `src/ipc_server.rs` | IPC handler | ~400 |
| `src/daemon/mod.rs` | DaemonState, run() | ~300 |
| `src/daemon/resource.rs` | Resource collection | ~200 |
| `src/daemon/scheduler.rs` | Node scoring | ~150 |
| `src/daemon/network.rs` | TCP communication | ~400 |
| `src/daemon/job.rs` | Job execution | ~300 |

---

## Testing

### Test Commands

```bash
# Run all tests
cargo test --all

# Run specific crate tests
cargo test -p luci-common    # 12 tests
cargo test -p luci           # 49 tests
cargo test -p lucid          # 48 tests

# Run with output
cargo test --all -- --nocapture

# Run specific test
cargo test -p luci test_parse_bytes_gb

# Run tests matching pattern
cargo test -p luci parse
```

### Test Structure

| Crate | Tests | Coverage |
|-------|-------|----------|
| luci-common | 12 | NodeId, JobId, InviteCode, Config, Metrics |
| luci-cli | 49 | parse_bytes, detect_job_type, display, IPC client |
| lucid | 48 | IPC commands, distributed execution, integration |
| **Total** | **97** | All passing, zero warnings |

### Adding Tests

```rust
// In any source file
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() {
        let result = my_function("input");
        assert_eq!(result, "expected");
    }

    #[test]
    fn test_error_case() {
        let result = my_function("");
        assert!(result.is_err());
    }
}
```

---

## Linting

### Clippy

```bash
# Run clippy
cargo clippy --all

# Run clippy with all targets
cargo clippy --all -- -D warnings

# Auto-fix clippy warnings
cargo clippy --fix --allow-dirty --allow-no-vcs --all
```

### Formatting

```bash
# Check formatting
cargo fmt --all -- --check

# Apply formatting
cargo fmt --all
```

---

## Debugging

### Enable Logging

```bash
# Debug level
RUST_LOG=debug cargo run --bin lucid

# Trace level (very verbose)
RUST_LOG=trace cargo run --bin lucid

# Specific module
RUST_LOG=lucid::daemon=debug cargo run --bin lucid
```

### Common Debug Commands

```bash
# Check daemon is running
luci status

# View logs
luci logs --follow

# Test IPC manually
luci --port 9527 status

# Run specific test
cargo test -p luci test_parse_bytes_gb -- --nocapture
```

---

## Development Workflow

### 1. Make Changes

```bash
# Edit files
vim crates/luci-cli/src/commands/my_command.rs

# Check compilation
cargo check

# Run clippy
cargo clippy --all

# Format code
cargo fmt --all
```

### 2. Test Changes

```bash
# Run all tests
cargo test --all

# Run specific crate tests
cargo test -p luci

# Run with output
cargo test -p luci -- --nocapture
```

### 3. Build Release

```bash
# Build release
cargo build --release

# Verify binary
ls -la target/release/luci.exe
ls -la target/release/luci.exe
```

---

## Common Tasks

### Add New IPC Command

1. Add variant to `IpcCommand` in `crates/luci-common/src/ipc.rs`
2. Add handler in `crates/lucid/src/ipc_server.rs`
3. Add CLI command in `crates/luci-cli/src/commands/`
4. Add to match in `crates/luci-cli/src/main.rs`
5. Add tests

### Add New CLI Command

1. Create `crates/luci-cli/src/commands/my_command.rs`
2. Add `pub mod my_command;` to `crates/luci-cli/src/commands/mod.rs`
3. Add variant to `Commands` enum in `main.rs`
4. Add match arm in `main()` function
5. Add tests

### Add New Display Function

1. Add function to `crates/luci-cli/src/display.rs`
2. Use in command files
3. Add tests

---

## Architecture Decisions

| Decision | Choice | Reason |
|----------|--------|--------|
| IPC transport | TCP | Cross-platform |
| IPC framing | Length-prefixed JSON | Simple, debuggable |
| Resource collection | sysinfo crate | Cross-platform |
| GPU detection | nvidia-smi | Only option without CUDA |
| Scheduler | Score-based | Flexible, tunable weights |
| Invite codes | base64url, 15 chars | Short, URL-safe |
| Config storage | JSON file | Simple, human-readable |
| Job execution | Direct spawn | No container overhead |
| Protocol | 24 message types | Covers all use cases |
| Buffer | Page-based (4KB) | Standard page size |
| Discovery | UDP broadcast | Simple LAN discovery |
| Heartbeat | TCP-based | Reliable delivery |
| Auth | Auto-generated UUID | Defense-in-depth |
| Logging | Dual file + stdout | Debug in file, info on console |
