[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
& python scripts/phase16-ga-audit.py
if($LASTEXITCODE -ne 0){throw 'Phase 16 GA architecture/policy audit failed.'}
Write-Host 'Phase 16 GA architecture/policy audit passed.' -ForegroundColor Green
