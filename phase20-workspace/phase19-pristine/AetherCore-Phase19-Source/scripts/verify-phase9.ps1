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

Write-Host 'AetherCore Phase 9 — Security & Release Closure' -ForegroundColor Cyan

# Phase 8 remains an inherited gate. Phase 9 intentionally fails closed when the approved
# dependency freeze is absent; CI is not permitted to manufacture a release baseline implicitly.
$phase8 = @{}
if ($LiveTelemetry) { $phase8.LiveTelemetry = $true }
if ($IncludePhase5Inventory) { $phase8.IncludePhase5Inventory = $true }
if ($LibFuzzer) { $phase8.LibFuzzer = $true }
if ($SkipOnlineSupplyChain) { $phase8.SkipOnlineSupplyChain = $true }
if ($ReleasePackaging) { $phase8.ReleasePackaging = $true }
if ($InstallerLifecycle) { $phase8.InstallerLifecycle = $true }
if ($RequireSigning) { $phase8.RequireSigning = $true }
if ($UserShellPrivilegeCheck) { $phase8.UserShellPrivilegeCheck = $true }
& (Join-Path $PSScriptRoot 'verify-phase8.ps1') @phase8
if ($LASTEXITCODE -ne 0) { throw 'Inherited Phase 0-8 gate failed.' }

Write-Host 'Verifying Phase 9 dependency freeze (locks + dependency manifests)...' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'freeze-dependencies.ps1') -VerifyOnly
if ($LASTEXITCODE -ne 0) { throw 'Phase 9 dependency freeze verification failed.' }

Write-Host 'Running Phase 9 security/source audit...' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'phase9-security-audit.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Phase 9 source-security audit failed.' }

Write-Host 'Running principal/authorization/filesystem regression tests...' -ForegroundColor Cyan
& cargo test --locked -p aethercore-security -p aethercore-operation-engine -p aethercore-persistence -p aethercore-driver-hub -p aethercore-driver-install -p aethercore-system-repair -p aethercore-cleaner -p aethercore-startup-manager -p aethercore-diagnostic-engine
if ($LASTEXITCODE -ne 0) { throw 'Phase 9 native security/regression tests failed.' }

Write-Host 'Checking full locked workspace after production sanitation...' -ForegroundColor Cyan
& cargo check --workspace --locked
if ($LASTEXITCODE -ne 0) { throw 'Phase 9 locked workspace check failed.' }
& cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { throw 'Phase 9 Rust formatting gate failed.' }

Write-Host 'Checking Svelte/TypeScript shell after retired command removal...' -ForegroundColor Cyan
& pnpm --dir apps/ui check
if ($LASTEXITCODE -ne 0) { throw 'Phase 9 UI type/accessibility check failed.' }
& pnpm --dir apps/ui build
if ($LASTEXITCODE -ne 0) { throw 'Phase 9 UI production build failed.' }

Write-Host 'Running platform-neutral Phase 0-9 invariants...' -ForegroundColor Cyan
& python scripts/static_validate.py
if ($LASTEXITCODE -ne 0) { throw 'Phase 0-9 static validation failed.' }

Write-Host 'Phase 9 authoritative Windows verification gate passed.' -ForegroundColor Green
if (-not $InstallerLifecycle) {
    Write-Host 'GA qualification still requires -InstallerLifecycle on a disposable Windows VM.' -ForegroundColor DarkYellow
}
if (-not $UserShellPrivilegeCheck) {
    Write-Host 'Run -UserShellPrivilegeCheck from a normal installed non-elevated user session before GA.' -ForegroundColor DarkYellow
}
