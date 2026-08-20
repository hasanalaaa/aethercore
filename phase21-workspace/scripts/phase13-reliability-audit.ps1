[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
python scripts/phase13-reliability-audit.py
if ($LASTEXITCODE -ne 0) { throw 'Phase 13 reliability source audit failed.' }
