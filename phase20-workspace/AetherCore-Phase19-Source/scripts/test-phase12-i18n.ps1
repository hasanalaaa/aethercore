[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root

Write-Host 'Running Phase 12 catalog/parity/bidi audit...' -ForegroundColor Cyan
& python scripts/phase12-localization-audit.py
if ($LASTEXITCODE -ne 0) { throw 'Phase 12 localization audit failed.' }

Write-Host 'Running Intl Arabic pluralization/formatting tests...' -ForegroundColor Cyan
& node scripts/phase12-i18n-tests.cjs
if ($LASTEXITCODE -ne 0) { throw 'Phase 12 Intl/pluralization tests failed.' }

Write-Host 'Phase 12 localization runtime tests passed.' -ForegroundColor Green
