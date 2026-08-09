# Luci Installer for Windows
# Run: powershell -ExecutionPolicy Bypass -File install.ps1

$ErrorActionPreference = "Stop"

$InstallDir = "C:\luci"
$BuildDir = Join-Path $PSScriptRoot "..\target\release"

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Luci CLI Installer" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check build exists
if (!(Test-Path "$BuildDir\luci.exe")) {
    Write-Host "Build not found. Run 'cargo build --release' first." -ForegroundColor Red
    exit 1
}

# Create install directory
if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir | Out-Null
}

# Copy binaries
Write-Host "Installing to $InstallDir..." -ForegroundColor Yellow
Copy-Item "$BuildDir\luci.exe" "$InstallDir\luci.exe" -Force
Copy-Item "$BuildDir\lucid.exe" "$InstallDir\lucid.exe" -Force
Write-Host "  Copied luci.exe" -ForegroundColor Green
Write-Host "  Copied lucid.exe" -ForegroundColor Green

# Add to user PATH
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$UserPath;$InstallDir", "User")
    $env:Path += ";$InstallDir"
    Write-Host "  Added to PATH" -ForegroundColor Green
} else {
    Write-Host "  Already in PATH" -ForegroundColor Green
}

# Add firewall rules (no admin needed for user rules)
Write-Host "Configuring firewall..." -ForegroundColor Yellow

try {
    # Remove old rules if they exist
    Remove-NetFirewallRule -DisplayName "Luci Node Communication" -ErrorAction SilentlyContinue
    Remove-NetFirewallRule -DisplayName "Luci UDP Discovery" -ErrorAction SilentlyContinue
    Remove-NetFirewallRule -DisplayName "Luci Daemon" -ErrorAction SilentlyContinue

    # Allow lucid.exe to accept inbound TCP connections on port 9528
    New-NetFirewallRule -DisplayName "Luci Daemon" -Direction Inbound -Action Allow -Program "$InstallDir\lucid.exe" -Protocol TCP -LocalPort 9528 -Profile Private
    Write-Host "  Added rule: Luci Daemon (TCP:9528)" -ForegroundColor Green

    # Allow UDP broadcast on port 9529 for discovery
    New-NetFirewallRule -DisplayName "Luci UDP Discovery" -Direction Inbound -Action Allow -Program "$InstallDir\lucid.exe" -Protocol UDP -LocalPort 9529 -Profile Private
    Write-Host "  Added rule: Luci UDP Discovery (UDP:9529)" -ForegroundColor Green

    # Allow lucid.exe to send outbound connections
    New-NetFirewallRule -DisplayName "Luci Node Communication" -Direction Outbound -Action Allow -Program "$InstallDir\lucid.exe" -Protocol TCP -RemotePort 9528 -Profile Private
    Write-Host "  Added rule: Outbound TCP:9528" -ForegroundColor Green

    New-NetFirewallRule -DisplayName "Luci Node Outbound UDP" -Direction Outbound -Action Allow -Program "$InstallDir\lucid.exe" -Protocol UDP -RemotePort 9529 -Profile Private
    Write-Host "  Added rule: Outbound UDP:9529" -ForegroundColor Green
} catch {
    Write-Host "  Firewall rules skipped (run as Admin for auto-setup)" -ForegroundColor Yellow
    Write-Host "  Or run manually:" -ForegroundColor Yellow
    Write-Host '    netsh advfirewall firewall add rule name="Luci" dir=in action=allow program="C:\luci\lucid.exe"' -ForegroundColor Gray
}

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  Done!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  On THIS machine:" -ForegroundColor White
Write-Host "    lucid          # Start daemon" -ForegroundColor Yellow
Write-Host "    luci status    # Check status" -ForegroundColor Yellow
Write-Host ""
Write-Host "  On ANOTHER machine:" -ForegroundColor White
Write-Host "    1. Copy C:\luci\luci.exe and C:\luci\lucid.exe to it" -ForegroundColor Gray
Write-Host "    2. Run this same install script on that machine" -ForegroundColor Gray
Write-Host "    3. Run: lucid" -ForegroundColor Yellow
Write-Host "    4. Daemons auto-discover each other!" -ForegroundColor Green
Write-Host ""
