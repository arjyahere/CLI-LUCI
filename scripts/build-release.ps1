Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path $PSScriptRoot -Parent
$luciDir = Join-Path $repoRoot 'luci'

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    throw 'cargo not found on PATH. Install Rust from https://rustup.rs/'
}

Push-Location $luciDir
try {
    cargo build --release
} finally {
    Pop-Location
}
