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

Write-Host 'AetherCore Phase 12 — Full Localization & RTL Perfection' -ForegroundColor Cyan

# Run the complete inherited compile/security/design gate first, but deliberately defer
# release packaging/signing/lifecycle until Phase 12 localization and renderer checks pass.
$phase11 = @{}
foreach ($name in @('LiveTelemetry','IncludePhase5Inventory','LibFuzzer','SkipOnlineSupplyChain','UserShellPrivilegeCheck')) {
    if (Get-Variable -Name $name -ValueOnly) { $phase11[$name] = $true }
}
& (Join-Path $PSScriptRoot 'verify-phase11.ps1') @phase11
if ($LASTEXITCODE -ne 0) { throw 'Inherited Phase 0-11 gate failed.' }

Write-Host 'Running typed catalog, parity, bidi and service-message audit...' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'phase12-localization-audit.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Phase 12 localization audit failed.' }

Write-Host 'Running Arabic Intl/pluralization runtime checks...' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'test-phase12-i18n.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Phase 12 Intl/pluralization checks failed.' }

Write-Host 'Rechecking localized Svelte/TypeScript renderer...' -ForegroundColor Cyan
& pnpm --dir apps/ui check
if ($LASTEXITCODE -ne 0) { throw 'Phase 12 Svelte type/accessibility check failed.' }
& pnpm --dir apps/ui build
if ($LASTEXITCODE -ne 0) { throw 'Phase 12 production renderer build failed.' }

Write-Host 'Checking consent broker / desktop Rust after locale-safe UAC presentation changes...' -ForegroundColor Cyan
& cargo check --locked -p aethercore-consent-broker -p aethercore-desktop
if ($LASTEXITCODE -ne 0) { throw 'Phase 12 consent/desktop Rust check failed.' }

Write-Host 'Running platform-neutral Phase 0-12 invariants...' -ForegroundColor Cyan
& python scripts/static_validate.py
if ($LASTEXITCODE -ne 0) { throw 'Phase 0-12 static validation failed.' }

if ($ReleasePackaging -or $InstallerLifecycle) {
    Write-Host 'Running release reproducibility double-build after all Phase 12 source/UI gates...' -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot 'verify-reproducible.ps1') -NativeDoubleBuild
    if ($LASTEXITCODE -ne 0) { throw 'Phase 12 release reproducibility gate failed.' }

    Write-Host 'Building Phase 12 production release candidate only after localization qualification...' -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot 'build-release.ps1') -RequireSigning:$RequireSigning -SkipOnlineSupplyChain
    if ($LASTEXITCODE -ne 0) { throw 'Phase 12 production release packaging failed.' }

    $versionMatch = [regex]::Match((Get-Content 'Cargo.toml' -Raw), '(?ms)\[workspace\.package\].*?version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"')
    if (-not $versionMatch.Success) { throw 'Unable to determine workspace package version for Phase 12 installer verification.' }
    $version = $versionMatch.Groups[1].Value
    $msi = Join-Path $Root "out\release\$version\artifacts\AetherCore-$version-x64.msi"
    if ($InstallerLifecycle) {
        & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -InstallLifecycle -AcknowledgeDisposableMachine -RequireSignedArtifacts:$RequireSigning
        if ($LASTEXITCODE -ne 0) { throw 'Phase 12 installer lifecycle/security verification failed.' }
    }
}

Write-Host 'Phase 12 authoritative Windows verification gate passed.' -ForegroundColor Green
Write-Host 'EN/AR catalog parity, Arabic plural rules, semantic routing, technical-data isolation and localized UAC presentation are release-gated.' -ForegroundColor Green
if (-not $InstallerLifecycle) { Write-Host 'GA qualification still requires -InstallerLifecycle on a disposable Windows VM.' -ForegroundColor DarkYellow }
if (-not $UserShellPrivilegeCheck) { Write-Host 'Run -UserShellPrivilegeCheck from a normal installed non-elevated user session before GA.' -ForegroundColor DarkYellow }
Write-Host 'Final localization QA additionally requires Arabic Windows 11 + English Windows 11 manual review at 100/125/150/200% DPI with Narrator, high contrast and mixed technical strings.' -ForegroundColor DarkYellow
