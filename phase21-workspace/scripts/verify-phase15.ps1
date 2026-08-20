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
    [switch]$LiveReadOnlySchedulerProbe,
    [string]$UpdateTrustPath
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
Write-Host 'AetherCore Phase 15 — Secure In-App Update & Cryptographic Diagnostic Export' -ForegroundColor Cyan

# Preserve every Phase 0-14 security/reliability/scheduler/UI gate, but deliberately defer
# packaging and lifecycle work until Phase 15 cryptographic/update boundaries qualify.
$phase14=@{}
foreach($name in @('LiveTelemetry','IncludePhase5Inventory','LibFuzzer','SkipOnlineSupplyChain','UserShellPrivilegeCheck','LiveReadOnlyFaultInjection','LiveReadOnlySchedulerProbe')){
    if(Get-Variable -Name $name -ValueOnly){$phase14[$name]=$true}
}
& (Join-Path $PSScriptRoot 'verify-phase14.ps1') @phase14
if($LASTEXITCODE -ne 0){throw 'Inherited Phase 0-14 gate failed.'}

& (Join-Path $PSScriptRoot 'phase15-security-audit.ps1')
if($LASTEXITCODE -ne 0){throw 'Phase 15 update/support security architecture audit failed.'}

Write-Host 'Compiling complete Rust workspace including user-scope downloader and fixed update broker...' -ForegroundColor Cyan
& cargo check --workspace --locked
if($LASTEXITCODE -ne 0){throw 'Phase 15 Windows Rust compile gate failed.'}

& (Join-Path $PSScriptRoot 'phase15-crypto-tests.ps1')
if($LASTEXITCODE -ne 0){throw 'Phase 15 cryptographic/tamper regression gate failed.'}

Write-Host 'Rechecking Phase 15 update/support Svelte integration...' -ForegroundColor Cyan
& pnpm --dir apps/ui check
if($LASTEXITCODE -ne 0){throw 'Phase 15 Svelte type/accessibility gate failed.'}
& pnpm --dir apps/ui build
if($LASTEXITCODE -ne 0){throw 'Phase 15 renderer production build failed.'}

& python scripts/phase12-localization-audit.py
if($LASTEXITCODE -ne 0){throw 'Phase 15 EN/AR catalog parity regression failed.'}
& python scripts/static_validate.py
if($LASTEXITCODE -ne 0){throw 'Phase 0-15 aggregate static validation failed.'}

if($ReleasePackaging -or $InstallerLifecycle){
    Write-Host 'Running reproducibility gate after all Phase 15 security/UI/crypto qualification...' -ForegroundColor Cyan
    & (Join-Path $PSScriptRoot 'verify-reproducible.ps1') -NativeDoubleBuild
    if($LASTEXITCODE -ne 0){throw 'Phase 15 reproducibility gate failed.'}

    if(-not $UpdateTrustPath -and $env:AETHERCORE_UPDATE_TRUST_PATH){$UpdateTrustPath=$env:AETHERCORE_UPDATE_TRUST_PATH}
    if($RequireSigning -and -not $UpdateTrustPath){throw 'Signed Phase 15 release requires UpdateTrustPath or protected AETHERCORE_UPDATE_TRUST_PATH.'}
    if($UpdateTrustPath){& (Join-Path $PSScriptRoot 'validate-update-trust.ps1') -Path $UpdateTrustPath -RequireEnabled:$RequireSigning}

    $releaseArgs=@{RequireSigning=[bool]$RequireSigning;SkipOnlineSupplyChain=[bool]$SkipOnlineSupplyChain}
    if($UpdateTrustPath){$releaseArgs['UpdateTrustPath']=$UpdateTrustPath}
    & (Join-Path $PSScriptRoot 'build-release.ps1') @releaseArgs
    if($LASTEXITCODE -ne 0){throw 'Phase 15 release packaging failed.'}

    $versionMatch=[regex]::Match((Get-Content 'Cargo.toml' -Raw),'(?ms)\[workspace\.package\].*?version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"')
    if(-not $versionMatch.Success){throw 'Unable to determine workspace package version.'}
    $version=$versionMatch.Groups[1].Value;$releaseRoot=Join-Path $Root "out\release\$version";$payload=Join-Path $releaseRoot 'payload'
    & (Join-Path $PSScriptRoot 'validate-update-trust.ps1') -Path (Join-Path $payload 'update-trust.json') -RequireEnabled:$RequireSigning
    if($LASTEXITCODE -ne 0){throw 'Packaged update trust validation failed.'}
    if(-not (Test-Path (Join-Path $payload 'aethercore-update-broker.exe'))){throw 'Packaged update broker is missing.'}

    if($InstallerLifecycle){
        $msi=Join-Path $releaseRoot "artifacts\AetherCore-$version-x64.msi"
        & (Join-Path $PSScriptRoot 'verify-installer-security.ps1') -MsiPath $msi -InstallLifecycle -AcknowledgeDisposableMachine -RequireSignedArtifacts:$RequireSigning
        if($LASTEXITCODE -ne 0){throw 'Phase 15 installer lifecycle/security verification failed.'}
    }
}

Write-Host 'Phase 15 authoritative Windows verification gate passed.' -ForegroundColor Green
Write-Host 'Update network authority is user-scope; service staging is descriptor/chunk/hash bound; execution is Update mutation leased; support export is previewed, sanitized, deterministic and independently verifiable.' -ForegroundColor Green
if(-not $InstallerLifecycle){Write-Host 'GA qualification still requires -InstallerLifecycle on a disposable Windows VM.' -ForegroundColor DarkYellow}
