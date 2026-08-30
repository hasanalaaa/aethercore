[CmdletBinding()]
param(
    [switch]$LiveTelemetry,[switch]$IncludePhase5Inventory,[switch]$LibFuzzer,[switch]$SkipOnlineSupplyChain,
    [switch]$UserShellPrivilegeCheck,[switch]$LiveReadOnlyFaultInjection,[switch]$LiveReadOnlySchedulerProbe,
    [switch]$ReleasePackaging,[switch]$RequireSigning,[string]$UpdateTrustPath
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
Write-Host 'AetherCore Phase 16 — Final Production Qualification, Stress Matrix & GA Seal' -ForegroundColor Cyan
$phase15=@{}
foreach($name in @('LiveTelemetry','IncludePhase5Inventory','LibFuzzer','SkipOnlineSupplyChain','UserShellPrivilegeCheck','LiveReadOnlyFaultInjection','LiveReadOnlySchedulerProbe')){if(Get-Variable -Name $name -ValueOnly){$phase15[$name]=$true}}
& (Join-Path $PSScriptRoot 'verify-phase15.ps1') @phase15
if($LASTEXITCODE -ne 0){throw 'Inherited Phase 0-15 verification failed.'}
& (Join-Path $PSScriptRoot 'phase16-ga-audit.ps1');if($LASTEXITCODE -ne 0){throw 'Phase 16 architecture/GA policy audit failed.'}
& cargo check --workspace --locked;if($LASTEXITCODE -ne 0){throw 'Phase 16 full workspace compile gate failed.'}
& (Join-Path $PSScriptRoot 'invoke-cargo-test-case.ps1') -Package aethercore-ipc -TestName deterministic_malformed_frame_corpus_never_panics;if($LASTEXITCODE -ne 0){throw 'Phase 16 IPC malformed corpus regression failed.'}
& cargo build --locked --release -p aethercore-ga-probe;if($LASTEXITCODE -ne 0){throw 'Phase 16 live GA probe build failed.'}
& python scripts/static_validate.py;if($LASTEXITCODE -ne 0){throw 'Phase 0-16 aggregate static validation failed.'}
if($ReleasePackaging){
    Write-Host 'Running final reproducibility and signed release-candidate packaging after all Phase 16 source/native gates...' -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot 'verify-reproducible.ps1') -NativeDoubleBuild
    if($LASTEXITCODE -ne 0){throw 'Phase 16 reproducibility gate failed.'}
    if(-not $UpdateTrustPath -and $env:AETHERCORE_UPDATE_TRUST_PATH){$UpdateTrustPath=$env:AETHERCORE_UPDATE_TRUST_PATH}
    if($RequireSigning -and -not $UpdateTrustPath){throw 'Signed Phase 16 release requires UpdateTrustPath or protected AETHERCORE_UPDATE_TRUST_PATH.'}
    if($UpdateTrustPath){& (Join-Path $PSScriptRoot 'validate-update-trust.ps1') -Path $UpdateTrustPath -RequireEnabled:$RequireSigning;if($LASTEXITCODE -ne 0){throw 'Phase 16 update trust validation failed.'}}
    $releaseArgs=@{RequireSigning=[bool]$RequireSigning;SkipOnlineSupplyChain=[bool]$SkipOnlineSupplyChain};if($UpdateTrustPath){$releaseArgs['UpdateTrustPath']=$UpdateTrustPath}
    & (Join-Path $PSScriptRoot 'build-release.ps1') @releaseArgs
    if($LASTEXITCODE -ne 0){throw 'Phase 16 release candidate packaging failed.'}
    $versionMatch=[regex]::Match((Get-Content 'Cargo.toml' -Raw),'(?ms)\[workspace\.package\].*?version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"')
    if(-not $versionMatch.Success){throw 'Unable to determine workspace package version.'}
    $version=$versionMatch.Groups[1].Value;$payload=Join-Path $Root "out\release\$version\payload"
    & (Join-Path $PSScriptRoot 'validate-update-trust.ps1') -Path (Join-Path $payload 'update-trust.json') -RequireEnabled:$RequireSigning
    if($LASTEXITCODE -ne 0){throw 'Packaged update trust validation failed.'}
    if(-not(Test-Path (Join-Path $payload 'aethercore-update-broker.exe'))){throw 'Packaged update broker is missing.'}
}
Write-Host 'Phase 16 source/native master gate passed.' -ForegroundColor Green
Write-Host 'GA status is NOT conferred by this gate alone; verify-production.ps1 requires matrix, lifecycle, extended soak and resilience evidence before creating GA-SEAL.json.' -ForegroundColor DarkYellow
