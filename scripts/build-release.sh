#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
luci_dir="$repo_root/luci"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo not found on PATH. Install Rust from https://rustup.rs/" >&2
  exit 1
fi

cd "$luci_dir"
cargo build --release
