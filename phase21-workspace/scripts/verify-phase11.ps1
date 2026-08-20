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

Write-Host 'AetherCore Phase 11 — Aether Design System & Apple Fluid Motion' -ForegroundColor Cyan

$phase10 = @{}
foreach ($name in @('LiveTelemetry','IncludePhase5Inventory','ReleasePackaging','InstallerLifecycle','LibFuzzer','SkipOnlineSupplyChain','RequireSigning','UserShellPrivilegeCheck')) {
    if (Get-Variable -Name $name -ValueOnly) { $phase10[$name] = $true }
}
& (Join-Path $PSScriptRoot 'verify-phase10.ps1') @phase10
if ($LASTEXITCODE -ne 0) { throw 'Inherited Phase 0-10 gate failed.' }

Write-Host 'Running Phase 11 design-system and fluid-interaction source audit...' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'phase11-design-audit.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Phase 11 design-system source audit failed.' }

Write-Host 'Running deterministic spring / momentum / interruption tests...' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'test-phase11-motion.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Phase 11 motion physics tests failed.' }

Write-Host 'Rechecking decomposed Svelte/TypeScript renderer after Phase 11 gates...' -ForegroundColor Cyan
& pnpm --dir apps/ui check
if ($LASTEXITCODE -ne 0) { throw 'Phase 11 Svelte type/accessibility check failed.' }
& pnpm --dir apps/ui build
if ($LASTEXITCODE -ne 0) { throw 'Phase 11 production renderer build failed.' }

Write-Host 'Running platform-neutral Phase 0-11 invariants...' -ForegroundColor Cyan
& python scripts/static_validate.py
if ($LASTEXITCODE -ne 0) { throw 'Phase 0-11 static validation failed.' }

Write-Host 'Phase 11 authoritative Windows verification gate passed.' -ForegroundColor Green
Write-Host 'Feature decomposition, event-driven rendering, fluid springs, tactile primitives and accessibility adaptations are release-gated.' -ForegroundColor Green
if (-not $InstallerLifecycle) {
    Write-Host 'GA qualification still requires -InstallerLifecycle on a disposable Windows VM.' -ForegroundColor DarkYellow
}
if (-not $UserShellPrivilegeCheck) {
    Write-Host 'Run -UserShellPrivilegeCheck from a normal installed non-elevated user session before GA.' -ForegroundColor DarkYellow
}
Write-Host 'Motion-quality qualification additionally requires the documented 60/120/144 Hz, DPI, RTL and accessibility visual matrix on Windows 11 hardware.' -ForegroundColor DarkYellow
