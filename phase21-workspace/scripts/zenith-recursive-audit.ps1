[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
& python (Join-Path $PSScriptRoot 'zenith-recursive-audit.py')
if($LASTEXITCODE -ne 0){throw 'Zenith recursive adversarial source audit failed.'}
Write-Host 'Zenith recursive adversarial source audit passed.' -ForegroundColor Green
