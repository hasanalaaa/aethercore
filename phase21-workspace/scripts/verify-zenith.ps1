[CmdletBinding()]
param(
    [switch]$LiveTelemetry,[switch]$IncludePhase5Inventory,[switch]$LibFuzzer,[switch]$SkipOnlineSupplyChain,
    [switch]$UserShellPrivilegeCheck,[switch]$LiveReadOnlyFaultInjection,[switch]$LiveReadOnlySchedulerProbe,
    [switch]$RuntimeStress,[switch]$ExtendedSoak,[switch]$ReleasePackaging,[switch]$RequireSigning,[string]$UpdateTrustPath
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
Write-Host 'AetherCore Zenith — additive Enterprise + Zenith verification gate' -ForegroundColor Cyan

# First execute the inherited Enterprise gate without packaging. Packaging must happen only after
# the additional Zenith source/interaction/accessibility audit has passed.
$enterpriseArgs=@{}
foreach($name in @('LiveTelemetry','IncludePhase5Inventory','LibFuzzer','SkipOnlineSupplyChain','UserShellPrivilegeCheck','LiveReadOnlyFaultInjection','LiveReadOnlySchedulerProbe','RuntimeStress','ExtendedSoak')){
    if(Get-Variable -Name $name -ValueOnly){$enterpriseArgs[$name]=$true}
}
& (Join-Path $PSScriptRoot 'verify-enterprise.ps1') @enterpriseArgs
if($LASTEXITCODE -ne 0){throw 'Inherited Enterprise convergence gate failed.'}

& (Join-Path $PSScriptRoot 'zenith-adversarial-audit.ps1')
if($LASTEXITCODE -ne 0){throw 'Zenith adversarial source audit failed.'}

& (Join-Path $PSScriptRoot 'zenith-recursive-audit.ps1')
if($LASTEXITCODE -ne 0){throw 'Zenith recursive adversarial source audit failed.'}

if($ReleasePackaging){
    Write-Host 'Running authoritative Phase 16 packaging only after Enterprise and Zenith gates passed.' -ForegroundColor Cyan
    $packageArgs=@{ReleasePackaging=$true;RequireSigning=[bool]$RequireSigning;SkipOnlineSupplyChain=[bool]$SkipOnlineSupplyChain}
    if($UpdateTrustPath){$packageArgs['UpdateTrustPath']=$UpdateTrustPath}
    & (Join-Path $PSScriptRoot 'verify-phase16.ps1') @packageArgs
    if($LASTEXITCODE -ne 0){throw 'Zenith release packaging/signing boundary failed.'}
}
Write-Host 'AetherCore Zenith verification gate passed.' -ForegroundColor Green
Write-Host 'GA still requires verify-production.ps1 with signed lifecycle, stress/soak and release evidence.' -ForegroundColor DarkYellow
