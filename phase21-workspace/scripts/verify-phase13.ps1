[CmdletBinding()]
param(
    [switch]$LiveTelemetry,
    [switch]$IncludePhase5Inventory,
    [switch]$ReleasePackaging,
    [switch]$InstallerLifecycle,
    [switch]$LibFuzzer,
    [switch]$SkipOnlineSupplyChain,
    [switch]$RequireSigning,
    [switch]$UserShellPrivilegeCheck,
    [switch]$LiveReadOnlyFaultInjection
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
Write-Host 'AetherCore Phase 13 — Engine Reliability Evolution' -ForegroundColor Cyan

# Preserve the complete Phase 0-12 source/security/design/localization gate, while deferring
# packaging/signing until the new native reliability tests have passed.
$phase12=@{}
foreach($name in @('LiveTelemetry','IncludePhase5Inventory','LibFuzzer','SkipOnlineSupplyChain','UserShellPrivilegeCheck')){
    if(Get-Variable -Name $name -ValueOnly){$phase12[$name]=$true}
}
& (Join-Path $PSScriptRoot 'verify-phase12.ps1') @phase12
if($LASTEXITCODE -ne 0){throw 'Inherited Phase 0-12 gate failed.'}

Write-Host 'Running Phase 13 reliability source audit...' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'phase13-reliability-audit.ps1')
if($LASTEXITCODE -ne 0){throw 'Phase 13 reliability audit failed.'}

Write-Host 'Compiling the complete Rust workspace with Windows bindings...' -ForegroundColor Cyan
& cargo check --workspace --locked
if($LASTEXITCODE -ne 0){throw 'Phase 13 Windows Rust compile gate failed.'}

Write-Host 'Running collector-runtime / parser / isolation unit tests...' -ForegroundColor Cyan
& cargo test --locked -p aethercore-collector-runtime -p aethercore-hardware-telemetry -p aethercore-crash-diagnostics -p aethercore-diagnostic-engine
if($LASTEXITCODE -ne 0){throw 'Phase 13 reliability unit/integration tests failed.'}

Write-Host 'Running deterministic fault injection cases...' -ForegroundColor Cyan
$fi=@{}
if($LiveReadOnlyFaultInjection){$fi['LiveReadOnly']=$true}
& (Join-Path $PSScriptRoot 'phase13-fault-injection.ps1') @fi
if($LASTEXITCODE -ne 0){throw 'Phase 13 fault injection gate failed.'}

Write-Host 'Re-running complete platform-neutral invariants...' -ForegroundColor Cyan
& python scripts/static_validate.py
if($LASTEXITCODE -ne 0){throw 'Phase 0-13 static validation failed.'}

if($ReleasePackaging -or $InstallerLifecycle){
    Write-Host 'Running reproducibility gate after Phase 13 reliability qualification...' -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot 'verify-reproducible.ps1') -NativeDoubleBuild
    if($LASTEXITCODE -ne 0){throw 'Phase 13 reproducibility gate failed.'}
    Write-Host 'Building release candidate after reliability qualification...' -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot 'build-release.ps1') -RequireSigning:$RequireSigning -SkipOnlineSupplyChain
    if($LASTEXITCODE -ne 0){throw 'Phase 13 release packaging failed.'}
    $versionMatch=[regex]::Match((Get-Content 'Cargo.toml' -Raw),'(?ms)\[workspace\.package\].*?version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"')
    if(-not $versionMatch.Success){throw 'Unable to determine workspace package version.'}
    $msi=Join-Path $Root "out\release\$($versionMatch.Groups[1].Value)\artifacts\AetherCore-$($versionMatch.Groups[1].Value)-x64.msi"
    if($InstallerLifecycle){
        & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -InstallLifecycle -AcknowledgeDisposableMachine -RequireSignedArtifacts:$RequireSigning
        if($LASTEXITCODE -ne 0){throw 'Phase 13 installer lifecycle/security verification failed.'}
    }
}

Write-Host 'Phase 13 authoritative Windows verification gate passed.' -ForegroundColor Green
Write-Host 'Finite WMI/EventLog waits, structured EvtRender parsing, bounded storage parsers, watchdog isolation and typed provider faults are release-gated.' -ForegroundColor Green
if(-not $LiveReadOnlyFaultInjection){Write-Host 'Run -LiveReadOnlyFaultInjection on a disposable Windows lab machine to probe real WMI/EventLog/storage providers.' -ForegroundColor DarkYellow}
if(-not $InstallerLifecycle){Write-Host 'GA qualification still requires -InstallerLifecycle on a disposable Windows VM.' -ForegroundColor DarkYellow}
