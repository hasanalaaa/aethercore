[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$ReleaseRoot,
    [string]$EvidenceDirectory='out\ga-evidence',
    [string]$CertificateThumbprint=$env:AETHERCORE_CODESIGN_THUMBPRINT,
    [ValidateSet('CurrentUser','LocalMachine')][string]$CertificateStore='CurrentUser',
    [switch]$SkipCertificateChainCheck
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
Write-Host 'AetherCore definitive production/GA seal gate' -ForegroundColor Cyan
& (Join-Path $PSScriptRoot 'zenith-adversarial-audit.ps1');if($LASTEXITCODE -ne 0){throw 'Zenith convergence source audit failed.'}
& (Join-Path $PSScriptRoot 'zenith-recursive-audit.ps1');if($LASTEXITCODE -ne 0){throw 'Zenith recursive source audit failed.'}
& (Join-Path $PSScriptRoot 'enterprise-adversarial-audit.ps1');if($LASTEXITCODE -ne 0){throw 'Enterprise convergence source audit failed.'}
& (Join-Path $PSScriptRoot 'phase16-ga-audit.ps1');if($LASTEXITCODE -ne 0){throw 'Phase 16 policy audit failed.'}
& (Join-Path $PSScriptRoot 'phase16-seal-release.ps1') -ReleaseRoot $ReleaseRoot -EvidenceDirectory $EvidenceDirectory -CertificateThumbprint $CertificateThumbprint -CertificateStore $CertificateStore -SkipCertificateChainCheck:$SkipCertificateChainCheck
if($LASTEXITCODE -ne 0){throw 'GA release sealing failed.'}
& (Join-Path $PSScriptRoot 'verify-ga-seal.ps1') -ReleaseRoot $ReleaseRoot -SkipCertificateChainCheck:$SkipCertificateChainCheck
if($LASTEXITCODE -ne 0){throw 'Final GA seal self-verification failed.'}
Write-Host 'AetherCore General Availability release seal: PASS' -ForegroundColor Green
