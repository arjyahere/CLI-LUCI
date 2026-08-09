Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path $PSScriptRoot -Parent
$target = Join-Path $repoRoot 'luci\target'

if (Test-Path $target) {
    Remove-Item -Recurse -Force $target
    Write-Host "Removed $target"
} else {
    Write-Host "Nothing to clean: $target not found"
}
