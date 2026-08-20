$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
& python scripts/phase14-scheduler-audit.py
if($LASTEXITCODE -ne 0){throw 'Phase 14 scheduler architecture audit failed.'}
