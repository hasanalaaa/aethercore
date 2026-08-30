[CmdletBinding()]param()
$ErrorActionPreference='Stop';$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path;Set-Location $Root
& python scripts/phase15-security-audit.py
if($LASTEXITCODE -ne 0){throw 'Phase 15 secure-update/support-export source audit failed.'}
