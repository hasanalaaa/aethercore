[CmdletBinding()]
param(
    [switch]$InstallPrerequisites,
    [switch]$RefreshDependencyFreeze,
    [switch]$SkipVerify
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path $PSScriptRoot -Parent
Set-Location $Root

if ($InstallPrerequisites) {
    & (Join-Path $PSScriptRoot 'bootstrap.ps1') -InstallPrerequisites -RefreshDependencyFreeze:$RefreshDependencyFreeze
} else {
    & (Join-Path $PSScriptRoot 'bootstrap.ps1') -RefreshDependencyFreeze:$RefreshDependencyFreeze
}

if (-not $SkipVerify) {
    & (Join-Path $PSScriptRoot 'verify-phase16.ps1')
}

& (Join-Path $PSScriptRoot 'run-dev.ps1')
