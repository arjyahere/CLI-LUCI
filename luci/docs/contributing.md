# Contributing to Luci

## Getting Started

1. Fork the repository
2. Clone your fork
3. Create a feature branch
4. Make your changes
5. Submit a pull request

## Development Setup

### Prerequisites

- Rust 1.75+ (via [rustup](https://rustup.rs/))
- Git
- (Optional) nvidia-smi for GPU testing

### Build

```bash
git clone <your-fork-url>
cd luci
cargo build
```

### Test

```bash
cargo test --all
```

### Lint

```bash
cargo clippy --all
cargo fmt --all -- --check
```

## Project Structure

```
luci/
├── crates/
│   ├── luci-common/     # Shared types — edit here for type changes
│   ├── luci-cli/        # CLI — edit here for new commands
│   ├── lucid/           # Daemon — edit here for daemon behavior
│   ├── luci-protocol/   # Wire protocol — edit here for message types
│   └── luci-runtime/    # Runtime — edit here for execution logic
├── sdks/
│   └── rust/            # Rust SDK
├── tests/               # Integration tests
└── docs/                # Documentation
```

## Adding a New Feature

### Adding a CLI Command

1. Add variant to `Commands` enum in `crates/luci-cli/src/main.rs`
2. Create new file `crates/luci-cli/src/commands/mycommand.rs`
3. Implement `pub async fn execute(client: &IpcClient, ...) -> Result<()>`
4. Add to `mod.rs` and match in `main.rs`
5. Add IPC command variant if needed

### Adding an IPC Command

1. Add variant to `IpcCommand` in `crates/luci-common/src/ipc.rs`
2. Add handler in `crates/lucid/src/ipc_server.rs` `handle_command()`
3. Add CLI command (see above)
4. Add test

### Adding a Protocol Message

1. Add variant to `MessageType` in `crates/luci-protocol/src/message.rs`
2. Add payload variant to `Payload`
3. Add to `message_type()` match
4. Add handler in `crates/lucid/src/daemon/network.rs` `process_message()`

## Code Style

- **No comments** unless requested
- Follow existing patterns in the file
- Use `anyhow::Result` for error handling
- Use `tracing` for logging (not `println!`)
- Prefer `&str` over `&String` in function parameters
- Use `snake_case` for variables/functions, `PascalCase` for types
- Zero warnings on `cargo build --release`
- Zero warnings on `cargo clippy --all`

## Commit Messages

- Use imperative mood ("Add feature" not "Added feature")
- Keep first line under 72 characters
- Reference issue numbers when applicable

## Pull Requests

1. Ensure `cargo build --release` succeeds with zero warnings
2. Ensure `cargo clippy --all` has zero warnings
3. Ensure `cargo test --all` passes (14 tests)
4. Update documentation if adding user-facing features
5. Keep PRs focused — one feature per PR

## Questions?

Open an issue or start a discussion on GitHub.
