[CmdletBinding()]
param(
    [switch]$SkipNative,
    [switch]$SkipBrowserHarness
)
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root

Write-Host 'AetherCore Omega — fail-closed convergence verification' -ForegroundColor Cyan
& python (Join-Path $PSScriptRoot 'check-dependency-freeze.py')
if($LASTEXITCODE -ne 0){throw 'Dependency freeze is not approved; Omega verification is release-blocked.'}

& python (Join-Path $PSScriptRoot 'check-pipe-teardown-qualification.py')
if($LASTEXITCODE -ne 0){throw 'Named-pipe teardown is not natively qualified; Omega release remains blocked.'}

& (Join-Path $PSScriptRoot 'verify-zenith-recursive.ps1')
if($LASTEXITCODE -ne 0){throw 'Inherited Zenith Recursive verification failed.'}

if(-not $SkipBrowserHarness){
    & python (Join-Path $PSScriptRoot 'omega-ui-interaction-harness.py')
    if($LASTEXITCODE -ne 0){throw 'Omega browser interaction harness failed.'}
}

if(-not $SkipNative){
    if(-not $IsWindows){throw 'Native Omega qualification requires Windows; use -SkipNative only for source-authoring verification.'}
    & (Join-Path $PSScriptRoot 'verify-production.ps1')
    if($LASTEXITCODE -ne 0){throw 'Windows production verification failed.'}
}
Write-Host 'AetherCore Omega verification passed for every requested executable gate.' -ForegroundColor Green
