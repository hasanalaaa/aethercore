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

$phase7 = @{}
if ($LiveTelemetry) { $phase7.LiveTelemetry = $true }
if ($IncludePhase5Inventory) { $phase7.IncludePhase5Inventory = $true }
& (Join-Path $PSScriptRoot 'verify-phase7.ps1') @phase7
if ($LASTEXITCODE -ne 0) { throw 'Phase 0-7 verification failed.' }

if (-not (Test-Path 'Cargo.lock') -or -not (Test-Path 'pnpm-lock.yaml') -or -not (Test-Path 'release\dependency-locks.sha256') -or -not (Test-Path 'release\dependency-manifests.sha256') -or -not (Test-Path 'release\dependency-freeze.json')) {
    throw 'Phase 8 inherited gate requires the complete approved dependency freeze. Run scripts\freeze-dependencies.ps1 on the trusted freeze workstation first.'
}
& (Join-Path $PSScriptRoot 'freeze-dependencies.ps1') -VerifyOnly
if ($LASTEXITCODE -ne 0) { throw 'Dependency lock baseline verification failed.' }

Write-Host 'Running Phase 8 security boundary audit...' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'security-hardening-audit.ps1')
if ($LASTEXITCODE -ne 0) { throw 'Security hardening source audit failed.' }

Write-Host 'Running hardened IPC/security tests...' -ForegroundColor Cyan
& cargo test --locked -p aethercore-ipc -p aethercore-security -p aethercore-cleaner
if ($LASTEXITCODE -ne 0) { throw 'Phase 8 native security tests failed.' }
& (Join-Path $PSScriptRoot 'run-ipc-fuzz.ps1') -LibFuzzer:$LibFuzzer
if ($LASTEXITCODE -ne 0) { throw 'IPC fuzz validation failed.' }

Write-Host 'Checking full locked workspace...' -ForegroundColor Cyan
& cargo check --workspace --locked
if ($LASTEXITCODE -ne 0) { throw 'Locked Rust workspace check failed.' }
& cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { throw 'Rust formatting gate failed.' }

$epochFile = Join-Path $Root 'release\source-date-epoch.txt'
if (-not $env:SOURCE_DATE_EPOCH -and (Test-Path $epochFile)) { $env:SOURCE_DATE_EPOCH = (Get-Content $epochFile -Raw).Trim() }
& (Join-Path $PSScriptRoot 'verify-reproducible.ps1') -NativeDoubleBuild:$ReleasePackaging
if ($LASTEXITCODE -ne 0) { throw 'Reproducibility control gate failed.' }

if (-not $SkipOnlineSupplyChain) {
    Write-Host 'Running online dependency/security/license audit...' -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot 'audit-dependencies.ps1')
    if ($LASTEXITCODE -ne 0) { throw 'Supply-chain audit failed.' }
}

if ($ReleasePackaging -or $InstallerLifecycle) {
    Write-Host 'Building Phase 8 production release candidate...' -ForegroundColor Cyan
    # The supply-chain audit above is authoritative for this gate; avoid downloading/auditing it twice inside the packaging step.
    & (Join-Path $PSScriptRoot 'build-release.ps1') -RequireSigning:$RequireSigning -SkipOnlineSupplyChain
    if ($LASTEXITCODE -ne 0) { throw 'Production release packaging failed.' }
    $version = (Get-Content 'Cargo.toml' -Raw | Select-String -Pattern '(?ms)\[workspace\.package\].*?version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"').Matches[0].Groups[1].Value
    $msi = Join-Path $Root "out\release\$version\artifacts\AetherCore-$version-x64.msi"
    if ($InstallerLifecycle) {
        & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -InstallLifecycle -AcknowledgeDisposableMachine -RequireSignedArtifacts:$RequireSigning
        if ($LASTEXITCODE -ne 0) { throw 'Installer lifecycle/security verification failed.' }
    }
}


if ($UserShellPrivilegeCheck) {
    Write-Host 'Verifying non-elevated desktop token boundary...' -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot 'verify-user-shell-privilege.ps1')
    if ($LASTEXITCODE -ne 0) { throw 'User-shell privilege separation verification failed.' }
}

Write-Host 'Phase 8 verification complete.' -ForegroundColor Green
if (-not $InstallerLifecycle) {
    Write-Host 'Run with -InstallerLifecycle only on a disposable Windows VM to validate install/repair/uninstall ACL and service behavior.' -ForegroundColor DarkYellow
    Write-Host 'After installation, run -UserShellPrivilegeCheck from a normal non-elevated user session to verify the desktop token boundary.' -ForegroundColor DarkYellow
}
