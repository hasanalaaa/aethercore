[CmdletBinding()]
param(
    [switch]$LiveAssessment,
    [switch]$LiveCleanupScan,
    [switch]$IncludePhase3Probes
)

$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

& (Join-Path $PSScriptRoot 'verify.ps1')

Push-Location $Root
try {
    Write-Host 'Running Phase 4 System Repair coordinator tests...' -ForegroundColor Cyan
    cargo test -p aethercore-system-repair --test coordinator -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'System Repair coordinator tests failed' }

    Write-Host 'Running Phase 4 Cleanup coordinator tests...' -ForegroundColor Cyan
    cargo test -p aethercore-cleaner --test coordinator -- --nocapture
    if ($LASTEXITCODE -ne 0) { throw 'Cleanup coordinator tests failed' }

    if ($LiveAssessment -or $LiveCleanupScan) {
        if ($env:OS -ne 'Windows_NT') { throw 'Live Phase 4 probes require Windows.' }
    }

    if ($LiveAssessment) {
        $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
        $principal = [Security.Principal.WindowsPrincipal]::new($identity)
        if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
            throw 'The live DISM/SFC/CHKDSK assessment must run from an elevated PowerShell window.'
        }
        Write-Host 'Running read-only Windows integrity assessment...' -ForegroundColor Yellow
        cargo test -p aethercore-system-repair --test live_assessment -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Live System Repair assessment failed' }
    }

    if ($LiveCleanupScan) {
        Write-Host 'Running allowlist cleanup scan only; no deletion is performed...' -ForegroundColor Yellow
        cargo test -p aethercore-cleaner --test live_scan -- --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw 'Live cleanup scan failed' }
    }

    if ($IncludePhase3Probes) {
        & (Join-Path $PSScriptRoot 'verify-phase3.ps1') -LiveDiscovery
        if ($LASTEXITCODE -ne 0) { throw 'Phase 3 discovery probes failed' }
    }
}
finally {
    Pop-Location
}

Write-Host 'Phase 4 verification complete.' -ForegroundColor Green
Write-Host 'This script never automates cleanup deletion or DISM/SFC repair mutation. Execute real mutations only through the reviewed AetherCore service-plan/UAC path on a staging machine.' -ForegroundColor DarkYellow
