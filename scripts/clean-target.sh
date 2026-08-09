#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="$repo_root/luci/target"

if [ -d "$target" ]; then
  rm -rf "$target"
  echo "Removed $target"
else
  echo "Nothing to clean: $target not found"
fi
