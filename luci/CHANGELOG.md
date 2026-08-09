# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- CLI Dashboard UI with ANSI colors, box layouts, progress bars, status indicators
- 22 CLI commands with beautiful formatted output
- `parse_bytes()` function for RAM/VRAM string parsing (KB/MB/GB/TB)
- `detect_job_type()` function for script extension detection
- `RunConfig` struct for job configuration
- `display.rs` module with 25 formatting functions and utilities
- 49 CLI unit tests (parse_bytes, detect_job_type, display, IPC client)
- Pre-flight daemon checks in start/stop/restart commands
- Ctrl+C handling in logs follow mode
- File truncation in box layouts for long values
- Real-time monitoring with single-node and cluster views
- File synchronization with manifest-based diffing
- Reverse proxy management (start/stop/status)
- Auto-failover status display
- Peer discovery list
- Health check with status indicator
- Metrics in text, JSON, and Prometheus formats
- TLS encryption support for node-to-node communication
- Remote memory page read/write across nodes
- Job completion callback infrastructure
- PeerStream enum supporting TLS and plain TCP fallback

### Changed
- Refactored `run.rs` to use `RunConfig` struct (fixes too_many_arguments clippy warning)
- Updated `daemon.rs` to accept `&IpcClient` parameter for pre-flight checks
- Improved `resources.rs` to use `print_kv`, `status_indicator`, `print_kv_status`
- Enhanced `health.rs` with BRIGHT_GREEN/BRIGHT_RED status indicators
- Upgraded `proxy.rs` with BRIGHT_MAGENTA port display and status_indicator
- Enhanced `memory.rs` with MAGENTA pool name display
- Fixed `progress_bar()` to use `.clamp(0.0, 1.0)` instead of manual implementation
- Updated all documentation with new test counts and features

### Fixed
- Fixed `run.rs` ram/vram parameters being ignored (now parsed to bytes and sent to daemon)
- Fixed `monitor.rs` --node filter being ignored (now shows single-node view)
- Fixed `logs.rs` filter parameter being ignored (now displayed and used)
- Fixed `daemon.rs` missing pre-flight checks (now uses is_daemon_running())
- Fixed duplicate closing braces in display.rs
- Fixed format string argument count errors in logs.rs
- Fixed unused import warnings
- Fixed unused variable warnings

### Removed
- Removed `#![allow(dead_code)]` from display.rs
- Removed unused `dirs` dependency from luci-cli
- Removed unused display functions (table_row, table_separator, print_box, print_card, format_network_status)

## [0.1.0] - 2024-01-15

### Added
- Core daemon with resource monitoring
- CLI with all basic commands
- IPC communication between CLI and daemon
- Job submission and status tracking
- Memory pool creation and management
- Network protocol definitions
- Runtime buffer and executor
- Python SDK with client, memory, and job modules
- Node.js SDK with client, memory, and job modules
- Rust SDK with client, memory, and job modules
- Architecture documentation
- Commands reference
- Memory model documentation
- Networking documentation
- Security documentation
- Contributing guidelines
- Unit tests
- Integration tests
- CI/CD pipeline
- README with quick start guide
- MIT License
- Changelog

## [0.0.1] - 2024-01-01

### Added
- Project initialization
- Workspace setup
- Basic crate structure
