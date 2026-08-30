[CmdletBinding()]
param(
    [switch]$LiveTelemetry,
    [switch]$IncludePhase5Inventory,
    [switch]$ReleasePackaging,
    [switch]$InstallerLifecycle,
    [switch]$LibFuzzer,
    [switch]$SkipOnlineSupplyChain,
    [switch]$RequireSigning,
    [switch]$UserShellPrivilegeCheck
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root

Write-Host 'AetherCore Phase 10 — Operation Kernel & IPC v7 Evolution' -ForegroundColor Cyan

# Phase 9 is inherited wholesale. Dependency freeze remains verify-only outside the trusted
# refresh workstation; Windows security/installer/signing gates are never weakened by Phase 10.
$phase9 = @{}
foreach ($name in @('LiveTelemetry','IncludePhase5Inventory','ReleasePackaging','InstallerLifecycle','LibFuzzer','SkipOnlineSupplyChain','RequireSigning','UserShellPrivilegeCheck')) {
    if (Get-Variable -Name $name -ValueOnly) { $phase9[$name] = $true }
}
& (Join-Path $PSScriptRoot 'verify-phase9.ps1') @phase9
if ($LASTEXITCODE -ne 0) { throw 'Inherited Phase 0-9 gate failed.' }

Write-Host 'Running Phase 10 kernel/session source audit...' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'phase10-architecture-audit.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Phase 10 Operation Kernel / IPC source audit failed.' }

Write-Host 'Running Phase 10 kernel, transport, contract and journal tests...' -ForegroundColor Cyan
& cargo test --locked -p aethercore-contracts -p aethercore-operation-kernel -p aethercore-ipc -p aethercore-persistence -p aethercore-driver-install -p aethercore-system-repair -p aethercore-cleaner -p aethercore-startup-manager
if ($LASTEXITCODE -ne 0) { throw 'Phase 10 native kernel/session regression tests failed.' }

# Compile the decomposed Windows service and Tauri bridge as part of the current architecture gate.
Write-Host 'Checking decomposed service and persistent desktop session...' -ForegroundColor Cyan
& cargo check --locked -p aethercore-maintenance-service -p aethercore-desktop
if ($LASTEXITCODE -ne 0) { throw 'Phase 10 service/desktop compilation gate failed.' }
& cargo check --workspace --locked
if ($LASTEXITCODE -ne 0) { throw 'Phase 10 locked workspace check failed.' }
& cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { throw 'Phase 10 Rust formatting gate failed.' }

Write-Host 'Checking streaming Svelte/TypeScript renderer...' -ForegroundColor Cyan
& pnpm --dir apps/ui check
if ($LASTEXITCODE -ne 0) { throw 'Phase 10 UI type/accessibility check failed.' }
& pnpm --dir apps/ui build
if ($LASTEXITCODE -ne 0) { throw 'Phase 10 UI production build failed.' }

Write-Host 'Running platform-neutral Phase 0-10 invariants...' -ForegroundColor Cyan
& python scripts/static_validate.py
if ($LASTEXITCODE -ne 0) { throw 'Phase 0-10 static validation failed.' }

Write-Host 'Phase 10 authoritative Windows verification gate passed.' -ForegroundColor Green
Write-Host 'Persistent sessions, replay/reset semantics, global mutation exclusion and transient telemetry are now release-gated.' -ForegroundColor Green
if (-not $InstallerLifecycle) {
    Write-Host 'GA qualification still requires -InstallerLifecycle on a disposable Windows VM.' -ForegroundColor DarkYellow
}
if (-not $UserShellPrivilegeCheck) {
    Write-Host 'Run -UserShellPrivilegeCheck from a normal installed non-elevated user session before GA.' -ForegroundColor DarkYellow
}
