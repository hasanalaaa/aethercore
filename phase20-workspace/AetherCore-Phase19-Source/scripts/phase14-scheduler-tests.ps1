[CmdletBinding()]
param()
$ErrorActionPreference='Stop'
$Root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $Root
Write-Host 'Phase 14 deterministic idle-scheduler and passive-publication tests...' -ForegroundColor Cyan
& cargo test --locked -p aethercore-collector-runtime -p aethercore-idle-scheduler -p aethercore-operation-kernel -p aethercore-driver-hub -p aethercore-cleaner -p aethercore-startup-manager -p aethercore-diagnostic-engine
if($LASTEXITCODE -ne 0){throw 'Phase 14 scheduler/passive workload tests failed.'}
