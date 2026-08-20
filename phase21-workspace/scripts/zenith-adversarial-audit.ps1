[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
Write-Host 'AetherCore Zenith — platform-neutral adversarial source audit' -ForegroundColor Cyan
& python (Join-Path $PSScriptRoot 'zenith-adversarial-audit.py')
if($LASTEXITCODE -ne 0){throw 'Zenith adversarial source audit failed.'}
Write-Host 'AetherCore Zenith source audit passed.' -ForegroundColor Green
