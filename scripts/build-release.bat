@echo off
setlocal

set "repo_root=%~dp0.."
set "luci_dir=%repo_root%\luci"

where cargo >nul 2>nul
if errorlevel 1 (
  echo cargo not found on PATH. Install Rust from https://rustup.rs/ >&2
  exit /b 1
)

cd /d "%luci_dir%" || exit /b 1
cargo build --release
exit /b %errorlevel%
