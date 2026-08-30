[CmdletBinding()]param()
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
Write-Host 'AetherCore Enterprise adversarial architecture audit' -ForegroundColor Cyan
& python (Join-Path $PSScriptRoot 'enterprise-adversarial-audit.py')
if($LASTEXITCODE -ne 0){throw 'Enterprise adversarial source audit failed.'}
Write-Host 'Enterprise adversarial source audit passed.' -ForegroundColor Green
