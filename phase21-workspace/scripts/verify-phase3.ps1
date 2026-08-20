[CmdletBinding()]
param(
    [switch]$LiveDiscovery,
    [Alias('LiveProtectionProbe')]
    [switch]$LiveProtection
)

$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

& (Join-Path $PSScriptRoot 'verify.ps1')

Push-Location $Root
try {
    Write-Host 'Running Phase 3 coordinator integration test...' -ForegroundColor Cyan
    cargo test -p aethercore-driver-install --test coordinator -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Phase 3 coordinator integration test failed' }

    if ($LiveDiscovery) {
        if ($env:OS -ne 'Windows_NT') { throw 'Live driver discovery probes require Windows.' }
        Write-Host 'Running physical PnP smoke test...' -ForegroundColor Cyan
        cargo test -p aethercore-windows-pnp --test live_inventory -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Live PnP inventory smoke test failed' }

        Write-Host 'Running live WUA discovery smoke test...' -ForegroundColor Cyan
        cargo test -p aethercore-windows-update --test live_wua -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Live WUA discovery smoke test failed' }
    }

    if ($LiveProtection) {
        if ($env:OS -ne 'Windows_NT') { throw 'Live protection probes require Windows.' }
        $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
        $principal = [Security.Principal.WindowsPrincipal]::new($identity)
        if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
            throw 'Live protection probes must run from an elevated PowerShell window.'
        }

        Write-Host 'Creating, verifying, and cancelling a real System Restore probe...' -ForegroundColor Yellow
        cargo test -p aethercore-restore-point --test live_restore -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Live System Restore probe failed' }

        Write-Host 'Exporting one currently bound OEM driver package as a read-only backup probe...' -ForegroundColor Yellow
        cargo test -p aethercore-driver-backup --test live_backup -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Live driver-backup probe failed' }
    }
}
finally {
    Pop-Location
}

Write-Host 'Phase 3 verification complete.' -ForegroundColor Green
Write-Host 'No live driver installation is automated by this script. Mutation validation must use the reviewed AetherCore plan/UAC flow on sacrificial physical hardware.' -ForegroundColor DarkYellow
