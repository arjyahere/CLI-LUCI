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
if errorlevel 1 exit /b %errorlevel%

if not exist "%repo_root%\dist" mkdir "%repo_root%\dist"
copy /y "%luci_dir%\target\release\luci.exe" "%repo_root%\dist\" >nul
copy /y "%luci_dir%\target\release\lucid.exe" "%repo_root%\dist\" >nul
echo Copied binaries to "%repo_root%\dist"
exit /b 0
