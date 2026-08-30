[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root

Write-Host 'AetherCore Phase 12 — typed localization / RTL source audit' -ForegroundColor Cyan
& python scripts/phase12-localization-audit.py
if ($LASTEXITCODE -ne 0) { throw 'Phase 12 catalog/parity/bidi audit failed.' }

& python scripts/test-phase12-localization.py
if ($LASTEXITCODE -ne 0) { throw 'Phase 12 deep catalog/native-prose localization audit failed.' }

$Broker = Get-Content 'apps/consent-broker/src/main.rs' -Raw
foreach ($Marker in @('--intent-id','--locale','MB_RTLREADING','localized_operation_ar','GetUserDefaultUILanguage')) {
    if (-not $Broker.Contains($Marker)) { throw "Consent broker localization invariant missing: $Marker" }
}
if ($Broker.Contains('--challenge')) { throw 'Retired consent challenge must not return.' }

$Desktop = Get-Content 'apps/desktop/src/main.rs' -Raw
if (-not $Desktop.Contains('anyhow::bail!(error.message_key.clone())')) { throw 'Desktop bridge must preserve service message_key.' }
if ($Desktop.Contains('error.technical_detail.is_empty()')) { throw 'Desktop bridge must not prefer technical_detail as user-facing prose.' }

Write-Host 'Phase 12 localization/RTL source audit passed.' -ForegroundColor Green
