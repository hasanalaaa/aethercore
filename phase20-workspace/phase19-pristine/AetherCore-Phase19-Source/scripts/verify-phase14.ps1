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
    [switch]$LiveReadOnlyFaultInjection,
    [switch]$LiveReadOnlySchedulerProbe
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
Write-Host 'AetherCore Phase 14 — Autonomous Maintenance Intelligence & Idle Scheduler' -ForegroundColor Cyan

$phase13=@{}
foreach($name in @('LiveTelemetry','IncludePhase5Inventory','LibFuzzer','SkipOnlineSupplyChain','UserShellPrivilegeCheck','LiveReadOnlyFaultInjection')){
    if(Get-Variable -Name $name -ValueOnly){$phase13[$name]=$true}
}
& (Join-Path $PSScriptRoot 'verify-phase13.ps1') @phase13
if($LASTEXITCODE -ne 0){throw 'Inherited Phase 0-13 gate failed.'}

& (Join-Path $PSScriptRoot 'phase14-scheduler-audit.ps1')
if($LASTEXITCODE -ne 0){throw 'Phase 14 scheduler architecture audit failed.'}

Write-Host 'Compiling complete Rust workspace with Phase 14 Windows state probes...' -ForegroundColor Cyan
& cargo check --workspace --locked
if($LASTEXITCODE -ne 0){throw 'Phase 14 Windows Rust compile gate failed.'}

& (Join-Path $PSScriptRoot 'phase14-scheduler-tests.ps1')
if($LASTEXITCODE -ne 0){throw 'Phase 14 deterministic scheduler tests failed.'}

$faultArgs=@{}
if($LiveReadOnlySchedulerProbe){$faultArgs['LiveReadOnly']=$true}
& (Join-Path $PSScriptRoot 'phase14-scheduler-fault-injection.ps1') @faultArgs
if($LASTEXITCODE -ne 0){throw 'Phase 14 scheduler/preemption fault-injection gate failed.'}

Write-Host 'Re-running complete platform-neutral invariants...' -ForegroundColor Cyan
& python scripts/static_validate.py
if($LASTEXITCODE -ne 0){throw 'Phase 0-14 static validation failed.'}

if($ReleasePackaging -or $InstallerLifecycle){
    Write-Host 'Running reproducibility gate after Phase 14 qualification...' -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot 'verify-reproducible.ps1') -NativeDoubleBuild
    if($LASTEXITCODE -ne 0){throw 'Phase 14 reproducibility gate failed.'}
    & (Join-Path $PSScriptRoot 'build-release.ps1') -RequireSigning:$RequireSigning -SkipOnlineSupplyChain
    if($LASTEXITCODE -ne 0){throw 'Phase 14 release packaging failed.'}
    $versionMatch=[regex]::Match((Get-Content 'Cargo.toml' -Raw),'(?ms)\[workspace\.package\].*?version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"')
    if(-not $versionMatch.Success){throw 'Unable to determine workspace package version.'}
    $msi=Join-Path $Root "out\release\$($versionMatch.Groups[1].Value)\artifacts\AetherCore-$($versionMatch.Groups[1].Value)-x64.msi"
    if($InstallerLifecycle){
        & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -InstallLifecycle -AcknowledgeDisposableMachine -RequireSignedArtifacts:$RequireSigning
        if($LASTEXITCODE -ne 0){throw 'Phase 14 installer lifecycle/security verification failed.'}
    }
}
Write-Host 'Phase 14 authoritative Windows verification gate passed.' -ForegroundColor Green
Write-Host 'Autonomous work is owner-bound, read-only, mutation-excluded, preemptible, jittered and resource-throttled.' -ForegroundColor Green
if(-not $InstallerLifecycle){Write-Host 'GA qualification still requires -InstallerLifecycle on a disposable Windows VM.' -ForegroundColor DarkYellow}
